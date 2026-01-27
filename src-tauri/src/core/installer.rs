use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tauri::Manager;
use uuid::Uuid;

use super::cache_cleanup::get_git_cache_ttl_secs;
use super::central_repo::{ensure_central_repo, resolve_central_repo_path};
use super::content_hash::hash_dir;
use super::git_fetcher::clone_or_pull;
use super::skill_store::{SkillRecord, SkillStore};
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

pub fn install_local_skill<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    store: &SkillStore,
    source_path: &Path,
    name: Option<String>,
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
        anyhow::bail!("skill already exists in central repo: {:?}", central_path);
    }

    copy_dir_recursive(source_path, &central_path)
        .with_context(|| format!("copy {:?} -> {:?}", source_path, central_path))?;

    let now = now_ms();
    let content_hash = compute_content_hash(&central_path);

    let record = SkillRecord {
        id: Uuid::new_v4().to_string(),
        name,
        source_type: "local".to_string(),
        source_ref: Some(source_path.to_string_lossy().to_string()),
        source_revision: None,
        central_path: central_path.to_string_lossy().to_string(),
        content_hash: content_hash.clone(),
        created_at: now,
        updated_at: now,
        last_sync_at: None,
        last_seen_at: now,
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
) -> Result<InstallResult> {
    let parsed = parse_github_url(repo_url);
    let name = name.unwrap_or_else(|| {
        if let Some(subpath) = &parsed.subpath {
            subpath
                .rsplit('/')
                .next()
                .map(|s| s.to_string())
                .unwrap_or_else(|| derive_name_from_repo_url(&parsed.clone_url))
        } else {
            derive_name_from_repo_url(&parsed.clone_url)
        }
    });

    let central_dir = resolve_central_repo_path(app, store)?;
    ensure_central_repo(&central_dir)?;
    let central_path = central_dir.join(&name);

    if central_path.exists() {
        anyhow::bail!("skill already exists in central repo: {:?}", central_path);
    }

    // Always clone into a temp dir first, then copy the skill directory into central repo.
    // This avoids storing a full git repo (with .git) inside central repo and allows
    // handling GitHub folder URLs (/tree/<branch>/<path>).
    let (repo_dir, rev) = clone_to_cache(app, store, &parsed.clone_url, parsed.branch.as_deref())?;

    let copy_src = if let Some(subpath) = &parsed.subpath {
        let sub_src = repo_dir.join(subpath);
        if !sub_src.exists() {
            anyhow::bail!("subpath not found in repo: {:?}", sub_src);
        }
        sub_src
    } else {
        // Repo root URL: if it looks like a multi-skill repo, ask user to provide a folder URL.
        let skills_dir = repo_dir.join("skills");
        if skills_dir.exists() {
            let mut count = 0usize;
            if let Ok(rd) = std::fs::read_dir(&skills_dir) {
                for entry in rd.flatten() {
                    let p = entry.path();
                    if p.is_dir() && p.join("SKILL.md").exists() {
                        count += 1;
                    }
                }
            }
            if count >= 2 {
                anyhow::bail!(
          "MULTI_SKILLS|该仓库包含多个 Skills，请复制具体 Skill 文件夹链接（例如 GitHub 的 /tree/<branch>/skills/<name>），再导入。"
        );
            }
        }
        repo_dir.clone()
    };

    copy_dir_recursive(&copy_src, &central_path)
        .with_context(|| format!("copy {:?} -> {:?}", copy_src, central_path))?;

    let revision = rev;
    let now = now_ms();
    let content_hash = compute_content_hash(&central_path);

    let record = SkillRecord {
        id: Uuid::new_v4().to_string(),
        name,
        source_type: "git".to_string(),
        source_ref: Some(repo_url.to_string()),
        source_revision: Some(revision),
        central_path: central_path.to_string_lossy().to_string(),
        content_hash: content_hash.clone(),
        created_at: now,
        updated_at: now,
        last_sync_at: None,
        last_seen_at: now,
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
            Some(parts[4..].join("/"))
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

        let (repo_dir, rev) =
            clone_to_cache(app, store, &parsed.clone_url, parsed.branch.as_deref())?;
        new_revision = Some(rev);

        let copy_src = if let Some(subpath) = &parsed.subpath {
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

    // Update DB skill row.
    let updated = SkillRecord {
        id: record.id.clone(),
        name: record.name.clone(),
        source_type: record.source_type.clone(),
        source_ref: record.source_ref.clone(),
        source_revision: new_revision.clone().or(record.source_revision.clone()),
        central_path: record.central_path.clone(),
        content_hash: content_hash.clone(),
        created_at: record.created_at,
        updated_at: now,
        last_sync_at: record.last_sync_at,
        last_seen_at: now,
        status: "ok".to_string(),
    };
    store.upsert_skill(&updated)?;

    // If any targets are "copy", re-sync them so changes propagate. Symlinks update automatically.
    // Cursor 目前不支持软链/junction，因此无论历史 mode 如何，都需要强制 copy 回灌。
    let targets = store.list_skill_targets(skill_id)?;
    let mut updated_targets: Vec<String> = Vec::new();
    for t in targets {
        // Skip if tool not installed anymore.
        if let Some(adapter) = adapter_by_key(&t.tool) {
            if !is_tool_installed(&adapter).unwrap_or(false) {
                continue;
            }
        }
        let force_copy = t.mode == "copy" || t.tool == "cursor";
        if force_copy {
            let target_path = PathBuf::from(&t.target_path);
            let sync_res = sync_dir_copy_with_overwrite(&central_path, &target_path, true)?;
            let record = super::skill_store::SkillTargetRecord {
                id: t.id.clone(),
                skill_id: t.skill_id.clone(),
                tool: t.tool.clone(),
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

pub fn list_git_skills<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    store: &SkillStore,
    repo_url: &str,
) -> Result<Vec<GitSkillCandidate>> {
    let parsed = parse_github_url(repo_url);
    let (repo_dir, _rev) = clone_to_cache(app, store, &parsed.clone_url, parsed.branch.as_deref())?;

    let mut out: Vec<GitSkillCandidate> = Vec::new();

    // If user provided a folder URL, treat it as a single candidate.
    if let Some(subpath) = &parsed.subpath {
        let dir = repo_dir.join(subpath);
        if dir.is_dir() && dir.join("SKILL.md").exists() {
            let (name, desc) = parse_skill_md(&dir.join("SKILL.md")).unwrap_or((
                dir.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string(),
                None,
            ));
            out.push(GitSkillCandidate {
                name,
                description: desc,
                subpath: subpath.to_string(),
            });
        }
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

    // Standard discovery locations (subset aligned with add-skill):
    // skills/*, skills/.curated/*, skills/.experimental/*, skills/.system/*
    for base in [
        "skills",
        "skills/.curated",
        "skills/.experimental",
        "skills/.system",
    ] {
        let base_dir = repo_dir.join(base);
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
                if !skill_md.exists() {
                    continue;
                }
                let (name, desc) = parse_skill_md(&skill_md).unwrap_or((
                    p.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string(),
                    None,
                ));
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
    let display_name = name.unwrap_or_else(|| {
        subpath
            .rsplit('/')
            .next()
            .map(|s| s.to_string())
            .unwrap_or_else(|| derive_name_from_repo_url(&parsed.clone_url))
    });

    let central_dir = resolve_central_repo_path(app, store)?;
    ensure_central_repo(&central_dir)?;
    let central_path = central_dir.join(&display_name);
    if central_path.exists() {
        anyhow::bail!("skill already exists in central repo: {:?}", central_path);
    }

    let (repo_dir, revision) =
        clone_to_cache(app, store, &parsed.clone_url, parsed.branch.as_deref())?;

    let copy_src = if subpath == "." {
        repo_dir.clone()
    } else {
        repo_dir.join(subpath)
    };
    if !copy_src.exists() {
        anyhow::bail!("path not found in repo: {:?}", copy_src);
    }

    copy_dir_recursive(&copy_src, &central_path)
        .with_context(|| format!("copy {:?} -> {:?}", copy_src, central_path))?;

    let now = now_ms();
    let content_hash = compute_content_hash(&central_path);
    let record = SkillRecord {
        id: Uuid::new_v4().to_string(),
        name: display_name,
        source_type: "git".to_string(),
        source_ref: Some(repo_url.to_string()),
        source_revision: Some(revision),
        central_path: central_path.to_string_lossy().to_string(),
        content_hash: content_hash.clone(),
        created_at: now,
        updated_at: now,
        last_sync_at: None,
        last_seen_at: now,
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
) -> Result<(PathBuf, String)> {
    let started = std::time::Instant::now();
    let cache_dir = app
        .path()
        .app_cache_dir()
        .context("failed to resolve app cache dir")?;
    let cache_root = cache_dir.join("skills-hub-git-cache");
    std::fs::create_dir_all(&cache_root)
        .with_context(|| format!("failed to create cache dir {:?}", cache_root))?;

    let repo_dir = cache_root.join(repo_cache_key(clone_url, branch));
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

    let rev = match clone_or_pull(clone_url, &repo_dir, branch) {
        Ok(rev) => rev,
        Err(err) => {
            // If cache got corrupted, retry once from a clean state.
            if repo_dir.exists() {
                let _ = std::fs::remove_dir_all(&repo_dir);
            }
            clone_or_pull(clone_url, &repo_dir, branch).with_context(|| format!("{:#}", err))?
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

fn repo_cache_key(clone_url: &str, branch: Option<&str>) -> String {
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    hasher.update(clone_url.as_bytes());
    hasher.update(b"\n");
    if let Some(b) = branch {
        hasher.update(b.as_bytes());
    }
    hex::encode(hasher.finalize())
}

fn parse_skill_md(path: &Path) -> Option<(String, Option<String>)> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut lines = text.lines();
    if lines.next()?.trim() != "---" {
        return None;
    }
    let mut name: Option<String> = None;
    let mut desc: Option<String> = None;
    for line in lines.by_ref() {
        let l = line.trim();
        if l == "---" {
            break;
        }
        if let Some(v) = l.strip_prefix("name:") {
            name = Some(v.trim().trim_matches('"').to_string());
        } else if let Some(v) = l.strip_prefix("description:") {
            desc = Some(v.trim().trim_matches('"').to_string());
        }
    }
    let name = name?;
    Some((name, desc))
}

#[cfg(test)]
#[path = "tests/installer.rs"]
mod tests;
