use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tauri::Manager;
use uuid::Uuid;

use super::cache_cleanup::get_git_cache_ttl_secs;
use super::cancel_token::CancelToken;
use super::central_repo::{ensure_central_repo, resolve_central_repo_path};
use super::content_hash::hash_dir;
use super::git_fetcher::{clone_or_pull, clone_or_pull_sparse};
use super::github_download::{
    download_github_directory, parse_github_api_params, GithubDownloadOptions,
};
use super::network_proxy::get_github_proxy_url;
use super::skill_store::{SkillRecord, SkillStore, SkillTargetRecord};
use super::sync_engine::copy_dir_recursive;
use super::sync_engine::sync_dir_copy_with_overwrite;
use super::tool_adapters::adapter_by_key;
use super::tool_adapters::is_tool_installed;

pub struct InstallResult {
    pub skill_id: String,
    pub name: String,
    pub central_path: PathBuf,
    pub content_hash: Option<String>,
}

fn record_target_sync_failure(
    store: &SkillStore,
    target: &SkillTargetRecord,
    error: &str,
) -> Result<()> {
    let mut failed_target = target.clone();
    failed_target.status = "error".to_string();
    failed_target.last_error = Some(error.to_string());
    store.upsert_skill_target(&failed_target)
}

pub fn install_local_skill<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    store: &SkillStore,
    source_path: &Path,
    name: Option<String>,
) -> Result<InstallResult> {
    install_local_skill_with_existing_policy(app, store, source_path, name, false)
}

pub fn import_existing_local_skill<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    store: &SkillStore,
    source_path: &Path,
    name: Option<String>,
) -> Result<InstallResult> {
    install_local_skill_with_existing_policy(app, store, source_path, name, true)
}

fn install_local_skill_with_existing_policy<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    store: &SkillStore,
    source_path: &Path,
    name: Option<String>,
    reuse_identical_existing: bool,
) -> Result<InstallResult> {
    if !source_path.exists() {
        anyhow::bail!("source path not found: {:?}", source_path);
    }

    let name = name.unwrap_or_else(|| {
        source_path
            .file_name()
            .map(|v| v.to_string_lossy().to_string())
            .unwrap_or_else(|| "unnamed-skill".to_string())
    });

    let central_dir = resolve_central_repo_path(app, store)?;
    ensure_central_repo(&central_dir)?;
    let central_path = central_dir.join(&name);

    if central_path.exists() {
        if reuse_identical_existing {
            let existing = store
                .list_skills()?
                .into_iter()
                .find(|skill| Path::new(&skill.central_path) == central_path);
            let source_hash = hash_dir(source_path).ok();
            let central_hash = hash_dir(&central_path).ok();
            if let (Some(record), Some(src_hash), Some(dst_hash)) =
                (existing, source_hash, central_hash)
            {
                if src_hash == dst_hash {
                    return Ok(InstallResult {
                        skill_id: record.id,
                        name: record.name,
                        central_path,
                        content_hash: record.content_hash,
                    });
                }
            }
        }
        anyhow::bail!("skill already exists in central repo: {:?}", central_path);
    }

    copy_dir_recursive(source_path, &central_path)
        .with_context(|| format!("copy {:?} -> {:?}", source_path, central_path))?;

    let now = now_ms();
    let content_hash = compute_content_hash(&central_path);
    let description = parse_skill_md(&central_path.join("SKILL.md")).and_then(|(_, desc)| desc);

    let record = SkillRecord {
        id: Uuid::new_v4().to_string(),
        name,
        description,
        source_type: "local".to_string(),
        source_ref: Some(source_path.to_string_lossy().to_string()),
        source_subpath: None,
        source_revision: None,
        central_path: central_path.to_string_lossy().to_string(),
        content_hash: content_hash.clone(),
        created_at: now,
        updated_at: now,
        last_sync_at: None,
        last_seen_at: now,
        enabled: true,
        status: "ok".to_string(),
    };

    store.upsert_skill(&record)?;

    Ok(InstallResult {
        skill_id: record.id,
        name: record.name,
        central_path,
        content_hash,
    })
}

pub fn install_git_skill<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    store: &SkillStore,
    repo_url: &str,
    name: Option<String>,
    cancel: Option<&CancelToken>,
) -> Result<InstallResult> {
    let parsed = parse_github_url(repo_url);
    let user_provided_name = name.is_some();
    let mut name = name.unwrap_or_else(|| {
        if let Some(subpath) = &parsed.subpath {
            if subpath == "." {
                derive_name_from_repo_url(&parsed.clone_url)
            } else {
                subpath
                    .rsplit('/')
                    .next()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| derive_name_from_repo_url(&parsed.clone_url))
            }
        } else {
            derive_name_from_repo_url(&parsed.clone_url)
        }
    });

    let central_dir = resolve_central_repo_path(app, store)?;
    ensure_central_repo(&central_dir)?;
    let mut central_path = central_dir.join(&name);

    if central_path.exists() {
        anyhow::bail!("skill already exists in central repo: {:?}", central_path);
    }

    // Fast path: for subpath installs, prefer sparse git checkout.
    // The old GitHub Contents API path is much slower on large repos because it performs
    // one directory/file request at a time and can time out before we even attempt git.
    let github_token = store.get_setting("github_token")?.unwrap_or_default();
    let github_token_opt = if github_token.is_empty() {
        None
    } else {
        Some(github_token.as_str())
    };
    let github_proxy_url = get_github_proxy_url(store)?;
    let revision;
    if let Some((owner, repo, branch, subpath)) = parse_github_api_params(
        &parsed.clone_url,
        parsed.branch.as_deref(),
        parsed.subpath.as_deref(),
    ) {
        log::info!(
            "[installer] using sparse git checkout for subpath install: {}/{} path={}",
            owner,
            repo,
            subpath
        );
        match clone_to_cache_subpath(
            app,
            store,
            &parsed.clone_url,
            Some(branch.as_str()),
            &subpath,
            cancel,
        ) {
            Ok((repo_dir, rev)) => {
                let sub_src = repo_dir.join(&subpath);
                if !sub_src.exists() {
                    anyhow::bail!("subpath not found in repo: {:?}", sub_src);
                }
                ensure_installable_skill_dir(&sub_src)?;
                copy_dir_recursive(&sub_src, &central_path)
                    .with_context(|| format!("copy {:?} -> {:?}", sub_src, central_path))?;
                revision = rev;
            }
            Err(err) => {
                // Clean up partial content before fallback.
                let _ = std::fs::remove_dir_all(&central_path);
                let err_msg = format!("{:#}", err);
                if err_msg.contains("CANCELLED|") {
                    return Err(err);
                }
                log::warn!(
                    "[installer] sparse git checkout failed, falling back to GitHub API download: {:#}",
                    err
                );
                match download_github_directory(
                    &owner,
                    &repo,
                    &branch,
                    &subpath,
                    &central_path,
                    GithubDownloadOptions {
                        cancel,
                        token: github_token_opt,
                        proxy_url: &github_proxy_url,
                    },
                ) {
                    Ok(()) => {
                        revision = format!("api-download-{}", branch);
                    }
                    Err(err) => {
                        let _ = std::fs::remove_dir_all(&central_path);
                        let err_msg = format!("{:#}", err);
                        if err_msg.contains("CANCELLED|") {
                            return Err(err);
                        }
                        if err_msg.contains("404") || err_msg.contains("Not Found") {
                            anyhow::bail!(
                                "该 Skill 在 GitHub 上未找到（可能已被删除或路径已变更）。\n请检查链接是否正确：{}/tree/{}/{}",
                                parsed.clone_url.trim_end_matches(".git"),
                                branch,
                                subpath
                            );
                        }
                        if let Some(rest) = err_msg.strip_prefix("RATE_LIMITED|") {
                            let mins: i64 = rest.trim().parse().unwrap_or(0);
                            if mins > 0 {
                                anyhow::bail!(
                                    "GitHub API 频率限制已触发，约 {} 分钟后重置。可在设置中配置 GitHub Token 以提升限额。",
                                    mins
                                );
                            }
                            anyhow::bail!(
                                "GitHub API 频率限制已触发。可在设置中配置 GitHub Token 以提升限额。"
                            );
                        }
                        if err_msg.contains("403") || err_msg.contains("Forbidden") {
                            anyhow::bail!(
                                "GitHub API 访问被拒绝（可能触发了频率限制）。请稍后再试。"
                            );
                        }
                        return Err(err);
                    }
                }
            }
        }
    } else {
        // Standard git clone path (no subpath or non-GitHub URL)
        let (repo_dir, rev) = clone_to_cache(
            app,
            store,
            &parsed.clone_url,
            parsed.branch.as_deref(),
            cancel,
        )?;

        let copy_src = if let Some(subpath) = &parsed.subpath {
            let sub_src = repo_dir.join(subpath);
            if !sub_src.exists() {
                anyhow::bail!("subpath not found in repo: {:?}", sub_src);
            }
            ensure_installable_skill_dir(&sub_src)?;
            sub_src
        } else {
            // Repo root URL: detect multi-skill repos and ask user to pick one.
            let skill_count = count_skills_in_repo(&repo_dir);
            if skill_count >= 2 {
                anyhow::bail!(
                    "MULTI_SKILLS|该仓库包含多个 Skills，请复制具体 Skill 文件夹链接（例如 GitHub 的 /tree/<branch>/<skill-folder>），再导入。"
                );
            }
            ensure_installable_skill_dir(&repo_dir)?;
            repo_dir.clone()
        };

        copy_dir_recursive(&copy_src, &central_path)
            .with_context(|| format!("copy {:?} -> {:?}", copy_src, central_path))?;
        revision = rev;
    }
    // After download, prefer the name from SKILL.md over the derived name (fixes #28:
    // when subpath is "skills", the derived name collides with tool directory names).
    let (mut description, md_name) = match parse_skill_md(&central_path.join("SKILL.md")) {
        Some((n, d)) => (d, Some(n)),
        None => (None, None),
    };
    if !user_provided_name {
        if let Some(ref better_name) = md_name {
            if *better_name != name {
                let new_central = central_dir.join(better_name);
                if !new_central.exists() {
                    std::fs::rename(&central_path, &new_central).with_context(|| {
                        format!("rename {:?} -> {:?}", central_path, new_central)
                    })?;
                    name = better_name.clone();
                    central_path = new_central;
                }
                // Re-read description after rename (path changed)
                description = parse_skill_md(&central_path.join("SKILL.md")).and_then(|(_, d)| d);
            }
        }
    }

    let now = now_ms();
    let content_hash = compute_content_hash(&central_path);

    let record = SkillRecord {
        id: Uuid::new_v4().to_string(),
        name,
        description,
        source_type: "git".to_string(),
        source_ref: Some(repo_url.to_string()),
        source_subpath: parsed.subpath.clone(),
        source_revision: Some(revision),
        central_path: central_path.to_string_lossy().to_string(),
        content_hash: content_hash.clone(),
        created_at: now,
        updated_at: now,
        last_sync_at: None,
        last_seen_at: now,
        enabled: true,
        status: "ok".to_string(),
    };

    store.upsert_skill(&record)?;

    Ok(InstallResult {
        skill_id: record.id,
        name: record.name,
        central_path,
        content_hash,
    })
}

#[derive(Clone, Debug)]
struct ParsedGitSource {
    clone_url: String,
    branch: Option<String>,
    subpath: Option<String>,
}

fn parse_github_url(input: &str) -> ParsedGitSource {
    // Supports:
    // - https://github.com/owner/repo
    // - https://github.com/owner/repo.git
    // - https://github.com/owner/repo/tree/<branch>/<path>
    // - https://github.com/owner/repo/blob/<branch>/<path>
    let trimmed = input.trim().trim_end_matches('/');

    // Convenience: allow GitHub shorthand inputs like `owner/repo` (and `owner/repo/tree/<branch>/...`).
    // This keeps the UI friendly while still allowing local paths or other git remotes.
    let normalized = if trimmed.starts_with("https://github.com/") {
        trimmed.to_string()
    } else if trimmed.starts_with("http://github.com/") {
        trimmed.replacen("http://github.com/", "https://github.com/", 1)
    } else if trimmed.starts_with("github.com/") {
        format!("https://{}", trimmed)
    } else if looks_like_github_shorthand(trimmed) {
        format!("https://github.com/{}", trimmed)
    } else {
        trimmed.to_string()
    };

    let trimmed = normalized.trim_end_matches('/');
    let gh_prefix = "https://github.com/";
    if !trimmed.starts_with(gh_prefix) {
        return ParsedGitSource {
            clone_url: trimmed.to_string(),
            branch: None,
            subpath: None,
        };
    }

    let rest = &trimmed[gh_prefix.len()..];
    let parts: Vec<&str> = rest.split('/').collect();
    if parts.len() < 2 {
        return ParsedGitSource {
            clone_url: trimmed.to_string(),
            branch: None,
            subpath: None,
        };
    }

    let owner = parts[0];
    let mut repo = parts[1].to_string();
    if let Some(stripped) = repo.strip_suffix(".git") {
        repo = stripped.to_string();
    }
    let clone_url = format!("https://github.com/{}/{}.git", owner, repo);

    if parts.len() >= 4 && (parts[2] == "tree" || parts[2] == "blob") {
        let branch = Some(parts[3].to_string());
        let subpath = if parts.len() > 4 {
            Some(normalize_github_skill_subpath(&parts[4..].join("/")))
        } else {
            None
        };
        return ParsedGitSource {
            clone_url,
            branch,
            subpath,
        };
    }

    ParsedGitSource {
        clone_url,
        branch: None,
        subpath: None,
    }
}

fn normalize_github_skill_subpath(subpath: &str) -> String {
    let trimmed = subpath.trim_matches('/');
    if trimmed.eq_ignore_ascii_case("SKILL.md") {
        return ".".to_string();
    }
    trimmed
        .strip_suffix("/SKILL.md")
        .or_else(|| trimmed.strip_suffix("/skill.md"))
        .unwrap_or(trimmed)
        .to_string()
}

fn looks_like_github_shorthand(input: &str) -> bool {
    if input.is_empty() {
        return false;
    }
    if input.starts_with('/') || input.starts_with('~') || input.starts_with('.') {
        return false;
    }
    // Avoid scp-like ssh URLs (git@github.com:owner/repo) and any explicit schemes.
    if input.contains("://") || input.contains('@') || input.contains(':') {
        return false;
    }

    let parts: Vec<&str> = input.split('/').collect();
    if parts.len() < 2 {
        return false;
    }

    let owner = parts[0];
    let repo = parts[1];
    if owner.is_empty()
        || repo.is_empty()
        || owner == "."
        || owner == ".."
        || repo == "."
        || repo == ".."
    {
        return false;
    }

    let is_safe_segment = |s: &str| {
        s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
    };
    if !is_safe_segment(owner) || !is_safe_segment(repo.trim_end_matches(".git")) {
        return false;
    }

    // If there are more path parts, only accept the GitHub UI patterns we can parse.
    if parts.len() > 2 {
        matches!(parts[2], "tree" | "blob")
    } else {
        true
    }
}

fn now_ms() -> i64 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    now.as_millis() as i64
}

fn derive_name_from_repo_url(repo_url: &str) -> String {
    let mut name = repo_url
        .split('/')
        .next_back()
        .unwrap_or("skill")
        .to_string();
    if let Some(stripped) = name.strip_suffix(".git") {
        name = stripped.to_string();
    }
    if name.is_empty() {
        "skill".to_string()
    } else {
        name
    }
}

/// Scan base directories used for skill discovery.
const SKILL_SCAN_BASES: [&str; 5] = [
    "skills",
    "skills/.curated",
    "skills/.experimental",
    "skills/.system",
    ".claude/skills",
];

/// Check if a directory is a valid skill (has SKILL.md or is under .claude/skills/).
fn is_skill_dir(p: &Path) -> bool {
    p.is_dir() && (p.join("SKILL.md").exists() || is_claude_skill_dir(p))
}

fn ensure_installable_skill_dir(p: &Path) -> Result<()> {
    if is_skill_dir(p) {
        Ok(())
    } else {
        anyhow::bail!(
            "SKILL_INVALID|missing_skill_md|该路径不是有效 Skill 目录：未找到 SKILL.md。请粘贴具体 Skill 文件夹链接。"
        );
    }
}

/// Check if a directory is a Claude plugin skill (under .claude/skills/ without SKILL.md).
fn is_claude_skill_dir(p: &Path) -> bool {
    // A directory under .claude/skills/ is treated as a valid skill even without SKILL.md
    if let Some(parent) = p.parent() {
        let parent_str = parent.to_string_lossy();
        if parent_str.ends_with(".claude/skills") || parent_str.ends_with(".claude\\skills") {
            return p.is_dir();
        }
    }
    false
}

/// Try to read the description for a skill from .claude-plugin/plugin.json.
fn read_plugin_description(repo_dir: &Path) -> Option<String> {
    let plugin_json = repo_dir.join(".claude-plugin/plugin.json");
    if !plugin_json.exists() {
        return None;
    }
    let content = std::fs::read_to_string(&plugin_json).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    json.get("description")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// Extract name and description for a skill directory.
/// Prefers SKILL.md frontmatter; falls back to folder name + plugin.json description.
fn extract_skill_info(skill_dir: &Path, repo_dir: &Path) -> (String, Option<String>) {
    let skill_md = skill_dir.join("SKILL.md");
    if skill_md.exists() {
        if let Some((name, desc)) = parse_skill_md(&skill_md) {
            return (name, desc);
        }
    }
    // Fallback: folder name + optional plugin.json description
    let name = skill_dir
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let desc = read_plugin_description(repo_dir);
    (name, desc)
}

fn is_hidden_dir_name(name: &str) -> bool {
    name.starts_with('.')
}

fn is_known_root_scan_dir(name: &str) -> bool {
    SKILL_SCAN_BASES
        .iter()
        .filter_map(|base| base.split('/').next())
        .any(|base| base == name)
}

fn is_skill_container_dir_name(name: &str) -> bool {
    let normalized = name.to_ascii_lowercase();
    normalized.contains("skill")
}

fn push_skill_dirs_from_base(out: &mut Vec<PathBuf>, base_dir: &Path) {
    if let Ok(rd) = std::fs::read_dir(base_dir) {
        for entry in rd.flatten() {
            let p = entry.path();
            if is_skill_dir(&p) {
                out.push(p);
            }
        }
    }
}

fn collect_skill_dirs(repo_dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();

    // 1) Fast path: known skill locations such as skills/* and .claude/skills/*.
    for base in SKILL_SCAN_BASES {
        push_skill_dirs_from_base(&mut out, &repo_dir.join(base));
    }

    // 2) Root-level skills: repo/my-skill/SKILL.md.
    // 3) Root-level skill containers: repo/*skill*/my-skill/SKILL.md.
    if let Ok(rd) = std::fs::read_dir(repo_dir) {
        for entry in rd.flatten() {
            let p = entry.path();
            if !p.is_dir() {
                continue;
            }
            let dir_name = entry.file_name();
            let dir_name = dir_name.to_string_lossy();
            if is_hidden_dir_name(&dir_name) || is_known_root_scan_dir(&dir_name) {
                continue;
            }
            if p.join("SKILL.md").exists() {
                out.push(p);
            } else if is_skill_container_dir_name(&dir_name) {
                push_skill_dirs_from_base(&mut out, &p);
            }
        }
    }

    out.sort();
    out.dedup();
    out
}

/// Scan all skill candidates in a repo directory, returning (name, relative_subpath) pairs.
/// Used for auto-matching when updating legacy skills with missing source_subpath.
fn scan_skill_candidates_in_dir(repo_dir: &Path) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for p in collect_skill_dirs(repo_dir) {
        let (name, _) = extract_skill_info(&p, repo_dir);
        let rel = p
            .strip_prefix(repo_dir)
            .unwrap_or(&p)
            .to_string_lossy()
            .to_string();
        out.push((name, rel));
    }
    out
}

/// Count skill directories in a repo: checks both `skills/*` and root-level subdirectories.
fn count_skills_in_repo(repo_dir: &Path) -> usize {
    collect_skill_dirs(repo_dir).len()
}

fn compute_content_hash(path: &Path) -> Option<String> {
    if should_compute_content_hash() {
        hash_dir(path).ok()
    } else {
        None
    }
}

fn should_compute_content_hash() -> bool {
    if cfg!(debug_assertions) {
        return true;
    }
    std::env::var("SKILLS_HUB_COMPUTE_HASH")
        .ok()
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

pub struct UpdateResult {
    pub skill_id: String,
    pub name: String,
    #[allow(dead_code)]
    pub central_path: PathBuf,
    pub content_hash: Option<String>,
    pub source_revision: Option<String>,
    pub updated_targets: Vec<String>,
}

pub fn update_managed_skill_from_source<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    store: &SkillStore,
    skill_id: &str,
) -> Result<UpdateResult> {
    let mut source_updated = false;
    let result = update_managed_skill_from_source_inner(app, store, skill_id, &mut source_updated);
    if result.is_err() && !source_updated {
        if let Ok(Some(mut skill)) = store.get_skill_by_id(skill_id) {
            skill.status = "error".to_string();
            if let Err(err) = store.upsert_skill(&skill) {
                eprintln!("[update] failed to persist Skill error status: {err:#}");
            }
        }
    }
    result
}

fn update_managed_skill_from_source_inner<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    store: &SkillStore,
    skill_id: &str,
    source_updated: &mut bool,
) -> Result<UpdateResult> {
    let record = store
        .get_skill_by_id(skill_id)?
        .ok_or_else(|| anyhow::anyhow!("skill not found"))?;

    let central_path = PathBuf::from(record.central_path.clone());
    if !central_path.exists() {
        anyhow::bail!("central path not found: {:?}", central_path);
    }
    let central_parent = central_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("invalid central path"))?
        .to_path_buf();

    let now = now_ms();

    // Build new content in a sibling temp dir for safe swap.
    let staging_dir = central_parent.join(format!(".skills-hub-update-{}", Uuid::new_v4()));
    if staging_dir.exists() {
        let _ = std::fs::remove_dir_all(&staging_dir);
    }

    let mut new_revision: Option<String> = None;

    if record.source_type == "git" {
        let repo_url = record
            .source_ref
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("missing source_ref for git skill"))?;
        let parsed = parse_github_url(repo_url);

        let (repo_dir, rev) = if let Some(subpath) = record.source_subpath.as_deref() {
            clone_to_cache_subpath(
                app,
                store,
                &parsed.clone_url,
                parsed.branch.as_deref(),
                subpath,
                None,
            )?
        } else {
            clone_to_cache(
                app,
                store,
                &parsed.clone_url,
                parsed.branch.as_deref(),
                None,
            )?
        };
        new_revision = Some(rev);

        // Prefer stored source_subpath (from install time) over URL-parsed subpath.
        // For legacy records where source_subpath is NULL and URL has no subpath,
        // try to auto-match by skill name in the repo (backfill).
        let mut resolved_subpath = record
            .source_subpath
            .as_deref()
            .or(parsed.subpath.as_deref())
            .map(|s| s.to_string());
        if resolved_subpath.is_none() && count_skills_in_repo(&repo_dir) >= 2 {
            // Multi-skill repo with no stored subpath: match by name
            let candidates = scan_skill_candidates_in_dir(&repo_dir);
            let skill_name = record.name.to_lowercase();
            if let Some(matched) = candidates.iter().find(|c| c.0 == record.name).or_else(|| {
                // Fuzzy: bidirectional containment (e.g. "react-best-practices" vs "vercel-react-best-practices")
                let fuzzy: Vec<_> = candidates
                    .iter()
                    .filter(|c| {
                        let cn = c.0.to_lowercase();
                        cn.contains(&skill_name) || skill_name.contains(&cn)
                    })
                    .collect();
                if fuzzy.len() == 1 {
                    Some(fuzzy[0])
                } else {
                    None
                }
            }) {
                resolved_subpath = Some(matched.1.clone());
                // Backfill source_subpath for future updates
                let mut patched = record.clone();
                patched.source_subpath = Some(matched.1.clone());
                let _ = store.upsert_skill(&patched);
            }
        }
        let copy_src = if let Some(subpath) = &resolved_subpath {
            repo_dir.join(subpath)
        } else {
            repo_dir.clone()
        };
        if !copy_src.exists() {
            anyhow::bail!("path not found in repo: {:?}", copy_src);
        }

        copy_dir_recursive(&copy_src, &staging_dir)
            .with_context(|| format!("copy {:?} -> {:?}", copy_src, staging_dir))?;
    } else if record.source_type == "local" {
        let source = record
            .source_ref
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("missing source_ref for local skill"))?;
        let source_path = PathBuf::from(source);
        if !source_path.exists() {
            anyhow::bail!("source path not found: {:?}", source_path);
        }
        copy_dir_recursive(&source_path, &staging_dir)
            .with_context(|| format!("copy {:?} -> {:?}", source_path, staging_dir))?;
    } else {
        anyhow::bail!("unsupported source_type for update: {}", record.source_type);
    }

    // Swap: remove old dir and rename staging into place (best effort).
    std::fs::remove_dir_all(&central_path)
        .with_context(|| format!("failed to remove old central dir {:?}", central_path))?;
    if let Err(err) = std::fs::rename(&staging_dir, &central_path) {
        // Fallback for cross-device rename: copy then delete staging.
        copy_dir_recursive(&staging_dir, &central_path)
            .with_context(|| format!("fallback copy {:?} -> {:?}", staging_dir, central_path))?;
        let _ = std::fs::remove_dir_all(&staging_dir);
        // Still surface original rename error in logs for troubleshooting.
        eprintln!("[update] rename warning: {}", err);
    }

    let content_hash = compute_content_hash(&central_path);
    let description = parse_skill_md(&central_path.join("SKILL.md"))
        .and_then(|(_, desc)| desc)
        .or(record.description.clone());

    // Update DB skill row.
    let updated = SkillRecord {
        id: record.id.clone(),
        name: record.name.clone(),
        description,
        source_type: record.source_type.clone(),
        source_ref: record.source_ref.clone(),
        source_subpath: record.source_subpath.clone(),
        source_revision: new_revision.clone().or(record.source_revision.clone()),
        central_path: record.central_path.clone(),
        content_hash: content_hash.clone(),
        created_at: record.created_at,
        updated_at: now,
        last_sync_at: record.last_sync_at,
        last_seen_at: now,
        enabled: record.enabled,
        status: "ok".to_string(),
    };
    store.upsert_skill(&updated)?;
    *source_updated = true;

    // If any targets are "copy", re-sync them so changes propagate. Symlinks update automatically.
    // Cursor 目前不支持软链/junction，因此无论历史 mode 如何，都需要强制 copy 回灌。
    let targets = store.list_skill_targets(skill_id)?;
    let mut updated_targets: Vec<String> = Vec::new();
    for t in targets {
        if t.status == "disabled" {
            continue;
        }
        // Project scoped targets live under a project root and do not require global tool install detection.
        if t.scope == "global" {
            if let Some(adapter) = adapter_by_key(&t.tool) {
                if !is_tool_installed(&adapter).unwrap_or(false) {
                    continue;
                }
            }
        }
        let force_copy = t.mode == "copy" || t.tool == "cursor";
        if force_copy {
            let target_path = PathBuf::from(&t.target_path);
            let sync_res = match sync_dir_copy_with_overwrite(&central_path, &target_path, true) {
                Ok(result) => result,
                Err(err) => {
                    record_target_sync_failure(store, &t, &format!("{err:#}"))?;
                    return Err(err);
                }
            };
            let record = super::skill_store::SkillTargetRecord {
                id: t.id.clone(),
                skill_id: t.skill_id.clone(),
                tool: t.tool.clone(),
                scope: t.scope.clone(),
                project_path: t.project_path.clone(),
                target_path: sync_res.target_path.to_string_lossy().to_string(),
                mode: "copy".to_string(),
                status: "ok".to_string(),
                last_error: None,
                synced_at: Some(now),
            };
            store.upsert_skill_target(&record)?;
            updated_targets.push(t.tool.clone());
        }
    }

    Ok(UpdateResult {
        skill_id: record.id,
        name: record.name,
        central_path,
        content_hash,
        source_revision: new_revision,
        updated_targets,
    })
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct GitSkillCandidate {
    pub name: String,
    pub description: Option<String>,
    pub subpath: String,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct LocalSkillCandidate {
    pub name: String,
    pub description: Option<String>,
    pub subpath: String,
    pub valid: bool,
    pub reason: Option<String>,
}

pub fn list_git_skills<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    store: &SkillStore,
    repo_url: &str,
) -> Result<Vec<GitSkillCandidate>> {
    let parsed = parse_github_url(repo_url);
    let (repo_dir, _rev) = clone_to_cache(
        app,
        store,
        &parsed.clone_url,
        parsed.branch.as_deref(),
        None,
    )?;

    let mut out: Vec<GitSkillCandidate> = Vec::new();

    // If user provided a folder URL, treat it as a single candidate.
    if let Some(subpath) = &parsed.subpath {
        let dir = repo_dir.join(subpath);
        if dir.is_dir() && (dir.join("SKILL.md").exists() || is_claude_skill_dir(&dir)) {
            let (name, desc) = extract_skill_info(&dir, &repo_dir);
            out.push(GitSkillCandidate {
                name,
                description: desc,
                subpath: subpath.to_string(),
            });
        } else if dir.is_dir() {
            for p in collect_skill_dirs(&dir) {
                let (name, desc) = extract_skill_info(&p, &repo_dir);
                let rel = p
                    .strip_prefix(&repo_dir)
                    .unwrap_or(&p)
                    .to_string_lossy()
                    .to_string();
                out.push(GitSkillCandidate {
                    name,
                    description: desc,
                    subpath: rel,
                });
            }
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));
        out.dedup_by(|a, b| a.subpath == b.subpath);
        return Ok(out);
    }

    // Root-level skill
    let root_skill = repo_dir.join("SKILL.md");
    if root_skill.exists() {
        let (name, desc) = parse_skill_md(&root_skill).unwrap_or(("root-skill".to_string(), None));
        out.push(GitSkillCandidate {
            name,
            description: desc,
            subpath: ".".to_string(),
        });
    }

    for p in collect_skill_dirs(&repo_dir) {
        let (name, desc) = extract_skill_info(&p, &repo_dir);
        let rel = p
            .strip_prefix(&repo_dir)
            .unwrap_or(&p)
            .to_string_lossy()
            .to_string();
        out.push(GitSkillCandidate {
            name,
            description: desc,
            subpath: rel,
        });
    }

    out.sort_by(|a, b| a.name.cmp(&b.name));
    out.dedup_by(|a, b| a.subpath == b.subpath);

    Ok(out)
}

pub fn list_local_skills(base_path: &Path) -> Result<Vec<LocalSkillCandidate>> {
    if !base_path.exists() {
        anyhow::bail!("source path not found: {:?}", base_path);
    }

    let mut out: Vec<LocalSkillCandidate> = Vec::new();

    let root_skill = base_path.join("SKILL.md");
    if root_skill.exists() {
        match parse_skill_md_with_reason(&root_skill) {
            Ok((name, desc)) => {
                out.push(LocalSkillCandidate {
                    name,
                    description: desc,
                    subpath: ".".to_string(),
                    valid: true,
                    reason: None,
                });
            }
            Err(reason) => {
                let fallback_name = base_path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                out.push(LocalSkillCandidate {
                    name: if fallback_name.is_empty() {
                        "root-skill".to_string()
                    } else {
                        fallback_name
                    },
                    description: None,
                    subpath: ".".to_string(),
                    valid: false,
                    reason: Some(reason.to_string()),
                });
            }
        }
    }

    for base in SKILL_SCAN_BASES {
        let base_dir = base_path.join(base);
        if !base_dir.exists() {
            continue;
        }
        if let Ok(rd) = std::fs::read_dir(&base_dir) {
            for entry in rd.flatten() {
                let p = entry.path();
                if !p.is_dir() {
                    continue;
                }
                let skill_md = p.join("SKILL.md");
                let rel = p
                    .strip_prefix(base_path)
                    .unwrap_or(&p)
                    .to_string_lossy()
                    .to_string();
                if skill_md.exists() {
                    match parse_skill_md_with_reason(&skill_md) {
                        Ok((name, desc)) => {
                            out.push(LocalSkillCandidate {
                                name,
                                description: desc,
                                subpath: rel,
                                valid: true,
                                reason: None,
                            });
                        }
                        Err(reason) => {
                            out.push(LocalSkillCandidate {
                                name: p
                                    .file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy()
                                    .to_string(),
                                description: None,
                                subpath: rel,
                                valid: false,
                                reason: Some(reason.to_string()),
                            });
                        }
                    }
                } else if is_claude_skill_dir(&p) {
                    // .claude/skills/* directories are valid without SKILL.md
                    let name = p
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    let desc = read_plugin_description(base_path);
                    out.push(LocalSkillCandidate {
                        name,
                        description: desc,
                        subpath: rel,
                        valid: true,
                        reason: None,
                    });
                } else {
                    out.push(LocalSkillCandidate {
                        name: p
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string(),
                        description: None,
                        subpath: rel,
                        valid: false,
                        reason: Some("missing_skill_md".to_string()),
                    });
                }
            }
        }
    }

    // Also scan root-level directories for skills (matching collect_skill_dirs behavior).
    // This handles the case where the user selects a directory that directly contains
    // skill subdirectories (e.g. a "skills" directory with article-writer/SKILL.md).
    if let Ok(rd) = std::fs::read_dir(base_path) {
        for entry in rd.flatten() {
            let p = entry.path();
            if !p.is_dir() {
                continue;
            }
            let dir_name = entry.file_name();
            let dir_name = dir_name.to_string_lossy();
            if is_hidden_dir_name(&dir_name) || is_known_root_scan_dir(&dir_name) {
                continue;
            }
            let rel = p
                .strip_prefix(base_path)
                .unwrap_or(&p)
                .to_string_lossy()
                .to_string();
            if p.join("SKILL.md").exists() {
                match parse_skill_md_with_reason(&p.join("SKILL.md")) {
                    Ok((name, desc)) => {
                        out.push(LocalSkillCandidate {
                            name,
                            description: desc,
                            subpath: rel,
                            valid: true,
                            reason: None,
                        });
                    }
                    Err(reason) => {
                        out.push(LocalSkillCandidate {
                            name: dir_name.to_string(),
                            description: None,
                            subpath: rel,
                            valid: false,
                            reason: Some(reason.to_string()),
                        });
                    }
                }
            } else if is_skill_container_dir_name(&dir_name) {
                // Scan children of skill container directories.
                if let Ok(sub_rd) = std::fs::read_dir(&p) {
                    for sub_entry in sub_rd.flatten() {
                        let sub_p = sub_entry.path();
                        if !sub_p.is_dir() {
                            continue;
                        }
                        let sub_rel = sub_p
                            .strip_prefix(base_path)
                            .unwrap_or(&sub_p)
                            .to_string_lossy()
                            .to_string();
                        if sub_p.join("SKILL.md").exists() {
                            match parse_skill_md_with_reason(&sub_p.join("SKILL.md")) {
                                Ok((name, desc)) => {
                                    out.push(LocalSkillCandidate {
                                        name,
                                        description: desc,
                                        subpath: sub_rel,
                                        valid: true,
                                        reason: None,
                                    });
                                }
                                Err(reason) => {
                                    out.push(LocalSkillCandidate {
                                        name: sub_entry.file_name().to_string_lossy().to_string(),
                                        description: None,
                                        subpath: sub_rel,
                                        valid: false,
                                        reason: Some(reason.to_string()),
                                    });
                                }
                            }
                        } else if is_claude_skill_dir(&sub_p) {
                            let name = sub_entry.file_name().to_string_lossy().to_string();
                            let desc = read_plugin_description(base_path);
                            out.push(LocalSkillCandidate {
                                name,
                                description: desc,
                                subpath: sub_rel,
                                valid: true,
                                reason: None,
                            });
                        } else {
                            out.push(LocalSkillCandidate {
                                name: sub_entry.file_name().to_string_lossy().to_string(),
                                description: None,
                                subpath: sub_rel,
                                valid: false,
                                reason: Some("missing_skill_md".to_string()),
                            });
                        }
                    }
                }
            }
        }
    }

    out.sort_by(|a, b| a.name.cmp(&b.name));
    out.dedup_by(|a, b| a.subpath == b.subpath);

    Ok(out)
}

pub fn install_git_skill_from_selection<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    store: &SkillStore,
    repo_url: &str,
    subpath: &str,
    name: Option<String>,
) -> Result<InstallResult> {
    let parsed = parse_github_url(repo_url);
    let user_provided_name = name.is_some();
    let mut display_name = name.unwrap_or_else(|| {
        if subpath == "." {
            derive_name_from_repo_url(&parsed.clone_url)
        } else {
            subpath
                .rsplit('/')
                .next()
                .map(|s| s.to_string())
                .unwrap_or_else(|| derive_name_from_repo_url(&parsed.clone_url))
        }
    });

    let central_dir = resolve_central_repo_path(app, store)?;
    ensure_central_repo(&central_dir)?;
    let mut central_path = central_dir.join(&display_name);
    if central_path.exists() {
        anyhow::bail!("skill already exists in central repo: {:?}", central_path);
    }

    let (repo_dir, revision) = clone_to_cache(
        app,
        store,
        &parsed.clone_url,
        parsed.branch.as_deref(),
        None,
    )?;

    let copy_src = if subpath == "." {
        repo_dir.clone()
    } else {
        repo_dir.join(subpath)
    };
    if !copy_src.exists() {
        anyhow::bail!("path not found in repo: {:?}", copy_src);
    }
    ensure_installable_skill_dir(&copy_src)?;

    copy_dir_recursive(&copy_src, &central_path)
        .with_context(|| format!("copy {:?} -> {:?}", copy_src, central_path))?;

    // Prefer name from SKILL.md over derived name (fixes #28).
    let (mut description, md_name) = match parse_skill_md(&central_path.join("SKILL.md")) {
        Some((n, d)) => (d, Some(n)),
        None => (None, None),
    };
    if !user_provided_name {
        if let Some(ref better_name) = md_name {
            if *better_name != display_name {
                let new_central = central_dir.join(better_name);
                if !new_central.exists() {
                    std::fs::rename(&central_path, &new_central).with_context(|| {
                        format!("rename {:?} -> {:?}", central_path, new_central)
                    })?;
                    display_name = better_name.clone();
                    central_path = new_central;
                    description =
                        parse_skill_md(&central_path.join("SKILL.md")).and_then(|(_, d)| d);
                }
            }
        }
    }

    let now = now_ms();
    let content_hash = compute_content_hash(&central_path);
    let source_subpath = if subpath == "." {
        None
    } else {
        Some(subpath.to_string())
    };
    let record = SkillRecord {
        id: Uuid::new_v4().to_string(),
        name: display_name,
        description,
        source_type: "git".to_string(),
        source_ref: Some(repo_url.to_string()),
        source_subpath,
        source_revision: Some(revision),
        central_path: central_path.to_string_lossy().to_string(),
        content_hash: content_hash.clone(),
        created_at: now,
        updated_at: now,
        last_sync_at: None,
        last_seen_at: now,
        enabled: true,
        status: "ok".to_string(),
    };
    store.upsert_skill(&record)?;

    Ok(InstallResult {
        skill_id: record.id,
        name: record.name,
        central_path,
        content_hash,
    })
}

pub fn install_local_skill_from_selection<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    store: &SkillStore,
    base_path: &Path,
    subpath: &str,
    name: Option<String>,
) -> Result<InstallResult> {
    if !base_path.exists() {
        anyhow::bail!("source path not found: {:?}", base_path);
    }

    let selected_dir = if subpath == "." {
        base_path.to_path_buf()
    } else {
        base_path.join(subpath)
    };
    if !selected_dir.exists() {
        anyhow::bail!("source path not found: {:?}", selected_dir);
    }

    let skill_md = selected_dir.join("SKILL.md");
    if !skill_md.exists() {
        anyhow::bail!("SKILL_INVALID|missing_skill_md");
    }
    let (parsed_name, _desc) = parse_skill_md_with_reason(&skill_md)
        .map_err(|reason| anyhow::anyhow!("SKILL_INVALID|{}", reason))?;

    let display_name = name.unwrap_or(parsed_name);

    install_local_skill(app, store, &selected_dir, Some(display_name))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct RepoCacheMeta {
    last_fetched_ms: i64,
    head: Option<String>,
}

static GIT_CACHE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn clone_to_cache<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    store: &SkillStore,
    clone_url: &str,
    branch: Option<&str>,
    cancel: Option<&CancelToken>,
) -> Result<(PathBuf, String)> {
    let started = std::time::Instant::now();
    let cache_dir = app
        .path()
        .app_cache_dir()
        .context("failed to resolve app cache dir")?;
    let cache_root = cache_dir.join("skills-hub-git-cache");
    std::fs::create_dir_all(&cache_root)
        .with_context(|| format!("failed to create cache dir {:?}", cache_root))?;

    let repo_dir = cache_root.join(repo_cache_key(clone_url, branch, None));
    let meta_path = repo_dir.join(".skills-hub-cache.json");

    let lock = GIT_CACHE_LOCK.get_or_init(|| Mutex::new(()));
    let _guard = lock.lock().unwrap_or_else(|err| err.into_inner());

    if repo_dir.join(".git").exists() {
        if let Ok(meta) = std::fs::read_to_string(&meta_path) {
            if let Ok(meta) = serde_json::from_str::<RepoCacheMeta>(&meta) {
                if let Some(head) = meta.head {
                    let ttl_ms = get_git_cache_ttl_secs(store).saturating_mul(1000);
                    if ttl_ms > 0 && now_ms().saturating_sub(meta.last_fetched_ms) < ttl_ms {
                        log::info!(
                            "[installer] git cache hit (fresh) {}s url={} branch={:?} repo_dir={:?}",
                            started.elapsed().as_secs_f32(),
                            clone_url,
                            branch,
                            repo_dir
                        );
                        return Ok((repo_dir, head));
                    }
                }
            }
        }
    }

    log::info!(
        "[installer] git cache miss/stale; fetching {} url={} branch={:?} repo_dir={:?}",
        started.elapsed().as_secs_f32(),
        clone_url,
        branch,
        repo_dir
    );

    let proxy_url = get_github_proxy_url(store)?;
    let rev = match clone_or_pull(clone_url, &repo_dir, branch, cancel, Some(&proxy_url)) {
        Ok(rev) => rev,
        Err(err) => {
            // If cache got corrupted, retry once from a clean state.
            if repo_dir.exists() {
                let _ = std::fs::remove_dir_all(&repo_dir);
            }
            clone_or_pull(clone_url, &repo_dir, branch, cancel, Some(&proxy_url))
                .with_context(|| format!("{:#}", err))?
        }
    };

    let _ = std::fs::write(
        &meta_path,
        serde_json::to_string(&RepoCacheMeta {
            last_fetched_ms: now_ms(),
            head: Some(rev.clone()),
        })
        .unwrap_or_else(|_| "{}".to_string()),
    );

    log::info!(
        "[installer] git cache ready {}s url={} branch={:?} head={}",
        started.elapsed().as_secs_f32(),
        clone_url,
        branch,
        rev
    );
    Ok((repo_dir, rev))
}

fn clone_to_cache_subpath<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    store: &SkillStore,
    clone_url: &str,
    branch: Option<&str>,
    subpath: &str,
    cancel: Option<&CancelToken>,
) -> Result<(PathBuf, String)> {
    let started = std::time::Instant::now();
    let cache_dir = app
        .path()
        .app_cache_dir()
        .context("failed to resolve app cache dir")?;
    let cache_root = cache_dir.join("skills-hub-git-cache");
    std::fs::create_dir_all(&cache_root)
        .with_context(|| format!("failed to create cache dir {:?}", cache_root))?;

    let repo_dir = cache_root.join(repo_cache_key(clone_url, branch, Some(subpath)));
    let meta_path = repo_dir.join(".skills-hub-cache.json");

    let lock = GIT_CACHE_LOCK.get_or_init(|| Mutex::new(()));
    let _guard = lock.lock().unwrap_or_else(|err| err.into_inner());

    if repo_dir.join(".git").exists() {
        if let Ok(meta) = std::fs::read_to_string(&meta_path) {
            if let Ok(meta) = serde_json::from_str::<RepoCacheMeta>(&meta) {
                if let Some(head) = meta.head {
                    let ttl_ms = get_git_cache_ttl_secs(store).saturating_mul(1000);
                    if ttl_ms > 0 && now_ms().saturating_sub(meta.last_fetched_ms) < ttl_ms {
                        log::info!(
                            "[installer] sparse git cache hit (fresh) {}s url={} branch={:?} subpath={} repo_dir={:?}",
                            started.elapsed().as_secs_f32(),
                            clone_url,
                            branch,
                            subpath,
                            repo_dir
                        );
                        return Ok((repo_dir, head));
                    }
                }
            }
        }
    }

    log::info!(
        "[installer] sparse git cache miss/stale; fetching {} url={} branch={:?} subpath={} repo_dir={:?}",
        started.elapsed().as_secs_f32(),
        clone_url,
        branch,
        subpath,
        repo_dir
    );

    let proxy_url = get_github_proxy_url(store)?;
    let rev = match clone_or_pull_sparse(
        clone_url,
        &repo_dir,
        branch,
        subpath,
        cancel,
        Some(&proxy_url),
    ) {
        Ok(rev) => rev,
        Err(err) => {
            if repo_dir.exists() {
                let _ = std::fs::remove_dir_all(&repo_dir);
            }
            clone_or_pull_sparse(
                clone_url,
                &repo_dir,
                branch,
                subpath,
                cancel,
                Some(&proxy_url),
            )
            .with_context(|| format!("{:#}", err))?
        }
    };

    let _ = std::fs::write(
        &meta_path,
        serde_json::to_string(&RepoCacheMeta {
            last_fetched_ms: now_ms(),
            head: Some(rev.clone()),
        })
        .unwrap_or_else(|_| "{}".to_string()),
    );

    log::info!(
        "[installer] sparse git cache ready {}s url={} branch={:?} subpath={} head={}",
        started.elapsed().as_secs_f32(),
        clone_url,
        branch,
        subpath,
        rev
    );
    Ok((repo_dir, rev))
}

fn repo_cache_key(clone_url: &str, branch: Option<&str>, subpath: Option<&str>) -> String {
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    hasher.update(clone_url.as_bytes());
    hasher.update(b"\n");
    if let Some(b) = branch {
        hasher.update(b.as_bytes());
    }
    hasher.update(b"\n");
    if let Some(s) = subpath {
        hasher.update(s.as_bytes());
    }
    hex::encode(hasher.finalize())
}

/// Backfill description for skills from SKILL.md.
pub fn backfill_skill_descriptions(store: &SkillStore) {
    let skills = match store.list_skills() {
        Ok(s) => s,
        Err(_) => return,
    };
    for skill in skills {
        let central = std::path::Path::new(&skill.central_path);
        let skill_md = central.join("SKILL.md");
        if let Some((_, Some(desc))) = parse_skill_md(&skill_md) {
            if skill.description.as_deref() != Some(desc.as_str()) {
                let _ = store.update_skill_description(&skill.id, Some(&desc));
            }
        }
    }
}

fn parse_skill_md(path: &Path) -> Option<(String, Option<String>)> {
    parse_skill_md_with_reason(path).ok()
}

fn parse_skill_md_with_reason(path: &Path) -> Result<(String, Option<String>), &'static str> {
    let text = std::fs::read_to_string(path).map_err(|_| "read_failed")?;
    let lines: Vec<&str> = text.lines().collect();
    if lines.first().map(|v| v.trim()) != Some("---") {
        return Err("invalid_frontmatter");
    }
    let mut name: Option<String> = None;
    let mut desc: Option<String> = None;
    let mut found_end = false;
    let mut i = 1usize;
    while i < lines.len() {
        let raw = lines[i];
        let l = raw.trim();
        if l == "---" {
            found_end = true;
            break;
        }
        if let Some(v) = l.strip_prefix("name:") {
            name = Some(clean_frontmatter_value(v));
        } else if let Some(v) = l.strip_prefix("description:") {
            let v = v.trim();
            if let Some(block_style) = frontmatter_block_style(v) {
                let folded = block_style == '>';
                let mut block_lines: Vec<String> = Vec::new();
                while i + 1 < lines.len() {
                    let next = lines[i + 1];
                    if next.trim() == "---" {
                        break;
                    }
                    if !next.trim().is_empty() && !next.starts_with(char::is_whitespace) {
                        break;
                    }
                    block_lines.push(next.strip_prefix("  ").unwrap_or(next).to_string());
                    i += 1;
                }
                let value = if folded {
                    block_lines
                        .iter()
                        .map(|line| line.trim())
                        .filter(|line| !line.is_empty())
                        .collect::<Vec<_>>()
                        .join(" ")
                } else {
                    block_lines.join("\n").trim().to_string()
                };
                desc = Some(value);
            } else {
                desc = Some(clean_frontmatter_value(v));
            }
        }
        i += 1;
    }
    if !found_end {
        return Err("invalid_frontmatter");
    }
    let name = name.ok_or("missing_name")?;
    Ok((name, desc))
}

fn clean_frontmatter_value(value: &str) -> String {
    let value = value.trim();
    if value.len() >= 2
        && ((value.starts_with('"') && value.ends_with('"'))
            || (value.starts_with('\'') && value.ends_with('\'')))
    {
        value[1..value.len() - 1].to_string()
    } else {
        value.to_string()
    }
}

fn frontmatter_block_style(value: &str) -> Option<char> {
    let mut chars = value.chars();
    let style = chars.next()?;
    if style != '|' && style != '>' {
        return None;
    }
    match chars.next() {
        None => Some(style),
        Some('-' | '+') if chars.next().is_none() => Some(style),
        _ => None,
    }
}

#[cfg(test)]
#[path = "tests/installer.rs"]
mod tests;
