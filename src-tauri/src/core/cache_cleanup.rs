use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result};
use serde::Deserialize;
use tauri::Manager;

use super::skill_store::SkillStore;

pub const CACHE_DIR_NAME: &str = "skills-syncer-git-cache";
pub const CACHE_META_FILE: &str = ".skills-syncer-cache.json";
pub const LEGACY_CACHE_DIR_NAME: &str = "skills-hub-git-cache";
pub const LEGACY_CACHE_META_FILE: &str = ".skills-hub-cache.json";
pub const GIT_CACHE_CLEANUP_DAYS_KEY: &str = "git_cache_cleanup_days";
pub const DEFAULT_GIT_CACHE_CLEANUP_DAYS: i64 = 30;
const MAX_GIT_CACHE_CLEANUP_DAYS: i64 = 3650;
pub const GIT_CACHE_TTL_SECS_KEY: &str = "git_cache_ttl_secs";
pub const DEFAULT_GIT_CACHE_TTL_SECS: i64 = 60;
const MAX_GIT_CACHE_TTL_SECS: i64 = 3600;

#[derive(Debug, Deserialize)]
struct RepoCacheMeta {
    last_fetched_ms: i64,
}

pub fn get_git_cache_cleanup_days(store: &SkillStore) -> i64 {
    let raw = store.get_setting(GIT_CACHE_CLEANUP_DAYS_KEY).ok().flatten();
    parse_cleanup_days(raw).unwrap_or(DEFAULT_GIT_CACHE_CLEANUP_DAYS)
}

pub fn set_git_cache_cleanup_days(store: &SkillStore, days: i64) -> Result<i64> {
    if !(0..=MAX_GIT_CACHE_CLEANUP_DAYS).contains(&days) {
        anyhow::bail!(
            "cleanup days must be between 0 and {}",
            MAX_GIT_CACHE_CLEANUP_DAYS
        );
    }
    store.set_setting(GIT_CACHE_CLEANUP_DAYS_KEY, &days.to_string())?;
    Ok(days)
}

pub fn get_git_cache_ttl_secs(store: &SkillStore) -> i64 {
    let raw = store.get_setting(GIT_CACHE_TTL_SECS_KEY).ok().flatten();
    parse_cache_ttl_secs(raw).unwrap_or(DEFAULT_GIT_CACHE_TTL_SECS)
}

pub fn set_git_cache_ttl_secs(store: &SkillStore, secs: i64) -> Result<i64> {
    if !(0..=MAX_GIT_CACHE_TTL_SECS).contains(&secs) {
        anyhow::bail!(
            "cache ttl seconds must be between 0 and {}",
            MAX_GIT_CACHE_TTL_SECS
        );
    }
    store.set_setting(GIT_CACHE_TTL_SECS_KEY, &secs.to_string())?;
    Ok(secs)
}

pub fn cleanup_git_cache_dirs<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    max_age: Duration,
) -> Result<usize> {
    let cache_dir = app
        .path()
        .app_cache_dir()
        .context("failed to resolve app cache dir")?;
    cleanup_git_cache_dirs_in(&cache_dir, max_age)
}

fn cleanup_git_cache_dirs_in(cache_dir: &Path, max_age: Duration) -> Result<usize> {
    let mut removed = 0usize;
    for cache_name in [CACHE_DIR_NAME, LEGACY_CACHE_DIR_NAME] {
        removed += cleanup_git_cache_root(&cache_dir.join(cache_name), max_age)?;
    }
    Ok(removed)
}

fn cleanup_git_cache_root(cache_root: &Path, max_age: Duration) -> Result<usize> {
    if !cache_root.exists() {
        return Ok(0);
    }

    let cutoff_ms = now_ms().saturating_sub(max_age.as_millis().try_into().unwrap_or(i64::MAX));
    let cutoff_time = SystemTime::now()
        .checked_sub(max_age)
        .unwrap_or(SystemTime::UNIX_EPOCH);

    let mut removed = 0usize;
    let rd = match std::fs::read_dir(cache_root) {
        Ok(v) => v,
        Err(err) => {
            return Err(anyhow::anyhow!(
                "failed to read cache dir {:?}: {}",
                cache_root,
                err
            ));
        }
    };

    for entry in rd.flatten() {
        let path: PathBuf = entry.path();
        if !path.is_dir() {
            continue;
        }

        if !path.join(".git").exists() {
            continue;
        }

        let meta_path = path.join(CACHE_META_FILE);
        let legacy_meta_path = path.join(LEGACY_CACHE_META_FILE);
        let mut should_remove = false;

        for candidate in [&meta_path, &legacy_meta_path] {
            if let Ok(raw) = std::fs::read_to_string(candidate) {
                if let Ok(meta) = serde_json::from_str::<RepoCacheMeta>(&raw) {
                    if meta.last_fetched_ms > 0 && meta.last_fetched_ms <= cutoff_ms {
                        should_remove = true;
                        break;
                    }
                }
            }
        }

        if !should_remove {
            let meta = match std::fs::metadata(&path) {
                Ok(m) => m,
                Err(_) => continue,
            };
            let modified = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
            if modified <= cutoff_time {
                should_remove = true;
            }
        }

        if should_remove && std::fs::remove_dir_all(&path).is_ok() {
            removed += 1;
        }
    }

    Ok(removed)
}

fn parse_cleanup_days(raw: Option<String>) -> Option<i64> {
    let value = raw?.trim().parse::<i64>().ok()?;
    if !(0..=MAX_GIT_CACHE_CLEANUP_DAYS).contains(&value) {
        None
    } else {
        Some(value)
    }
}

fn parse_cache_ttl_secs(raw: Option<String>) -> Option<i64> {
    let value = raw?.trim().parse::<i64>().ok()?;
    if !(0..=MAX_GIT_CACHE_TTL_SECS).contains(&value) {
        None
    } else {
        Some(value)
    }
}

fn now_ms() -> i64 {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    now.as_millis() as i64
}
