use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use dirs::home_dir;
use tauri::Manager;

use super::skill_store::{SkillRecord, SkillStore};
use super::sync_engine::paths_overlap;

const CENTRAL_DIR_NAME: &str = ".skillshub";

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

pub fn validate_central_repo_path_change(
    current: &Path,
    destination: &Path,
    tool_roots: &[PathBuf],
    local_sources: &[PathBuf],
) -> Result<()> {
    if paths_overlap(current, destination)? {
        anyhow::bail!(
            "UNSAFE_STORAGE_PATH|storage|{}|current and new storage directories overlap",
            current.to_string_lossy()
        );
    }

    for tool_root in tool_roots {
        if paths_overlap(destination, tool_root)? {
            anyhow::bail!(
                "UNSAFE_STORAGE_PATH|tool|{}|overlaps tool Skills directory",
                tool_root.to_string_lossy()
            );
        }
    }

    for source in local_sources {
        if paths_overlap(destination, source)? {
            anyhow::bail!(
                "UNSAFE_STORAGE_PATH|source|{}|overlaps original source directory",
                source.to_string_lossy()
            );
        }
    }

    Ok(())
}

#[derive(Clone, Debug)]
pub struct CentralRepoMigrationItem {
    pub skill: SkillRecord,
    pub old_path: PathBuf,
    pub new_path: PathBuf,
}

pub fn plan_central_repo_migration(
    skills: &[SkillRecord],
    new_base: &Path,
) -> Result<Vec<CentralRepoMigrationItem>> {
    let mut planned_paths = std::collections::HashSet::new();
    let mut plan = Vec::with_capacity(skills.len());

    for skill in skills {
        let old_path = PathBuf::from(&skill.central_path);
        if !old_path.is_dir() {
            anyhow::bail!("central path not found: {:?}", old_path);
        }
        let file_name = old_path
            .file_name()
            .with_context(|| format!("invalid central path: {:?}", old_path))?;
        let new_path = new_base.join(file_name);
        if std::fs::symlink_metadata(&new_path).is_ok() {
            anyhow::bail!("target path already exists: {:?}", new_path);
        }
        if !planned_paths.insert(new_path.clone()) {
            anyhow::bail!("duplicate target path: {:?}", new_path);
        }
        if paths_overlap(&old_path, &new_path)? {
            anyhow::bail!(
                "unsafe Skills storage path: source and destination overlap for {:?}",
                old_path
            );
        }
        plan.push(CentralRepoMigrationItem {
            skill: skill.clone(),
            old_path,
            new_path,
        });
    }

    Ok(plan)
}

#[cfg(test)]
#[path = "tests/central_repo.rs"]
mod tests;
