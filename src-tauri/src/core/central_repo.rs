use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use dirs::home_dir;
use tauri::Manager;

use super::skill_store::SkillStore;

const CENTRAL_DIR_NAME: &str = ".skillsyncer";
const LEGACY_CENTRAL_DIR_NAME: &str = ".skillshub";

pub fn migrate_legacy_central_repo_if_needed(store: &SkillStore) -> Result<()> {
    if store.get_setting("central_repo_path")?.is_some() {
        return Ok(());
    }

    let Some(home) = home_dir() else {
        return Ok(());
    };

    let new_path = home.join(CENTRAL_DIR_NAME);
    let legacy_path = home.join(LEGACY_CENTRAL_DIR_NAME);
    if new_path.exists() || !legacy_path.exists() {
        return Ok(());
    }

    std::fs::rename(&legacy_path, &new_path).with_context(|| {
        format!(
            "failed to migrate central repo {:?} -> {:?}",
            legacy_path, new_path
        )
    })?;

    Ok(())
}

pub fn resolve_central_repo_path<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    store: &SkillStore,
) -> Result<PathBuf> {
    if let Some(path) = store.get_setting("central_repo_path")? {
        return Ok(PathBuf::from(path));
    }

    if let Some(home) = home_dir() {
        return Ok(home.join(CENTRAL_DIR_NAME));
    }

    let base = app
        .path()
        .app_data_dir()
        .context("failed to resolve app data dir")?;
    Ok(base.join(CENTRAL_DIR_NAME))
}

pub fn ensure_central_repo(path: &Path) -> Result<()> {
    std::fs::create_dir_all(path).with_context(|| format!("create {:?}", path))?;
    Ok(())
}

#[cfg(test)]
#[path = "tests/central_repo.rs"]
mod tests;
