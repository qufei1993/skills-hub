pub mod credentials;
mod git_repo;
pub mod manifest;
pub mod merge;
pub mod oauth;
pub mod providers;
pub mod scheduler;
pub mod types;

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use anyhow::{bail, Context, Result};
use uuid::Uuid;

use self::credentials::CredentialStore;
use self::manifest::{portable_hash, skill_dir, SyncManifest};
use self::merge::{plan_merge, MergePlan};
use self::types::{
    ConflictResolution, DeviceSyncConfig, DeviceSyncDevice, SyncChangeSummary, SyncConflict,
    SyncRunResult, SyncStatus, TrashEntry,
};
use crate::core::skill_store::SkillStore;

pub struct DeviceSyncService<'a> {
    store: &'a SkillStore,
    credentials: &'a dyn CredentialStore,
    workspace_root: PathBuf,
    central_root: PathBuf,
}

static SYNC_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
const INCOMPLETE_SYNC_KEY: &str = "device_sync.incomplete_sync";

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
        let token = self.token(&config)?;
        let repo_path = self.workspace_root.join("repository");
        let repo = git_repo::open_or_clone(&repo_path, &config, token.as_deref())?;
        let remote_oid = git_repo::fetch_and_checkout(&repo, &config, token.as_deref())?;
        self.ingest_discovered_devices(&repo, remote_oid)?;
        let remote = SyncManifest::read(&repo_path)?;
        let device = self.local_device_identity()?;
        let (base, _) = self.base_manifest(&repo, &config, remote_oid, &device.id)?;
        let export = self.fresh_export()?;
        let mut local = SyncManifest::read(&export)?;
        reconcile_identities(&mut local, &remote, &export)?;
        let plan = plan_merge(&base, &local, &remote);
        let summary = summarize(&plan);
        let _ = fs::remove_dir_all(export);
        if remote_oid.is_none() && local.skills.is_empty() {
            return Ok(SyncChangeSummary::default());
        }
        Ok(summary)
    }

    pub fn devices(&self) -> Result<Vec<DeviceSyncDevice>> {
        let config = self.require_config()?;
        let repo_path = self.workspace_root.join("repository");
        let current = self.local_device_identity()?;
        if repo_path.join(".git").is_dir() {
            let repo = git2::Repository::open(&repo_path).context("open device sync repository")?;
            if git_repo::origin_matches(&repo, &config) {
                self.ingest_discovered_devices(&repo, git_repo::remote_head(&repo, &config))?;
            }
        }
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
                Some(&err.to_string()),
            )?,
        }
        result
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
        if matches!(resolution, ConflictResolution::KeepLocal) {
            let mut config = self.require_config()?;
            config.last_synced_commit = Some(conflict.remote_commit);
            return self
                .store
                .resolve_device_sync_conflicts_and_save_config_if_clear(
                    &[conflict_id.to_string()],
                    &config,
                );
        }
        let mut config = self.require_config()?;
        let repo_root = self.workspace_root.join("repository");
        let manifest = SyncManifest::read(&repo_root)?;
        let remote = manifest
            .skills
            .get(&conflict.skill_id)
            .context("remote skill no longer exists")?;
        if matches!(resolution, ConflictResolution::KeepBoth) {
            if let Some(local) = self.store.get_skill_by_id(&conflict.skill_id)? {
                let new_id = Uuid::new_v4().to_string();
                let source = PathBuf::from(&local.central_path);
                let duplicate_name = format!("{} (Local)", local.name);
                let destination = unique_skill_path(&self.central_root, &duplicate_name, &new_id);
                manifest::replace_directory(&source, &destination)?;
                let mut duplicate = local;
                duplicate.id = new_id;
                duplicate.name = duplicate_name;
                duplicate.central_path = destination.to_string_lossy().to_string();
                duplicate.updated_at = now_ms();
                self.store.upsert_skill(&duplicate)?;
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
            &repo_root,
            &BTreeSet::new(),
        )?;
        config.last_synced_commit = Some(conflict.remote_commit);
        self.store
            .resolve_device_sync_conflicts_and_save_config_if_clear(
                &[conflict_id.to_string()],
                &config,
            )
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
        let destination = unique_skill_path(&self.central_root, &entry.skill_name, &entry.skill_id);
        manifest::replace_directory(&source, &destination)?;
        let files = manifest::hash_files(&destination)?;
        let now = now_ms();
        self.store
            .upsert_skill(&crate::core::skill_store::SkillRecord {
                id: entry.skill_id,
                name: entry.skill_name,
                description: None,
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
            })?;
        let _ = fs::remove_dir_all(source);
        self.store.remove_device_sync_trash(trash_id)
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
                    ..SyncChangeSummary::default()
                },
                message: "device sync requires conflict resolution".to_string(),
            });
        }
        let token = self.token(&config)?;
        let repo_path = self.workspace_root.join("repository");
        let repo = git_repo::open_or_clone(&repo_path, &config, token.as_deref())?;
        let parent = git_repo::fetch_and_checkout(&repo, &config, token.as_deref())?;
        let mut remote = SyncManifest::read(&repo_path)?;
        let device = self.local_device_identity()?;
        let (base, base_commit) = self.base_manifest(&repo, &config, parent, &device.id)?;
        let export = self.fresh_export()?;
        let mut local = SyncManifest::read(&export)?;
        for (old_id, new_id) in reconcile_identities(&mut local, &remote, &export)? {
            self.store.adopt_skill_id(&old_id, &new_id)?;
        }
        let plan = plan_merge(&base, &local, &remote);
        self.record_conflicts(&plan, &local, &remote, base_commit, parent)?;
        if !plan.conflicts.is_empty() {
            let _ = fs::remove_dir_all(&export);
            return Ok(SyncRunResult {
                status: "conflicts".to_string(),
                commit: config.last_synced_commit,
                changes: summarize(&plan),
                message: "device sync requires conflict resolution".to_string(),
            });
        }
        apply_plan_to_repository(&plan, &local, &export, &mut remote, &repo_path)?;
        remote.write(&repo_path)?;

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
            git_repo::push(&repo, &config, token.as_deref(), oid)?;
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
        self.ingest_discovered_devices(&repo, final_oid)?;
        let mut current = device;
        current.last_commit = final_oid.map(|oid| oid.to_string());
        current.last_seen_at = now_ms();
        self.store.upsert_device_sync_device(&current)?;
        let changes = summarize(&plan);
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
        for device in git_repo::discover_devices_at(repo, remote_head)? {
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
        match config.credential_key.as_deref() {
            Some(key) => {
                let usage =
                    types::CredentialUsage::from_https_remote(config.provider, &config.remote_url)?;
                credentials::resolve_access_token(self.credentials, key, &usage)
            }
            None => Ok(None),
        }
    }

    fn fresh_export(&self) -> Result<PathBuf> {
        let export = self
            .workspace_root
            .join(format!("export-{}", Uuid::new_v4()));
        manifest::export_library(self.store, &export)?;
        Ok(export)
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
        fs::create_dir_all(&self.central_root)?;
        for (id, skill) in &manifest.skills {
            if conflicts.contains(id) {
                continue;
            }
            let source = skill_dir(repo_root, id);
            if !source.is_dir() {
                continue;
            }
            let existing = self.store.get_skill_by_id(id)?;
            let destination = existing
                .as_ref()
                .map(|record| PathBuf::from(&record.central_path))
                .unwrap_or_else(|| unique_skill_path(&self.central_root, &skill.name, id));
            manifest::replace_directory(&source, &destination)?;
            let mut record = manifest::record_from_portable(skill, &destination, now_ms());
            if let Some(existing) = existing {
                record.created_at = existing.created_at;
                record.enabled = existing.enabled;
            }
            self.store.upsert_skill(&record)?;
            self.store.set_skill_tag_names(id, &skill.tags)?;
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
            self.store.delete_skill(id)?;
            self.store.add_device_sync_trash(&TrashEntry {
                id: trash_id,
                skill_id: id.clone(),
                skill_name: record.name,
                trash_path: destination.to_string_lossy().to_string(),
                deleted_at,
                expires_at: deleted_at + 30 * 24 * 60 * 60 * 1000,
            })?;
        }
        Ok(())
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

fn summarize(plan: &MergePlan) -> SyncChangeSummary {
    SyncChangeSummary {
        added: plan.take_local.len() + plan.take_remote.len(),
        updated: plan.merge_files.len(),
        deleted: plan.delete_local.len() + plan.delete_remote.len(),
        conflicted: plan.conflicts.len(),
    }
}

fn unique_skill_path(root: &Path, name: &str, id: &str) -> PathBuf {
    let candidate = root.join(name);
    if !candidate.exists() {
        return candidate;
    }
    let short = id.chars().take(8).collect::<String>();
    root.join(format!("{}-{}", name, short))
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

    fn make_store(root: &Path, name: &str, config: &DeviceSyncConfig) -> SkillStore {
        let store = SkillStore::new(root.join(format!("{name}.db")));
        store.ensure_schema().unwrap();
        store.save_device_sync_config(config).unwrap();
        store
    }

    fn add_skill(store: &SkillStore, central: &Path, id: &str, content: &str) {
        add_skill_in_directory(store, central, id, "one", content);
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
        let first = service.sync().unwrap();
        let history = store.list_device_sync_history(20).unwrap();
        assert_eq!(history.len(), 1);
        let mut device = store.list_device_sync_devices("").unwrap().remove(0);
        device.last_seen_at = 1;
        store.upsert_device_sync_device(&device).unwrap();
        for _ in 0..2 {
            let result = service.sync().unwrap();
            assert_eq!(result.changes, SyncChangeSummary::default());
            assert_eq!(result.commit, first.commit);
        }
        assert_eq!(store.list_device_sync_history(20).unwrap().len(), 1);
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
            Some("success")
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
        let first_commit = first.commit.unwrap();
        let unchanged = service_a.sync().unwrap();
        assert_eq!(unchanged.commit.as_deref(), Some(first_commit.as_str()));
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
                project_path: Some("/computer-b/project".to_string()),
                target_path: "/computer-b/project/.cursor/skills/one".to_string(),
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
            Some("/computer-b/project")
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
        assert_eq!(history.len(), 3);
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
