use std::io::{Read, Seek};
use std::path::Path;
use std::process::Command;
use std::process::Stdio;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use git2::{build::RepoBuilder, Repository};

use super::cancel_token::CancelToken;
use super::network_proxy::git_fetch_options;
use super::process::background_command;

#[path = "git_process_tree.rs"]
mod process_tree;

pub fn clone_or_pull(
    repo_url: &str,
    dest: &Path,
    branch: Option<&str>,
    cancel: Option<&CancelToken>,
    proxy_url: Option<&str>,
) -> Result<String> {
    if let Some(revision) =
        branch.filter(|value| value.len() == 40 && value.bytes().all(|b| b.is_ascii_hexdigit()))
    {
        return clone_pinned_commit(repo_url, dest, revision, cancel, proxy_url);
    }
    // Prefer the system `git` binary if available. It tends to work better on macOS
    // networks because it respects user git config (proxy/certs) and OS trust store.
    if let Some(git_bin) = resolve_git_bin() {
        let started = Instant::now();
        match clone_or_pull_via_git_cli(repo_url, dest, branch, cancel, proxy_url) {
            Ok(head) => {
                log::info!(
                    "[git_fetcher] git-cli ok (bin={}) {}s url={}",
                    git_bin,
                    started.elapsed().as_secs_f32(),
                    repo_url
                );
                return Ok(head);
            }
            Err(err) => {
                if proxy_url.is_some_and(|v| !v.trim().is_empty()) {
                    anyhow::bail!(
                        "git 命令执行失败。已配置 GitHub 代理，为确保访问一定走代理，已停止并不回退到内置 git。\n{:#}",
                        err
                    );
                }
                let allow_fallback = std::env::var("SKILLS_HUB_ALLOW_LIBGIT2_FALLBACK")
                    .ok()
                    .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                    .unwrap_or(false);
                log::warn!(
                    "[git_fetcher] git-cli failed (bin={}) {}s url={} err={:#}",
                    git_bin,
                    started.elapsed().as_secs_f32(),
                    repo_url,
                    err
                );
                if !allow_fallback {
                    anyhow::bail!(
                        "git 命令执行失败（为避免卡死，已停止并不再回退到内置 git）。请检查系统 git/网络/代理；或设置环境变量 SKILLS_HUB_ALLOW_LIBGIT2_FALLBACK=1 允许回退。\n{:#}",
                        err
                    );
                }
                log::warn!(
                    "[git_fetcher] falling back to libgit2 (SKILLS_HUB_ALLOW_LIBGIT2_FALLBACK=1)"
                );
            }
        }
    } else {
        log::info!("[git_fetcher] system git not available; using libgit2");
    }

    clone_or_pull_via_libgit2(repo_url, dest, branch, proxy_url.unwrap_or_default())
}

pub(crate) fn clone_or_pull_via_libgit2(
    repo_url: &str,
    dest: &Path,
    branch: Option<&str>,
    proxy_url: &str,
) -> Result<String> {
    let repo = if dest.exists() {
        let repo = Repository::open(dest).with_context(|| format!("open repo at {:?}", dest))?;
        fetch_origin(&repo, proxy_url)?;
        repo
    } else {
        let mut builder = RepoBuilder::new();
        builder.fetch_options(git_fetch_options(proxy_url));
        builder
            .clone(repo_url, dest)
            .with_context(|| format!("clone {} into {:?}", repo_url, dest))?
    };

    // Best-effort: move working tree HEAD to the fetched remote head (so "pull" actually updates).
    if let Some(branch) = branch {
        if let Ok(obj) = repo.revparse_single(&format!("refs/remotes/origin/{}", branch)) {
            repo.checkout_tree(&obj, None)?;
            repo.set_head_detached(obj.id())?;
        }
    } else {
        let candidates = [
            "refs/remotes/origin/HEAD",
            "refs/remotes/origin/main",
            "refs/remotes/origin/master",
        ];
        for r in candidates {
            if let Ok(obj) = repo.revparse_single(r) {
                repo.checkout_tree(&obj, None)?;
                repo.set_head_detached(obj.id())?;
                break;
            }
        }
    }

    let head = repo.head()?.target().context("missing HEAD target")?;
    Ok(head.to_string())
}

pub fn clone_or_pull_sparse(
    repo_url: &str,
    dest: &Path,
    branch: Option<&str>,
    subpath: &str,
    cancel: Option<&CancelToken>,
    proxy_url: Option<&str>,
) -> Result<String> {
    if let Some(revision) =
        branch.filter(|value| value.len() == 40 && value.bytes().all(|b| b.is_ascii_hexdigit()))
    {
        return clone_pinned_commit(repo_url, dest, revision, cancel, proxy_url);
    }
    anyhow::ensure!(
        subpath
            .replace('\\', "/")
            .split('/')
            .any(|part| !part.is_empty() && part != "."),
        "sparse checkout path is empty"
    );
    clone_or_pull_sparse_many(
        repo_url,
        dest,
        branch,
        &[subpath.to_string()],
        cancel,
        proxy_url,
    )
}

/// Refresh one repository and materialize only the selected Skill directories.
/// The union is supplied for each check, rather than retained as an unbounded
/// set of all paths ever visited by the application.
pub(crate) fn clone_or_pull_sparse_many(
    repo_url: &str,
    dest: &Path,
    branch: Option<&str>,
    subpaths: &[String],
    cancel: Option<&CancelToken>,
    proxy_url: Option<&str>,
) -> Result<String> {
    let patterns = sparse_patterns(subpaths)?;
    clone_or_pull_sparse_patterns(repo_url, dest, branch, &patterns, cancel, proxy_url)
}

pub(crate) fn clone_or_pull_skill_manifests(
    repo_url: &str,
    dest: &Path,
    branch: Option<&str>,
    cancel: Option<&CancelToken>,
    proxy_url: Option<&str>,
) -> Result<String> {
    clone_or_pull_sparse_patterns(
        repo_url,
        dest,
        branch,
        &["/SKILL.md".into(), "/**/SKILL.md".into()],
        cancel,
        proxy_url,
    )
}

fn sparse_patterns(subpaths: &[String]) -> Result<Vec<String>> {
    anyhow::ensure!(!subpaths.is_empty(), "sparse checkout paths are empty");
    let mut sparse_patterns = Vec::new();
    for subpath in subpaths {
        let normalized = subpath.replace('\\', "/");
        let parts = normalized
            .split('/')
            .filter(|part| !part.is_empty() && *part != ".")
            .collect::<Vec<_>>();
        anyhow::ensure!(
            !parts.contains(&"..") && !normalized.starts_with('/'),
            "invalid sparse checkout path"
        );
        if parts.is_empty() {
            sparse_patterns.push("/*".to_string());
            continue;
        }
        let mut pattern = String::from("/");
        for character in parts.join("/").chars() {
            if matches!(character, '*' | '?' | '[' | ']' | '\\') {
                pattern.push('\\');
            }
            pattern.push(character);
        }
        pattern.push('/');
        sparse_patterns.push(pattern);
    }
    sparse_patterns.sort();
    sparse_patterns.dedup();

    Ok(sparse_patterns)
}

pub(crate) fn materialize_sparse_paths(
    repo_url: &str,
    dest: &Path,
    subpaths: &[String],
    cancel: Option<&CancelToken>,
    proxy_url: Option<&str>,
) -> Result<()> {
    let patterns = sparse_patterns(subpaths)?;
    let mut cmd = git_cmd_for_remote(proxy_url, Some(repo_url));
    cmd.arg("-C")
        .arg(dest)
        .args(["sparse-checkout", "set", "--no-cone"])
        .args(patterns);
    let out = run_cmd_with_timeout(
        cmd,
        git_fetch_timeout(),
        "materialize selected Skill paths".into(),
        cancel,
    )?;
    anyhow::ensure!(
        out.status.success(),
        "could not materialize selected Skill paths: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    Ok(())
}

#[derive(Debug)]
pub(crate) struct SparseCheckoutFailure {
    pub revision: String,
    reason: String,
}
impl std::fmt::Display for SparseCheckoutFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.reason)
    }
}
impl std::error::Error for SparseCheckoutFailure {}

pub(crate) fn materialize_sparse_paths_at_revision(
    repo_url: &str,
    dest: &Path,
    subpaths: &[String],
    revision: &str,
    cancel: Option<&CancelToken>,
    proxy_url: Option<&str>,
) -> Result<()> {
    materialize_sparse_patterns(
        repo_url,
        dest,
        &sparse_patterns(subpaths)?,
        revision,
        cancel,
        proxy_url,
    )
}

fn materialize_sparse_patterns(
    repo_url: &str,
    dest: &Path,
    patterns: &[String],
    revision: &str,
    cancel: Option<&CancelToken>,
    proxy_url: Option<&str>,
) -> Result<()> {
    let mut set = git_cmd_for_remote(proxy_url, Some(repo_url));
    set.arg("-C")
        .arg(dest)
        .args(["sparse-checkout", "set", "--no-cone"])
        .args(patterns);
    let out = run_cmd_with_timeout(
        set,
        git_fetch_timeout(),
        "configure sparse checkout".into(),
        cancel,
    )?;
    anyhow::ensure!(
        out.status.success(),
        "could not configure sparse checkout: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let mut checkout = git_cmd_for_remote(proxy_url, Some(repo_url));
    checkout
        .arg("-C")
        .arg(dest)
        .args(["reset", "--hard", revision]);
    let out = run_cmd_with_timeout(
        checkout,
        git_fetch_timeout(),
        "materialize sparse checkout".into(),
        cancel,
    )?;
    anyhow::ensure!(
        out.status.success(),
        "could not materialize sparse checkout: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    Ok(())
}

fn clone_or_pull_sparse_patterns(
    repo_url: &str,
    dest: &Path,
    branch: Option<&str>,
    sparse_patterns: &[String],
    cancel: Option<&CancelToken>,
    proxy_url: Option<&str>,
) -> Result<String> {
    anyhow::ensure!(
        resolve_git_bin().is_some(),
        "system git is required for sparse checkout"
    );
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let pinned =
        branch.filter(|value| value.len() == 40 && value.bytes().all(|b| b.is_ascii_hexdigit()));
    if pinned.is_some() && !dest.join(".git").exists() {
        let mut init = git_cmd(proxy_url);
        init.arg("init").arg(dest);
        let out = run_cmd_with_timeout(
            init,
            git_timeout(),
            "initialize pinned sparse cache".into(),
            cancel,
        )?;
        anyhow::ensure!(
            out.status.success(),
            "could not initialize pinned sparse cache"
        );
    }
    let reference = if dest.join(".git").exists() {
        for lock in ["index.lock", "shallow.lock", "HEAD.lock"] {
            let _ = std::fs::remove_file(dest.join(".git").join(lock));
        }
        let mut fetch = git_cmd_for_remote(proxy_url, Some(repo_url));
        fetch.arg("-C").arg(dest);
        if let Some(revision) = pinned {
            fetch.args(["fetch", "--depth", "1", "--no-tags", repo_url, revision]);
        } else {
            fetch.args(["fetch", "--prune", "origin"]);
        }
        let out = run_cmd_with_timeout(
            fetch,
            git_fetch_timeout(),
            "refresh sparse repository".into(),
            cancel,
        )?;
        anyhow::ensure!(
            out.status.success(),
            "git fetch failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        "FETCH_HEAD"
    } else {
        let mut clone = git_cmd_for_remote(proxy_url, Some(repo_url));
        clone.args([
            "clone",
            "--depth",
            "1",
            "--filter=blob:none",
            "--sparse",
            "--no-checkout",
            "--no-tags",
        ]);
        if let Some(branch) = branch {
            clone.args(["--branch", branch, "--single-branch"]);
        }
        clone.arg(repo_url).arg(dest);
        let out = run_cmd_with_timeout(
            clone,
            git_timeout(),
            "clone sparse repository".into(),
            cancel,
        )?;
        anyhow::ensure!(
            out.status.success(),
            "git clone failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        "HEAD"
    };
    let mut rev = git_cmd(proxy_url);
    rev.arg("-C").arg(dest).args(["rev-parse", reference]);
    let out = run_cmd_with_timeout(
        rev,
        git_fetch_timeout(),
        "read refreshed revision".into(),
        cancel,
    )?;
    anyhow::ensure!(out.status.success(), "could not read refreshed revision");
    let revision = String::from_utf8_lossy(&out.stdout).trim().to_string();
    materialize_sparse_patterns(
        repo_url,
        dest,
        sparse_patterns,
        &revision,
        cancel,
        proxy_url,
    )
    .map_err(|error| SparseCheckoutFailure {
        revision: revision.clone(),
        reason: format!("{error:#}"),
    })?;
    Ok(revision)
}

fn git_timeout() -> Duration {
    let secs = std::env::var("SKILLS_HUB_GIT_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(300);
    Duration::from_secs(secs)
}

fn git_fetch_timeout() -> Duration {
    let secs = std::env::var("SKILLS_HUB_GIT_FETCH_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(180);
    Duration::from_secs(secs)
}

static GIT_BIN: OnceLock<Option<String>> = OnceLock::new();

pub(crate) fn system_git_available() -> bool {
    resolve_git_bin().is_some()
}

fn resolve_git_bin() -> Option<String> {
    GIT_BIN
        .get_or_init(|| {
            // Allow overriding from environment for debugging / enterprise setups.
            for key in ["SKILLS_HUB_GIT_BIN", "SKILLS_HUB_GIT_PATH"] {
                if let Ok(v) = std::env::var(key) {
                    let v = v.trim().to_string();
                    if !v.is_empty() && git_bin_works(&v) {
                        log::info!("[git_fetcher] using git bin from {}: {}", key, v);
                        return Some(v);
                    }
                }
            }

            // Try PATH lookup first (works in dev; sometimes missing in macOS bundles).
            if git_bin_works("git") {
                log::info!("[git_fetcher] using git bin from PATH: git");
                return Some("git".to_string());
            }

            // Common macOS locations (system git and Homebrew).
            for cand in [
                "/usr/bin/git",
                "/opt/homebrew/bin/git",
                "/usr/local/bin/git",
            ] {
                if git_bin_works(cand) {
                    log::info!("[git_fetcher] using git bin: {}", cand);
                    return Some(cand.to_string());
                }
            }

            log::warn!("[git_fetcher] no usable git binary found");
            None
        })
        .clone()
}

fn git_bin_works(bin: &str) -> bool {
    background_command(bin)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn git_cmd(proxy_url: Option<&str>) -> Command {
    git_cmd_for_remote(proxy_url, None)
}

fn git_cmd_for_remote(proxy_url: Option<&str>, remote_url: Option<&str>) -> Command {
    let bin = resolve_git_bin().unwrap_or_else(|| "git".to_string());
    let mut cmd = background_command(bin);
    let proxy_url = proxy_url.map(str::trim).unwrap_or_default();
    cmd.arg("-c")
        .arg(format!("http.proxy={proxy_url}"))
        .arg("-c")
        .arg(format!("https.proxy={proxy_url}"));
    if let Some(remote_url) =
        remote_url.filter(|url| url.starts_with("https://") || url.starts_with("http://"))
    {
        cmd.arg("-c")
            .arg(format!("http.{remote_url}.proxy={proxy_url}"));
    }
    for name in [
        "http_proxy",
        "https_proxy",
        "all_proxy",
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "ALL_PROXY",
    ] {
        cmd.env_remove(name);
    }
    if !proxy_url.is_empty() {
        cmd.env("http_proxy", proxy_url)
            .env("https_proxy", proxy_url)
            .env("all_proxy", proxy_url)
            .env("HTTP_PROXY", proxy_url)
            .env("HTTPS_PROXY", proxy_url)
            .env("ALL_PROXY", proxy_url);
    }
    // Never block on interactive auth prompts inside a GUI app.
    cmd.env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_ASKPASS", "echo");
    // Abort stalled HTTPS transfers (helps avoid "spinner forever" on bad networks).
    cmd.env("GIT_HTTP_LOW_SPEED_LIMIT", "1024")
        .env("GIT_HTTP_LOW_SPEED_TIME", "120");
    cmd
}

fn run_cmd_with_timeout(
    mut cmd: Command,
    timeout: Duration,
    context: String,
    cancel: Option<&CancelToken>,
) -> Result<std::process::Output> {
    // Regular temporary files cannot fill a pipe or wait for inherited writer
    // handles to close. They are private, unlinked/delete-on-close and never
    // become persistent logs. Read only after the owned process tree is stopped.
    let mut stdout = tempfile::tempfile().context("create Git stdout capture")?;
    let mut stderr = tempfile::tempfile().context("create Git stderr capture")?;
    cmd.stdin(Stdio::null())
        .stdout(Stdio::from(stdout.try_clone()?))
        .stderr(Stdio::from(stderr.try_clone()?));

    let (mut child, tree) =
        process_tree::ProcessTree::spawn(&mut cmd).with_context(|| context.clone())?;
    let start = Instant::now();
    loop {
        if cancel.is_some_and(|c| c.is_cancelled()) {
            tree.terminate();
            let _ = child.kill();
            let _ = child.wait();
            anyhow::bail!("CANCELLED|操作已被用户取消。");
        }

        if start.elapsed() > timeout {
            tree.terminate();
            let _ = child.kill();
            let _ = child.wait();
            let error_output = read_capture(&mut stderr).unwrap_or_default();
            anyhow::bail!(
                "git 操作超时（{}s）。请检查网络/代理是否可访问 GitHub；也可设置环境变量 SKILLS_HUB_GIT_TIMEOUT_SECS 增大超时。\n{}",
                timeout.as_secs(),
                String::from_utf8_lossy(&error_output).trim()
            );
        }

        match child.try_wait() {
            Ok(Some(status)) => {
                tree.terminate();
                return Ok(std::process::Output {
                    status,
                    stdout: read_capture(&mut stdout).with_context(|| context.clone())?,
                    stderr: read_capture(&mut stderr).with_context(|| context.clone())?,
                });
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(err) => {
                tree.terminate();
                let _ = child.kill();
                let _ = child.wait();
                return Err(err).with_context(|| context.clone());
            }
        }
    }
}

fn read_capture(file: &mut std::fs::File) -> std::io::Result<Vec<u8>> {
    file.rewind()?;
    let mut output = Vec::new();
    file.read_to_end(&mut output)?;
    Ok(output)
}

fn clone_or_pull_via_git_cli(
    repo_url: &str,
    dest: &Path,
    branch: Option<&str>,
    cancel: Option<&CancelToken>,
    proxy_url: Option<&str>,
) -> Result<String> {
    // Ensure parent exists so `git clone` can create dest.
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create parent dir {:?}", parent))?;
    }

    if dest.exists() {
        // Remove stale lock files left by a previously crashed git process.
        let git_dir = dest.join(".git");
        for lock_name in &["index.lock", "shallow.lock", "HEAD.lock"] {
            let lock_path = git_dir.join(lock_name);
            if lock_path.exists() {
                log::warn!("[git_fetcher] removing stale lock file: {:?}", lock_path);
                let _ = std::fs::remove_file(&lock_path);
            }
        }

        // Fetch updates.
        let out = run_cmd_with_timeout(
            {
                let mut cmd = git_cmd_for_remote(proxy_url, Some(repo_url));
                cmd.arg("-C").arg(dest).args(["fetch", "--prune", "origin"]);
                cmd
            },
            git_fetch_timeout(),
            format!("git fetch in {:?}", dest),
            cancel,
        )?;
        if !out.status.success() {
            anyhow::bail!("git fetch failed: {}", String::from_utf8_lossy(&out.stderr));
        }

        // Move local HEAD to fetched commit.
        if let Some(branch) = branch {
            let out = run_cmd_with_timeout(
                {
                    let mut cmd = git_cmd(proxy_url);
                    cmd.arg("-C").arg(dest).args([
                        "checkout",
                        "-B",
                        branch,
                        &format!("origin/{}", branch),
                    ]);
                    cmd
                },
                git_fetch_timeout(),
                format!("git checkout -B {} in {:?}", branch, dest),
                cancel,
            )?;
            if !out.status.success() {
                anyhow::bail!(
                    "git checkout branch failed: {}",
                    String::from_utf8_lossy(&out.stderr)
                );
            }
        } else {
            let out = run_cmd_with_timeout(
                {
                    let mut cmd = git_cmd(proxy_url);
                    cmd.arg("-C")
                        .arg(dest)
                        .args(["reset", "--hard", "FETCH_HEAD"]);
                    cmd
                },
                git_fetch_timeout(),
                format!("git reset --hard in {:?}", dest),
                cancel,
            )?;
            if !out.status.success() {
                anyhow::bail!(
                    "git reset --hard failed: {}",
                    String::from_utf8_lossy(&out.stderr)
                );
            }
        }
    } else {
        // Clone.
        let mut cmd = git_cmd_for_remote(proxy_url, Some(repo_url));
        cmd.arg("clone")
            .args(["--depth", "1", "--filter=blob:none", "--no-tags"]);
        if let Some(branch) = branch {
            cmd.arg("--branch").arg(branch).arg("--single-branch");
        }
        cmd.arg(repo_url).arg(dest);
        let out = run_cmd_with_timeout(
            cmd,
            git_timeout(),
            format!("git clone {} into {:?}", repo_url, dest),
            cancel,
        )?;
        if !out.status.success() {
            anyhow::bail!("git clone failed: {}", String::from_utf8_lossy(&out.stderr));
        }
    }

    // Checkout desired branch if specified (best-effort; shallow clones may already be on it).
    if let Some(branch) = branch {
        let out = run_cmd_with_timeout(
            {
                let mut cmd = git_cmd(proxy_url);
                cmd.arg("-C").arg(dest).args(["checkout", branch]);
                cmd
            },
            git_fetch_timeout(),
            format!("git checkout {} in {:?}", branch, dest),
            cancel,
        )?;
        if !out.status.success() {
            // Don't hard-fail; still return HEAD for caller.
            // But include useful context for debugging.
            let stderr = String::from_utf8_lossy(&out.stderr);
            if !stderr.trim().is_empty() {
                eprintln!("[git_fetcher] checkout warning: {}", stderr);
            }
        }
    }

    // Read HEAD revision.
    let out = run_cmd_with_timeout(
        {
            let mut cmd = git_cmd(proxy_url);
            cmd.arg("-C").arg(dest).args(["rev-parse", "HEAD"]);
            cmd
        },
        git_fetch_timeout(),
        format!("git rev-parse HEAD in {:?}", dest),
        cancel,
    )?;
    if !out.status.success() {
        anyhow::bail!(
            "git rev-parse failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn fetch_origin(repo: &Repository, proxy_url: &str) -> Result<()> {
    let mut remote = repo.find_remote("origin")?;
    let mut opts = git_fetch_options(proxy_url);
    remote.fetch(
        &["refs/heads/*:refs/remotes/origin/*"],
        Some(&mut opts),
        None,
    )?;
    Ok(())
}

#[cfg(test)]
#[path = "tests/git_fetcher.rs"]
mod tests;

fn clone_pinned_commit(
    repo_url: &str,
    dest: &Path,
    revision: &str,
    cancel: Option<&CancelToken>,
    proxy_url: Option<&str>,
) -> Result<String> {
    if resolve_git_bin().is_none() {
        return clone_pinned_commit_via_libgit2(
            repo_url,
            dest,
            revision,
            proxy_url.unwrap_or_default(),
        );
    }
    std::fs::create_dir_all(dest)?;
    if !dest.join(".git").exists() {
        let mut init = git_cmd(proxy_url);
        init.arg("init").arg(dest);
        let output = run_cmd_with_timeout(
            init,
            git_timeout(),
            "initialize pinned cache".into(),
            cancel,
        )?;
        anyhow::ensure!(output.status.success(), "could not initialize pinned cache");
    }
    let mut fetch = git_cmd_for_remote(proxy_url, Some(repo_url));
    fetch
        .arg("-C")
        .arg(dest)
        .args(["fetch", "--depth", "1", "--no-tags", repo_url, revision]);
    let output = run_cmd_with_timeout(
        fetch,
        git_fetch_timeout(),
        "fetch pinned revision".into(),
        cancel,
    )?;
    anyhow::ensure!(
        output.status.success(),
        "could not fetch pinned revision: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut checkout = git_cmd(proxy_url);
    checkout
        .arg("-C")
        .arg(dest)
        .args(["checkout", "--detach", "--force", revision]);
    let output = run_cmd_with_timeout(
        checkout,
        git_timeout(),
        "checkout pinned revision".into(),
        cancel,
    )?;
    anyhow::ensure!(
        output.status.success(),
        "could not checkout pinned revision"
    );
    let repo = Repository::open(dest)?;
    let head = repo
        .head()?
        .target()
        .context("missing pinned HEAD")?
        .to_string();
    anyhow::ensure!(
        head.eq_ignore_ascii_case(revision),
        "pinned revision mismatch"
    );
    Ok(head)
}

fn clone_pinned_commit_via_libgit2(
    repo_url: &str,
    dest: &Path,
    revision: &str,
    proxy_url: &str,
) -> Result<String> {
    let repo = if dest.join(".git").exists() {
        Repository::open(dest)?
    } else {
        Repository::init(dest)?
    };
    let oid = git2::Oid::from_str(revision)?;
    let mut options = git_fetch_options(proxy_url);
    if repo_url.starts_with("https://") || repo_url.starts_with("http://") {
        options.depth(1);
    }
    repo.remote_anonymous(repo_url)?
        .fetch(&[revision], Some(&mut options), None)
        .context("fetch pinned revision")?;
    let commit = repo
        .find_commit(oid)
        .context("requested pinned revision is unavailable")?;
    let mut checkout = git2::build::CheckoutBuilder::new();
    checkout.force();
    repo.checkout_tree(commit.as_object(), Some(&mut checkout))?;
    repo.set_head_detached(oid)?;
    Ok(oid.to_string())
}

/// Resolve remote HEAD without consulting the installation cache.
pub fn resolve_default_revision(repo_url: &str, proxy_url: Option<&str>) -> Result<String> {
    if resolve_git_bin().is_none() {
        let mut remote = git2::Remote::create_detached(repo_url)?;
        let connection = remote.connect_auth(
            git2::Direction::Fetch,
            None,
            Some(super::network_proxy::git_proxy_options(
                proxy_url.unwrap_or_default(),
            )),
        )?;
        return connection
            .list()?
            .iter()
            .find(|head| head.name() == "HEAD")
            .map(|head| head.oid().to_string())
            .context("remote default branch is unavailable");
    }
    let mut command = git_cmd_for_remote(proxy_url, Some(repo_url));
    command.args(["ls-remote", repo_url, "HEAD"]);
    let output = run_cmd_with_timeout(
        command,
        git_fetch_timeout(),
        "resolve default branch".into(),
        None,
    )?;
    anyhow::ensure!(
        output.status.success(),
        "could not resolve default branch: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout)?;
    let revision = text
        .lines()
        .find_map(|line| {
            let (revision, name) = line.split_once('\t')?;
            (name == "HEAD"
                && revision.len() == 40
                && revision.bytes().all(|b| b.is_ascii_hexdigit()))
            .then(|| revision.to_lowercase())
        })
        .context("remote default branch is unavailable")?;
    Ok(revision)
}
