use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::device_sync::types::TrashEntry;
use super::skill_store::{SkillRecord, SkillStore, SkillTargetRecord};
use super::sync_engine::{
    copy_dir_recursive, ensure_paths_do_not_overlap, path_is_protected_real_content, paths_overlap,
    remove_path_permanently as remove_path, sync_dir_with_mode_with_overwrite, SyncMode,
};

pub const RETENTION_MS: i64 = 30 * 24 * 60 * 60 * 1_000;
static RECYCLE_BIN_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DeletionSource {
    Manual,
    Sync,
}

impl DeletionSource {
    fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Sync => "sync",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct RecycleBinSnapshot {
    skill: SkillRecord,
    tags: Vec<String>,
    targets: Vec<SkillTargetRecord>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct RecycleBinItem {
    pub id: String,
    pub skill_id: String,
    pub skill_name: String,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub deletion_source: String,
    pub deleted_at: i64,
    pub expires_at: i64,
    pub enabled: bool,
    pub source_type: String,
    pub source_ref: Option<String>,
    pub targets: Vec<SkillTargetRecord>,
    pub trash_path: String,
}

pub struct RecycleBinService<'a> {
    store: &'a SkillStore,
    trash_root: PathBuf,
}

impl<'a> RecycleBinService<'a> {
    pub fn new(store: &'a SkillStore, trash_root: PathBuf) -> Self {
        Self { store, trash_root }
    }

    pub fn archive(
        &self,
        skill_id: &str,
        deletion_source: DeletionSource,
        deleted_at: i64,
    ) -> Result<RecycleBinItem> {
        let _guard = lock_recycle_bin()?;
        let skill = self
            .store
            .get_skill_by_id(skill_id)?
            .with_context(|| format!("Skill not found: {skill_id}"))?;
        let source = PathBuf::from(&skill.central_path);
        let tags = self
            .store
            .get_skill_tags(skill_id)?
            .into_iter()
            .map(|tag| tag.name)
            .collect::<Vec<_>>();
        let targets = self.store.list_skill_targets(skill_id)?;

        // The folder can already be gone: the library can hold a record whose content was
        // never materialised again, which used to make deleting that record impossible —
        // every attempt failed with "Skill content is missing". There is nothing to keep in
        // the recycle bin in that case, so the record and its tool links are removed
        // instead. A device-sync peer that still holds the Skill restores it, with content,
        // on the next sync.
        if !source.is_dir() {
            let mut protected = vec![source.clone()];
            if let Some(original) = skill.external_local_source() {
                protected.push(PathBuf::from(original));
            }
            self.store.delete_skill(skill_id)?;
            let mut failures = Vec::new();
            for target in &targets {
                let target_path = PathBuf::from(&target.target_path);
                if path_is_protected_real_content(&target_path, &protected)? {
                    continue;
                }
                if let Err(error) = remove_path(&target_path) {
                    failures.push(format!("{:?}: {error:#}", target_path));
                }
            }
            if !failures.is_empty() {
                log::warn!(
                    "removed Skill {} with missing content but could not clear every tool link: {}",
                    skill.name,
                    failures.join("; ")
                );
            }
            log::warn!(
                "removed Skill {} without recycle bin content; {:?} is gone",
                skill.name,
                source
            );
            return Ok(RecycleBinItem {
                id: Uuid::new_v4().to_string(),
                skill_id: skill.id.clone(),
                skill_name: skill.name.clone(),
                description: skill.description.clone(),
                tags,
                deletion_source: deletion_source.as_str().to_string(),
                deleted_at,
                expires_at: deleted_at + RETENTION_MS,
                enabled: skill.enabled,
                source_type: skill.source_type.clone(),
                source_ref: skill.source_ref.clone(),
                targets,
                trash_path: String::new(),
            });
        }
        let snapshot = RecycleBinSnapshot {
            skill: skill.clone(),
            tags: tags.clone(),
            targets: targets.clone(),
        };
        let metadata_json = serde_json::to_string(&snapshot)?;
        let id = Uuid::new_v4().to_string();
        let destination = self.trash_root.join(&id);
        ensure_paths_do_not_overlap(&source, &destination)?;
        fs::create_dir_all(&self.trash_root)?;
        copy_dir_recursive(&source, &destination)
            .with_context(|| format!("copy Skill into recycle bin: {:?}", source))?;
        let entry = TrashEntry {
            id: id.clone(),
            skill_id: skill.id.clone(),
            skill_name: skill.name.clone(),
            trash_path: destination.to_string_lossy().into(),
            deleted_at,
            expires_at: deleted_at + RETENTION_MS,
        };
        let overlaps_original = skill
            .external_local_source()
            .map(|original| paths_overlap(&source, Path::new(original)))
            .transpose()?
            .unwrap_or(false);
        let mut protected = vec![source.clone(), destination.clone()];
        if let Some(original) = skill.external_local_source() {
            protected.push(PathBuf::from(original));
        }
        let mut cleanup_paths = Vec::new();
        for target in &targets {
            let target_path = PathBuf::from(&target.target_path);
            if !path_is_protected_real_content(&target_path, &protected)? {
                cleanup_paths.push(target_path);
            }
        }
        if let Err(error) =
            self.store
                .commit_recycle_bin_archive(&entry, deletion_source.as_str(), &metadata_json)
        {
            let _ = fs::remove_dir_all(&destination);
            return Err(error);
        }
        if !overlaps_original {
            cleanup_paths.push(source);
        }
        let mut failures = Vec::new();
        for path in cleanup_paths {
            if let Err(error) = remove_path(&path) {
                failures.push(format!("{:?}: {error:#}", path));
            }
        }
        anyhow::ensure!(
            failures.is_empty(),
            "Skill saved in recycle bin, but some original files could not be removed: {}",
            failures.join("; ")
        );
        Ok(item_from_snapshot(
            entry,
            deletion_source.as_str(),
            snapshot,
        ))
    }

    pub fn list(&self) -> Result<Vec<RecycleBinItem>> {
        self.store
            .list_recycle_bin_rows()?
            .into_iter()
            .map(|(entry, source, metadata)| {
                if let Some(metadata) = metadata {
                    let snapshot: RecycleBinSnapshot =
                        serde_json::from_str(&metadata).context("RECYCLE_BIN_SNAPSHOT_INVALID")?;
                    Ok(item_from_snapshot(entry, &source, snapshot))
                } else {
                    let legacy = self.store.device_sync_trash_metadata(&entry.id)?;
                    Ok(RecycleBinItem {
                        id: entry.id,
                        skill_id: entry.skill_id,
                        skill_name: entry.skill_name,
                        description: legacy.as_ref().and_then(|value| value.description.clone()),
                        tags: legacy.map(|value| value.tags).unwrap_or_default(),
                        deletion_source: source,
                        deleted_at: entry.deleted_at,
                        expires_at: entry.expires_at,
                        enabled: true,
                        source_type: "sync_restore".into(),
                        source_ref: None,
                        targets: vec![],
                        trash_path: entry.trash_path,
                    })
                }
            })
            .collect()
    }

    pub fn has_snapshot(&self, id: &str) -> Result<bool> {
        Ok(self
            .store
            .get_recycle_bin_row(id)?
            .and_then(|(_, _, metadata)| metadata)
            .is_some())
    }

    pub fn restore(&self, id: &str) -> Result<()> {
        self.restore_with_targets(id, None)
    }

    pub fn restore_with_targets(&self, id: &str, target_ids: Option<&[String]>) -> Result<()> {
        let _guard = lock_recycle_bin()?;
        let (entry, _, metadata) = self
            .store
            .get_recycle_bin_row(id)?
            .context("RECYCLE_BIN_ITEM_MISSING")?;
        let mut snapshot: RecycleBinSnapshot = serde_json::from_str(
            metadata
                .as_deref()
                .context("RECYCLE_BIN_SNAPSHOT_MISSING")?,
        )
        .context("RECYCLE_BIN_SNAPSHOT_INVALID")?;
        anyhow::ensure!(
            self.store.get_skill_by_id(&snapshot.skill.id)?.is_none(),
            "RECYCLE_BIN_SKILL_EXISTS"
        );
        let source = PathBuf::from(&entry.trash_path);
        anyhow::ensure!(source.is_dir(), "RECYCLE_BIN_CONTENT_MISSING");
        let destination = PathBuf::from(&snapshot.skill.central_path);
        let copied_destination = !destination.exists();
        if !destination.exists() {
            copy_dir_recursive(&source, &destination)?;
        } else {
            anyhow::ensure!(
                snapshot.skill.source_ref.as_deref() == Some(snapshot.skill.central_path.as_str()),
                "RECYCLE_BIN_LOCATION_OCCUPIED"
            );
        }
        if let Some(ids) = target_ids {
            for target in &mut snapshot.targets {
                if !ids.contains(&target.id) {
                    target.status = "disabled".into();
                    target.last_error = None;
                    target.synced_at = None;
                }
            }
        }
        let mut pending_targets = snapshot.targets.clone();
        for target in &mut pending_targets {
            if target.status == "disabled" {
                continue;
            }
            target.status = "pending".into();
            target.last_error = Some("RESTORE_PENDING".into());
            target.synced_at = None;
        }
        if let Err(error) = self.store.restore_recycle_bin_snapshot(
            id,
            &snapshot.skill,
            &pending_targets,
            &snapshot.tags,
        ) {
            if copied_destination {
                remove_path(&destination)
                    .context("remove restored Skill copy after database failure")?;
            }
            return Err(error);
        }
        for target in &mut snapshot.targets {
            if target.status == "disabled" {
                continue;
            }
            let target_path = PathBuf::from(&target.target_path);
            if target_path.exists() {
                target.status = "error".into();
                target.last_error = Some("RESTORE_TARGET_EXISTS".into());
            } else {
                match sync_dir_with_mode_with_overwrite(
                    parse_sync_mode(&target.mode),
                    &destination,
                    &target_path,
                    false,
                ) {
                    Ok(result) => {
                        target.mode = sync_mode_name(result.mode_used).into();
                        target.status = "ok".into();
                        target.last_error = None;
                        target.synced_at = Some(now_ms());
                    }
                    Err(error) => {
                        target.status = "error".into();
                        target.last_error = Some(format!("RESTORE_TARGET_FAILED|{error:#}"));
                    }
                }
            }
            if let Err(error) = self.store.upsert_skill_target(target) {
                log::warn!(
                    "persist restored Skill target {:?} failed: {error:#}",
                    target.target_path
                );
            }
        }
        let _ = fs::remove_dir_all(source);
        Ok(())
    }

    pub fn delete_permanently(&self, id: &str) -> Result<()> {
        let _guard = lock_recycle_bin()?;
        self.delete_permanently_inner(id)
    }

    pub fn clear(&self, confirmed_ids: &[String]) -> Result<usize> {
        let _guard = lock_recycle_bin()?;
        let ids = self
            .store
            .list_recycle_bin_rows()?
            .into_iter()
            .map(|(entry, _, _)| entry.id)
            .collect::<Vec<_>>();
        let mut current_ids = ids.clone();
        let mut expected_ids = confirmed_ids.to_vec();
        current_ids.sort_unstable();
        expected_ids.sort_unstable();
        anyhow::ensure!(current_ids == expected_ids, "RECYCLE_BIN_CHANGED");
        for id in &ids {
            self.delete_permanently_inner(id)?;
        }
        Ok(ids.len())
    }

    fn delete_permanently_inner(&self, id: &str) -> Result<()> {
        let (entry, _, _) = self
            .store
            .get_recycle_bin_row(id)?
            .context("RECYCLE_BIN_ITEM_MISSING")?;
        let path = PathBuf::from(entry.trash_path);
        if path.exists() {
            remove_path(&path).context("delete recycle bin content")?;
        }
        self.store.remove_recycle_bin_row(id)
    }

    pub fn cleanup_expired(&self, now: i64) -> Result<usize> {
        let _guard = lock_recycle_bin()?;
        let expired = self
            .store
            .list_recycle_bin_rows()?
            .into_iter()
            .filter(|(entry, _, _)| entry.expires_at <= now)
            .map(|(entry, _, _)| entry.id)
            .collect::<Vec<_>>();
        let mut removed = 0;
        for id in expired {
            if self.delete_permanently_inner(&id).is_ok() {
                removed += 1;
            }
        }
        Ok(removed)
    }
}

fn lock_recycle_bin() -> Result<MutexGuard<'static, ()>> {
    RECYCLE_BIN_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|_| anyhow::anyhow!("recycle bin operation lock is poisoned"))
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn item_from_snapshot(
    entry: TrashEntry,
    deletion_source: &str,
    snapshot: RecycleBinSnapshot,
) -> RecycleBinItem {
    RecycleBinItem {
        id: entry.id,
        skill_id: entry.skill_id,
        skill_name: entry.skill_name,
        description: snapshot.skill.description,
        tags: snapshot.tags,
        deletion_source: deletion_source.into(),
        deleted_at: entry.deleted_at,
        expires_at: entry.expires_at,
        enabled: snapshot.skill.enabled,
        source_type: snapshot.skill.source_type,
        source_ref: snapshot.skill.source_ref,
        targets: snapshot.targets,
        trash_path: entry.trash_path,
    }
}

fn parse_sync_mode(value: &str) -> SyncMode {
    match value {
        "symlink" => SyncMode::Symlink,
        "junction" => SyncMode::Junction,
        "copy" => SyncMode::Copy,
        _ => SyncMode::Auto,
    }
}
fn sync_mode_name(value: SyncMode) -> &'static str {
    match value {
        SyncMode::Auto => "auto",
        SyncMode::Symlink => "symlink",
        SyncMode::Junction => "junction",
        SyncMode::Copy => "copy",
    }
}
#[cfg(test)]
#[path = "tests/recycle_bin.rs"]
mod tests;
