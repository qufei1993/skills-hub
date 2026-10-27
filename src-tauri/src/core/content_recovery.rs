//! One independently recoverable successful update per Skill.
use super::content_hash::{hash_dir, hash_dir_for_sync_conflict, hash_dir_strict};
use super::runtime_paths::RuntimePaths;
use super::skill_store::{SkillRecord, SkillStore};
use super::sync_engine::{copy_dir_recursive, deployment_fingerprint, PreparedDirReplacement};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Recovery {
    pub skill_id: String,
    pub before: SkillRecord,
    pub before_baseline: Option<String>,
    pub backup: PathBuf,
    pub before_hash: String,
    pub after_hash: String,
    pub after_content_hash: String,
    pub after_source_revision: Option<String>,
    pub after_source_subpath: Option<String>,
    pub created_at: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileChange {
    pub path: String,
    pub kind: String,
    pub before: Option<String>,
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before_metadata: Option<ContentEntryMetadata>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after_metadata: Option<ContentEntryMetadata>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentEntryMetadata {
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub readonly: Option<bool>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentPreview {
    pub id: String,
    pub skill_id: String,
    pub name: String,
    pub changes: Vec<FileChange>,
    pub targets: Vec<String>,
    pub restore: bool,
    pub fingerprint: String,
}

struct Stage(PathBuf);
impl Drop for Stage {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[derive(Debug)]
pub struct ContentCheckItem {
    pub skill_id: String,
    pub name: String,
    pub changed: bool,
    pub source_revision: Option<String>,
    pub target_count: usize,
    pub central_hash: String,
    pub source_hash: String,
}

fn legacy_cache_artifact(record: &SkillRecord, source_has_metadata: bool) -> bool {
    let legacy = Path::new(&record.central_path).join(super::installer::LEGACY_CACHE_META);
    record.source_type == "git"
        && record
            .source_subpath
            .as_deref()
            .map_or(true, |path| path == ".")
        && !source_has_metadata
        && super::installer::is_legacy_cache_metadata(&legacy)
        && std::fs::read(&legacy)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
            .is_some_and(|value| {
                record
                    .source_revision
                    .as_deref()
                    .is_some_and(|revision| value["head"].as_str() == Some(revision))
            })
}

/// Read-only content comparison. Only Git caches are refreshed; the central
/// library, deployments, and confirmation-plan store are untouched.
pub fn check_batch(
    paths: &RuntimePaths,
    store: &SkillStore,
    skill_ids: &[String],
    cancel: Option<&super::cancel_token::CancelToken>,
    mut on_item: impl FnMut(&str, Result<ContentCheckItem>),
) -> Result<()> {
    let _lock = super::installer::acquire_skill_update_lock(paths, store)?;
    let records = skill_ids
        .iter()
        .map(|id| {
            (
                id,
                store
                    .get_skill_by_id(id)
                    .and_then(|record| record.context("skill not found")),
            )
        })
        .collect::<Vec<_>>();
    let mut groups: BTreeMap<(String, Option<String>), Vec<SkillRecord>> = BTreeMap::new();
    for (_, record) in &records {
        if let Ok(record) = record {
            if record.source_type == "git" {
                if let Ok(source) = super::installer::git_check_source(record) {
                    groups
                        .entry((source.clone_url, source.branch))
                        .or_default()
                        .push(record.clone());
                }
            }
        }
    }
    let mut fetched: BTreeMap<
        (String, Option<String>),
        std::result::Result<super::installer::GitCheckSnapshot, String>,
    > = BTreeMap::new();
    for (id, record) in records {
        if cancel.is_some_and(|token| token.is_cancelled()) {
            break;
        }
        let result = (|| -> Result<ContentCheckItem> {
            let record = record?;
            let mut stage = None;
            let (source_hash, revision, source_has_metadata) = match record.source_type.as_str() {
                "git" => {
                    let source = super::installer::git_check_source(&record)?;
                    let key = (source.clone_url.clone(), source.branch.clone());
                    let result = fetched.entry(key.clone()).or_insert_with(|| {
                        super::installer::refresh_git_check_group(
                            paths,
                            store,
                            &source,
                            &groups[&key],
                            cancel,
                        )
                        .map_err(|error| format!("{error:#}"))
                    });
                    let snapshot = result
                        .as_ref()
                        .map_err(|error| anyhow::anyhow!(error.clone()))?;
                    let (hash, has_metadata) = snapshot.contents[&record.id]
                        .as_ref()
                        .map_err(|error| anyhow::anyhow!(error.clone()))?;
                    (hash.clone(), Some(snapshot.revision.clone()), *has_metadata)
                }
                "local" => {
                    let path = PathBuf::from(
                        record
                            .source_ref
                            .as_deref()
                            .context("missing local source")?,
                    );
                    let (hash, has_metadata) = super::installer::content_check_source_hash(&path)?;
                    (hash, None, has_metadata)
                }
                "bundled" => {
                    let (path, revision, _) =
                        super::installer::stage_skill_source(paths, store, &record, cancel)?;
                    stage = Some(Stage(path.clone()));
                    let (hash, has_metadata) = super::installer::content_check_source_hash(&path)?;
                    (hash, revision, has_metadata)
                }
                _ => anyhow::bail!("unsupported source type for update"),
            };
            let central = Path::new(&record.central_path);
            let central_hash = if legacy_cache_artifact(&record, source_has_metadata) {
                super::content_hash::hash_dir_without_legacy_cache_metadata(central)?
            } else {
                hash_dir(central)?
            };
            let item = ContentCheckItem {
                skill_id: record.id.clone(),
                name: record.name,
                changed: central_hash != source_hash,
                source_revision: revision,
                target_count: store.list_skill_targets(&record.id)?.len(),
                central_hash,
                source_hash,
            };
            drop(stage);
            Ok(item)
        })();
        if cancel.is_some_and(|token| token.is_cancelled()) {
            break;
        }
        on_item(id, result);
    }
    Ok(())
}

pub(crate) fn update_fingerprint(
    store: &SkillStore,
    record: &SkillRecord,
    staging: &Path,
) -> Result<String> {
    let targets = store.list_skill_targets(&record.id)?;
    let fingerprints = targets
        .iter()
        .map(|target| deployment_fingerprint(Path::new(&target.target_path)))
        .collect::<Result<Vec<_>>>()?;
    let snapshot = serde_json::json!([
        record,
        hash_dir_strict(Path::new(&record.central_path))?,
        hash_dir_strict(staging)?,
        targets,
        fingerprints
    ]);
    Ok(format!(
        "{:x}",
        Sha256::digest(snapshot.to_string().as_bytes())
    ))
}

pub fn preview(
    paths: &RuntimePaths,
    store: &SkillStore,
    id: &str,
    restore: bool,
) -> Result<ContentPreview> {
    let _lock = super::installer::acquire_skill_update_lock(paths, store)?;
    let record = store.get_skill_by_id(id)?.context("skill not found")?;
    let central = Path::new(&record.central_path);
    super::installer::preflight_managed_skill_update_targets(store, id)?;
    let mut stage = None;
    let destination = if restore {
        let recovery = restore_record(store, &record)?;
        recovery.backup
    } else {
        let (path, _, _) = super::installer::stage_skill_source_fresh(paths, store, &record, None)?;
        stage = Some(Stage(path.clone()));
        path
    };
    let mut content_changes = changes(central, &destination)?;
    if !restore
        && legacy_cache_artifact(
            &record,
            destination
                .join(super::installer::LEGACY_CACHE_META)
                .exists(),
        )
    {
        content_changes.retain(|change| change.path != super::installer::LEGACY_CACHE_META);
    }
    let result = ContentPreview {
        id: uuid::Uuid::new_v4().to_string(),
        skill_id: id.into(),
        name: record.name.clone(),
        changes: content_changes,
        targets: store
            .list_skill_targets(id)?
            .into_iter()
            .map(|t| t.target_path)
            .collect(),
        restore,
        fingerprint: update_fingerprint(store, &record, &destination)?,
    };
    // A new check replaces only this Skill's matching intent. Keep distinct
    // UUIDs so a confirmation holding an older plan cannot execute the new one.
    for prior in store.workspace_list::<ContentPreview>("content-preview")? {
        if prior.skill_id == result.skill_id && prior.restore == result.restore {
            store.workspace_delete("content-preview", &prior.id)?;
        }
    }
    store.workspace_save("content-preview", &result.id, &result)?;
    drop(stage);
    Ok(result)
}

pub(crate) fn prepare(
    paths: &RuntimePaths,
    store: &SkillStore,
    before: &SkillRecord,
    staged: &Path,
    revision: Option<String>,
    subpath: Option<String>,
) -> Result<(Recovery, tempfile::TempDir)> {
    let root = paths.app_data_dir.join("content-recovery");
    std::fs::create_dir_all(&root)?;
    let backup = tempfile::Builder::new()
        .prefix("snapshot-")
        .tempdir_in(&root)?;
    copy_dir_recursive(Path::new(&before.central_path), backup.path())?;
    let before_hash = hash_dir_strict(backup.path())?;
    anyhow::ensure!(
        before_hash == hash_dir_strict(Path::new(&before.central_path))?,
        "PLAN_STALE"
    );
    Ok((
        Recovery {
            skill_id: before.id.clone(),
            before: before.clone(),
            before_baseline: store
                .get_setting(&format!("device_sync.source_baseline.{}", before.id))?,
            backup: backup.path().into(),
            before_hash,
            after_hash: hash_dir_strict(staged)?,
            after_content_hash: hash_dir(staged)?,
            after_source_revision: revision,
            after_source_subpath: subpath,
            created_at: chrono::Utc::now().timestamp_millis(),
        },
        backup,
    ))
}

fn restore_record(store: &SkillStore, current: &SkillRecord) -> Result<Recovery> {
    let recovery: Recovery = store
        .workspace_get("content-recovery", &current.id)?
        .context("no recoverable update")?;
    anyhow::ensure!(
        hash_dir_strict(Path::new(&current.central_path))? == recovery.after_hash,
        "CENTRAL_MODIFIED|{}",
        current.central_path
    );
    anyhow::ensure!(
        current.source_ref == recovery.before.source_ref
            && current.source_type == recovery.before.source_type
            && current.source_revision == recovery.after_source_revision
            && current.source_subpath == recovery.after_source_subpath,
        "PLAN_STALE"
    );
    anyhow::ensure!(
        hash_dir_strict(&recovery.backup)? == recovery.before_hash,
        "recovery content changed"
    );
    Ok(recovery)
}

pub fn restore(paths: &RuntimePaths, store: &SkillStore, preview_id: &str) -> Result<Vec<String>> {
    let _lock = super::installer::acquire_skill_update_lock(paths, store)?;
    let plan: ContentPreview = store
        .workspace_get("content-preview", preview_id)?
        .context("PLAN_STALE")?;
    anyhow::ensure!(plan.restore, "PLAN_STALE");
    let mut current = store
        .get_skill_by_id(&plan.skill_id)?
        .context("skill not found")?;
    let recovery = restore_record(store, &current)?;
    anyhow::ensure!(
        update_fingerprint(store, &current, &recovery.backup)? == plan.fingerprint,
        "PLAN_STALE"
    );
    super::installer::preflight_managed_skill_update_targets(store, &current.id)?;
    let old_conflict_hash = hash_dir_for_sync_conflict(Path::new(&current.central_path))?;
    let mut replacement = PreparedDirReplacement::prepare_copy(
        &recovery.backup,
        Path::new(&current.central_path),
        Some(recovery.after_hash.clone()),
        false,
    )?;
    replacement.activate()?;
    current.description = recovery.before.description.clone();
    current.content_hash = recovery.before.content_hash.clone();
    current.source_revision = recovery.before.source_revision.clone();
    current.source_subpath = recovery.before.source_subpath.clone();
    current.updated_at = chrono::Utc::now().timestamp_millis();
    if let Err(error) = replacement
        .verify_backup_unchanged()
        .and_then(|_| store.commit_skill_update_with_recovery(&current, &[], Some(&recovery), true))
    {
        replacement.rollback()?;
        return Err(error);
    }
    replacement.commit();
    // Deployment membership is read at restore time, including later additions.
    let mut failures = vec![];
    let mut seen = std::collections::HashSet::new();
    for target in store.list_skill_targets(&current.id)? {
        if target.mode == "copy"
            && target.status != "disabled"
            && seen.insert(target.target_path.clone())
        {
            if let Err(error) = super::tool_distribution::refresh_copy(
                store,
                &current.id,
                Path::new(&current.central_path),
                Path::new(&target.target_path),
                Some(&old_conflict_hash),
            ) {
                store.update_skill_target_status(
                    &current.id,
                    &target.tool,
                    &target.scope,
                    target.project_path.as_deref(),
                    "error",
                )?;
                log::warn!(
                    "restored content requires tool repair: {}",
                    super::skill_issues::safe_code(&error.to_string())
                );
                failures.push(target.tool);
            }
        }
    }
    store.workspace_delete("content-preview", preview_id)?;
    cleanup(paths, &recovery);
    Ok(failures)
}

#[derive(PartialEq, Eq)]
struct ContentEntry {
    metadata: ContentEntryMetadata,
    bytes: Option<Vec<u8>>,
}

fn files(root: &Path) -> Result<BTreeMap<String, ContentEntry>> {
    let mut result = BTreeMap::new();
    for entry in walkdir::WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| entry.file_name() != ".git")
    {
        let entry = entry?;
        if entry.depth() == 0 || !(entry.file_type().is_file() || entry.file_type().is_dir()) {
            continue;
        }
        let is_file = entry.file_type().is_file();
        // Match the entry kinds and file attributes used by hash_dir: these can
        // trigger an actual update even when every file's bytes are unchanged.
        let mut metadata = ContentEntryMetadata {
            kind: if is_file { "file" } else { "directory" }.into(),
            mode: None,
            readonly: None,
        };
        if is_file {
            let permissions = entry.metadata()?.permissions();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                metadata.mode = Some(permissions.mode() & 0o7777);
            }
            #[cfg(not(unix))]
            {
                metadata.readonly = Some(permissions.readonly());
            }
        }
        result.insert(
            entry
                .path()
                .strip_prefix(root)?
                .to_string_lossy()
                .into_owned(),
            ContentEntry {
                metadata,
                bytes: if is_file {
                    Some(std::fs::read(entry.path())?)
                } else {
                    None
                },
            },
        );
    }
    Ok(result)
}
fn text(entry: Option<&ContentEntry>) -> Option<String> {
    entry.and_then(|entry| entry.bytes.as_ref()).and_then(|b| {
        if b.len() <= 65536 {
            std::str::from_utf8(b)
                .ok()
                .filter(|s| !s.contains('\0'))
                .map(str::to_string)
        } else {
            None
        }
    })
}
fn changes(before: &Path, after: &Path) -> Result<Vec<FileChange>> {
    let a = files(before)?;
    let b = files(after)?;
    let paths = a
        .keys()
        .chain(b.keys())
        .collect::<std::collections::BTreeSet<_>>();
    Ok(paths
        .into_iter()
        .filter(|path| a.get(*path) != b.get(*path))
        .map(|path| FileChange {
            path: path.clone(),
            kind: if !a.contains_key(path) {
                "added"
            } else if !b.contains_key(path) {
                "removed"
            } else {
                "modified"
            }
            .into(),
            before: text(a.get(path)),
            after: text(b.get(path)),
            before_metadata: a.get(path).map(|entry| entry.metadata.clone()),
            after_metadata: b.get(path).map(|entry| entry.metadata.clone()),
        })
        .collect())
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceRepairPlan {
    pub id: String,
    pub skill_id: String,
    pub source_type: String,
    pub source_ref: String,
    pub source_subpath: Option<String>,
    pub fingerprint: String,
    pub targets: Vec<String>,
}
pub fn preview_source_repair(
    paths: &RuntimePaths,
    store: &SkillStore,
    id: &str,
    kind: &str,
    reference: &str,
    subpath: Option<String>,
) -> Result<SourceRepairPlan> {
    let _lock = super::installer::acquire_skill_update_lock(paths, store)?;
    let old = store.get_skill_by_id(id)?.context("skill not found")?;
    anyhow::ensure!(matches!(kind, "git" | "local"), "invalid source type");
    if let Some(path) = &subpath {
        anyhow::ensure!(
            !Path::new(path).is_absolute()
                && Path::new(path)
                    .components()
                    .all(|p| matches!(p, std::path::Component::Normal(_))),
            "invalid source subpath"
        );
    }
    let source_ref = if kind == "local" {
        let home = paths
            .default_central_repo
            .parent()
            .context("runtime home unavailable")?;
        let path = if reference == "~" {
            home.to_path_buf()
        } else if let Some(rest) = reference.strip_prefix("~/") {
            home.join(rest)
        } else {
            PathBuf::from(reference)
        };
        std::fs::canonicalize(path)?.to_string_lossy().into_owned()
    } else {
        let url = reqwest::Url::parse(reference)?;
        anyhow::ensure!(
            matches!(url.scheme(), "https" | "http")
                && url.username().is_empty()
                && url.password().is_none()
                && url.query().is_none()
                && url.fragment().is_none(),
            "use a repository URL without embedded credentials"
        );
        reference.trim().to_string()
    };
    let mut candidate = old.clone();
    candidate.source_type = kind.into();
    candidate.source_ref = Some(source_ref.clone());
    candidate.source_subpath = subpath.clone();
    candidate.source_revision = None;
    let (path, _, _) = super::installer::stage_skill_source(paths, store, &candidate, None)?;
    let stage = Stage(path);
    let plan = SourceRepairPlan {
        id: uuid::Uuid::new_v4().to_string(),
        skill_id: id.into(),
        source_type: kind.into(),
        source_ref,
        source_subpath: subpath,
        fingerprint: update_fingerprint(store, &old, &stage.0)?,
        targets: store
            .list_skill_targets(id)?
            .into_iter()
            .map(|r| r.target_path)
            .collect(),
    };
    store.workspace_save("source-repair", &plan.id, &plan)?;
    Ok(plan)
}
pub fn apply_source_repair(paths: &RuntimePaths, store: &SkillStore, id: &str) -> Result<()> {
    let _lock = super::installer::acquire_skill_update_lock(paths, store)?;
    let plan: SourceRepairPlan = store
        .workspace_get("source-repair", id)?
        .context("PLAN_STALE")?;
    let old = store
        .get_skill_by_id(&plan.skill_id)?
        .context("skill not found")?;
    let mut candidate = old.clone();
    candidate.source_type = plan.source_type;
    candidate.source_ref = Some(plan.source_ref);
    candidate.source_subpath = plan.source_subpath;
    candidate.source_revision = None;
    let (path, revision, subpath) =
        super::installer::stage_skill_source(paths, store, &candidate, None)?;
    let stage = Stage(path);
    anyhow::ensure!(
        update_fingerprint(store, &old, &stage.0)? == plan.fingerprint,
        "PLAN_STALE"
    );
    candidate.source_revision = revision;
    candidate.source_subpath = subpath;
    let obsolete: Option<Recovery> = store.workspace_get("content-recovery", &candidate.id)?;
    store.commit_source_repair(&candidate, hash_dir(&stage.0)?)?;
    if let Some(recovery) = obsolete {
        cleanup(paths, &recovery);
    }
    store.workspace_delete("source-repair", id)?;
    Ok(())
}

pub(crate) fn cleanup(paths: &RuntimePaths, recovery: &Recovery) {
    let root = paths.app_data_dir.join("content-recovery");
    if recovery.backup.parent() == Some(root.as_path())
        && recovery
            .backup
            .file_name()
            .is_some_and(|name| name.to_string_lossy().starts_with("snapshot-"))
        && std::fs::symlink_metadata(&recovery.backup)
            .is_ok_and(|metadata| metadata.is_dir() && !metadata.file_type().is_symlink())
        && std::fs::remove_dir_all(&recovery.backup).is_err()
    {
        log::warn!("obsolete content recovery could not be cleaned up");
    }
}

#[cfg(test)]
#[path = "tests/content_recovery.rs"]
mod tests;
