pub mod credentials;
mod device_registry;
pub(crate) mod errors;
mod git_repo;
pub mod manifest;
pub mod merge;
pub mod oauth;
pub mod providers;
pub mod scheduler;
mod text_merge;
pub mod types;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use anyhow::{bail, Context, Result};
use uuid::Uuid;

use self::credentials::CredentialStore;
use self::manifest::{portable_hash, skill_dir, SyncManifest};
use self::merge::{plan_merge_with_text, MergePlan};
use self::types::{
    ConflictResolution, DeviceSyncConfig, DeviceSyncDevice, SyncChangeItem, SyncChangeSummary,
    SyncConflict, SyncRunResult, SyncStatus, TrashEntry,
};
use crate::core::skill_store::SkillStore;

pub struct DeviceSyncService<'a> {
    store: &'a SkillStore,
    credentials: &'a dyn CredentialStore,
    workspace_root: PathBuf,
    central_root: PathBuf,
}

static SYNC_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
const RESOLVED_SYNC_KEY: &str = "device_sync.resolved_conflicts";
const INCOMPLETE_SYNC_KEY: &str = "device_sync.incomplete_sync";

#[derive(serde::Deserialize, serde::Serialize)]
struct ResolvedSyncConflict {
    base_commit: Option<String>,
    remote_url: String,
    branch: String,
    skill_id: String,
    remote_commit: String,
}

#[derive(serde::Deserialize, serde::Serialize)]
struct IncompleteSync {
    remote_url: String,
    branch: String,
    base_commit: Option<String>,
}

#[derive(Default, serde::Deserialize, serde::Serialize)]
struct IncompleteSyncJournal {
    entries: Vec<IncompleteSync>,
}

impl<'a> DeviceSyncService<'a> {
    pub fn new(
        store: &'a SkillStore,
        credentials: &'a dyn CredentialStore,
        workspace_root: PathBuf,
        central_root: PathBuf,
    ) -> Self {
        Self {
            store,
            credentials,
            workspace_root,
            central_root,
        }
    }

    pub fn status(&self) -> Result<SyncStatus> {
        let config = self.store.get_device_sync_config()?;
        let history = self.store.list_device_sync_history(1)?;
        let last_run = self
            .store
            .get_setting("device_sync_last_run")?
            .and_then(|value| serde_json::from_str::<(String, i64)>(&value).ok());
        let conflicts = self.store.list_device_sync_conflicts()?;
        let repository_head_commit = config.as_ref().and_then(|item| {
            let repo = git2::Repository::open(self.workspace_root.join("repository")).ok()?;
            if !git_repo::origin_matches(&repo, item) {
                return None;
            }
            git_repo::remote_head(&repo, item).map(|oid| oid.to_string())
        });
        Ok(SyncStatus {
            tool_issues: self.store.list_tool_sync_issues()?,
            schedule_status: None,
            configured: config.is_some(),
            is_running: is_device_sync_running(),
            provider: config
                .as_ref()
                .map(|item| item.provider)
                .unwrap_or_default(),
            remote_url: config
                .as_ref()
                .map(|item| item.remote_url.clone())
                .unwrap_or_default(),
            auto_check: config.as_ref().is_some_and(|item| item.auto_check),
            auto_sync: config.as_ref().is_some_and(|item| item.auto_sync),
            last_synced_commit: config
                .as_ref()
                .and_then(|item| item.last_synced_commit.clone()),
            repository_head_commit,
            pending_local_changes: 0,
            conflict_count: conflicts.len(),
            last_run_status: last_run
                .as_ref()
                .map(|item| item.0.clone())
                .or_else(|| history.first().map(|item| item.status.clone())),
            last_run_at: last_run
                .map(|item| item.1)
                .or_else(|| history.first().and_then(|item| item.finished_at)),
        })
    }

    pub fn repair_legacy_same_device_conflicts(&self) -> Result<bool> {
        let Some(mut config) = self.store.get_device_sync_config()? else {
            return Ok(false);
        };
        let conflicts = self.store.list_device_sync_conflicts()?;
        let Some(remote_commit) = conflicts
            .first()
            .map(|conflict| conflict.remote_commit.as_str())
        else {
            return Ok(false);
        };
        if conflicts.iter().any(|conflict| {
            conflict.base_commit.is_some() || conflict.remote_commit != remote_commit
        }) {
            return Ok(false);
        }
        let repo = match git2::Repository::open(self.workspace_root.join("repository")) {
            Ok(repo) if git_repo::origin_matches(&repo, &config) => repo,
            _ => return Ok(false),
        };
        let remote_oid = match git2::Oid::from_str(remote_commit) {
            Ok(oid) if repo.find_commit(oid).is_ok() => oid,
            _ => return Ok(false),
        };
        let Some(repository_head) = git_repo::remote_head(&repo, &config) else {
            return Ok(false);
        };
        let device = self.local_device_identity()?;
        if git_repo::latest_device_commit_at(&repo, remote_oid, &device.id)? != Some(remote_oid) {
            return Ok(false);
        }
        if let Some(current_oid) = config
            .last_synced_commit
            .as_deref()
            .and_then(|value| git2::Oid::from_str(value).ok())
        {
            if current_oid != repository_head || repo.find_commit(current_oid).is_err() {
                return Ok(false);
            }
            let current_is_same_device =
                git_repo::latest_device_commit_at(&repo, current_oid, &device.id)?
                    == Some(current_oid);
            let descends_from_conflict =
                current_oid == remote_oid || repo.graph_descendant_of(current_oid, remote_oid)?;
            if !current_is_same_device || !descends_from_conflict {
                return Ok(false);
            }
        } else if repository_head != remote_oid {
            return Ok(false);
        }
        config.last_synced_commit = Some(remote_commit.to_string());
        self.store
            .resolve_device_sync_conflicts_and_save_config_if_clear(
                &conflicts
                    .into_iter()
                    .map(|conflict| conflict.id)
                    .collect::<Vec<_>>(),
                &config,
            )?;
        Ok(true)
    }

    pub fn check(&self) -> Result<SyncChangeSummary> {
        let _guard = try_lock_device_sync()?;
        let config = self.require_config()?;
        let token = self.read_token(&config)?;
        let repo_path = self.workspace_root.join("repository");
        let repo = git_repo::open_or_clone(&repo_path, &config, token.as_deref())
            .context(read_failure_context(&config))?;
        let remote_oid = git_repo::fetch_and_checkout(&repo, &config, token.as_deref())
            .context(read_failure_context(&config))?;
        self.ingest_discovered_devices(&repo, remote_oid)?;
        let remote = SyncManifest::read(&repo_path)?;
        let device = self.local_device_identity()?;
        let (mut base, base_commit) = self.base_manifest(&repo, &config, remote_oid, &device.id)?;
        let export = self.fresh_export()?;
        let mut local = SyncManifest::read(&export)?;
        reconcile_identities(&mut local, &remote, &export)?;
        let resolved_bases = self.apply_resolved_bases(&repo, &config, base_commit, &mut base)?;
        let plan = plan_merge_with_text(&base, &local, &remote, |id, path| {
            merge_snapshot_file(
                &repo,
                resolved_bases.get(id).copied().or(base_commit),
                remote_oid,
                &export,
                [&base, &local, &remote],
                id,
                path,
            )
        })?;
        let summary = summarize(&plan, &local, &remote);
        let _ = fs::remove_dir_all(export);
        if remote_oid.is_none() && local.skills.is_empty() {
            return Ok(SyncChangeSummary::default());
        }
        Ok(summary)
    }

    pub fn devices(&self) -> Result<Vec<DeviceSyncDevice>> {
        self.require_config()?;
        let current = self.local_device_identity()?;
        self.store.list_device_sync_devices(&current.id)
    }

    pub fn sync(&self) -> Result<SyncRunResult> {
        let _guard = try_lock_device_sync()?;
        self.sync_locked()
    }

    pub fn sync_scheduled(&self, expected: &DeviceSyncConfig) -> Result<Option<SyncRunResult>> {
        let Ok(_guard) = try_lock_device_sync() else {
            return Ok(None);
        };
        let Some(current) = self.store.get_device_sync_config()? else {
            return Ok(None);
        };
        if !current.auto_sync
            || current.needs_visibility_confirmation()
            || current.needs_public_upload_confirmation()
            || current.auto_sync_schedule.is_none()
            || current.remote_url != expected.remote_url
            || current.branch != expected.branch
            || current.auto_sync_schedule != expected.auto_sync_schedule
            || !self.store.list_device_sync_conflicts()?.is_empty()
        {
            return Ok(None);
        }
        current.auto_sync_schedule.as_ref().unwrap().validate()?;
        self.sync_locked().map(Some)
    }

    fn sync_locked(&self) -> Result<SyncRunResult> {
        let run_id = Uuid::new_v4().to_string();
        let started_at = now_ms();
        self.store.start_device_sync_run(&run_id, started_at)?;
        let result = self.sync_inner();
        match &result {
            Ok(done) => self.store.finish_device_sync_run(
                &run_id,
                now_ms(),
                &done.status,
                done.changes.added,
                done.changes.updated,
                done.changes.deleted,
                done.changes.conflicted,
                done.commit.as_deref(),
                None,
                Some(&done.changes.items),
            )?,
            Err(err) => self.store.finish_device_sync_run(
                &run_id,
                now_ms(),
                "failed",
                0,
                0,
                0,
                0,
                None,
                Some(&errors::safe_message(&format!("{err:#}"))),
                None,
            )?,
        }
        result.map_err(|err| anyhow::anyhow!(errors::format_error(err)))
    }

    pub fn resolve_conflict(
        &self,
        conflict_id: &str,
        resolution: ConflictResolution,
    ) -> Result<()> {
        let _guard = try_lock_device_sync()?;
        let conflict = self
            .store
            .list_device_sync_conflicts()?
            .into_iter()
            .find(|item| item.id == conflict_id)
            .context("device sync conflict not found")?;
        let config = self.require_config()?;
        if matches!(resolution, ConflictResolution::KeepLocal) {
            return self.remember_resolved_conflict(&config, &conflict, &[]);
        }
        let repo_root = self.workspace_root.join("repository");
        let repo = git2::Repository::open(&repo_root)?;
        anyhow::ensure!(
            git_repo::origin_matches(&repo, &config),
            "sync repository changed"
        );
        let commit = git2::Oid::from_str(&conflict.remote_commit)?;
        let manifest = git_repo::manifest_at(&repo, commit)?;
        let remote = manifest.skills.get(&conflict.skill_id);
        let local = self.store.get_skill_by_id(&conflict.skill_id)?;
        let mut changes = Vec::new();
        if remote.is_none() {
            if matches!(resolution, ConflictResolution::UseRemote) {
                self.apply_remote_deletions(&MergePlan {
                    delete_local: BTreeSet::from([conflict.skill_id.clone()]),
                    ..MergePlan::default()
                })?;
            }
            if let Some(local) =
                local.filter(|_| matches!(resolution, ConflictResolution::UseRemote))
            {
                changes.push(SyncChangeItem {
                    skill_id: local.id,
                    name: local.name,
                    kind: "deleted".into(),
                    direction: "download".into(),
                });
            }
            return self.remember_resolved_conflict(&config, &conflict, &changes);
        }
        let remote = remote.unwrap();
        let snapshot = git_repo::skill_snapshot_at(&repo, commit, remote)?;
        let content_changed = if let Some(local) = &local {
            let staging = tempfile::tempdir()?;
            !Path::new(&local.central_path).is_dir()
                || manifest::export_skill(self.store, local.clone(), staging.path())?.content_hash
                    != remote.content_hash
        } else {
            true
        };
        if content_changed {
            changes.push(SyncChangeItem {
                skill_id: remote.id.clone(),
                name: remote.name.clone(),
                kind: if local.is_some() { "updated" } else { "added" }.into(),
                direction: "download".into(),
            });
        }
        if matches!(resolution, ConflictResolution::KeepBoth) {
            if let Some(local) = self.store.get_skill_by_id(&conflict.skill_id)? {
                let new_id = Uuid::new_v4().to_string();
                let source = PathBuf::from(&local.central_path);
                let duplicate_name = format!("{} (Local)", local.name);
                let destination = unique_skill_path(&self.central_root, &duplicate_name, &new_id)?;
                manifest::copy_local_files(&source, &destination)?;
                let mut duplicate = local;
                duplicate.id = new_id;
                duplicate.name = duplicate_name;
                if duplicate.source_type == "local"
                    && duplicate.source_ref.as_deref() == Some(duplicate.central_path.as_str())
                {
                    duplicate.source_ref = Some(destination.to_string_lossy().to_string());
                }
                duplicate.central_path = destination.to_string_lossy().to_string();
                duplicate.updated_at = now_ms();
                self.store.upsert_skill(&duplicate)?;
                changes.push(SyncChangeItem {
                    skill_id: duplicate.id.clone(),
                    name: duplicate.name.clone(),
                    kind: "added".into(),
                    direction: "merge".into(),
                });
                let tags = self.store.get_skill_tags(&conflict.skill_id)?;
                self.store.set_skill_tag_names(
                    &duplicate.id,
                    &tags.into_iter().map(|tag| tag.name).collect::<Vec<_>>(),
                )?;
            }
        }
        self.apply_repository_to_library(
            &SyncManifest {
                format_version: manifest.format_version,
                skills: std::collections::BTreeMap::from([(remote.id.clone(), remote.clone())]),
            },
            snapshot.path(),
            &BTreeSet::new(),
        )?;
        self.remember_resolved_conflict(&config, &conflict, &changes)
    }

    pub fn restore_trash(&self, trash_id: &str) -> Result<()> {
        let _guard = try_lock_device_sync()?;
        let entry = self
            .store
            .list_device_sync_trash()?
            .into_iter()
            .find(|item| item.id == trash_id)
            .context("device sync trash entry not found")?;
        let source = PathBuf::from(&entry.trash_path);
        if !source.is_dir() {
            bail!("device sync trash content is missing");
        }
        anyhow::ensure!(
            self.store.get_skill_by_id(&entry.skill_id)?.is_none(),
            "Skill already exists; cannot overwrite it from the recycle bin"
        );
        let metadata = self.store.device_sync_trash_metadata(trash_id)?;
        let description = match &metadata {
            Some(metadata) => metadata.description.clone(),
            None => crate::core::installer::parse_skill_md(&source.join("SKILL.md"))
                .and_then(|(_, description)| description),
        };
        let tags = metadata.map(|metadata| metadata.tags).unwrap_or_default();
        fs::create_dir_all(&self.central_root)?;
        let directory_name: String = entry
            .skill_name
            .chars()
            .filter(|c| c.is_alphanumeric() || matches!(c, '-' | '_'))
            .take(64)
            .collect();
        let restored_directory = tempfile::Builder::new()
            .prefix(&format!("{directory_name}-restored-"))
            .tempdir_in(&self.central_root)?;
        let destination = restored_directory.path().to_path_buf();
        (|| -> Result<()> {
            manifest::copy_local_files(&source, &destination)?;
            let files = manifest::hash_files(&destination)?;
            let now = now_ms();
            self.store.restore_device_sync_trash_record(
                trash_id,
                &crate::core::skill_store::SkillRecord {
                    id: entry.skill_id,
                    name: entry.skill_name,
                    description,
                    source_type: "sync_restore".to_string(),
                    source_ref: None,
                    source_subpath: None,
                    source_revision: None,
                    central_path: destination.to_string_lossy().to_string(),
                    content_hash: Some(manifest::aggregate_hash(&files)),
                    created_at: now,
                    updated_at: now,
                    last_sync_at: None,
                    last_seen_at: now,
                    enabled: true,
                    status: "ok".to_string(),
                },
                &tags,
            )
        })()?;
        let _ = restored_directory.keep();
        let _ = fs::remove_dir_all(source);
        Ok(())
    }

    fn sync_inner(&self) -> Result<SyncRunResult> {
        match self.sync_attempt() {
            Err(err)
                if err
                    .downcast_ref::<git2::Error>()
                    .is_some_and(|git| git.code() == git2::ErrorCode::NotFastForward) =>
            {
                self.sync_attempt()
            }
            result => result,
        }
    }

    fn sync_attempt(&self) -> Result<SyncRunResult> {
        let mut config = self.require_config()?;
        let pending_conflicts = self.store.list_device_sync_conflicts()?;
        if !pending_conflicts.is_empty() {
            return Ok(SyncRunResult {
                status: "conflicts".to_string(),
                commit: config.last_synced_commit,
                changes: SyncChangeSummary {
                    conflicted: pending_conflicts.len(),
                    items: pending_conflicts
                        .iter()
                        .map(|conflict| SyncChangeItem {
                            skill_id: conflict.skill_id.clone(),
                            name: conflict.skill_name.clone(),
                            kind: "conflicted".into(),
                            direction: "merge".into(),
                        })
                        .collect(),
                    ..SyncChangeSummary::default()
                },
                message: "device sync requires conflict resolution".to_string(),
            });
        }
        anyhow::ensure!(
            !config.needs_public_upload_confirmation(),
            "DEVICE_SYNC_PUBLIC_UPLOAD_CONFIRMATION"
        );
        let token = self.read_token(&config)?;
        let repo_path = self.workspace_root.join("repository");
        let repo = git_repo::open_or_clone(&repo_path, &config, token.as_deref())
            .context(read_failure_context(&config))?;
        let parent = git_repo::fetch_and_checkout(&repo, &config, token.as_deref())
            .context(read_failure_context(&config))?;
        let mut registry = device_registry::DeviceRegistry::read_at(&repo, parent)?;
        self.ingest_discovered_devices(&repo, parent)?;
        let mut remote = SyncManifest::read(&repo_path)?;
        let device = self.local_device_identity()?;
        let (mut base, base_commit) = self.base_manifest(&repo, &config, parent, &device.id)?;
        let export = self.fresh_export()?;
        let mut local = SyncManifest::read(&export)?;
        for (old_id, new_id) in reconcile_identities(&mut local, &remote, &export)? {
            self.store.adopt_skill_id(&old_id, &new_id)?;
        }
        let resolved_bases = self.apply_resolved_bases(&repo, &config, base_commit, &mut base)?;
        let plan = plan_merge_with_text(&base, &local, &remote, |id, path| {
            merge_snapshot_file(
                &repo,
                resolved_bases.get(id).copied().or(base_commit),
                parent,
                &export,
                [&base, &local, &remote],
                id,
                path,
            )
        })?;
        self.record_conflicts(&plan, &local, &remote, base_commit, parent)?;
        let changes = summarize(&plan, &local, &remote);
        if !plan.conflicts.is_empty() {
            let _ = fs::remove_dir_all(&export);
            return Ok(SyncRunResult {
                status: "conflicts".to_string(),
                commit: config.last_synced_commit,
                changes,
                message: "device sync requires conflict resolution".to_string(),
            });
        }
        apply_plan_to_repository(&plan, &local, &export, &mut remote, &repo_path)?;
        remote.write(&repo_path)?;
        registry.record(&device);
        registry.write(&repo_path)?;

        let message = format!(
            "Sync Skills Hub library\n\nSkills-Hub-Device-ID: {}\nSkills-Hub-Device-Name: {}",
            device.id, device.name
        );
        let parent_is_current_device = match parent {
            Some(parent) => {
                git_repo::latest_device_commit_at(&repo, parent, &device.id)? == Some(parent)
            }
            None => false,
        };
        let commit = if parent_is_current_device {
            git_repo::commit_all(&repo, &message, parent)?
        } else {
            git_repo::commit_all_allow_empty(&repo, &message, parent)?
        };
        let final_oid = if let Some(oid) = commit {
            self.remember_incomplete_sync(&config, base_commit)?;
            let write_token = if token.is_some() {
                token
            } else {
                self.token(&config)?
            };
            git_repo::push(&repo, &config, write_token.as_deref(), oid)?;
            git_repo::update_remote_head(&repo, &config, oid)?;
            Some(oid)
        } else {
            parent
        };
        self.apply_repository_to_library(
            &remote,
            &repo_path,
            &plan.conflicts.keys().cloned().collect(),
        )?;
        self.apply_remote_deletions(&plan)?;
        let _ = fs::remove_dir_all(&export);
        config.last_synced_commit = final_oid.map(|oid| oid.to_string());
        self.store.save_device_sync_config(&config)?;
        self.clear_incomplete_sync(&config)?;
        self.clear_resolved_conflicts(&config)?;
        self.ingest_discovered_devices(&repo, final_oid)?;
        let mut current = device;
        current.last_commit = final_oid.map(|oid| oid.to_string());
        current.last_seen_at = now_ms();
        self.store.upsert_device_sync_device(&current)?;
        Ok(SyncRunResult {
            status: if changes.conflicted > 0 {
                "conflicts"
            } else {
                "success"
            }
            .to_string(),
            commit: config.last_synced_commit,
            changes,
            message: "device sync completed".to_string(),
        })
    }

    fn local_device_identity(&self) -> Result<DeviceSyncDevice> {
        const DEVICE_ID_KEY: &str = "device_sync.local_device_id";
        let id = match self.store.get_setting(DEVICE_ID_KEY)? {
            Some(id) => id,
            None => {
                let id = Uuid::new_v4().to_string();
                self.store.set_setting(DEVICE_ID_KEY, &id)?;
                id
            }
        };
        Ok(DeviceSyncDevice {
            id,
            name: local_device_name(),
            alias: None,
            last_commit: None,
            last_seen_at: now_ms(),
            is_current: true,
        })
    }

    fn ingest_discovered_devices(
        &self,
        repo: &git2::Repository,
        remote_head: Option<git2::Oid>,
    ) -> Result<()> {
        let Some(remote_head) = remote_head else {
            return Ok(());
        };
        for device in device_registry::DeviceRegistry::read_at(repo, Some(remote_head))?.devices() {
            self.store.upsert_device_sync_device(&device)?;
        }
        Ok(())
    }

    fn require_config(&self) -> Result<DeviceSyncConfig> {
        let config = self
            .store
            .get_device_sync_config()?
            .context("device sync is not configured")?;
        if config.remote_url.trim().is_empty() {
            bail!("device sync repository URL is empty");
        }
        Ok(config)
    }

    fn token(&self, config: &DeviceSyncConfig) -> Result<Option<String>> {
        if !config.uses_https() {
            return Ok(None);
        }
        match config.credential_key.as_deref() {
            Some(key) => {
                let usage =
                    types::CredentialUsage::from_https_remote(config.provider, &config.remote_url)?;
                credentials::resolve_access_token(self.credentials, key, &usage)
            }
            None => Ok(None),
        }
    }

    fn read_token(&self, config: &DeviceSyncConfig) -> Result<Option<String>> {
        if !config.uses_https() {
            return Ok(None);
        }
        match config.visibility {
            types::RepositoryVisibility::Public => Ok(None),
            types::RepositoryVisibility::Unknown => bail!("DEVICE_SYNC_VISIBILITY_UNKNOWN"),
            types::RepositoryVisibility::Private | types::RepositoryVisibility::Internal => self
                .token(config)?
                .map(Some)
                .context("DEVICE_SYNC_READ_CREDENTIAL_REQUIRED"),
        }
    }

    fn fresh_export(&self) -> Result<PathBuf> {
        let export = self
            .workspace_root
            .join(format!("export-{}", Uuid::new_v4()));
        manifest::export_library(self.store, &export)?;
        Ok(export)
    }

    fn resolved_conflicts(&self) -> Result<Vec<ResolvedSyncConflict>> {
        self.store.get_setting(RESOLVED_SYNC_KEY)?.map_or_else(
            || Ok(Vec::new()),
            |raw| serde_json::from_str(&raw).context("decode resolved device sync conflicts"),
        )
    }

    fn remember_resolved_conflict(
        &self,
        config: &DeviceSyncConfig,
        conflict: &SyncConflict,
        changes: &[SyncChangeItem],
    ) -> Result<()> {
        let mut entries = self.resolved_conflicts()?;
        entries.retain(|entry| {
            entry.remote_url != config.remote_url
                || entry.branch != config.branch
                || entry.skill_id != conflict.skill_id
        });
        entries.push(ResolvedSyncConflict {
            base_commit: conflict.base_commit.clone(),
            remote_url: config.remote_url.clone(),
            branch: config.branch.clone(),
            skill_id: conflict.skill_id.clone(),
            remote_commit: conflict.remote_commit.clone(),
        });
        self.store.resolve_device_sync_conflict_with_state(
            &conflict.id,
            RESOLVED_SYNC_KEY,
            &serde_json::to_string(&entries)?,
            changes,
            &conflict.remote_commit,
        )
    }

    fn apply_resolved_bases(
        &self,
        repo: &git2::Repository,
        config: &DeviceSyncConfig,
        base_commit: Option<git2::Oid>,
        base: &mut SyncManifest,
    ) -> Result<BTreeMap<String, git2::Oid>> {
        let mut commits = BTreeMap::new();
        for entry in self.resolved_conflicts()?.into_iter().filter(|entry| {
            entry.remote_url == config.remote_url
                && entry.branch == config.branch
                && entry.base_commit == base_commit.map(|oid| oid.to_string())
        }) {
            let oid = git2::Oid::from_str(&entry.remote_commit)
                .context("decode resolved conflict commit")?;
            let mut snapshot = git_repo::manifest_at(repo, oid)?;
            match snapshot.skills.remove(&entry.skill_id) {
                Some(skill) => {
                    base.skills.insert(entry.skill_id.clone(), skill);
                }
                None => {
                    base.skills.remove(&entry.skill_id);
                }
            }
            commits.insert(entry.skill_id, oid);
        }
        Ok(commits)
    }

    fn clear_resolved_conflicts(&self, config: &DeviceSyncConfig) -> Result<()> {
        let mut entries = self.resolved_conflicts()?;
        entries
            .retain(|entry| entry.remote_url != config.remote_url || entry.branch != config.branch);
        if entries.is_empty() {
            self.store.delete_setting(RESOLVED_SYNC_KEY)
        } else {
            self.store
                .set_setting(RESOLVED_SYNC_KEY, &serde_json::to_string(&entries)?)
        }
    }

    fn base_manifest(
        &self,
        repo: &git2::Repository,
        config: &DeviceSyncConfig,
        remote_head: Option<git2::Oid>,
        device_id: &str,
    ) -> Result<(SyncManifest, Option<git2::Oid>)> {
        if let Some(oid) = config
            .last_synced_commit
            .as_deref()
            .and_then(|value| git2::Oid::from_str(value).ok())
            .filter(|oid| repo.find_commit(*oid).is_ok())
        {
            return Ok((git_repo::manifest_at(repo, oid)?, Some(oid)));
        }
        if let Some(incomplete) = self
            .load_incomplete_sync_journal()?
            .entries
            .into_iter()
            .find(|entry| entry.remote_url == config.remote_url && entry.branch == config.branch)
        {
            return match incomplete.base_commit {
                Some(value) => {
                    let oid = git2::Oid::from_str(&value)
                        .context("decode incomplete device sync base commit")?;
                    repo.find_commit(oid)
                        .context("incomplete device sync base commit is missing")?;
                    Ok((git_repo::manifest_at(repo, oid)?, Some(oid)))
                }
                None => Ok((SyncManifest::empty(), None)),
            };
        }
        let recovered = match remote_head {
            Some(head) => git_repo::latest_device_commit_at(repo, head, device_id)?,
            None => None,
        };
        match recovered {
            Some(oid) => Ok((git_repo::manifest_at(repo, oid)?, Some(oid))),
            None => Ok((SyncManifest::empty(), None)),
        }
    }

    fn remember_incomplete_sync(
        &self,
        config: &DeviceSyncConfig,
        base_commit: Option<git2::Oid>,
    ) -> Result<()> {
        let mut journal = self.load_incomplete_sync_journal()?;
        if journal
            .entries
            .iter()
            .any(|entry| entry.remote_url == config.remote_url && entry.branch == config.branch)
        {
            return Ok(());
        }
        journal.entries.push(IncompleteSync {
            remote_url: config.remote_url.clone(),
            branch: config.branch.clone(),
            base_commit: base_commit.map(|oid| oid.to_string()),
        });
        self.store
            .set_setting(INCOMPLETE_SYNC_KEY, &serde_json::to_string(&journal)?)
    }

    fn clear_incomplete_sync(&self, config: &DeviceSyncConfig) -> Result<()> {
        let mut journal = self.load_incomplete_sync_journal()?;
        journal
            .entries
            .retain(|entry| entry.remote_url != config.remote_url || entry.branch != config.branch);
        if journal.entries.is_empty() {
            self.store.delete_setting(INCOMPLETE_SYNC_KEY)
        } else {
            self.store
                .set_setting(INCOMPLETE_SYNC_KEY, &serde_json::to_string(&journal)?)
        }
    }

    fn load_incomplete_sync_journal(&self) -> Result<IncompleteSyncJournal> {
        match self.store.get_setting(INCOMPLETE_SYNC_KEY)? {
            Some(raw) => serde_json::from_str(&raw).context("decode incomplete device sync state"),
            None => Ok(IncompleteSyncJournal::default()),
        }
    }

    fn record_conflicts(
        &self,
        plan: &MergePlan,
        local: &SyncManifest,
        remote: &SyncManifest,
        base_commit: Option<git2::Oid>,
        remote_oid: Option<git2::Oid>,
    ) -> Result<()> {
        for (skill_id, files) in &plan.conflicts {
            let name = local
                .skills
                .get(skill_id)
                .or_else(|| remote.skills.get(skill_id))
                .map(|skill| skill.name.clone())
                .unwrap_or_else(|| skill_id.clone());
            self.store.upsert_device_sync_conflict(&SyncConflict {
                id: format!(
                    "{}:{}",
                    skill_id,
                    remote_oid.map(|oid| oid.to_string()).unwrap_or_default()
                ),
                skill_id: skill_id.clone(),
                skill_name: name,
                base_commit: base_commit.map(|oid| oid.to_string()),
                local_commit: "local".to_string(),
                remote_commit: remote_oid
                    .map(|oid| oid.to_string())
                    .unwrap_or_else(|| "empty".to_string()),
                files: files.clone(),
                created_at: now_ms(),
                status: "pending".to_string(),
            })?;
        }
        Ok(())
    }

    fn apply_repository_to_library(
        &self,
        manifest: &SyncManifest,
        repo_root: &Path,
        conflicts: &BTreeSet<String>,
    ) -> Result<()> {
        use crate::core::content_hash::{hash_dir_for_sync_conflict, hash_dir_strict};
        use crate::core::sync_engine::PreparedDirReplacement;
        fs::create_dir_all(&self.central_root)?;
        let mut replacements = Vec::<PreparedDirReplacement>::new();
        let mut records = Vec::new();
        let mut distribution = Vec::new();
        let mut reserved: BTreeSet<PathBuf> = self
            .store
            .list_skills()?
            .into_iter()
            .map(|skill| PathBuf::from(skill.central_path))
            .collect();
        for (id, skill) in &manifest.skills {
            if conflicts.contains(id) {
                continue;
            }
            let source = skill_dir(repo_root, id);
            if !source.is_dir() {
                continue;
            }
            let existing = self.store.get_skill_by_id(id)?;
            let destination = match &existing {
                Some(record) => PathBuf::from(&record.central_path),
                None => unique_skill_path_reserved(&self.central_root, &skill.name, id, &reserved)?,
            };
            reserved.insert(destination.clone());
            let (staging, expected) = manifest::prepare_library_directory(&source, &destination)?;
            let new_hash = hash_dir_strict(staging.path())?;
            let previous_conflict_hash = if destination.exists() {
                Some(hash_dir_for_sync_conflict(&destination)?)
            } else {
                None
            };
            if existing.as_ref().is_some_and(|record| record.enabled) {
                distribution.push((id.clone(), destination.clone(), previous_conflict_hash));
            }
            if expected.as_ref() != Some(&new_hash) {
                replacements.push(PreparedDirReplacement::from_staging(
                    staging.keep(),
                    destination.clone(),
                    expected,
                    true,
                )?);
            }
            let mut record = manifest::record_from_portable(skill, &destination, now_ms());
            if let Some(existing) = existing {
                if existing
                    .source_ref
                    .as_deref()
                    .is_some_and(|value| !value.trim().is_empty())
                    && (existing.source_type != "local"
                        || !self
                            .store
                            .was_skill_imported_by_device_sync(id, existing.created_at)?)
                {
                    record.source_type = existing.source_type.clone();
                    record.source_ref = existing.source_ref.clone();
                    record.source_subpath = existing.source_subpath.clone();
                    record.source_revision = existing.source_revision.clone();
                }
                record.created_at = existing.created_at;
                record.enabled = existing.enabled;
            }
            records.push((record, skill.tags.clone()));
        }
        for index in 0..replacements.len() {
            if let Err(error) = replacements[index].activate() {
                for replacement in replacements[..=index].iter_mut().rev() {
                    replacement.rollback()?;
                }
                return Err(error);
            }
        }
        for replacement in &replacements {
            replacement.verify_backup_unchanged()?;
        }
        let sources = manifest
            .skills
            .iter()
            .filter(|(id, _)| records.iter().any(|(record, _)| &record.id == *id))
            .map(|(id, skill)| (id.clone(), manifest::SharedSource::from_skill(skill)))
            .collect();
        self.store
            .commit_device_sync_library_with_sources(&records, &[], &sources)?;
        for replacement in &mut replacements {
            replacement.commit();
        }
        for (id, source, previous) in distribution {
            let targets = self.store.list_skill_targets(&id)?;
            let paths: BTreeSet<_> = targets
                .iter()
                .filter(|target| target.mode == "copy" && target.status != "disabled")
                .map(|target| PathBuf::from(&target.target_path))
                .collect();
            for path in paths {
                // Failures remain attached to the tool record, not the cloud merge.
                let _ = crate::core::tool_distribution::refresh_copy(
                    self.store,
                    &id,
                    &source,
                    &path,
                    previous.as_deref(),
                );
            }
        }
        Ok(())
    }

    fn apply_remote_deletions(&self, plan: &MergePlan) -> Result<()> {
        let trash_root = self.workspace_root.join("trash");
        fs::create_dir_all(&trash_root)?;
        for id in &plan.delete_local {
            let Some(record) = self.store.get_skill_by_id(id)? else {
                continue;
            };
            let source = PathBuf::from(&record.central_path);
            let trash_id = Uuid::new_v4().to_string();
            let destination = trash_root.join(&trash_id);
            if source.exists() {
                fs::rename(&source, &destination)
                    .with_context(|| format!("move deleted skill {:?} to trash", source))?;
            }
            let deleted_at = now_ms();
            self.store.add_device_sync_trash(&TrashEntry {
                id: trash_id,
                skill_id: id.clone(),
                skill_name: record.name,
                trash_path: destination.to_string_lossy().to_string(),
                deleted_at,
                expires_at: deleted_at + 30 * 24 * 60 * 60 * 1000,
            })?;
            self.store.delete_skill(id)?;
        }
        Ok(())
    }
}

fn read_failure_context(config: &DeviceSyncConfig) -> &'static str {
    if config.uses_https() && config.visibility == types::RepositoryVisibility::Public {
        "DEVICE_SYNC_PUBLIC_READ_FAILED"
    } else {
        "read device sync repository"
    }
}

fn local_device_name() -> String {
    let from_environment = ["COMPUTERNAME", "HOSTNAME"]
        .iter()
        .find_map(|key| std::env::var(key).ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    sanitize_device_name(from_environment.unwrap_or_else(|| "Skills Hub Device".to_string()))
}

fn sanitize_device_name(value: String) -> String {
    value
        .lines()
        .next()
        .unwrap_or("Skills Hub Device")
        .chars()
        .take(80)
        .collect()
}

fn merge_snapshot_file(
    repo: &git2::Repository,
    base_commit: Option<git2::Oid>,
    remote_commit: Option<git2::Oid>,
    local_root: &Path,
    manifests: [&SyncManifest; 3],
    id: &str,
    path: &str,
) -> Result<Option<Vec<u8>>> {
    let safe = |value: &str| {
        !value.is_empty()
            && !value.contains(['\\', ':'])
            && Path::new(value)
                .components()
                .all(|part| matches!(part, std::path::Component::Normal(_)))
    };
    anyhow::ensure!(
        safe(id) && !id.contains('/') && safe(path),
        "unsafe text merge path"
    );
    let (Some(base_commit), Some(remote_commit)) = (base_commit, remote_commit) else {
        return Ok(None);
    };
    let relative = Path::new("skills").join(id).join("content").join(path);
    let read_blob = |commit| -> Result<Option<Vec<u8>>> {
        let tree = repo.find_commit(commit)?.tree()?;
        let entry = tree
            .get_path(&relative)
            .context("text merge snapshot file missing")?;
        anyhow::ensure!(
            matches!(entry.filemode(), 0o100644 | 0o100755),
            "unsafe text merge file mode"
        );
        let blob = repo.find_blob(entry.id())?;
        Ok((blob.size() <= text_merge::MAX_TEXT_BYTES).then(|| blob.content().to_vec()))
    };
    let Some(base) = read_blob(base_commit)? else {
        return Ok(None);
    };
    let Some(remote) = read_blob(remote_commit)? else {
        return Ok(None);
    };
    let local_path = local_root.join(&relative);
    let metadata =
        fs::symlink_metadata(&local_path).context("local merge snapshot file missing")?;
    anyhow::ensure!(
        metadata.is_file()
            && local_path
                .canonicalize()?
                .starts_with(local_root.canonicalize()?),
        "unsafe local text merge snapshot"
    );
    if metadata.len() > text_merge::MAX_TEXT_BYTES as u64 {
        return Ok(None);
    }
    let local = fs::read(local_path)?;
    let mut versions = [base, local, remote];
    for (index, (manifest, bytes)) in manifests.into_iter().zip(versions.iter_mut()).enumerate() {
        let expected = manifest
            .skills
            .get(id)
            .and_then(|skill| skill.files.get(path))
            .context("text merge manifest file missing")?;
        *bytes = text_merge::verify_snapshot(std::mem::take(bytes), expected, index != 1)?;
    }
    text_merge::merge_text(&versions[0], &versions[1], &versions[2])
}

fn apply_plan_to_repository(
    plan: &MergePlan,
    local: &SyncManifest,
    local_root: &Path,
    remote: &mut SyncManifest,
    remote_root: &Path,
) -> Result<()> {
    for id in &plan.take_local {
        copy_local_skill(id, local, local_root, remote, remote_root)?;
    }
    for id in &plan.delete_remote {
        remote.skills.remove(id);
        let directory = remote_root.join("skills").join(id);
        if directory.exists() {
            fs::remove_dir_all(directory)?;
        }
    }
    for (id, paths) in &plan.merge_files {
        let local_skill = local.skills.get(id).context("local merge skill missing")?;
        let content_root = skill_dir(remote_root, id);
        for path in paths {
            let source = skill_dir(local_root, id).join(path);
            let destination = content_root.join(path);
            if source.exists() {
                if let Some(parent) = destination.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::copy(source, destination)?;
            } else if destination.exists() {
                fs::remove_file(destination)?;
            }
        }
        if let Some(files) = plan.merged_text.get(id) {
            for (path, bytes) in files {
                fs::write(content_root.join(path), bytes)?;
            }
            manifest::reject_private_keys(&content_root)?;
        }
        let mut merged = if plan.take_local_metadata.contains(id) {
            local_skill.clone()
        } else {
            remote
                .skills
                .get(id)
                .context("remote merge skill missing")?
                .clone()
        };
        merged.files = manifest::hash_files(&content_root)?;
        merged.content_hash = portable_hash(&merged);
        remote.skills.insert(id.clone(), merged);
    }
    Ok(())
}

fn copy_local_skill(
    id: &str,
    local: &SyncManifest,
    local_root: &Path,
    remote: &mut SyncManifest,
    remote_root: &Path,
) -> Result<()> {
    let skill = local.skills.get(id).context("local sync skill missing")?;
    let destination = skill_dir(remote_root, id);
    manifest::replace_directory(&skill_dir(local_root, id), &destination)?;
    remote.skills.insert(id.to_string(), skill.clone());
    Ok(())
}

fn summarize(plan: &MergePlan, local: &SyncManifest, remote: &SyncManifest) -> SyncChangeSummary {
    let mut summary = SyncChangeSummary::default();
    let mut add = |id: &String, kind: &str, direction: &str| {
        match kind {
            "added" => summary.added += 1,
            "updated" => summary.updated += 1,
            "deleted" => summary.deleted += 1,
            "conflicted" => summary.conflicted += 1,
            _ => unreachable!(),
        }
        summary.items.push(SyncChangeItem {
            skill_id: id.clone(),
            name: local
                .skills
                .get(id)
                .or_else(|| remote.skills.get(id))
                .map(|s| s.name.clone())
                .unwrap_or_else(|| id.clone()),
            kind: kind.into(),
            direction: direction.into(),
        });
    };
    for (ids, direction) in [
        (&plan.take_local, "upload"),
        (&plan.take_remote, "download"),
    ] {
        for id in ids {
            add(
                id,
                if local.skills.contains_key(id) && remote.skills.contains_key(id) {
                    "updated"
                } else {
                    "added"
                },
                direction,
            );
        }
    }
    for id in plan.merge_files.keys() {
        add(id, "updated", "merge");
    }
    for id in &plan.delete_local {
        add(id, "deleted", "download");
    }
    for id in &plan.delete_remote {
        add(id, "deleted", "upload");
    }
    for id in plan.conflicts.keys() {
        add(id, "conflicted", "merge");
    }
    summary
}

fn unique_skill_path(root: &Path, name: &str, id: &str) -> Result<PathBuf> {
    unique_skill_path_reserved(root, name, id, &BTreeSet::new())
}

fn unique_skill_path_reserved(
    root: &Path,
    name: &str,
    id: &str,
    reserved: &BTreeSet<PathBuf>,
) -> Result<PathBuf> {
    let available = |path: &Path| -> Result<bool> {
        match path.symlink_metadata() {
            Ok(_) => Ok(false),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Ok(!reserved.contains(path))
            }
            Err(error) => Err(error).with_context(|| format!("inspect Skill path {:?}", path)),
        }
    };
    let candidate = root.join(name);
    if available(&candidate)? {
        return Ok(candidate);
    }
    let short = id.chars().take(8).collect::<String>();
    let stem = format!("{}-{}", name, short);
    let mut candidate = root.join(&stem);
    let mut suffix = 2;
    while !available(&candidate)? {
        candidate = root.join(format!("{stem}-{suffix}"));
        suffix += 1;
    }
    Ok(candidate)
}

fn reconcile_identities(
    local: &mut SyncManifest,
    remote: &SyncManifest,
    local_root: &Path,
) -> Result<Vec<(String, String)>> {
    let mut mappings = Vec::new();
    let mut claimed = BTreeSet::new();
    for (remote_id, remote_skill) in &remote.skills {
        if local.skills.contains_key(remote_id) {
            continue;
        }
        let candidates = local
            .skills
            .iter()
            .filter(|(local_id, local_skill)| {
                !remote.skills.contains_key(*local_id)
                    && !claimed.contains(*local_id)
                    && same_portable_identity(local_skill, remote_skill)
            })
            .map(|(id, _)| id.clone())
            .collect::<Vec<_>>();
        if candidates.len() != 1 {
            continue;
        }
        let old_id = candidates[0].clone();
        let mut skill = local
            .skills
            .remove(&old_id)
            .context("identity candidate disappeared")?;
        skill.id = remote_id.clone();
        let old_dir = local_root.join("skills").join(&old_id);
        let new_dir = local_root.join("skills").join(remote_id);
        if old_dir.exists() {
            fs::rename(old_dir, new_dir)?;
        }
        local.skills.insert(remote_id.clone(), skill);
        claimed.insert(old_id.clone());
        mappings.push((old_id, remote_id.clone()));
    }
    local.write(local_root)?;
    Ok(mappings)
}

fn same_portable_identity(
    local: &manifest::PortableSkill,
    remote: &manifest::PortableSkill,
) -> bool {
    let same_source = local.source_type == remote.source_type
        && local.source_ref.is_some()
        && local.source_ref == remote.source_ref
        && local.source_subpath == remote.source_subpath;
    same_source || manifest::aggregate_hash(&local.files) == manifest::aggregate_hash(&remote.files)
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

pub fn try_lock_device_sync() -> Result<std::sync::MutexGuard<'static, ()>> {
    SYNC_LOCK
        .get_or_init(|| Mutex::new(()))
        .try_lock()
        .map_err(|_| anyhow::anyhow!("device sync is already running"))
}

fn is_device_sync_running() -> bool {
    SYNC_LOCK.get_or_init(|| Mutex::new(())).try_lock().is_err()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::device_sync::credentials::{CredentialStore, MemoryCredentialStore};
    use crate::core::skill_store::{SkillRecord, SkillTargetRecord};
    use git2::Repository;

    static TEST_SYNC_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn repository_visibility_controls_read_credentials_but_not_write_credentials() {
        use types::RepositoryVisibility;
        struct DenyCredentials;
        impl CredentialStore for DenyCredentials {
            fn get(&self, _: &str) -> Result<Option<String>> {
                bail!("credential read attempted")
            }
            fn set(&self, _: &str, _: &str) -> Result<()> {
                panic!("unexpected write")
            }
            fn delete(&self, _: &str) -> Result<()> {
                panic!("unexpected delete")
            }
        }
        let root = tempfile::tempdir().unwrap();
        let store = SkillStore::new(root.path().join("store.db"));
        store.ensure_schema().unwrap();
        let service = DeviceSyncService::new(
            &store,
            &DenyCredentials,
            root.path().join("workspace"),
            root.path().join("central"),
        );
        let mut config = DeviceSyncConfig {
            remote_url: "https://github.com/example/sync.git".into(),
            credential_key: Some("test-key".into()),
            visibility: RepositoryVisibility::Public,
            ..Default::default()
        };
        assert_eq!(service.read_token(&config).unwrap(), None);
        assert!(format!("{:#}", service.token(&config).unwrap_err())
            .contains("credential read attempted"));
        config.visibility = RepositoryVisibility::Private;
        assert!(format!("{:#}", service.read_token(&config).unwrap_err())
            .contains("credential read attempted"));
        config.visibility = RepositoryVisibility::Unknown;
        assert!(service
            .read_token(&config)
            .unwrap_err()
            .to_string()
            .contains("DEVICE_SYNC_VISIBILITY_UNKNOWN"));
        config.remote_url = "git@github.com:example/sync.git".into();
        assert_eq!(service.read_token(&config).unwrap(), None);
        assert_eq!(service.token(&config).unwrap(), None);
    }

    #[test]
    fn unknown_visibility_and_unconfirmed_public_upload_stop_before_credentials_or_network() {
        let _guard = TEST_SYNC_LOCK.lock().unwrap();
        let root = tempfile::tempdir().unwrap();
        let mut config = DeviceSyncConfig {
            remote_url: "https://example.invalid/sync.git".into(),
            credential_key: Some("missing-token".into()),
            auto_sync: true,
            auto_sync_schedule: Some(scheduler::SyncSchedule::Interval { minutes: 5 }),
            ..Default::default()
        };
        let store = make_store(root.path(), "unknown", &config);
        let credentials = MemoryCredentialStore::default();
        let workspace = root.path().join("workspace");
        let service = DeviceSyncService::new(
            &store,
            &credentials,
            workspace.clone(),
            root.path().join("central"),
        );
        assert!(service
            .check()
            .unwrap_err()
            .to_string()
            .contains("DEVICE_SYNC_VISIBILITY_UNKNOWN"));
        assert!(service.sync_scheduled(&config).unwrap().is_none());
        assert!(store.list_device_sync_history(10).unwrap().is_empty());
        config.visibility = types::RepositoryVisibility::Public;
        store.save_device_sync_config(&config).unwrap();
        assert!(service.sync_scheduled(&config).unwrap().is_none());
        assert!(service
            .sync()
            .unwrap_err()
            .to_string()
            .contains("DEVICE_SYNC_PUBLIC_UPLOAD_CONFIRMATION"));
        assert!(!workspace.exists());
    }

    fn seed_remote(root: &Path) -> (PathBuf, DeviceSyncConfig) {
        let bare = root.join("remote.git");
        Repository::init_bare(&bare).unwrap();
        let seed_path = root.join("seed");
        let seed = Repository::init(&seed_path).unwrap();
        SyncManifest::empty().write(&seed_path).unwrap();
        let first = git_repo::commit_all(&seed, "Initialize", None)
            .unwrap()
            .unwrap();
        seed.remote("origin", bare.to_str().unwrap()).unwrap();
        let config = DeviceSyncConfig {
            remote_url: bare.to_string_lossy().to_string(),
            ..DeviceSyncConfig::default()
        };
        git_repo::push(&seed, &config, None, first).unwrap();
        (bare, config)
    }

    #[test]
    fn sync_preserves_local_only_files_and_publishes_distribution_documents() {
        let _guard = TEST_SYNC_LOCK
            .lock()
            .unwrap_or_else(|lock| lock.into_inner());
        let root = tempfile::tempdir().unwrap();
        let (bare, config) = seed_remote(root.path());
        let store = make_store(root.path(), "preserve", &config);
        let central = root.path().join("central");
        add_skill_in_directory(&store, &central, "one", "one", "# One");
        for (path, content) in [
            ("dist/guide.md", "official guide"),
            ("build/manual.md", "manual"),
            (".env", "LOCAL_ONLY=value"),
            (".env.production", "LOCAL_ONLY_PROD=value"),
            ("scripts/__pycache__/test.pyc", "cache"),
        ] {
            let path = central.join("one").join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, content).unwrap();
        }
        let credentials = MemoryCredentialStore::default();
        let service = DeviceSyncService::new(
            &store,
            &credentials,
            root.path().join("workspace"),
            central.clone(),
        );
        service.sync().unwrap();
        service.sync().unwrap();
        assert_eq!(
            fs::read_to_string(central.join("one/.env")).unwrap(),
            "LOCAL_ONLY=value"
        );
        assert_eq!(
            fs::read_to_string(central.join("one/dist/guide.md")).unwrap(),
            "official guide"
        );
        assert!(central.join("one/scripts/__pycache__/test.pyc").exists());
        let repo = Repository::open_bare(bare).unwrap();
        let tree = repo
            .find_reference("refs/heads/main")
            .unwrap()
            .peel_to_tree()
            .unwrap();
        assert!(tree
            .get_path(Path::new("skills/one/content/dist/guide.md"))
            .is_ok());
        assert!(tree
            .get_path(Path::new("skills/one/content/build/manual.md"))
            .is_ok());
        assert!(tree
            .get_path(Path::new("skills/one/content/.env.production"))
            .is_err());
    }

    #[test]
    fn tool_edits_do_not_fail_cloud_sync_and_retry_clears_tool_issue() {
        let _guard = TEST_SYNC_LOCK
            .lock()
            .unwrap_or_else(|lock| lock.into_inner());
        let root = tempfile::tempdir().unwrap();
        let (bare, config) = seed_remote(root.path());
        let credentials = MemoryCredentialStore::default();
        let central = root.path().join("central");
        let store = make_store(root.path(), "tool-boundary", &config);
        add_skill_in_directory(&store, &central, "one", "one", "# Central");
        let target = root.path().join("tool/one");
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("SKILL.md"), "# Tool edit").unwrap();
        store
            .upsert_skill_target(&SkillTargetRecord {
                id: "copy".into(),
                skill_id: "one".into(),
                tool: "cursor".into(),
                scope: "global".into(),
                project_path: None,
                target_path: target.to_string_lossy().into(),
                mode: "copy".into(),
                status: "ok".into(),
                last_error: None,
                synced_at: None,
            })
            .unwrap();
        let service = DeviceSyncService::new(
            &store,
            &credentials,
            root.path().join("workspace"),
            central.clone(),
        );
        let healthy_path = root.path().join("other-tool/one");
        let mut healthy = store.list_skill_targets("one").unwrap().remove(0);
        healthy.id = "healthy".into();
        healthy.tool = "codex".into();
        healthy.target_path = healthy_path.to_string_lossy().into();
        store.upsert_skill_target(&healthy).unwrap();
        assert_eq!(service.sync().unwrap().status, "success");
        assert_eq!(
            fs::read_to_string(healthy_path.join("SKILL.md")).unwrap(),
            "# Central"
        );
        assert_eq!(
            service.status().unwrap().last_run_status.as_deref(),
            Some("success")
        );
        assert_eq!(service.status().unwrap().tool_issues.len(), 1);
        let repo = Repository::open_bare(bare).unwrap();
        let head = repo
            .find_reference("refs/heads/main")
            .unwrap()
            .target()
            .unwrap();
        let tree = repo.find_commit(head).unwrap().tree().unwrap();
        let entry = tree
            .get_path(Path::new("skills/one/content/SKILL.md"))
            .unwrap();
        assert_eq!(repo.find_blob(entry.id()).unwrap().content(), b"# Central");
        assert_eq!(
            fs::read_to_string(target.join("SKILL.md")).unwrap(),
            "# Tool edit"
        );
        assert_eq!(service.sync().unwrap().status, "success");
        let after = repo
            .find_reference("refs/heads/main")
            .unwrap()
            .peel_to_commit()
            .unwrap();
        assert_eq!(
            after
                .tree()
                .unwrap()
                .get_path(Path::new("skills"))
                .unwrap()
                .id(),
            tree.get_path(Path::new("skills")).unwrap().id()
        );
        fs::write(target.join("SKILL.md"), "# Central").unwrap();
        crate::core::tool_distribution::refresh_copy(
            &store,
            "one",
            &central.join("one"),
            &target,
            None,
        )
        .unwrap();
        assert!(service.status().unwrap().tool_issues.is_empty());
        assert_eq!(store.list_skill_targets("one").unwrap()[0].status, "ok");
    }

    #[test]
    fn remote_updates_refresh_copies_without_overwriting_edited_targets() {
        let _guard = TEST_SYNC_LOCK
            .lock()
            .unwrap_or_else(|lock| lock.into_inner());
        let root = tempfile::tempdir().unwrap();
        let (_, config) = seed_remote(root.path());
        let credentials = MemoryCredentialStore::default();
        let central = root.path().join("central");
        let store = make_store(root.path(), "copies", &config);
        add_skill_in_directory(&store, &central, "one", "one", "# Original");
        let service = DeviceSyncService::new(
            &store,
            &credentials,
            root.path().join("workspace"),
            central.clone(),
        );
        let target = root.path().join("tool/one");
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("SKILL.md"), "# Original").unwrap();
        #[cfg(unix)]
        for directory in [&target, &central.join("one")] {
            fs::create_dir_all(directory.join("node_modules/.bin")).unwrap();
            std::os::unix::fs::symlink("../tool", directory.join("node_modules/.bin/tool"))
                .unwrap();
        }
        store
            .upsert_skill_target(&SkillTargetRecord {
                id: "copy".into(),
                skill_id: "one".into(),
                tool: "cursor".into(),
                scope: "project".into(),
                project_path: Some(root.path().to_string_lossy().into()),
                target_path: target.to_string_lossy().into(),
                mode: "copy".into(),
                status: "ok".into(),
                last_error: None,
                synced_at: None,
            })
            .unwrap();
        let incoming = root.path().join("incoming");
        let mut shared = store.list_skill_targets("one").unwrap().remove(0);
        shared.id = "shared-copy".into();
        shared.tool = "codex".into();
        store.upsert_skill_target(&shared).unwrap();
        store
            .set_setting(
                "device_sync.target_baseline.copy",
                &serde_json::to_string(&(target.to_string_lossy(), "stale-before-source-update"))
                    .unwrap(),
            )
            .unwrap();
        manifest::export_library(&store, &incoming).unwrap();
        fs::write(skill_dir(&incoming, "one").join("SKILL.md"), "# Remote").unwrap();
        let remote = SyncManifest::read(&incoming).unwrap();
        service
            .apply_repository_to_library(&remote, &incoming, &BTreeSet::new())
            .unwrap();
        assert_eq!(
            fs::read_to_string(target.join("SKILL.md")).unwrap(),
            "# Remote"
        );
        for record in store.list_skill_targets("one").unwrap() {
            assert!(record.synced_at.is_some());
            assert!(store
                .get_setting(&format!("device_sync.target_baseline.{}", record.id))
                .unwrap()
                .is_some());
        }
        #[cfg(unix)]
        assert_eq!(
            fs::read_link(target.join("node_modules/.bin/tool")).unwrap(),
            PathBuf::from("../tool")
        );
        fs::write(target.join("SKILL.md"), "# User edit").unwrap();
        fs::write(skill_dir(&incoming, "one").join("SKILL.md"), "# Next").unwrap();
        service
            .apply_repository_to_library(&remote, &incoming, &BTreeSet::new())
            .unwrap();
        assert!(store
            .list_skill_targets("one")
            .unwrap()
            .iter()
            .all(|t| t.status == "error"));
        assert_eq!(
            fs::read_to_string(target.join("SKILL.md")).unwrap(),
            "# User edit"
        );
        assert_eq!(
            fs::read_to_string(central.join("one/SKILL.md")).unwrap(),
            "# Next"
        );
        add_skill_in_directory(&store, &central, "other", "other", "# Other");
        let mut foreign = store.list_skill_targets("one").unwrap().remove(0);
        foreign.id = "foreign".into();
        foreign.skill_id = "other".into();
        store.upsert_skill_target(&foreign).unwrap();
        service
            .apply_repository_to_library(&remote, &incoming, &BTreeSet::new())
            .unwrap();
        assert!(store
            .list_skill_targets("one")
            .unwrap()
            .iter()
            .all(|t| t.status == "error"));
        assert_eq!(
            fs::read_to_string(target.join("SKILL.md")).unwrap(),
            "# User edit"
        );
        assert_eq!(
            fs::read_to_string(central.join("one/SKILL.md")).unwrap(),
            "# Next"
        );
    }

    #[test]
    fn accepting_remote_deletion_keeps_recoverable_local_content() {
        let _guard = TEST_SYNC_LOCK
            .lock()
            .unwrap_or_else(|lock| lock.into_inner());
        let root = tempfile::tempdir().unwrap();
        let (_, config) = seed_remote(root.path());
        let credentials = MemoryCredentialStore::default();
        let store = make_store(root.path(), "deletion", &config);
        let central = root.path().join("central");
        add_skill_in_directory(&store, &central, "one", "one", "# Local edit");
        let workspace = root.path().join("workspace");
        let repo =
            git2::Repository::clone(&config.remote_url, workspace.join("repository")).unwrap();
        let remote_commit = repo
            .find_reference("refs/remotes/origin/main")
            .unwrap()
            .target()
            .unwrap()
            .to_string();
        let conflict = SyncConflict {
            id: "delete-conflict".into(),
            skill_id: "one".into(),
            skill_name: "one".into(),
            base_commit: None,
            local_commit: String::new(),
            remote_commit,
            files: vec!["*".into()],
            created_at: 1,
            status: "pending".into(),
        };
        store.upsert_device_sync_conflict(&conflict).unwrap();
        let service = DeviceSyncService::new(&store, &credentials, workspace, central.clone());
        service
            .resolve_conflict(&conflict.id, ConflictResolution::UseRemote)
            .unwrap();
        assert!(store.get_skill_by_id("one").unwrap().is_none());
        let trash = store.list_device_sync_trash().unwrap();
        assert_eq!(
            fs::read_to_string(Path::new(&trash[0].trash_path).join("SKILL.md")).unwrap(),
            "# Local edit"
        );
        assert!(store.list_device_sync_conflicts().unwrap().is_empty());
    }

    #[test]
    fn trash_restore_preserves_description_and_tags_after_restart() {
        let _guard = TEST_SYNC_LOCK
            .lock()
            .unwrap_or_else(|lock| lock.into_inner());
        let root = tempfile::tempdir().unwrap();
        let (_, config) = seed_remote(root.path());
        let store = make_store(root.path(), "restore-meta", &config);
        let central = root.path().join("central");
        add_skill_in_directory(
            &store,
            &central,
            "one",
            "one",
            "---\nname: one\ndescription: file description\n---\n# One",
        );
        let mut original = store.get_skill_by_id("one").unwrap().unwrap();
        original.description = Some("Custom saved description".into());
        store.upsert_skill(&original).unwrap();
        store
            .set_skill_tag_names("one", &["写作".into(), "Research".into()])
            .unwrap();
        let credentials = MemoryCredentialStore::default();
        let workspace = root.path().join("workspace");
        let service =
            DeviceSyncService::new(&store, &credentials, workspace.clone(), central.clone());
        service
            .apply_remote_deletions(&MergePlan {
                delete_local: BTreeSet::from(["one".into()]),
                ..MergePlan::default()
            })
            .unwrap();
        let trash = store.list_device_sync_trash().unwrap().remove(0);
        let metadata_key = format!("device_sync.trash_metadata.{}", trash.id);
        let saved = store.get_setting(&metadata_key).unwrap().unwrap();
        store
            .set_setting(&metadata_key, r#"{"description":"Saved","tags":[""]}"#)
            .unwrap();
        assert!(service.restore_trash(&trash.id).is_err());
        assert!(store.get_skill_by_id("one").unwrap().is_none());
        assert!(Path::new(&trash.trash_path).join("SKILL.md").is_file());
        assert_eq!(store.list_device_sync_trash().unwrap().len(), 1);
        assert_eq!(fs::read_dir(&central).unwrap().count(), 0);
        store.set_setting(&metadata_key, &saved).unwrap();
        for tag in store.list_tags_with_counts().unwrap() {
            store.delete_tag(tag.id).unwrap();
        }
        let reopened = SkillStore::new(store.db_path().to_path_buf());
        reopened.ensure_schema().unwrap();
        DeviceSyncService::new(&reopened, &credentials, workspace, central)
            .restore_trash(&trash.id)
            .unwrap();
        let restored = reopened.get_skill_by_id("one").unwrap().unwrap();
        assert_eq!(restored.description, original.description);
        assert_eq!(
            reopened
                .get_skill_tags("one")
                .unwrap()
                .into_iter()
                .map(|t| t.name)
                .collect::<BTreeSet<_>>(),
            BTreeSet::from(["写作".into(), "Research".into()])
        );
        assert!(Path::new(&restored.central_path).join("SKILL.md").is_file());
        assert!(reopened.list_device_sync_trash().unwrap().is_empty());
    }

    #[test]
    fn legacy_trash_restore_reads_description_from_skill_md() {
        let _guard = TEST_SYNC_LOCK
            .lock()
            .unwrap_or_else(|lock| lock.into_inner());
        let root = tempfile::tempdir().unwrap();
        let (_, config) = seed_remote(root.path());
        let store = make_store(root.path(), "legacy-restore", &config);
        let source = root.path().join("old-trash");
        fs::create_dir(&source).unwrap();
        fs::write(
            source.join("SKILL.md"),
            "---\nname: one\ndescription: Legacy description\n---\n# One",
        )
        .unwrap();
        store
            .add_device_sync_trash(&TrashEntry {
                id: "old".into(),
                skill_id: "one".into(),
                skill_name: "one".into(),
                trash_path: source.to_string_lossy().into(),
                deleted_at: 1,
                expires_at: i64::MAX,
            })
            .unwrap();
        let credentials = MemoryCredentialStore::default();
        let service = DeviceSyncService::new(
            &store,
            &credentials,
            root.path().join("workspace"),
            root.path().join("central"),
        );
        let central = root.path().join("central");
        fs::create_dir_all(central.join("one")).unwrap();
        fs::create_dir_all(central.join("one-one/SKILL.md")).unwrap();
        fs::write(central.join("one-one/keep.txt"), "existing skill").unwrap();
        service.restore_trash("old").unwrap();
        assert_eq!(
            fs::read_to_string(central.join("one-one/keep.txt")).unwrap(),
            "existing skill"
        );
        assert_eq!(
            store
                .get_skill_by_id("one")
                .unwrap()
                .unwrap()
                .description
                .as_deref(),
            Some("Legacy description")
        );
        assert!(store.get_skill_tags("one").unwrap().is_empty());
    }

    #[test]
    fn library_metadata_failure_rolls_back_all_files_and_records() {
        let root = tempfile::tempdir().unwrap();
        let (_, config) = seed_remote(root.path());
        let store = make_store(root.path(), "metadata", &config);
        let central = root.path().join("central");
        for id in ["one", "two"] {
            add_skill_in_directory(&store, &central, id, id, "original");
        }
        let before = store.get_skill_by_id("one").unwrap().unwrap();
        let incoming = root.path().join("incoming");
        let mut remote = manifest::export_library(&store, &incoming).unwrap();
        remote.skills.get_mut("one").unwrap().description = Some("new metadata".into());
        remote.skills.get_mut("two").unwrap().tags = vec![String::new()];
        fs::write(skill_dir(&incoming, "one").join("SKILL.md"), "new content").unwrap();
        let credentials = MemoryCredentialStore::default();
        let service = DeviceSyncService::new(
            &store,
            &credentials,
            root.path().join("workspace"),
            central.clone(),
        );
        assert!(service
            .apply_repository_to_library(&remote, &incoming, &BTreeSet::new())
            .is_err());
        assert_eq!(
            store.get_skill_by_id("one").unwrap().unwrap().description,
            before.description
        );
        assert_eq!(
            fs::read_to_string(central.join("one/SKILL.md")).unwrap(),
            "original"
        );
    }

    fn make_store(root: &Path, name: &str, config: &DeviceSyncConfig) -> SkillStore {
        let store = SkillStore::new(root.join(format!("{name}.db")));
        store.ensure_schema().unwrap();
        store.save_device_sync_config(config).unwrap();
        store
    }

    fn add_skill(store: &SkillStore, central: &Path, id: &str, content: &str) {
        add_skill_in_directory(store, central, id, "one", content);
    }

    #[test]
    fn shared_device_registry_publishes_all_devices_and_cache_reads_need_no_repository() {
        let _guard = TEST_SYNC_LOCK
            .lock()
            .unwrap_or_else(|lock| lock.into_inner());
        let root = tempfile::tempdir().unwrap();
        let (bare, config) = seed_remote(root.path());
        let credentials = MemoryCredentialStore::default();
        let a = make_store(root.path(), "registry-a", &config);
        let b = make_store(root.path(), "registry-b", &config);
        a.set_setting("device_sync.local_device_id", "office")
            .unwrap();
        b.set_setting("device_sync.local_device_id", "home")
            .unwrap();
        let service_a = DeviceSyncService::new(
            &a,
            &credentials,
            root.path().join("wa"),
            root.path().join("ca"),
        );
        let service_b = DeviceSyncService::new(
            &b,
            &credentials,
            root.path().join("wb"),
            root.path().join("cb"),
        );
        service_a.sync().unwrap();
        service_b.sync().unwrap();
        service_a.sync().unwrap();
        let remote = Repository::open_bare(bare).unwrap();
        let commit = remote
            .find_reference("refs/heads/main")
            .unwrap()
            .peel_to_commit()
            .unwrap();
        let tree = commit.tree().unwrap();
        let entry = tree
            .get_path(Path::new("devices.json"))
            .expect("shared registry must be published");
        let blob = remote.find_blob(entry.id()).unwrap();
        let registry: serde_json::Value = serde_json::from_slice(blob.content()).unwrap();
        assert_eq!(registry["version"], 1);
        assert_eq!(registry["devices"].as_object().unwrap().len(), 2);
        assert!(
            registry["devices"]["office"]["lastSyncedAt"]
                .as_i64()
                .unwrap()
                > 0
        );
        assert!(registry["devices"]["home"]["name"].is_string());
        assert_eq!(registry["devices"]["office"].as_object().unwrap().len(), 2);
        assert_eq!(service_b.devices().unwrap().len(), 2);
        service_b.check().unwrap();
        fs::write(
            root.path().join("wb/repository/.git/HEAD"),
            "invalid git head",
        )
        .unwrap();
        assert_eq!(service_b.devices().unwrap().len(), 2);
    }

    #[test]
    fn device_registry_migrates_legacy_devices_and_reads_later_legacy_commits() {
        let _guard = TEST_SYNC_LOCK
            .lock()
            .unwrap_or_else(|lock| lock.into_inner());
        let root = tempfile::tempdir().unwrap();
        let (_, config) = seed_remote(root.path());
        let seed = Repository::open(root.path().join("seed")).unwrap();
        let old = git_repo::commit_all_allow_empty(
            &seed,
            "Legacy sync\n\nSkills-Hub-Device-ID: legacy\nSkills-Hub-Device-Name: Old Mac",
            seed.head().unwrap().target(),
        )
        .unwrap()
        .unwrap();
        git_repo::push(&seed, &config, None, old).unwrap();
        let credentials = MemoryCredentialStore::default();
        let store = make_store(root.path(), "new-client", &config);
        store
            .set_setting("device_sync.local_device_id", "new")
            .unwrap();
        let service = DeviceSyncService::new(
            &store,
            &credentials,
            root.path().join("workspace"),
            root.path().join("central"),
        );
        service.check().unwrap();
        assert_eq!(service.devices().unwrap()[0].name, "Old Mac");
        service.sync().unwrap();
        let parent = git_repo::fetch_and_checkout(&seed, &config, None).unwrap();
        let later = git_repo::commit_all_allow_empty(
            &seed,
            "Legacy sync\n\nSkills-Hub-Device-ID: later\nSkills-Hub-Device-Name: Old PC",
            parent,
        )
        .unwrap()
        .unwrap();
        git_repo::push(&seed, &config, None, later).unwrap();
        service.check().unwrap();
        assert_eq!(service.devices().unwrap().len(), 3);
        service.sync().unwrap();
        let registry: serde_json::Value = serde_json::from_slice(
            &fs::read(root.path().join("workspace/repository/devices.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(registry["devices"]["legacy"]["name"], "Old Mac");
        assert_eq!(registry["devices"]["later"]["name"], "Old PC");
        // Previous clients continue to discover new clients from commit trailers.
        let repo = Repository::open(root.path().join("workspace/repository")).unwrap();
        assert_eq!(git_repo::discover_devices(&repo).unwrap().len(), 3);
    }

    #[test]
    fn device_registry_stale_push_is_rejected_and_retry_preserves_both_writers() {
        let _guard = TEST_SYNC_LOCK
            .lock()
            .unwrap_or_else(|lock| lock.into_inner());
        let root = tempfile::tempdir().unwrap();
        let (_, config) = seed_remote(root.path());
        let credentials = MemoryCredentialStore::default();
        let store = make_store(root.path(), "current", &config);
        store
            .set_setting("device_sync.local_device_id", "current")
            .unwrap();
        let workspace = root.path().join("workspace");
        let service = DeviceSyncService::new(
            &store,
            &credentials,
            workspace.clone(),
            root.path().join("central"),
        );
        service.check().unwrap();
        let stale = Repository::open(workspace.join("repository")).unwrap();
        let parent = git_repo::remote_head(&stale, &config);
        let seed_path = root.path().join("seed");
        let other = Repository::open(&seed_path).unwrap();
        let mut registry = device_registry::DeviceRegistry::read_at(&other, parent).unwrap();
        registry.record(&DeviceSyncDevice {
            id: "other".into(),
            name: "Other computer".into(),
            alias: None,
            last_commit: None,
            last_seen_at: 1234,
            is_current: false,
        });
        registry.write(&seed_path).unwrap();
        let other_head = git_repo::commit_all(&other, "Other writer", parent)
            .unwrap()
            .unwrap();
        git_repo::push(&other, &config, None, other_head).unwrap();
        let mut registry = device_registry::DeviceRegistry::read_at(&stale, parent).unwrap();
        registry.record(&service.local_device_identity().unwrap());
        registry.write(&workspace.join("repository")).unwrap();
        let stale_head = git_repo::commit_all(&stale, "Stale writer", parent)
            .unwrap()
            .unwrap();
        assert!(git_repo::push(&stale, &config, None, stale_head).is_err());
        service.sync().unwrap();
        let records = service.devices().unwrap();
        assert_eq!(records.len(), 2);
        assert!(records
            .iter()
            .any(|device| device.name == "Other computer" && device.last_seen_at == 1234));
    }

    #[test]
    fn device_registry_rejects_invalid_remote_data_without_publishing_or_exposing_contents() {
        let _guard = TEST_SYNC_LOCK
            .lock()
            .unwrap_or_else(|lock| lock.into_inner());
        let root = tempfile::tempdir().unwrap();
        let (bare, config) = seed_remote(root.path());
        let seed_path = root.path().join("seed");
        let seed = Repository::open(&seed_path).unwrap();
        let parent = seed.head().unwrap().target();
        fs::write(seed_path.join("devices.json"), "do-not-display-secret").unwrap();
        let oid = git_repo::commit_all(&seed, "invalid registry", parent)
            .unwrap()
            .unwrap();
        git_repo::push(&seed, &config, None, oid).unwrap();
        let credentials = MemoryCredentialStore::default();
        let store = make_store(root.path(), "invalid-registry", &config);
        let service = DeviceSyncService::new(
            &store,
            &credentials,
            root.path().join("workspace"),
            root.path().join("central"),
        );
        let error = service
            .sync()
            .expect_err("invalid registry must not be overwritten");
        assert!(!format!("{error:#}").contains("do-not-display-secret"));
        assert_eq!(
            Repository::open_bare(bare)
                .unwrap()
                .refname_to_id("refs/heads/main")
                .unwrap(),
            oid
        );
    }

    #[test]
    fn text_merge_syncs_disjoint_paragraphs_and_check_does_not_modify_library() {
        let _guard = TEST_SYNC_LOCK
            .lock()
            .unwrap_or_else(|lock| lock.into_inner());
        let root = tempfile::tempdir().unwrap();
        let (bare, config) = seed_remote(root.path());
        let credentials = MemoryCredentialStore::default();
        let central_a = root.path().join("central-a");
        let central_b = root.path().join("central-b");
        let store_a = make_store(root.path(), "text-a", &config);
        let store_b = make_store(root.path(), "text-b", &config);
        let base = "# Skill\n\nFirst paragraph.\n\nSeparator.\n\nLast paragraph.\n";
        let ours = "# Skill\n\nFirst paragraph.\n\nSeparator.\n\nUpdated by B.\n";
        let theirs = "# Skill\n\nUpdated by A.\n\nSeparator.\n\nLast paragraph.\n";
        let expected = "# Skill\n\nUpdated by A.\n\nSeparator.\n\nUpdated by B.\n";
        add_skill(&store_a, &central_a, "one", base);
        fs::write(central_a.join("one/remove.txt"), "remove later").unwrap();
        let a = DeviceSyncService::new(
            &store_a,
            &credentials,
            root.path().join("ws-a"),
            central_a.clone(),
        );
        let b = DeviceSyncService::new(
            &store_b,
            &credentials,
            root.path().join("ws-b"),
            central_b.clone(),
        );
        a.sync().unwrap();
        b.sync().unwrap();
        fs::write(central_a.join("one/SKILL.md"), theirs).unwrap();
        fs::write(central_a.join("one/remote.txt"), "remote-only").unwrap();
        a.sync().unwrap();
        fs::write(central_b.join("one/SKILL.md"), ours).unwrap();
        fs::remove_file(central_b.join("one/remove.txt")).unwrap();
        fs::write(central_b.join("one/local.txt"), "local-only").unwrap();
        store_b
            .set_skill_tag_names("one", &["merged-tag".to_string()])
            .unwrap();
        let before = Repository::open_bare(&bare)
            .unwrap()
            .refname_to_id("refs/heads/main")
            .unwrap();
        let check = b.check().unwrap();
        assert_eq!(check.conflicted, 0);
        assert_eq!(check.updated, 1);
        assert_eq!(
            fs::read_to_string(central_b.join("one/SKILL.md")).unwrap(),
            ours
        );
        assert_eq!(
            Repository::open_bare(&bare)
                .unwrap()
                .refname_to_id("refs/heads/main")
                .unwrap(),
            before
        );
        assert!(store_b.list_device_sync_conflicts().unwrap().is_empty());
        let result = b.sync().unwrap();
        assert_eq!(result.status, "success");
        assert_eq!(
            fs::read_to_string(central_b.join("one/SKILL.md")).unwrap(),
            expected
        );
        assert!(!central_b.join("one/remove.txt").exists());
        assert_eq!(
            fs::read_to_string(central_b.join("one/local.txt")).unwrap(),
            "local-only"
        );
        assert_eq!(
            fs::read_to_string(central_b.join("one/remote.txt")).unwrap(),
            "remote-only"
        );
        let remote_repo = Repository::open_bare(&bare).unwrap();
        let oid = remote_repo.refname_to_id("refs/heads/main").unwrap();
        let merged_manifest = git_repo::manifest_at(&remote_repo, oid).unwrap();
        assert_eq!(
            merged_manifest.skills["one"].files,
            manifest::hash_files(&central_b.join("one")).unwrap()
        );
        assert_eq!(
            merged_manifest.skills["one"].content_hash,
            portable_hash(&merged_manifest.skills["one"])
        );
        assert_eq!(merged_manifest.skills["one"].tags, ["merged-tag"]);
        a.sync().unwrap();
        assert_eq!(
            fs::read_to_string(central_a.join("one/SKILL.md")).unwrap(),
            expected
        );
        let repeated = a.sync().unwrap();
        assert_eq!(repeated.changes, SyncChangeSummary::default());
    }

    #[test]
    fn text_merge_conflicts_and_push_failures_never_apply_or_publish_partial_results() {
        let _guard = TEST_SYNC_LOCK
            .lock()
            .unwrap_or_else(|lock| lock.into_inner());
        for conflicting in [true, false] {
            let root = tempfile::tempdir().unwrap();
            let (bare, config) = seed_remote(root.path());
            let credentials = MemoryCredentialStore::default();
            let central_a = root.path().join("central-a");
            let central_b = root.path().join("central-b");
            let store_a = make_store(root.path(), "text-a", &config);
            let store_b = make_store(root.path(), "text-b", &config);
            add_skill(&store_a, &central_a, "one", "# Original\n");
            let base = "a\nb\nc\nd\ne\n";
            fs::write(central_a.join("one/shared.md"), base).unwrap();
            let a = DeviceSyncService::new(
                &store_a,
                &credentials,
                root.path().join("ws-a"),
                central_a.clone(),
            );
            let b = DeviceSyncService::new(
                &store_b,
                &credentials,
                root.path().join("ws-b"),
                central_b.clone(),
            );
            a.sync().unwrap();
            b.sync().unwrap();
            let baseline = store_b
                .get_device_sync_config()
                .unwrap()
                .unwrap()
                .last_synced_commit;
            fs::write(central_a.join("one/shared.md"), "A\nb\nc\nd\ne\n").unwrap();
            if conflicting {
                fs::write(central_a.join("one/SKILL.md"), "# From A\n").unwrap();
            }
            a.sync().unwrap();
            fs::write(central_b.join("one/shared.md"), "a\nb\nc\nd\nB\n").unwrap();
            if conflicting {
                fs::write(central_b.join("one/SKILL.md"), "# From B\n").unwrap();
            }
            let before = Repository::open_bare(&bare)
                .unwrap()
                .refname_to_id("refs/heads/main")
                .unwrap();
            if !conflicting {
                fs::write(bare.join("refs/heads/main.lock"), b"locked for test").unwrap();
            }
            let result = b.sync();
            if conflicting {
                assert_eq!(result.unwrap().status, "conflicts");
                assert_eq!(
                    store_b.list_device_sync_conflicts().unwrap()[0].files,
                    ["SKILL.md"]
                );
                assert_eq!(
                    fs::read_to_string(central_b.join("one/SKILL.md")).unwrap(),
                    "# From B\n"
                );
            } else {
                assert!(result.is_err(), "a rejected push must be an error");
                assert!(store_b.list_device_sync_conflicts().unwrap().is_empty());
            }
            assert_eq!(
                fs::read_to_string(central_b.join("one/shared.md")).unwrap(),
                "a\nb\nc\nd\nB\n"
            );
            assert_eq!(
                Repository::open_bare(&bare)
                    .unwrap()
                    .refname_to_id("refs/heads/main")
                    .unwrap(),
                before
            );
            assert_eq!(
                store_b
                    .get_device_sync_config()
                    .unwrap()
                    .unwrap()
                    .last_synced_commit,
                baseline
            );
            if !conflicting {
                fs::remove_file(bare.join("refs/heads/main.lock")).unwrap();
                assert_eq!(b.sync().unwrap().status, "success");
                assert_eq!(
                    fs::read_to_string(central_b.join("one/shared.md")).unwrap(),
                    "A\nb\nc\nd\nB\n"
                );
                assert_eq!(a.sync().unwrap().status, "success");
                assert_eq!(
                    fs::read_to_string(central_a.join("one/shared.md")).unwrap(),
                    "A\nb\nc\nd\nB\n"
                );
                assert_eq!(b.sync().unwrap().changes, SyncChangeSummary::default());
            }
        }
    }

    #[test]
    fn text_merge_handles_git_eol_normalization_between_device_snapshots() {
        let _guard = TEST_SYNC_LOCK
            .lock()
            .unwrap_or_else(|lock| lock.into_inner());
        let root = tempfile::tempdir().unwrap();
        let (bare, config) = seed_remote(root.path());
        let credentials = MemoryCredentialStore::default();
        let central_a = root.path().join("central-a");
        let central_b = root.path().join("central-b");
        let store_a = make_store(root.path(), "eol-a", &config);
        let store_b = make_store(root.path(), "eol-b", &config);
        add_skill(
            &store_a,
            &central_a,
            "one",
            "# Skill\r\n\r\nfirst\r\n\r\nseparator\r\n\r\nlast\r\n",
        );
        fs::write(central_a.join("one/.gitattributes"), "*.md text eol=lf\n").unwrap();
        let a = DeviceSyncService::new(
            &store_a,
            &credentials,
            root.path().join("ws-a"),
            central_a.clone(),
        );
        let b = DeviceSyncService::new(
            &store_b,
            &credentials,
            root.path().join("ws-b"),
            central_b.clone(),
        );
        a.sync().unwrap();
        b.sync().unwrap();
        fs::write(
            central_a.join("one/SKILL.md"),
            "# Skill\r\n\r\nfrom A\r\n\r\nseparator\r\n\r\nlast\r\n",
        )
        .unwrap();
        a.sync().unwrap();
        fs::write(
            central_b.join("one/SKILL.md"),
            "# Skill\n\nfirst\n\nseparator\n\nfrom B\n",
        )
        .unwrap();
        assert_eq!(b.check().unwrap().conflicted, 0);
        assert_eq!(b.sync().unwrap().status, "success");
        assert_eq!(
            fs::read_to_string(central_b.join("one/SKILL.md")).unwrap(),
            "# Skill\n\nfrom A\n\nseparator\n\nfrom B\n"
        );
        let remote = Repository::open_bare(bare).unwrap();
        let manifest =
            git_repo::manifest_at(&remote, remote.refname_to_id("refs/heads/main").unwrap())
                .unwrap();
        assert_eq!(
            manifest.skills["one"].files,
            manifest::hash_files(&central_b.join("one")).unwrap()
        );
    }

    #[test]
    fn text_merge_snapshot_rejects_tampering_missing_files_and_unsafe_paths() {
        let root = tempfile::tempdir().unwrap();
        let repo = Repository::init(root.path().join("repo")).unwrap();
        let central = root.path().join("central");
        let store = make_store(root.path(), "snapshot", &DeviceSyncConfig::default());
        add_skill(&store, &central, "one", "# Original\n");
        let repo_path = repo.workdir().unwrap();
        let manifest = manifest::export_library(&store, repo_path).unwrap();
        let oid = git_repo::commit_all(&repo, "snapshot", None)
            .unwrap()
            .unwrap();
        let local = root.path().join("local");
        manifest::export_library(&store, &local).unwrap();
        let run = |base, id, path, manifests| {
            merge_snapshot_file(&repo, base, Some(oid), &local, manifests, id, path)
        };
        assert!(run(Some(oid), "one", "SKILL.md", [&manifest; 3])
            .unwrap()
            .is_some());
        assert!(run(None, "one", "SKILL.md", [&manifest; 3])
            .unwrap()
            .is_none());
        for (id, path) in [
            ("../one", "SKILL.md"),
            ("one", "../SKILL.md"),
            ("one", "/secret"),
            ("one", "C:\\secret"),
        ] {
            assert!(run(Some(oid), id, path, [&manifest; 3]).is_err());
        }
        let mut invalid = manifest.clone();
        invalid
            .skills
            .get_mut("one")
            .unwrap()
            .files
            .insert("SKILL.md".into(), "wrong".into());
        for index in 0..3 {
            let mut manifests = [&manifest; 3];
            manifests[index] = &invalid;
            assert!(run(Some(oid), "one", "SKILL.md", manifests).is_err());
        }
        fs::write(skill_dir(&local, "one").join("SKILL.md"), "tampered").unwrap();
        assert!(run(Some(oid), "one", "SKILL.md", [&manifest; 3]).is_err());
        fs::remove_file(skill_dir(&local, "one").join("SKILL.md")).unwrap();
        assert!(run(Some(oid), "one", "SKILL.md", [&manifest; 3]).is_err());
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(
                central.join("one/SKILL.md"),
                skill_dir(&local, "one").join("SKILL.md"),
            )
            .unwrap();
            assert!(run(Some(oid), "one", "SKILL.md", [&manifest; 3]).is_err());
        }
    }

    #[test]
    fn text_merge_revalidates_generated_content_before_it_can_be_published() {
        let root = tempfile::tempdir().unwrap();
        let central = root.path().join("central");
        let store = make_store(root.path(), "generated", &DeviceSyncConfig::default());
        add_skill(&store, &central, "one", "# Safe");
        let local_root = root.path().join("local");
        let remote_root = root.path().join("remote");
        let local = manifest::export_library(&store, &local_root).unwrap();
        let mut remote = manifest::export_library(&store, &remote_root).unwrap();
        let mut plan = MergePlan::default();
        plan.merge_files.insert("one".into(), BTreeSet::new());
        plan.merged_text.insert(
            "one".into(),
            std::collections::BTreeMap::from([(
                "SKILL.md".into(),
                format!("{}\n-----BEGIN PRIVATE KEY-----", "safe text\n".repeat(600)).into_bytes(),
            )]),
        );
        assert!(
            apply_plan_to_repository(&plan, &local, &local_root, &mut remote, &remote_root)
                .is_err()
        );
        assert_eq!(
            fs::read_to_string(central.join("one/SKILL.md")).unwrap(),
            "# Safe"
        );
    }

    fn add_skill_in_directory(
        store: &SkillStore,
        central: &Path,
        id: &str,
        directory: &str,
        content: &str,
    ) {
        let path = central.join(directory);
        fs::create_dir_all(&path).unwrap();
        fs::write(path.join("SKILL.md"), content).unwrap();
        store
            .upsert_skill(&SkillRecord {
                id: id.to_string(),
                name: directory.to_string(),
                description: Some("test".to_string()),
                source_type: "git".to_string(),
                source_ref: Some("https://example/source.git".to_string()),
                source_subpath: Some(format!("skills/{id}")),
                source_revision: None,
                central_path: path.to_string_lossy().to_string(),
                content_hash: None,
                created_at: 1,
                updated_at: 1,
                last_sync_at: None,
                last_seen_at: 1,
                enabled: true,
                status: "ok".to_string(),
            })
            .unwrap();
    }

    #[test]
    fn sync_history_counts_updates_and_ignores_internal_cache() {
        let _guard = TEST_SYNC_LOCK
            .lock()
            .unwrap_or_else(|lock| lock.into_inner());
        let root = tempfile::tempdir().unwrap();
        let (_bare, config) = seed_remote(root.path());
        let credentials = MemoryCredentialStore::default();
        let central = root.path().join("central");
        let store = make_store(root.path(), "counts", &config);
        add_skill(&store, &central, "one", "# Original");
        let service = DeviceSyncService::new(
            &store,
            &credentials,
            root.path().join("workspace"),
            central.clone(),
        );
        assert_eq!(service.sync().unwrap().changes.added, 1);
        let other_store = make_store(root.path(), "other-counts", &config);
        let other = DeviceSyncService::new(
            &other_store,
            &credentials,
            root.path().join("other-workspace"),
            root.path().join("other-central"),
        );
        other.sync().unwrap();
        fs::write(central.join("one/SKILL.md"), "# Updated").unwrap();
        let preview = service.check().unwrap();
        assert_eq!((preview.added, preview.updated), (0, 1));
        let result = service.sync().unwrap();
        assert_eq!((result.changes.added, result.changes.updated), (0, 1));
        let incoming = other.check().unwrap();
        assert_eq!((incoming.added, incoming.updated), (0, 1));
        assert_eq!(incoming.items[0].direction, "download");
        other.sync().unwrap();
        let stored = store.list_device_sync_history(10).unwrap();
        let item = &stored[0].items.as_ref().unwrap()[0];
        assert_eq!(
            (&*item.skill_id, &*item.name, &*item.kind, &*item.direction),
            ("one", "one", "updated", "upload")
        );
        fs::write(
            central.join("one/.skills-hub-cache.json"),
            r#"{"last_fetched_ms":123}"#,
        )
        .unwrap();
        assert_eq!(service.check().unwrap(), SyncChangeSummary::default());
        assert_eq!(
            service.sync().unwrap().changes,
            SyncChangeSummary::default()
        );
        assert_eq!(store.list_device_sync_history(10).unwrap().len(), 2);
    }

    #[test]
    fn synced_content_survives_unchanged_local_source_updates() {
        let _guard = TEST_SYNC_LOCK
            .lock()
            .unwrap_or_else(|lock| lock.into_inner());
        let root = tempfile::tempdir().unwrap();
        let (_, config) = seed_remote(root.path());
        let credentials = MemoryCredentialStore::default();
        let store = make_store(root.path(), "source-guard", &config);
        let central = root.path().join("central");
        add_skill(&store, &central, "one", "old");
        let source = root.path().join("source");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("SKILL.md"), "old").unwrap();
        let mut record = store.get_skill_by_id("one").unwrap().unwrap();
        record.source_type = "local".into();
        record.source_ref = Some(source.to_string_lossy().into());
        store.upsert_skill(&record).unwrap();
        let service = DeviceSyncService::new(
            &store,
            &credentials,
            root.path().join("ws"),
            central.clone(),
        );
        let repo = root.path().join("incoming");
        let mut incoming = SyncManifest::default();
        manifest::export_library(&store, &repo).unwrap();
        fs::write(skill_dir(&repo, "one").join("SKILL.md"), "remote new").unwrap();
        incoming.skills = SyncManifest::read(&repo).unwrap().skills;
        service
            .apply_repository_to_library(&incoming, &repo, &BTreeSet::new())
            .unwrap();
        store
            .delete_setting("device_sync.source_baseline.one")
            .unwrap();
        let app = tauri::test::mock_app();
        let result = crate::core::installer::update_managed_skill_from_source_with_lock_held(
            app.handle(),
            &store,
            "one",
            true,
        )
        .unwrap();
        assert!(!result.changed);
        assert_eq!(
            fs::read_to_string(central.join("one/SKILL.md")).unwrap(),
            "remote new"
        );
        fs::write(source.join("SKILL.md"), "local new").unwrap();
        let result = crate::core::installer::update_managed_skill_from_source_with_lock_held(
            app.handle(),
            &store,
            "one",
            true,
        )
        .unwrap();
        assert!(result.changed);
        assert_eq!(
            fs::read_to_string(central.join("one/SKILL.md")).unwrap(),
            "local new"
        );
    }

    #[test]
    fn incoming_source_type_never_replaces_an_existing_device_binding() {
        for (local_type, remote_type) in [("local", "git"), ("git", "local")] {
            let root = tempfile::tempdir().unwrap();
            let (_, config) = seed_remote(root.path());
            let store = make_store(root.path(), "binding", &config);
            let central = root.path().join("central");
            add_skill(&store, &central, "one", "old");
            let mut original = store.get_skill_by_id("one").unwrap().unwrap();
            original.source_type = local_type.into();
            original.source_ref = Some("device-owned-source".into());
            store.upsert_skill(&original).unwrap();
            let repo = root.path().join("incoming");
            let mut manifest = manifest::export_library(&store, &repo).unwrap();
            manifest.skills.get_mut("one").unwrap().source_type = remote_type.into();
            if remote_type == "local" {
                manifest.skills.get_mut("one").unwrap().source_ref = None;
            }
            let credentials = MemoryCredentialStore::default();
            let service =
                DeviceSyncService::new(&store, &credentials, root.path().join("ws"), central);
            service
                .apply_repository_to_library(&manifest, &repo, &BTreeSet::new())
                .unwrap();
            let actual = store.get_skill_by_id("one").unwrap().unwrap();
            assert_eq!(actual.source_type, original.source_type);
            assert_eq!(actual.source_ref, original.source_ref);
            assert_eq!(actual.source_subpath, original.source_subpath);
            let exported =
                manifest::export_library(&store, &root.path().join("next-export")).unwrap();
            assert_eq!(
                manifest::metadata_hash(&exported.skills["one"]),
                manifest::metadata_hash(&manifest.skills["one"]),
                "device bindings must not create new shared metadata changes"
            );
        }
    }

    #[test]
    fn same_name_downloads_preserve_both_skills_and_existing_directories() {
        let root = tempfile::tempdir().unwrap();
        let (_, config) = seed_remote(root.path());
        let source = make_store(root.path(), "sender", &config);
        let central_a = root.path().join("a");
        add_skill_in_directory(
            &source,
            &central_a,
            "aaaaaaaa-one",
            "first",
            "first content",
        );
        add_skill_in_directory(
            &source,
            &central_a,
            "aaaaaaaa-two",
            "second",
            "second content",
        );
        for mut skill in source.list_skills().unwrap() {
            skill.name = "shared".into();
            source.upsert_skill(&skill).unwrap();
        }
        let repo = root.path().join("repo");
        let manifest = manifest::export_library(&source, &repo).unwrap();
        let destination = make_store(root.path(), "receiver", &config);
        let central_b = root.path().join("b");
        fs::create_dir_all(central_b.join("shared-aaaaaaaa")).unwrap();
        fs::write(central_b.join("shared-aaaaaaaa/keep"), "untouched").unwrap();
        let credentials = MemoryCredentialStore::default();
        let service = DeviceSyncService::new(
            &destination,
            &credentials,
            root.path().join("ws"),
            central_b.clone(),
        );
        service
            .apply_repository_to_library(&manifest, &repo, &BTreeSet::new())
            .unwrap();
        for (id, expected) in [
            ("aaaaaaaa-one", "first content"),
            ("aaaaaaaa-two", "second content"),
        ] {
            let record = destination.get_skill_by_id(id).unwrap().unwrap();
            assert_eq!(
                fs::read_to_string(Path::new(&record.central_path).join("SKILL.md")).unwrap(),
                expected
            );
        }
        assert_eq!(
            fs::read_to_string(central_b.join("shared-aaaaaaaa/keep")).unwrap(),
            "untouched"
        );
    }

    #[test]
    fn local_source_paths_stay_on_their_device_and_legacy_imports_are_repaired() {
        let _guard = TEST_SYNC_LOCK
            .lock()
            .unwrap_or_else(|lock| lock.into_inner());
        let root = tempfile::tempdir().unwrap();
        let (_bare, config) = seed_remote(root.path());
        let credentials = MemoryCredentialStore::default();
        let ca = root.path().join("central-a");
        let a = make_store(root.path(), "local-a", &config);
        add_skill(&a, &ca, "one", "# Local skill");
        let source = root.path().join("device-a-project");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("SKILL.md"), "# Local skill").unwrap();
        let mut owner = a.get_skill_by_id("one").unwrap().unwrap();
        owner.source_type = "local".into();
        owner.source_ref = Some(source.to_string_lossy().into());
        owner.source_subpath = Some("/device-a-only/subpath".into());
        a.upsert_skill(&owner).unwrap();
        let wa = root.path().join("workspace-a");
        let sa = DeviceSyncService::new(&a, &credentials, wa.clone(), ca);
        sa.sync().unwrap();
        let raw = fs::read_to_string(wa.join("repository/.skills-hub/manifest.json")).unwrap();
        assert!(
            !raw.contains("device-a-project"),
            "local source paths must not be published"
        );
        assert!(!raw.contains("device-a-only"));
        assert_eq!(
            a.get_skill_by_id("one").unwrap().unwrap().source_ref,
            owner.source_ref
        );
        let b = make_store(root.path(), "local-b", &config);
        let wb = root.path().join("workspace-b");
        let sb = DeviceSyncService::new(&b, &credentials, wb, root.path().join("central-b"));
        sb.sync().unwrap();
        let mut imported = b.get_skill_by_id("one").unwrap().unwrap();
        assert_eq!(imported.source_ref.as_deref(), None);
        imported.source_ref = Some(imported.central_path.clone());
        b.upsert_skill(&imported).unwrap();
        assert_eq!(sb.sync().unwrap().changes, SyncChangeSummary::default());
        assert!(b
            .get_skill_by_id("one")
            .unwrap()
            .unwrap()
            .source_ref
            .is_none());
        // Simulate the previous release's imported foreign path, even though it exists here.
        b.delete_setting("device_sync.source_origin.one").unwrap();
        imported.source_ref = owner.source_ref.clone();
        imported.source_subpath = owner.source_subpath.clone();
        b.upsert_skill(&imported).unwrap();
        b.record_source_failure("one", "source path not found")
            .unwrap();
        assert_eq!(sb.sync().unwrap().changes, SyncChangeSummary::default());
        let repaired = b.get_skill_by_id("one").unwrap().unwrap();
        assert_eq!(repaired.source_ref.as_deref(), None);
        assert!(repaired.source_subpath.is_none());
        assert_eq!(repaired.status, "ok");
        assert!(b.source_checks().unwrap().get("one").unwrap().0.is_none());
        let mut obsolete = repaired.clone();
        obsolete.source_ref = owner.source_ref.clone();
        b.upsert_skill(&obsolete).unwrap();
        b.record_source_failure("one", "Permission denied").unwrap();
        sb.sync().unwrap();
        assert_eq!(b.get_skill_by_id("one").unwrap().unwrap().status, "ok");
        assert!(b.source_checks().unwrap().get("one").unwrap().0.is_none());
        let app = tauri::test::mock_app();
        let err = crate::core::installer::update_managed_skill_from_source(app.handle(), &b, "one")
            .err()
            .unwrap();
        assert!(err.to_string().starts_with("SKILL_SOURCE_UNBOUND|"));
        assert_eq!(b.get_skill_by_id("one").unwrap().unwrap().status, "ok");
        assert!(b.source_checks().unwrap().get("one").unwrap().0.is_none());
        b.delete_skill("one").unwrap();
        assert!(b
            .get_setting("device_sync.source_origin.one")
            .unwrap()
            .is_none());
        add_skill(
            &b,
            &root.path().join("central-b"),
            "reinstalled",
            "# Local skill",
        );
        let own_source_b = root.path().join("device-b-project");
        fs::create_dir(&own_source_b).unwrap();
        fs::write(own_source_b.join("SKILL.md"), "# Local skill").unwrap();
        let mut reinstalled = b.get_skill_by_id("reinstalled").unwrap().unwrap();
        reinstalled.source_type = "local".into();
        reinstalled.source_ref = Some(own_source_b.to_string_lossy().into());
        b.upsert_skill(&reinstalled).unwrap();
        sb.sync().unwrap();
        assert!(b.get_skill_by_id("reinstalled").unwrap().is_none());
        assert_eq!(
            b.get_skill_by_id("one").unwrap().unwrap().source_ref,
            reinstalled.source_ref
        );
        fs::remove_dir_all(&source).unwrap();
        a.record_source_failure("one", "source path not found")
            .unwrap();
        sa.sync().unwrap();
        let still_local = a.get_skill_by_id("one").unwrap().unwrap();
        assert_eq!(still_local.source_ref, owner.source_ref);
        assert_eq!(
            still_local.status, "error",
            "a genuinely missing local source must stay visible"
        );
    }

    #[test]
    fn remote_conflict_resolution_records_applied_changes() {
        let _guard = TEST_SYNC_LOCK
            .lock()
            .unwrap_or_else(|lock| lock.into_inner());
        let root = tempfile::tempdir().unwrap();
        let (_, config) = seed_remote(root.path());
        let credentials = MemoryCredentialStore::default();
        let ca = root.path().join("ca");
        let cb = root.path().join("cb");
        let a = make_store(root.path(), "review-a", &config);
        let b = make_store(root.path(), "review-b", &config);
        add_skill(&a, &ca, "shared", "# Remote");
        add_skill(&b, &cb, "shared", "# Local");
        let sa = DeviceSyncService::new(&a, &credentials, root.path().join("wa"), ca.clone());
        let sb = DeviceSyncService::new(&b, &credentials, root.path().join("wb"), cb.clone());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::write(ca.join("one/run.sh"), "#!/bin/sh\necho ok\n").unwrap();
            fs::set_permissions(ca.join("one/run.sh"), fs::Permissions::from_mode(0o755)).unwrap();
        }
        sa.sync().unwrap();
        assert_eq!(sb.sync().unwrap().changes.conflicted, 1);
        let conflict = b.list_device_sync_conflicts().unwrap().remove(0);
        b.delete_setting("device_sync_last_run").unwrap();
        sb.resolve_conflict(&conflict.id, ConflictResolution::UseRemote)
            .unwrap();
        assert_eq!(
            fs::read_to_string(cb.join("one/SKILL.md")).unwrap(),
            "# Remote"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_ne!(
                fs::metadata(cb.join("one/run.sh"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o111,
                0
            );
        }
        let immediate = b.list_device_sync_history(10).unwrap();
        let resolution = immediate
            .iter()
            .find(|run| run.status == "resolved")
            .unwrap();
        assert_eq!(resolution.updated, 1);
        let items = resolution.items.as_ref().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].skill_id, "shared");
        assert_eq!(items[0].direction, "download");
        assert_eq!(
            resolution.commit.as_deref(),
            Some(conflict.remote_commit.as_str())
        );
        assert_eq!(
            sb.status().unwrap().last_run_status.as_deref(),
            Some("conflicts")
        );
        assert_eq!(sb.sync().unwrap().changes, SyncChangeSummary::default());
        let history = b.list_device_sync_history(10).unwrap();
        assert!(history.iter().any(|run| run.status == "resolved"), "successfully imported remote conflict resolution must appear in history; actual statuses: {:?}", history.iter().map(|r| &r.status).collect::<Vec<_>>());
    }

    #[test]
    fn conflict_resolution_uses_its_immutable_snapshot() {
        let _guard = TEST_SYNC_LOCK
            .lock()
            .unwrap_or_else(|lock| lock.into_inner());
        let root = tempfile::tempdir().unwrap();
        let (_, config) = seed_remote(root.path());
        let credentials = MemoryCredentialStore::default();
        let ca = root.path().join("ca");
        let cb = root.path().join("cb");
        let a = make_store(root.path(), "review-a", &config);
        let b = make_store(root.path(), "review-b", &config);
        add_skill(&a, &ca, "shared", "# A");
        add_skill(&b, &cb, "shared", "# Local");
        let sa = DeviceSyncService::new(&a, &credentials, root.path().join("wa"), ca.clone());
        let sb = DeviceSyncService::new(&b, &credentials, root.path().join("wb"), cb.clone());
        sa.sync().unwrap();
        assert_eq!(sb.sync().unwrap().changes.conflicted, 1);
        let conflict = b.list_device_sync_conflicts().unwrap().remove(0);
        fs::write(ca.join("one/SKILL.md"), "# B").unwrap();
        sa.sync().unwrap();
        sb.check().unwrap();
        sb.resolve_conflict(&conflict.id, ConflictResolution::UseRemote)
            .unwrap();
        assert_eq!(fs::read_to_string(cb.join("one/SKILL.md")).unwrap(), "# A");
        fs::write(ca.join("one/SKILL.md"), "# A").unwrap();
        sa.sync().unwrap();
        let result = sb.sync().unwrap();
        assert_eq!(result.changes.updated, 0);
        sa.sync().unwrap();
        assert_eq!(
            fs::read_to_string(ca.join("one/SKILL.md")).unwrap(),
            "# A",
            "remote revert must not be overwritten by imported snapshot"
        );
    }

    #[test]
    fn first_sync_conflict_resolution_does_not_delete_unseen_remote_skills() {
        let _guard = TEST_SYNC_LOCK
            .lock()
            .unwrap_or_else(|lock| lock.into_inner());
        for resolution in [
            ConflictResolution::KeepLocal,
            ConflictResolution::UseRemote,
            ConflictResolution::KeepBoth,
        ] {
            let root = tempfile::tempdir().unwrap();
            let (_bare, config) = seed_remote(root.path());
            let credentials = MemoryCredentialStore::default();
            let central_a = root.path().join("central-a");
            let store_a = make_store(root.path(), "a", &config);
            add_skill(&store_a, &central_a, "shared", "# Remote shared");
            for n in 0..31 {
                add_skill_in_directory(
                    &store_a,
                    &central_a,
                    &format!("remote-{n}"),
                    &format!("remote-{n}"),
                    &format!("# Remote {n}"),
                );
            }
            let service_a = DeviceSyncService::new(
                &store_a,
                &credentials,
                root.path().join("workspace-a"),
                central_a,
            );
            service_a.sync().unwrap();
            let central_b = root.path().join("central-b");
            let store_b = make_store(root.path(), "b", &config);
            add_skill(&store_b, &central_b, "shared", "# Local shared");
            let workspace_b = root.path().join("workspace-b");
            let service_b = DeviceSyncService::new(
                &store_b,
                &credentials,
                workspace_b.clone(),
                central_b.clone(),
            );
            assert_eq!(service_b.sync().unwrap().changes.conflicted, 1);
            let conflict = store_b.list_device_sync_conflicts().unwrap().remove(0);
            service_b
                .resolve_conflict(&conflict.id, resolution)
                .unwrap();
            let saved_choices = store_b.get_setting(RESOLVED_SYNC_KEY).unwrap().unwrap();
            let restarted = DeviceSyncService::new(&store_b, &credentials, workspace_b, central_b);
            let preview = restarted.check().unwrap();
            assert_eq!(
                preview.deleted, 0,
                "unseen remote skills must be downloaded"
            );
            let result = restarted.sync().unwrap();
            assert_eq!(result.changes.deleted, 0);
            assert_eq!(result.changes.conflicted, 0);
            for n in 0..31 {
                assert!(store_b
                    .get_skill_by_id(&format!("remote-{n}"))
                    .unwrap()
                    .is_some());
            }
            assert_eq!(service_a.sync().unwrap().changes.deleted, 0);
            store_b
                .set_setting(RESOLVED_SYNC_KEY, &saved_choices)
                .unwrap();
            let shared = store_a.get_skill_by_id("shared").unwrap().unwrap();
            fs::write(
                Path::new(&shared.central_path).join("SKILL.md"),
                "# Later remote update",
            )
            .unwrap();
            service_a.sync().unwrap();
            assert_eq!(restarted.check().unwrap().conflicted, 0);
            assert_eq!(restarted.sync().unwrap().changes.conflicted, 0);
        }
    }

    #[test]
    fn reconnecting_the_same_device_recovers_its_repository_baseline() {
        let _test_guard = TEST_SYNC_LOCK
            .lock()
            .unwrap_or_else(|lock| lock.into_inner());
        let root = tempfile::tempdir().unwrap();
        let (_bare, config) = seed_remote(root.path());
        let credentials = MemoryCredentialStore::default();
        let central = root.path().join("central");
        let store = make_store(root.path(), "same-device", &config);
        add_skill(&store, &central, "one", "# Version one");
        let service = DeviceSyncService::new(
            &store,
            &credentials,
            root.path().join("workspace"),
            central.clone(),
        );

        service.sync().unwrap();
        fs::write(central.join("one/SKILL.md"), "# Version two").unwrap();
        let mut reconnected = store.get_device_sync_config().unwrap().unwrap();
        reconnected.last_synced_commit = None;
        store.save_device_sync_config(&reconnected).unwrap();

        let result = service.sync().unwrap();

        assert_eq!(result.status, "success");
        assert_eq!(result.changes.conflicted, 0);
        assert!(store.list_device_sync_conflicts().unwrap().is_empty());
    }

    #[test]
    fn repairs_legacy_false_conflicts_created_after_same_device_reconnection() {
        let _test_guard = TEST_SYNC_LOCK
            .lock()
            .unwrap_or_else(|lock| lock.into_inner());
        let root = tempfile::tempdir().unwrap();
        let (_bare, config) = seed_remote(root.path());
        let credentials = MemoryCredentialStore::default();
        let central = root.path().join("central");
        let store = make_store(root.path(), "repair", &config);
        add_skill(&store, &central, "one", "# Version one");
        let service =
            DeviceSyncService::new(&store, &credentials, root.path().join("workspace"), central);
        let commit = service.sync().unwrap().commit.unwrap();
        store
            .upsert_device_sync_conflict(&SyncConflict {
                id: format!("one:{commit}"),
                skill_id: "one".to_string(),
                skill_name: "One".to_string(),
                base_commit: None,
                local_commit: "local".to_string(),
                remote_commit: commit.clone(),
                files: vec!["*".to_string()],
                created_at: now_ms(),
                status: "pending".to_string(),
            })
            .unwrap();

        assert!(service.repair_legacy_same_device_conflicts().unwrap());

        assert!(store.list_device_sync_conflicts().unwrap().is_empty());
        assert_eq!(
            store
                .get_device_sync_config()
                .unwrap()
                .unwrap()
                .last_synced_commit
                .as_deref(),
            Some(commit.as_str())
        );
    }

    #[test]
    fn a_conflicted_sync_does_not_push_partial_changes_or_advance_the_baseline() {
        let _test_guard = TEST_SYNC_LOCK
            .lock()
            .unwrap_or_else(|lock| lock.into_inner());
        let root = tempfile::tempdir().unwrap();
        let (bare, config) = seed_remote(root.path());
        let credentials = MemoryCredentialStore::default();
        let central_a = root.path().join("central-a");
        let store_a = make_store(root.path(), "atomic-a", &config);
        add_skill(&store_a, &central_a, "one", "# Shared");
        let service_a = DeviceSyncService::new(
            &store_a,
            &credentials,
            root.path().join("workspace-a"),
            central_a.clone(),
        );
        service_a.sync().unwrap();

        let central_b = root.path().join("central-b");
        let store_b = make_store(root.path(), "atomic-b", &config);
        let service_b = DeviceSyncService::new(
            &store_b,
            &credentials,
            root.path().join("workspace-b"),
            central_b.clone(),
        );
        service_b.sync().unwrap();
        let baseline = store_b
            .get_device_sync_config()
            .unwrap()
            .unwrap()
            .last_synced_commit
            .unwrap();

        fs::write(central_a.join("one/SKILL.md"), "# From A").unwrap();
        service_a.sync().unwrap();
        fs::write(central_b.join("one/SKILL.md"), "# From B").unwrap();
        add_skill_in_directory(&store_b, &central_b, "two", "two", "# Local-only skill");
        let remote_before = Repository::open_bare(&bare)
            .unwrap()
            .refname_to_id("refs/heads/main")
            .unwrap();

        let result = service_b.sync().unwrap();

        assert_eq!(result.status, "conflicts");
        assert_eq!(
            Repository::open_bare(&bare)
                .unwrap()
                .refname_to_id("refs/heads/main")
                .unwrap(),
            remote_before
        );
        assert_eq!(
            store_b
                .get_device_sync_config()
                .unwrap()
                .unwrap()
                .last_synced_commit
                .as_deref(),
            Some(baseline.as_str())
        );
        assert!(!service_b.repair_legacy_same_device_conflicts().unwrap());
        assert_eq!(store_b.list_device_sync_conflicts().unwrap().len(), 1);
        let conflict = store_b.list_device_sync_conflicts().unwrap().remove(0);
        service_b
            .resolve_conflict(&conflict.id, ConflictResolution::KeepLocal)
            .unwrap();
        assert_eq!(service_b.sync().unwrap().status, "success");
    }

    #[test]
    fn unresolved_conflicts_block_sync_and_baseline_advancement() {
        let _test_guard = TEST_SYNC_LOCK
            .lock()
            .unwrap_or_else(|lock| lock.into_inner());
        let root = tempfile::tempdir().unwrap();
        let (bare, config) = seed_remote(root.path());
        let credentials = MemoryCredentialStore::default();
        let central_a = root.path().join("central-a");
        let store_a = make_store(root.path(), "pending-a", &config);
        add_skill_in_directory(&store_a, &central_a, "one", "one", "# Shared one");
        add_skill_in_directory(&store_a, &central_a, "two", "two", "# Shared two");
        let service_a = DeviceSyncService::new(
            &store_a,
            &credentials,
            root.path().join("workspace-a"),
            central_a.clone(),
        );
        service_a.sync().unwrap();

        let central_b = root.path().join("central-b");
        let store_b = make_store(root.path(), "pending-b", &config);
        let service_b = DeviceSyncService::new(
            &store_b,
            &credentials,
            root.path().join("workspace-b"),
            central_b.clone(),
        );
        service_b.sync().unwrap();
        let baseline = store_b
            .get_device_sync_config()
            .unwrap()
            .unwrap()
            .last_synced_commit
            .unwrap();

        fs::write(central_a.join("one/SKILL.md"), "# From A one").unwrap();
        fs::write(central_a.join("two/SKILL.md"), "# From A two").unwrap();
        service_a.sync().unwrap();
        fs::write(central_b.join("one/SKILL.md"), "# From B one").unwrap();
        fs::write(central_b.join("two/SKILL.md"), "# From B two").unwrap();
        assert_eq!(service_b.sync().unwrap().changes.conflicted, 2);
        let remote_before = Repository::open_bare(&bare)
            .unwrap()
            .refname_to_id("refs/heads/main")
            .unwrap();
        let conflicts = store_b.list_device_sync_conflicts().unwrap();

        service_b
            .resolve_conflict(&conflicts[0].id, ConflictResolution::KeepLocal)
            .unwrap();

        assert_eq!(
            store_b
                .get_device_sync_config()
                .unwrap()
                .unwrap()
                .last_synced_commit
                .as_deref(),
            Some(baseline.as_str())
        );
        let blocked = service_b.sync().unwrap();
        assert_eq!(blocked.status, "conflicts");
        assert_eq!(blocked.changes.conflicted, 1);
        assert_eq!(
            Repository::open_bare(&bare)
                .unwrap()
                .refname_to_id("refs/heads/main")
                .unwrap(),
            remote_before
        );
    }

    #[test]
    fn scheduled_sync_never_reads_credentials_when_disabled_busy_or_conflicted() {
        struct NoCredentials;
        impl CredentialStore for NoCredentials {
            fn get(&self, _: &str) -> Result<Option<String>> {
                panic!("unexpected credential read")
            }
            fn set(&self, _: &str, _: &str) -> Result<()> {
                panic!("unexpected credential write")
            }
            fn delete(&self, _: &str) -> Result<()> {
                panic!("unexpected credential delete")
            }
        }
        let _test_guard = TEST_SYNC_LOCK
            .lock()
            .unwrap_or_else(|lock| lock.into_inner());
        let root = tempfile::tempdir().unwrap();
        let (_bare, mut config) = seed_remote(root.path());
        config.credential_key = Some("must-not-read".into());
        config.auto_sync_schedule = Some(scheduler::SyncSchedule::Interval { minutes: 5 });
        let store = make_store(root.path(), "scheduled", &config);
        let service = DeviceSyncService::new(
            &store,
            &NoCredentials,
            root.path().join("workspace"),
            root.path().join("central"),
        );
        assert!(service.sync_scheduled(&config).unwrap().is_none());
        config.auto_sync = true;
        store.save_device_sync_config(&config).unwrap();
        {
            let _guard = try_lock_device_sync().unwrap();
            assert!(service.sync_scheduled(&config).unwrap().is_none());
        }
        let mut stale = config.clone();
        stale.auto_sync_schedule = Some(scheduler::SyncSchedule::Daily {
            time: "09:00".into(),
        });
        assert!(service.sync_scheduled(&stale).unwrap().is_none());
        store
            .upsert_device_sync_conflict(&SyncConflict {
                id: "conflict".into(),
                skill_id: "one".into(),
                skill_name: "one".into(),
                base_commit: None,
                local_commit: "local".into(),
                remote_commit: "remote".into(),
                files: vec!["SKILL.md".into()],
                created_at: 1,
                status: "pending".into(),
            })
            .unwrap();
        assert!(service.sync_scheduled(&config).unwrap().is_none());
        assert!(store.list_device_sync_history(20).unwrap().is_empty());
    }

    #[test]
    fn unchanged_sync_refreshes_device_without_adding_history() {
        let _test_guard = TEST_SYNC_LOCK
            .lock()
            .unwrap_or_else(|lock| lock.into_inner());
        let root = tempfile::tempdir().unwrap();
        let (_bare, config) = seed_remote(root.path());
        let credentials = MemoryCredentialStore::default();
        let central = root.path().join("central");
        let store = make_store(root.path(), "no-change", &config);
        add_skill(&store, &central, "one", "# Version one");
        let service = DeviceSyncService::new(
            &store,
            &credentials,
            root.path().join("workspace"),
            central.clone(),
        );
        service.sync().unwrap();
        let history = store.list_device_sync_history(20).unwrap();
        assert_eq!(history.len(), 1);
        let mut source_failed = store.get_skill_by_id("one").unwrap().unwrap();
        source_failed.status = "error".into();
        store.upsert_skill(&source_failed).unwrap();
        let mut device = store.list_device_sync_devices("").unwrap().remove(0);
        device.last_seen_at = 1;
        store.upsert_device_sync_device(&device).unwrap();
        for _ in 0..2 {
            let result = service.sync().unwrap();
            assert_eq!(result.changes, SyncChangeSummary::default());
            let repo = Repository::open(root.path().join("workspace/repository")).unwrap();
            let head = repo
                .find_commit(git2::Oid::from_str(result.commit.as_deref().unwrap()).unwrap())
                .unwrap();
            assert_eq!(
                head.tree()
                    .unwrap()
                    .get_path(Path::new("skills"))
                    .unwrap()
                    .id(),
                head.parent(0)
                    .unwrap()
                    .tree()
                    .unwrap()
                    .get_path(Path::new("skills"))
                    .unwrap()
                    .id()
            );
            assert_eq!(
                store.get_skill_by_id("one").unwrap().unwrap().status,
                "error"
            );
        }
        assert_eq!(store.list_device_sync_history(20).unwrap().len(), 1);
        assert_eq!(
            service.status().unwrap().last_run_status.as_deref(),
            Some("unchanged")
        );
        assert!(store.list_device_sync_devices("").unwrap()[0].last_seen_at > 1);
        fs::write(central.join("one/SKILL.md"), "# Updated").unwrap();
        service.sync().unwrap();
        assert_eq!(store.list_device_sync_history(20).unwrap().len(), 2);
        store.start_device_sync_run("failed-check", 1).unwrap();
        store
            .finish_device_sync_run(
                "failed-check",
                2,
                "failed",
                0,
                0,
                0,
                0,
                None,
                Some("network unavailable"),
                None,
            )
            .unwrap();
        assert_eq!(
            service.status().unwrap().last_run_status.as_deref(),
            Some("failed")
        );
        service.sync().unwrap();
        assert_eq!(store.list_device_sync_history(20).unwrap().len(), 3);
        assert_eq!(
            service.status().unwrap().last_run_status.as_deref(),
            Some("unchanged")
        );
    }

    #[test]
    fn no_op_remote_pull_leaves_a_recoverable_device_baseline() {
        let _test_guard = TEST_SYNC_LOCK
            .lock()
            .unwrap_or_else(|lock| lock.into_inner());
        let root = tempfile::tempdir().unwrap();
        let (_bare, config) = seed_remote(root.path());
        let credentials = MemoryCredentialStore::default();
        let central_a = root.path().join("central-a");
        let store_a = make_store(root.path(), "marker-a", &config);
        add_skill(&store_a, &central_a, "one", "# Version one");
        let service_a = DeviceSyncService::new(
            &store_a,
            &credentials,
            root.path().join("workspace-a"),
            central_a.clone(),
        );
        service_a.sync().unwrap();

        let central_b = root.path().join("central-b");
        let store_b = make_store(root.path(), "marker-b", &config);
        let service_b = DeviceSyncService::new(
            &store_b,
            &credentials,
            root.path().join("workspace-b"),
            central_b.clone(),
        );
        service_b.sync().unwrap();
        fs::write(central_b.join("one/SKILL.md"), "# Version two").unwrap();
        service_b.sync().unwrap();

        service_a.sync().unwrap();
        let mut reconnected = store_a.get_device_sync_config().unwrap().unwrap();
        reconnected.last_synced_commit = None;
        store_a.save_device_sync_config(&reconnected).unwrap();
        fs::write(central_a.join("one/SKILL.md"), "# Version three").unwrap();

        let result = service_a.sync().unwrap();

        assert_eq!(result.status, "success");
        assert_eq!(result.changes.conflicted, 0);
    }

    #[test]
    fn retry_after_push_and_local_apply_failure_does_not_delete_remote_skills() {
        let _test_guard = TEST_SYNC_LOCK
            .lock()
            .unwrap_or_else(|lock| lock.into_inner());
        let root = tempfile::tempdir().unwrap();
        let (bare, config) = seed_remote(root.path());
        let credentials = MemoryCredentialStore::default();
        let central_a = root.path().join("central-a");
        let store_a = make_store(root.path(), "recovery-a", &config);
        add_skill(&store_a, &central_a, "remote", "# Remote skill");
        DeviceSyncService::new(
            &store_a,
            &credentials,
            root.path().join("workspace-a"),
            central_a,
        )
        .sync()
        .unwrap();

        let blocked_central = root.path().join("central-b");
        fs::write(&blocked_central, "not a directory").unwrap();
        let store_b = make_store(root.path(), "recovery-b", &config);
        let workspace_b = root.path().join("workspace-b");
        let failing_service = DeviceSyncService::new(
            &store_b,
            &credentials,
            workspace_b.clone(),
            blocked_central.clone(),
        );

        assert!(failing_service.sync().is_err());
        assert!(store_b
            .get_device_sync_config()
            .unwrap()
            .unwrap()
            .last_synced_commit
            .is_none());
        let first_recovery_state = store_b.get_setting(INCOMPLETE_SYNC_KEY).unwrap().unwrap();
        let local_library = root.path().join("local-library-b");
        add_skill_in_directory(&store_b, &local_library, "local", "local", "# Local skill");
        assert!(failing_service.sync().is_err());
        assert_eq!(
            store_b.get_setting(INCOMPLETE_SYNC_KEY).unwrap().unwrap(),
            first_recovery_state
        );
        fs::remove_file(&blocked_central).unwrap();
        fs::create_dir_all(&blocked_central).unwrap();
        let retry = DeviceSyncService::new(&store_b, &credentials, workspace_b, blocked_central)
            .sync()
            .unwrap();

        assert_eq!(retry.status, "success");
        let remote_head = Repository::open_bare(&bare)
            .unwrap()
            .refname_to_id("refs/heads/main")
            .unwrap();
        assert!(
            git_repo::manifest_at(&Repository::open_bare(&bare).unwrap(), remote_head)
                .unwrap()
                .skills
                .contains_key("remote")
        );
        assert!(
            git_repo::manifest_at(&Repository::open_bare(&bare).unwrap(), remote_head)
                .unwrap()
                .skills
                .contains_key("local")
        );
        assert!(store_b.get_setting(INCOMPLETE_SYNC_KEY).unwrap().is_none());
    }

    #[test]
    fn switching_repositories_preserves_each_incomplete_sync_baseline() {
        let _test_guard = TEST_SYNC_LOCK
            .lock()
            .unwrap_or_else(|lock| lock.into_inner());
        let root = tempfile::tempdir().unwrap();
        let remote_a_root = root.path().join("remote-a-root");
        let remote_b_root = root.path().join("remote-b-root");
        fs::create_dir_all(&remote_a_root).unwrap();
        fs::create_dir_all(&remote_b_root).unwrap();
        let (bare_a, config_a) = seed_remote(&remote_a_root);
        let (_bare_b, config_b) = seed_remote(&remote_b_root);
        let credentials = MemoryCredentialStore::default();

        let central_a = root.path().join("source-a");
        let source_store = make_store(root.path(), "switch-source", &config_a);
        add_skill(&source_store, &central_a, "remote", "# Remote A skill");
        DeviceSyncService::new(
            &source_store,
            &credentials,
            root.path().join("source-workspace"),
            central_a,
        )
        .sync()
        .unwrap();

        let blocked_central = root.path().join("switch-central");
        fs::write(&blocked_central, "not a directory").unwrap();
        let store = make_store(root.path(), "switch-client", &config_a);
        let workspace = root.path().join("switch-workspace");
        assert!(DeviceSyncService::new(
            &store,
            &credentials,
            workspace.clone(),
            blocked_central.clone(),
        )
        .sync()
        .is_err());

        fs::remove_file(&blocked_central).unwrap();
        fs::create_dir_all(&blocked_central).unwrap();
        store.save_device_sync_config(&config_b).unwrap();
        DeviceSyncService::new(
            &store,
            &credentials,
            workspace.clone(),
            blocked_central.clone(),
        )
        .sync()
        .unwrap();

        store.save_device_sync_config(&config_a).unwrap();
        let result = DeviceSyncService::new(&store, &credentials, workspace, blocked_central)
            .sync()
            .unwrap();

        assert_eq!(result.status, "success");
        let remote_head = Repository::open_bare(&bare_a)
            .unwrap()
            .refname_to_id("refs/heads/main")
            .unwrap();
        assert!(
            git_repo::manifest_at(&Repository::open_bare(&bare_a).unwrap(), remote_head)
                .unwrap()
                .skills
                .contains_key("remote")
        );
    }

    #[test]
    fn a_new_device_without_a_common_baseline_keeps_ambiguous_content_as_a_conflict() {
        let _test_guard = TEST_SYNC_LOCK
            .lock()
            .unwrap_or_else(|lock| lock.into_inner());
        let root = tempfile::tempdir().unwrap();
        let (_bare, config) = seed_remote(root.path());
        let credentials = MemoryCredentialStore::default();
        let central_a = root.path().join("central-a");
        let store_a = make_store(root.path(), "new-device-a", &config);
        add_skill(&store_a, &central_a, "one", "# From A");
        DeviceSyncService::new(
            &store_a,
            &credentials,
            root.path().join("workspace-a"),
            central_a,
        )
        .sync()
        .unwrap();

        let central_b = root.path().join("central-b");
        let store_b = make_store(root.path(), "new-device-b", &config);
        add_skill(&store_b, &central_b, "one", "# From B");
        let result = DeviceSyncService::new(
            &store_b,
            &credentials,
            root.path().join("workspace-b"),
            central_b,
        )
        .sync()
        .unwrap();

        assert_eq!(result.status, "conflicts");
        assert_eq!(result.changes.conflicted, 1);
    }

    #[test]
    fn full_device_sync_flow_preserves_local_relations_and_recovers_deletions() {
        let _test_guard = TEST_SYNC_LOCK
            .lock()
            .unwrap_or_else(|lock| lock.into_inner());
        let root = tempfile::tempdir().unwrap();
        let (_bare, config) = seed_remote(root.path());
        let credentials = MemoryCredentialStore::default();
        credentials
            .set("credential", "never-commit-this-token")
            .unwrap();

        let central_a = root.path().join("central-a");
        let store_a = make_store(root.path(), "a", &config);
        add_skill(&store_a, &central_a, "one", "# One");
        let service_a = DeviceSyncService::new(
            &store_a,
            &credentials,
            root.path().join("workspace-a"),
            central_a.clone(),
        );
        let first = service_a.sync().unwrap();
        assert_eq!(first.changes.added, 1);
        let unchanged = service_a.sync().unwrap();
        assert_eq!(unchanged.changes, SyncChangeSummary::default());

        let central_b = root.path().join("central-b");
        let store_b = make_store(root.path(), "b", &config);
        add_skill(&store_b, &central_b, "local-one", "# One");
        store_b
            .upsert_skill_target(&SkillTargetRecord {
                id: "project-target".to_string(),
                skill_id: "local-one".to_string(),
                tool: "cursor".to_string(),
                scope: "project".to_string(),
                project_path: Some(root.path().join("project").to_string_lossy().into()),
                target_path: root
                    .path()
                    .join("project/.cursor/skills/one")
                    .to_string_lossy()
                    .into(),
                mode: "copy".to_string(),
                status: "ok".to_string(),
                last_error: None,
                synced_at: None,
            })
            .unwrap();
        let service_b = DeviceSyncService::new(
            &store_b,
            &credentials,
            root.path().join("workspace-b"),
            central_b.clone(),
        );
        service_b.sync().unwrap();
        assert!(store_b.get_skill_by_id("local-one").unwrap().is_none());
        let tag = store_b.create_tag("portable-tag").unwrap();
        store_b.set_skill_tags("one", &[tag.id]).unwrap();
        assert_eq!(
            store_b.list_skill_targets("one").unwrap()[0]
                .project_path
                .as_deref(),
            Some(root.path().join("project").to_str().unwrap())
        );

        fs::write(central_a.join("one/notes.txt"), "from a").unwrap();
        service_a.sync().unwrap();
        service_b.sync().unwrap();
        assert_eq!(
            fs::read_to_string(central_b.join("one/notes.txt")).unwrap(),
            "from a"
        );

        fs::write(central_a.join("one/SKILL.md"), "# From A").unwrap();
        fs::write(central_b.join("one/SKILL.md"), "# From B").unwrap();
        service_a.sync().unwrap();
        let conflicted = service_b.sync().unwrap();
        assert_eq!(conflicted.changes.conflicted, 1);
        let conflict = store_b.list_device_sync_conflicts().unwrap().remove(0);
        service_b
            .resolve_conflict(&conflict.id, ConflictResolution::KeepBoth)
            .unwrap();
        assert_eq!(store_b.list_skills().unwrap().len(), 2);
        assert_eq!(
            fs::read_to_string(central_b.join("one/SKILL.md")).unwrap(),
            "# From A"
        );

        fs::remove_dir_all(central_a.join("one")).unwrap();
        store_a.delete_skill("one").unwrap();
        service_a.sync().unwrap();
        service_b.sync().unwrap();
        let trash = store_b.list_device_sync_trash().unwrap();
        assert_eq!(trash.len(), 1);
        service_b.restore_trash(&trash[0].id).unwrap();
        assert!(store_b.get_skill_by_id("one").unwrap().is_some());
        let history = store_b.list_device_sync_history(20).unwrap();
        assert_eq!(history.len(), 4);
        assert!(history.iter().any(|run| run.status == "resolved"));
        assert!(history.iter().any(|run| run.status == "conflicts"));
        assert!(history
            .iter()
            .all(|run| run.added + run.updated + run.deleted + run.conflicted > 0));

        let db_bytes = fs::read(store_a.db_path()).unwrap();
        assert!(!String::from_utf8_lossy(&db_bytes).contains("never-commit-this-token"));
        for entry in walkdir::WalkDir::new(root.path().join("workspace-a/repository")) {
            let entry = entry.unwrap();
            if entry.file_type().is_file() {
                let bytes = fs::read(entry.path()).unwrap();
                assert!(!String::from_utf8_lossy(&bytes).contains("never-commit-this-token"));
            }
        }
    }
}
