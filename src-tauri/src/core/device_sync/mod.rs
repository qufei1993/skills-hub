pub mod credentials;
mod git_repo;
pub mod manifest;
pub mod merge;
pub mod oauth;
pub mod providers;
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
        let conflicts = self.store.list_device_sync_conflicts()?;
        let repository_head_commit = config.as_ref().and_then(|item| {
            let repo = git2::Repository::open(self.workspace_root.join("repository")).ok()?;
            if !git_repo::origin_matches(&repo, item) {
                return None;
            }
            git_repo::remote_head(&repo, item).map(|oid| oid.to_string())
        });
        Ok(SyncStatus {
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
            auto_check: config.as_ref().map_or(true, |item| item.auto_check),
            auto_sync: config.as_ref().is_some_and(|item| item.auto_sync),
            last_synced_commit: config
                .as_ref()
                .and_then(|item| item.last_synced_commit.clone()),
            repository_head_commit,
            pending_local_changes: 0,
            conflict_count: conflicts.len(),
            last_run_status: history.first().map(|item| item.status.clone()),
            last_run_at: history.first().and_then(|item| item.finished_at),
        })
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
        let base = self.base_manifest(&repo, &config)?;
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
            self.store.resolve_device_sync_conflict(conflict_id)?;
            return Ok(());
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
        self.store.resolve_device_sync_conflict(conflict_id)?;
        config.last_synced_commit = Some(conflict.remote_commit);
        self.store.save_device_sync_config(&config)
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
        let token = self.token(&config)?;
        let repo_path = self.workspace_root.join("repository");
        let repo = git_repo::open_or_clone(&repo_path, &config, token.as_deref())?;
        let parent = git_repo::fetch_and_checkout(&repo, &config, token.as_deref())?;
        let mut remote = SyncManifest::read(&repo_path)?;
        let base = self.base_manifest(&repo, &config)?;
        let export = self.fresh_export()?;
        let mut local = SyncManifest::read(&export)?;
        for (old_id, new_id) in reconcile_identities(&mut local, &remote, &export)? {
            self.store.adopt_skill_id(&old_id, &new_id)?;
        }
        let plan = plan_merge(&base, &local, &remote);
        self.record_conflicts(&plan, &local, &remote, &config, parent)?;
        apply_plan_to_repository(&plan, &local, &export, &mut remote, &repo_path)?;
        remote.write(&repo_path)?;

        let device = self.local_device_identity()?;
        let message = format!(
            "Sync Skills Hub library\n\nSkills-Hub-Device-ID: {}\nSkills-Hub-Device-Name: {}",
            device.id, device.name
        );
        let device_is_known = match parent {
            Some(parent) => git_repo::discover_devices_at(&repo, parent)?
                .iter()
                .any(|known| known.id == device.id),
            None => false,
        };
        let commit = if device_is_known {
            git_repo::commit_all(&repo, &message, parent)?
        } else {
            git_repo::commit_all_allow_empty(&repo, &message, parent)?
        };
        let final_oid = if let Some(oid) = commit {
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
    ) -> Result<SyncManifest> {
        match config.last_synced_commit.as_deref() {
            Some(value) => match git2::Oid::from_str(value) {
                Ok(oid) if repo.find_commit(oid).is_ok() => git_repo::manifest_at(repo, oid),
                _ => Ok(SyncManifest::empty()),
            },
            None => Ok(SyncManifest::empty()),
        }
    }

    fn record_conflicts(
        &self,
        plan: &MergePlan,
        local: &SyncManifest,
        remote: &SyncManifest,
        config: &DeviceSyncConfig,
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
                base_commit: config.last_synced_commit.clone(),
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
        let path = central.join("one");
        fs::create_dir_all(&path).unwrap();
        fs::write(path.join("SKILL.md"), content).unwrap();
        store
            .upsert_skill(&SkillRecord {
                id: id.to_string(),
                name: "One".to_string(),
                description: Some("test".to_string()),
                source_type: "git".to_string(),
                source_ref: Some("https://example/source.git".to_string()),
                source_subpath: Some("skills/one".to_string()),
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
    fn full_device_sync_flow_preserves_local_relations_and_recovers_deletions() {
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
        assert!(store_b.list_device_sync_history(20).unwrap().len() >= 4);

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
