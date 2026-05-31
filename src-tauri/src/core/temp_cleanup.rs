use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result};
use tauri::Manager;

const TEMP_PREFIX: &str = "skills-syncer-git-";
const LEGACY_TEMP_PREFIX: &str = "skills-hub-git-";
const TEMP_MARKER: &str = ".skills-syncer-git-temp";
const LEGACY_TEMP_MARKER: &str = ".skills-hub-git-temp";

#[allow(dead_code)]
pub fn mark_temp_dir(dir: &Path) -> Result<()> {
    let marker = dir.join(TEMP_MARKER);
    if marker.exists() {
        return Ok(());
    }
    std::fs::write(&marker, b"skills-syncer-git-temp-v1\n")
        .with_context(|| format!("failed to write marker {:?}", marker))?;
    Ok(())
}

pub fn cleanup_old_git_temp_dirs<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    max_age: Duration,
) -> Result<usize> {
    let cache_dir = app
        .path()
        .app_cache_dir()
        .context("failed to resolve app cache dir")?;

    cleanup_old_git_temp_dirs_in(&cache_dir, max_age)
}

fn cleanup_old_git_temp_dirs_in(cache_dir: &Path, max_age: Duration) -> Result<usize> {
    if !cache_dir.exists() {
        return Ok(0);
    }

    let cutoff = SystemTime::now()
        .checked_sub(max_age)
        .unwrap_or(SystemTime::UNIX_EPOCH);

    let mut removed = 0usize;
    let rd = match std::fs::read_dir(cache_dir) {
        Ok(v) => v,
        Err(err) => {
            // No permission / missing dir is not fatal.
            return Err(anyhow::anyhow!(
                "failed to read cache dir {:?}: {}",
                cache_dir,
                err
            ));
        }
    };

    for entry in rd.flatten() {
        let path: PathBuf = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if !is_managed_temp_dir(&name, &path) {
            continue;
        }

        // Safety: only delete directories we have explicitly marked.
        if !path.join(TEMP_MARKER).exists() && !path.join(LEGACY_TEMP_MARKER).exists() {
            continue;
        }

        let meta = match std::fs::metadata(&path) {
            Ok(m) => m,
            Err(_) => continue,
        };
        let modified = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        if modified > cutoff {
            continue;
        }

        // Best-effort delete; do not fail the whole cleanup.
        if std::fs::remove_dir_all(&path).is_ok() {
            removed += 1;
        }
    }

    Ok(removed)
}

fn is_managed_temp_dir(name: &str, path: &Path) -> bool {
    (name.starts_with(TEMP_PREFIX) || name.starts_with(LEGACY_TEMP_PREFIX)) && path.is_dir()
}

#[cfg(test)]
#[path = "tests/temp_cleanup.rs"]
mod tests;
