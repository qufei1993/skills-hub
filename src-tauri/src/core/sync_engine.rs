use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::content_hash::hash_dir_strict;

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SyncMode {
    #[default]
    Auto,
    Symlink,
    Junction,
    Copy,
}

#[derive(Clone, Debug)]
pub struct SyncOutcome {
    pub mode_used: SyncMode,
    pub target_path: PathBuf,
    pub replaced: bool,
}

pub fn sync_dir_hybrid(source: &Path, target: &Path) -> Result<SyncOutcome> {
    if is_same_link(target, source) {
        return Ok(SyncOutcome {
            mode_used: SyncMode::Symlink,
            target_path: target.to_path_buf(),
            replaced: false,
        });
    }
    ensure_paths_do_not_overlap(source, target)?;
    if target.exists() {
        anyhow::bail!("target already exists: {:?}", target);
    }

    ensure_parent_dir(target)?;

    if try_link_dir(source, target).is_ok() {
        return Ok(SyncOutcome {
            mode_used: SyncMode::Symlink,
            target_path: target.to_path_buf(),
            replaced: false,
        });
    }

    #[cfg(windows)]
    if try_junction(source, target).is_ok() {
        return Ok(SyncOutcome {
            mode_used: SyncMode::Junction,
            target_path: target.to_path_buf(),
            replaced: false,
        });
    }

    copy_dir_recursive(source, target)?;
    Ok(SyncOutcome {
        mode_used: SyncMode::Copy,
        target_path: target.to_path_buf(),
        replaced: false,
    })
}

pub fn sync_dir_hybrid_with_overwrite(
    source: &Path,
    target: &Path,
    overwrite: bool,
) -> Result<SyncOutcome> {
    if is_same_link(target, source) {
        return Ok(SyncOutcome {
            mode_used: SyncMode::Symlink,
            target_path: target.to_path_buf(),
            replaced: false,
        });
    }
    ensure_paths_do_not_overlap(source, target)?;
    let mut did_replace = false;
    if std::fs::symlink_metadata(target).is_ok() {
        if overwrite {
            remove_path_any(target)
                .with_context(|| format!("remove existing target {:?}", target))?;
            did_replace = true;
        } else {
            anyhow::bail!("target already exists: {:?}", target);
        }
    }

    // reuse normal flow
    sync_dir_hybrid(source, target).map(|mut out| {
        out.replaced = did_replace;
        out
    })
}

pub fn sync_dir_copy_with_overwrite(
    source: &Path,
    target: &Path,
    overwrite: bool,
) -> Result<SyncOutcome> {
    ensure_paths_do_not_overlap(source, target)?;
    let mut did_replace = false;
    if std::fs::symlink_metadata(target).is_ok() {
        if overwrite {
            remove_path_any(target)
                .with_context(|| format!("remove existing target {:?}", target))?;
            did_replace = true;
        } else {
            anyhow::bail!("target already exists: {:?}", target);
        }
    }

    ensure_parent_dir(target)?;
    copy_dir_recursive(source, target)?;

    Ok(SyncOutcome {
        mode_used: SyncMode::Copy,
        target_path: target.to_path_buf(),
        replaced: did_replace,
    })
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn sync_managed_copy_with_expected_hash(
    source: &Path,
    target: &Path,
    expected_hash: &str,
) -> Result<SyncOutcome> {
    let mut replacement = PreparedDirReplacement::prepare_copy(
        source,
        target,
        Some(expected_hash.to_string()),
        true,
    )?;
    let replaced = replacement.activate()?;
    replacement.verify_backup_unchanged()?;
    replacement.commit();

    Ok(SyncOutcome {
        mode_used: SyncMode::Copy,
        target_path: target.to_path_buf(),
        replaced,
    })
}

pub(crate) struct PreparedDirReplacement {
    target: PathBuf,
    staging: Option<PathBuf>,
    backup: Option<PathBuf>,
    expected_hash: Option<String>,
    prepared_hash: String,
    allow_missing: bool,
    activated: bool,
}

impl PreparedDirReplacement {
    pub(crate) fn prepare_copy(
        source: &Path,
        target: &Path,
        expected_hash: Option<String>,
        allow_missing: bool,
    ) -> Result<Self> {
        ensure_paths_do_not_overlap(source, target)?;
        ensure_parent_dir(target)?;
        let parent = target
            .parent()
            .context("managed copy target has no parent")?;
        let staging = parent.join(format!(".skills-hub-sync-{}", Uuid::new_v4()));
        if let Err(err) = copy_dir_recursive(source, &staging) {
            let _ = remove_path_permanently(&staging);
            return Err(err);
        }
        Self::from_staging(staging, target.to_path_buf(), expected_hash, allow_missing)
    }

    pub(crate) fn from_staging(
        staging: PathBuf,
        target: PathBuf,
        expected_hash: Option<String>,
        allow_missing: bool,
    ) -> Result<Self> {
        if std::fs::symlink_metadata(&staging).is_err() {
            anyhow::bail!("replacement staging path not found: {:?}", staging);
        }
        let prepared_hash = match hash_dir_strict(&staging)
            .with_context(|| format!("hash replacement staging {:?}", staging))
        {
            Ok(hash) => hash,
            Err(err) => {
                let _ = remove_path_permanently(&staging);
                return Err(err);
            }
        };
        if let Err(err) = ensure_parent_dir(&target) {
            let _ = remove_path_permanently(&staging);
            return Err(err);
        }
        Ok(Self {
            target,
            staging: Some(staging),
            backup: None,
            expected_hash,
            prepared_hash,
            allow_missing,
            activated: false,
        })
    }

    pub(crate) fn activate(&mut self) -> Result<bool> {
        let parent = self
            .target
            .parent()
            .context("replacement target has no parent")?;
        let backup = parent.join(format!(".skills-hub-backup-{}", Uuid::new_v4()));
        let had_target = match std::fs::symlink_metadata(&self.target) {
            Ok(_) => true,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => false,
            Err(err) => return Err(err).with_context(|| format!("stat {:?}", self.target)),
        };

        if !had_target && !self.allow_missing {
            anyhow::bail!("replacement target not found: {:?}", self.target);
        }

        if had_target {
            std::fs::rename(&self.target, &backup)
                .with_context(|| format!("prepare replacement backup {:?}", self.target))?;
            self.backup = Some(backup);
            if let Err(err) = self.verify_backup_unchanged() {
                self.restore_backup_before_activation()?;
                return Err(err);
            }
        }

        let staging = self
            .staging
            .as_ref()
            .context("replacement staging path already consumed")?;
        if let Err(replace_err) = std::fs::rename(staging, &self.target) {
            if let Err(restore_err) = self.restore_backup_before_activation() {
                anyhow::bail!(
                    "replace {:?} failed: {}; restore backup failed: {:#}",
                    self.target,
                    replace_err,
                    restore_err
                );
            }
            return Err(replace_err).with_context(|| format!("replace {:?}", self.target));
        }
        self.staging = None;
        self.activated = true;
        Ok(had_target)
    }

    pub(crate) fn verify_backup_unchanged(&self) -> Result<()> {
        let (Some(expected_hash), Some(backup)) = (&self.expected_hash, &self.backup) else {
            return Ok(());
        };
        let metadata = std::fs::symlink_metadata(backup)
            .with_context(|| format!("stat replacement backup {:?}", backup))?;
        let matches = metadata.is_dir()
            && !metadata.file_type().is_symlink()
            && hash_dir_strict(backup)
                .map(|actual_hash| actual_hash == *expected_hash)
                .unwrap_or(false);
        if !matches {
            anyhow::bail!("TARGET_MODIFIED|{}", self.target.to_string_lossy());
        }
        Ok(())
    }

    pub(crate) fn rollback(&mut self) -> Result<()> {
        if self.activated {
            let parent = self
                .target
                .parent()
                .context("replacement target has no parent")?;
            let recovery = parent.join(format!(".skills-hub-recovery-{}", Uuid::new_v4()));
            let had_backup = self.backup.is_some();
            let current_exists = match std::fs::symlink_metadata(&self.target) {
                Ok(_) => true,
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => false,
                Err(err) => {
                    return Err(err).with_context(|| format!("stat {:?}", self.target));
                }
            };
            if current_exists {
                std::fs::rename(&self.target, &recovery)
                    .with_context(|| format!("isolate rollback content {:?}", self.target))?;
            }
            self.activated = false;
            self.restore_backup_before_activation()?;

            if current_exists {
                let metadata = std::fs::symlink_metadata(&recovery)
                    .with_context(|| format!("stat rollback content {:?}", recovery))?;
                let unchanged = metadata.is_dir()
                    && !metadata.file_type().is_symlink()
                    && hash_dir_strict(&recovery)
                        .map(|hash| hash == self.prepared_hash)
                        .unwrap_or(false);
                if unchanged {
                    remove_path_permanently(&recovery)
                        .with_context(|| format!("remove rolled back content {:?}", recovery))?;
                } else {
                    let preserved_at = if had_backup {
                        recovery
                    } else {
                        std::fs::rename(&recovery, &self.target).with_context(|| {
                            format!("restore concurrently modified target {:?}", self.target)
                        })?;
                        self.target.clone()
                    };
                    let detail = serde_json::json!({
                        "target": self.target.to_string_lossy(),
                        "recovery": preserved_at.to_string_lossy(),
                    });
                    anyhow::bail!("ROLLBACK_CONFLICT|{detail}");
                }
            }
        } else {
            self.restore_backup_before_activation()?;
        }
        if let Some(staging) = self.staging.take() {
            remove_path_permanently(&staging)
                .with_context(|| format!("remove replacement staging {:?}", staging))?;
        }
        Ok(())
    }

    pub(crate) fn commit(&mut self) {
        self.activated = false;
        if let Some(backup) = self.backup.take() {
            if let Err(err) = remove_path_permanently(&backup) {
                eprintln!(
                    "[sync] failed to clean committed backup {:?}: {err:#}",
                    backup
                );
            }
        }
        if let Some(staging) = self.staging.take() {
            if let Err(err) = remove_path_permanently(&staging) {
                eprintln!(
                    "[sync] failed to clean committed staging {:?}: {err:#}",
                    staging
                );
            }
        }
    }

    fn restore_backup_before_activation(&mut self) -> Result<()> {
        if let Some(backup) = self.backup.as_ref() {
            std::fs::rename(backup, &self.target)
                .with_context(|| format!("restore replacement backup {:?}", backup))?;
            self.backup = None;
        }
        Ok(())
    }
}

impl Drop for PreparedDirReplacement {
    fn drop(&mut self) {
        if self.activated || self.backup.is_some() {
            if let Err(err) = self.rollback() {
                eprintln!("[sync] failed to roll back {:?}: {err:#}", self.target);
            }
        } else if let Some(staging) = self.staging.take() {
            if let Err(err) = remove_path_permanently(&staging) {
                eprintln!("[sync] failed to clean staging {:?}: {err:#}", staging);
            }
        }
    }
}

pub fn sync_dir_with_mode_with_overwrite(
    mode: SyncMode,
    source: &Path,
    target: &Path,
    overwrite: bool,
) -> Result<SyncOutcome> {
    match mode {
        SyncMode::Auto => sync_dir_hybrid_with_overwrite(source, target, overwrite),
        SyncMode::Copy => sync_dir_copy_with_overwrite(source, target, overwrite),
        SyncMode::Symlink | SyncMode::Junction => {
            sync_dir_link_with_overwrite(mode, source, target, overwrite)
        }
    }
}

fn sync_dir_link_with_overwrite(
    mode: SyncMode,
    source: &Path,
    target: &Path,
    overwrite: bool,
) -> Result<SyncOutcome> {
    if is_same_link(target, source) {
        return Ok(SyncOutcome {
            mode_used: mode,
            target_path: target.to_path_buf(),
            replaced: false,
        });
    }
    ensure_paths_do_not_overlap(source, target)?;
    let mut did_replace = false;
    if std::fs::symlink_metadata(target).is_ok() {
        if overwrite {
            remove_path_any(target)
                .with_context(|| format!("remove existing target {:?}", target))?;
            did_replace = true;
        } else {
            anyhow::bail!("target already exists: {:?}", target);
        }
    }

    ensure_parent_dir(target)?;
    match mode {
        SyncMode::Symlink => try_link_dir(source, target)?,
        SyncMode::Junction => try_junction(source, target)?,
        SyncMode::Auto | SyncMode::Copy => unreachable!("link mode required"),
    }

    Ok(SyncOutcome {
        mode_used: mode,
        target_path: target.to_path_buf(),
        replaced: did_replace,
    })
}

pub fn sync_dir_for_tool_with_overwrite(
    tool_key: &str,
    source: &Path,
    target: &Path,
    overwrite: bool,
) -> Result<SyncOutcome> {
    // Cursor 目前不支持软链/junction：强制使用 copy，避免同步后在 Cursor 内不可用。
    if tool_key.eq_ignore_ascii_case("cursor") {
        return sync_dir_copy_with_overwrite(source, target, overwrite);
    }
    sync_dir_hybrid_with_overwrite(source, target, overwrite)
}

fn ensure_parent_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("create dir {:?}", parent))?;
    }
    Ok(())
}

fn ensure_paths_do_not_overlap(source: &Path, target: &Path) -> Result<()> {
    if paths_overlap(source, target)? {
        anyhow::bail!(
            "source and target paths overlap: {:?} and {:?}",
            source,
            target
        );
    }
    Ok(())
}

pub(crate) fn paths_overlap(first: &Path, second: &Path) -> Result<bool> {
    let first = path_for_comparison(first)?;
    let second = path_for_comparison(second)?;
    Ok(first == second || first.starts_with(&second) || second.starts_with(&first))
}

pub(crate) fn path_is_protected_real_content(
    path: &Path,
    protected_paths: &[PathBuf],
) -> Result<bool> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(err) => return Err(err).with_context(|| format!("stat {:?}", path)),
    };
    if metadata.file_type().is_symlink() {
        return Ok(false);
    }
    for protected in protected_paths {
        if paths_overlap(path, protected)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn path_for_comparison(path: &Path) -> Result<PathBuf> {
    if let Ok(canonical) = std::fs::canonicalize(path) {
        return Ok(canonical);
    }

    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let mut missing = Vec::new();
    let mut existing = absolute.as_path();
    while !existing.exists() {
        let name = existing
            .file_name()
            .context("path has no existing ancestor")?;
        missing.push(name.to_os_string());
        existing = existing.parent().context("path has no existing ancestor")?;
    }
    let mut normalized = std::fs::canonicalize(existing)?;
    for component in missing.iter().rev() {
        normalized.push(component);
    }
    Ok(normalized)
}

pub(crate) fn remove_path_any(path: &Path) -> Result<()> {
    remove_path_safely_with(path, recycle_path)
}

#[cfg(not(test))]
fn recycle_path(path: &Path) -> Result<()> {
    trash::delete(path).map_err(anyhow::Error::from)
}

#[cfg(test)]
fn recycle_path(path: &Path) -> Result<()> {
    remove_path_permanently(path)
}

fn remove_path_permanently(path: &Path) -> Result<()> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err).with_context(|| format!("stat {:?}", path)),
    };
    if metadata.file_type().is_symlink() || metadata.is_file() {
        std::fs::remove_file(path).with_context(|| format!("remove file {:?}", path))
    } else {
        std::fs::remove_dir_all(path).with_context(|| format!("remove dir {:?}", path))
    }
}

pub(crate) fn remove_path_safely_with<F>(path: &Path, recycle: F) -> Result<()>
where
    F: FnOnce(&Path) -> Result<()>,
{
    let meta = match std::fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err).with_context(|| format!("stat {:?}", path)),
    };
    let ft = meta.file_type();

    // 删除链接本身：symlink 用 remove_file；Windows junction 虽然 is_symlink()==true，
    // 但底层是目录 reparse point，remove_file 会报 os error 5，必须用 remove_dir
    // （RemoveDirectoryW 只移除链接本身，不会穿透到目标）
    if ft.is_symlink() {
        #[cfg(windows)]
        {
            if std::fs::remove_dir(path).is_ok() {
                return Ok(());
            }
        }
        std::fs::remove_file(path).with_context(|| format!("remove symlink {:?}", path))?;
        return Ok(());
    }
    recycle(path).with_context(|| format!("move path to system recycle bin {:?}", path))
}

fn is_same_link(link_path: &Path, target: &Path) -> bool {
    if let Ok(existing) = std::fs::read_link(link_path) {
        return existing == target;
    }
    false
}

fn try_link_dir(source: &Path, target: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(source, target)
            .with_context(|| format!("symlink {:?} -> {:?}", target, source))?;
        Ok(())
    }

    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_dir(source, target)
            .with_context(|| format!("symlink {:?} -> {:?}", target, source))?;
        return Ok(());
    }

    #[cfg(not(any(unix, windows)))]
    anyhow::bail!("symlink not supported on this platform");
}

#[cfg(windows)]
fn try_junction(source: &Path, target: &Path) -> Result<()> {
    junction::create(source, target)
        .with_context(|| format!("junction {:?} -> {:?}", target, source))?;
    Ok(())
}

#[cfg(not(windows))]
fn try_junction(_source: &Path, _target: &Path) -> Result<()> {
    anyhow::bail!("junction not supported on this platform");
}

fn should_skip_copy(entry: &walkdir::DirEntry) -> bool {
    entry.file_name() == ".git"
}

pub fn copy_dir_recursive(source: &Path, target: &Path) -> Result<()> {
    ensure_paths_do_not_overlap(source, target)?;
    let profile = std::env::var("SKILLS_HUB_PROFILE_IO")
        .ok()
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let started = std::time::Instant::now();
    let mut copied_files: u64 = 0;
    let mut copied_bytes: u64 = 0;

    for entry in walkdir::WalkDir::new(source)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| !should_skip_copy(entry))
    {
        let entry = entry?;
        if should_skip_copy(&entry) {
            continue;
        }
        let relative = entry.path().strip_prefix(source)?;
        let target_path = target.join(relative);

        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&target_path)
                .with_context(|| format!("create dir {:?}", target_path))?;
        } else if entry.file_type().is_file() {
            if let Some(parent) = target_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let bytes = std::fs::copy(entry.path(), &target_path)
                .with_context(|| format!("copy file {:?} -> {:?}", entry.path(), target_path))?;
            if profile {
                copied_files += 1;
                copied_bytes = copied_bytes.saturating_add(bytes);
            }
        }
    }
    if profile {
        log::info!(
            "[sync_engine] copy_dir_recursive {} files, {} bytes in {}s (src={:?} dst={:?})",
            copied_files,
            copied_bytes,
            started.elapsed().as_secs_f32(),
            source,
            target
        );
    }
    Ok(())
}

#[cfg(test)]
#[path = "tests/sync_engine.rs"]
mod tests;
