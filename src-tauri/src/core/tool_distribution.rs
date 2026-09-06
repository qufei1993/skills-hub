use std::path::Path;

use anyhow::Result;

use super::content_hash::{hash_dir_for_sync_conflict, hash_dir_strict};
use super::skill_store::SkillStore;
use super::sync_engine::{ensure_paths_do_not_overlap, PreparedDirReplacement};

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
        let (staging, expected) =
            super::device_sync::manifest::prepare_library_directory(source, target)?;
        let next_hash = hash_dir_for_sync_conflict(staging.path())?;
        let mut replacement = None;
        if expected.as_ref() != Some(&hash_dir_strict(staging.path())?) {
            if target.exists() {
                let actual = hash_dir_for_sync_conflict(target)?;
                let mut trusted = previous_hash == Some(actual.as_str());
                for record in &records {
                    let saved = store
                        .get_setting(&format!("device_sync.target_baseline.{}", record.id))?
                        .and_then(|value| serde_json::from_str::<(String, String)>(&value).ok());
                    trusted |= saved
                        .as_ref()
                        .is_some_and(|(path, hash)| Path::new(path) == target && hash == &actual);
                }
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
