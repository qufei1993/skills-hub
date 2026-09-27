use std::path::Path;

use anyhow::Result;

use super::content_hash::{hash_dir_for_sync_conflict, hash_dir_strict};
use super::skill_store::{SkillStore, SkillTargetRecord};
use super::sync_engine::{ensure_paths_do_not_overlap, PreparedDirReplacement};

pub(crate) fn matches_saved_target_baseline(
    value: &str,
    target: &Path,
    actual: Option<&str>,
) -> bool {
    serde_json::from_str::<(String, String)>(value).ok().is_some_and(|(path, hash)| {
        let same_path = Path::new(&path) == target || matches!((super::sync_engine::path_for_comparison(Path::new(&path)), super::sync_engine::path_for_comparison(target)), (Ok(first), Ok(second)) if first == second);
        same_path && actual == Some(hash.as_str())
    })
}

fn copy_target_matches_baseline(
    store: &SkillStore,
    records: &[SkillTargetRecord],
    target: &Path,
    previous_hash: Option<&str>,
    actual: &str,
) -> Result<bool> {
    if previous_hash == Some(actual) {
        return Ok(true);
    }
    for record in records {
        let saved = store.get_setting(&format!("device_sync.target_baseline.{}", record.id))?;
        if saved
            .as_ref()
            .is_some_and(|value| matches_saved_target_baseline(value, target, Some(actual)))
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn validate_copy_target_location(
    store: &SkillStore,
    skill_id: &str,
    source: &Path,
    target: &Path,
) -> Result<()> {
    ensure_paths_do_not_overlap(source, target)?;
    if let Some(skill) = store.get_skill_by_id(skill_id)? {
        if skill.source_type == "local" {
            if let Some(original) = skill.source_ref.filter(|value| !value.trim().is_empty()) {
                ensure_paths_do_not_overlap(Path::new(&original), target)?;
            }
        }
    }
    anyhow::ensure!(
        !store.is_target_used_by_other_skill(&target.to_string_lossy(), skill_id)?,
        "unsafe shared tool target"
    );
    Ok(())
}

pub(crate) fn preflight_copy_refresh(
    store: &SkillStore,
    source: &Path,
    record: &SkillTargetRecord,
    previous_hash: &str,
) -> Result<()> {
    let target = Path::new(&record.target_path);
    validate_copy_target_location(store, &record.skill_id, source, target)?;
    let metadata = match std::fs::symlink_metadata(target) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    anyhow::ensure!(
        metadata.is_dir() && !metadata.file_type().is_symlink(),
        "TARGET_MODIFIED|{}",
        target.display()
    );
    let actual = hash_dir_for_sync_conflict(target)?;
    anyhow::ensure!(
        copy_target_matches_baseline(
            store,
            std::slice::from_ref(record),
            target,
            Some(previous_hash),
            &actual
        )?,
        "TARGET_MODIFIED|{}",
        target.display()
    );
    Ok(())
}

// Both explicit tool sync and device sync use the same guarded copy refresh.
pub fn refresh_copy(
    store: &SkillStore,
    skill_id: &str,
    source: &Path,
    target: &Path,
    previous_hash: Option<&str>,
) -> Result<()> {
    let records: Vec<_> = store
        .list_skill_targets(skill_id)?
        .into_iter()
        .filter(|record| {
            record.mode == "copy"
                && record.status != "disabled"
                && Path::new(&record.target_path) == target
        })
        .collect();
    anyhow::ensure!(!records.is_empty(), "copy target not registered");
    let result = (|| -> Result<()> {
        validate_copy_target_location(store, skill_id, source, target)?;
        let (staging, expected) =
            super::device_sync::manifest::prepare_library_directory(source, target)?;
        let next_hash = hash_dir_for_sync_conflict(staging.path())?;
        let mut replacement = None;
        if expected.as_ref() != Some(&hash_dir_strict(staging.path())?) {
            if target.exists() {
                let actual = hash_dir_for_sync_conflict(target)?;
                let trusted =
                    copy_target_matches_baseline(store, &records, target, previous_hash, &actual)?;
                anyhow::ensure!(trusted, "TARGET_MODIFIED|{}", target.display());
            }
            let mut prepared = PreparedDirReplacement::from_staging(
                staging.keep(),
                target.to_path_buf(),
                expected,
                true,
            )?;
            prepared.activate()?;
            prepared.verify_backup_unchanged()?;
            replacement = Some(prepared);
        }
        let updated = records
            .iter()
            .cloned()
            .map(|mut record| {
                record.status = "ok".into();
                record.last_error = None;
                record.synced_at = Some(
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as i64,
                );
                (record, next_hash.clone())
            })
            .collect::<Vec<_>>();
        store.commit_device_sync_library(&[], &updated)?;
        if let Some(replacement) = replacement.as_mut() {
            replacement.commit();
        }
        Ok(())
    })();
    if let Err(error) = &result {
        for mut record in records {
            record.status = "error".into();
            record.last_error = Some(format!(
                "SKILL_ISSUE|{}",
                super::skill_issues::safe_code(&format!("{error:#}"))
            ));
            store.upsert_skill_target(&record)?;
        }
    }
    result
}
