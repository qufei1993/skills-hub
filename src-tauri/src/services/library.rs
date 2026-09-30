use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::core::central_repo::resolve_central_repo_path;
use crate::core::installer::{import_existing_local_skills_batch, BatchImportCandidate};
use crate::core::onboarding::{
    scan_adopt_directory, AdoptScanCandidate, AdoptScanExclusion, AdoptScanResult,
};
use crate::core::recycle_bin::{DeletionSource, RecycleBinItem, RecycleBinService};
use crate::core::skill_store::{SkillRecord, SkillTargetRecord, TagWithCountRecord};
use crate::core::sync_engine::{deployment_fingerprint, path_for_comparison};

use super::error::{ErrorCode, ServiceError};
use super::install::InstallOutcome;
use super::operation_lock::OperationKind;
use super::skills_hub::SkillsHubService;
use super::types::{SkillSelector, SkillTag};

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct AdoptCandidate {
    pub name: String,
    pub source_path: String,
    pub content_hash: String,
    pub target_path: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct AdoptExclusion {
    pub source_path: String,
    pub reason: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct AdoptPlan {
    pub id: String,
    pub source_path: String,
    pub candidates: Vec<AdoptCandidate>,
    pub excluded: Vec<AdoptExclusion>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct AdoptRequest {
    pub plan_id: String,
    pub confirmed: bool,
}

impl AdoptRequest {
    pub fn unconfirmed(plan_id: impl Into<String>) -> Self {
        Self {
            plan_id: plan_id.into(),
            confirmed: false,
        }
    }

    pub fn confirmed(plan_id: impl Into<String>) -> Self {
        Self {
            plan_id: plan_id.into(),
            confirmed: true,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct AdoptOutcome {
    pub adopted: Vec<InstallOutcome>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub enum TagSelector {
    Id(i64),
    Name(String),
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum TagAction {
    Create {
        name: String,
    },
    Add {
        skill: SkillSelector,
        tags: Vec<String>,
    },
    Remove {
        skill: SkillSelector,
        tags: Vec<String>,
    },
    Set {
        skill: SkillSelector,
        tags: Vec<String>,
    },
    SetIds {
        skill: SkillSelector,
        tag_ids: Vec<i64>,
    },
    Rename {
        tag: TagSelector,
        name: String,
    },
    Delete {
        plan_id: String,
        confirmed: bool,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct TagDeletePlan {
    pub id: String,
    pub tag: SkillTag,
    pub affected_skill_ids: Vec<String>,
    pub affected_skill_count: i64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct TagOutcome {
    pub action: String,
    pub tag: Option<SkillTag>,
    pub skill_id: Option<String>,
    pub tags: Vec<SkillTag>,
    pub affected_skill_count: i64,
    pub changed: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct RemoveTarget {
    pub id: String,
    pub agent: String,
    pub scope: String,
    pub project_path: Option<String>,
    pub target_path: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct RemovePlan {
    pub id: String,
    pub skill_id: String,
    pub skill_name: String,
    pub central_path: String,
    pub targets: Vec<RemoveTarget>,
    pub creates_recycle_bin_entry: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct RemoveRequest {
    pub plan_id: String,
    pub confirmed: bool,
}

impl RemoveRequest {
    pub fn unconfirmed(plan_id: impl Into<String>) -> Self {
        Self {
            plan_id: plan_id.into(),
            confirmed: false,
        }
    }

    pub fn confirmed(plan_id: impl Into<String>) -> Self {
        Self {
            plan_id: plan_id.into(),
            confirmed: true,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RemoveOutcome {
    pub skill_id: String,
    pub item: RecycleBinItem,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
struct AdoptSnapshot {
    operation: &'static str,
    mode: AdoptMode,
    source_path: PathBuf,
    name_override: Option<String>,
    central_root: PathBuf,
    target_paths: Vec<PathBuf>,
    scan: AdoptScanResult,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
enum AdoptMode {
    Container,
    Direct,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
struct TagDeleteSnapshot {
    operation: &'static str,
    tag_id: i64,
    tag_name: String,
    skill_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
struct PathSnapshot {
    path: String,
    fingerprint: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
struct RemoveSnapshot {
    operation: &'static str,
    skill: SkillRecord,
    tags: Vec<String>,
    targets: Vec<SkillTargetRecord>,
    files: Vec<PathSnapshot>,
}

#[derive(Clone, Debug)]
enum LibraryPlanSnapshot {
    Adopt(AdoptSnapshot),
    TagDelete(TagDeleteSnapshot),
    Remove(Box<RemoveSnapshot>),
}

#[cfg(test)]
thread_local! {
    static ADOPT_AFTER_REVALIDATION_HOOK: std::cell::RefCell<Option<Box<dyn FnOnce()>>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
pub(crate) fn set_adopt_after_revalidation_hook(hook: impl FnOnce() + 'static) {
    ADOPT_AFTER_REVALIDATION_HOOK.with(|slot| *slot.borrow_mut() = Some(Box::new(hook)));
}

#[cfg(test)]
fn run_adopt_after_revalidation_hook() {
    let hook = ADOPT_AFTER_REVALIDATION_HOOK.with(|slot| slot.borrow_mut().take());
    if let Some(hook) = hook {
        hook();
    }
}

#[cfg(not(test))]
fn run_adopt_after_revalidation_hook() {}

#[derive(Clone, Debug, Default)]
pub(crate) struct LibraryPlanStore(Arc<Mutex<HashMap<String, LibraryPlanSnapshot>>>);

impl LibraryPlanStore {
    fn insert(&self, id: String, snapshot: LibraryPlanSnapshot) -> Result<(), ServiceError> {
        let mut plans = self
            .0
            .lock()
            .map_err(|_| ServiceError::internal("library plan store is unavailable"))?;
        if plans.len() >= 128 && !plans.contains_key(&id) {
            plans.clear();
        }
        plans.insert(id, snapshot);
        Ok(())
    }

    fn get(&self, id: &str) -> Result<Option<LibraryPlanSnapshot>, ServiceError> {
        Ok(self
            .0
            .lock()
            .map_err(|_| ServiceError::internal("library plan store is unavailable"))?
            .get(id)
            .cloned())
    }

    fn remove(&self, id: &str) {
        if let Ok(mut plans) = self.0.lock() {
            plans.remove(id);
        }
    }
}

impl SkillsHubService {
    pub fn plan_adopt(&self, source: impl AsRef<Path>) -> Result<AdoptPlan, ServiceError> {
        self.plan_adopt_in_mode(source.as_ref(), None, AdoptMode::Container)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn plan_adopt_with_name(
        &self,
        source: impl AsRef<Path>,
        name_override: Option<String>,
    ) -> Result<AdoptPlan, ServiceError> {
        self.plan_adopt_in_mode(source.as_ref(), name_override, AdoptMode::Container)
    }

    pub(crate) fn plan_adopt_direct_with_name(
        &self,
        source: impl AsRef<Path>,
        name_override: Option<String>,
    ) -> Result<AdoptPlan, ServiceError> {
        self.plan_adopt_in_mode(source.as_ref(), name_override, AdoptMode::Direct)
    }

    fn plan_adopt_in_mode(
        &self,
        source: &Path,
        name_override: Option<String>,
        mode: AdoptMode,
    ) -> Result<AdoptPlan, ServiceError> {
        self.ensure_database_compatible()?;
        if name_override
            .as_deref()
            .is_some_and(|name| name.trim().is_empty())
        {
            return Err(invalid_argument("name", "skill name must not be empty"));
        }
        let snapshot = self.capture_adopt(source, name_override, mode)?;
        let id = plan_id(&snapshot)?;
        let plan = adopt_plan(&id, &snapshot);
        self.library_plans
            .insert(id, LibraryPlanSnapshot::Adopt(snapshot))?;
        Ok(plan)
    }

    pub fn adopt(&self, request: AdoptRequest) -> Result<AdoptOutcome, ServiceError> {
        let snapshot = match self.library_plans.get(&request.plan_id)? {
            Some(LibraryPlanSnapshot::Adopt(snapshot)) => snapshot,
            _ => return Err(plan_stale()),
        };
        if !request.confirmed {
            return Err(ServiceError::new(
                ErrorCode::ConfirmationRequired,
                "adopting skills requires explicit confirmation",
                json!({ "candidate_count": snapshot.scan.candidates.len() }),
            ));
        }
        let _operation_lock = self.begin_write(OperationKind::Install)?;
        validate_planned_source(&snapshot.source_path)?;
        let fresh = self
            .capture_adopt(
                &snapshot.source_path,
                snapshot.name_override.clone(),
                snapshot.mode,
            )
            .map_err(map_adopt_revalidation_error)?;
        if fresh != snapshot || plan_id(&fresh)? != request.plan_id {
            return Err(plan_stale());
        }
        run_adopt_after_revalidation_hook();
        let candidates = snapshot
            .scan
            .candidates
            .iter()
            .zip(&snapshot.target_paths)
            .map(|(candidate, target_path)| BatchImportCandidate {
                name: candidate.name.clone(),
                source_path: candidate.path.clone(),
                target_path: target_path.clone(),
                expected_content_hash: candidate.content_hash.clone(),
            })
            .collect::<Vec<_>>();
        let installed = import_existing_local_skills_batch(self.store(), &candidates)
            .map_err(map_batch_adopt_error)?;
        let mut adopted = Vec::with_capacity(installed.len());
        for installed in installed {
            let skill = self.show_skill(SkillSelector::Id(installed.skill_id.clone()))?;
            adopted.push(InstallOutcome {
                action: super::install::InstallAction::Installed,
                pending_targets: Vec::new(),
                id: skill.id,
                name: installed.name,
                central_path: installed.central_path.to_string_lossy().into_owned(),
                content_hash: installed.content_hash,
                source: skill.source,
                targets: skill.targets,
            });
        }
        self.library_plans.remove(&request.plan_id);
        Ok(AdoptOutcome { adopted })
    }

    pub fn plan_tag_delete(&self, selector: TagSelector) -> Result<TagDeletePlan, ServiceError> {
        self.ensure_database_compatible()?;
        let tag = self.resolve_tag(&selector)?;
        let snapshot = self.capture_tag_delete(tag.id)?.ok_or_else(plan_stale)?;
        let id = plan_id(&snapshot)?;
        let plan = TagDeletePlan {
            id: id.clone(),
            tag: SkillTag {
                id: snapshot.tag_id,
                name: snapshot.tag_name.clone(),
            },
            affected_skill_count: snapshot.skill_ids.len() as i64,
            affected_skill_ids: snapshot.skill_ids.clone(),
        };
        self.library_plans
            .insert(id, LibraryPlanSnapshot::TagDelete(snapshot))?;
        Ok(plan)
    }

    pub fn apply_tag_action(&self, action: TagAction) -> Result<TagOutcome, ServiceError> {
        self.ensure_database_compatible()?;
        if let TagAction::Delete {
            plan_id: delete_plan_id,
            confirmed,
        } = &action
        {
            let snapshot = match self.library_plans.get(delete_plan_id)? {
                Some(LibraryPlanSnapshot::TagDelete(snapshot)) => snapshot,
                _ => return Err(plan_stale()),
            };
            if !confirmed {
                return Err(ServiceError::new(
                    ErrorCode::ConfirmationRequired,
                    "deleting a tag requires explicit confirmation",
                    json!({
                        "affected_skill_count": snapshot.skill_ids.len(),
                        "tag": snapshot.tag_name
                    }),
                ));
            }
            let _operation_lock = self.begin_write(OperationKind::Update)?;
            let fresh = self
                .capture_tag_delete(snapshot.tag_id)?
                .ok_or_else(plan_stale)?;
            if fresh != snapshot || plan_id(&fresh)? != *delete_plan_id {
                return Err(plan_stale());
            }
            self.store()
                .delete_tag(snapshot.tag_id)
                .map_err(|_| ServiceError::internal("failed to delete tag"))?;
            self.library_plans.remove(delete_plan_id);
            return Ok(tag_outcome(
                "delete",
                Some(snapshot.tag_id),
                Some(snapshot.tag_name),
                None,
                snapshot.skill_ids.len() as i64,
                true,
            ));
        }
        let _operation_lock = self.begin_write(OperationKind::Update)?;
        match action {
            TagAction::Create { name } => {
                let name = self.available_tag_name(&name, None)?;
                let tag = self
                    .store()
                    .create_tag(&name)
                    .map_err(|_| ServiceError::internal("failed to create tag"))?;
                Ok(tag_outcome(
                    "create",
                    Some(tag.id),
                    Some(tag.name),
                    None,
                    0,
                    true,
                ))
            }
            TagAction::Add { skill, tags } => self.mutate_skill_tag_names("add", skill, tags),
            TagAction::Remove { skill, tags } => self.mutate_skill_tag_names("remove", skill, tags),
            TagAction::Set { skill, tags } => self.mutate_skill_tag_names("set", skill, tags),
            TagAction::SetIds { skill, tag_ids } => {
                let skill = self.show_skill(skill)?;
                let before = self
                    .store()
                    .get_skill_tags(&skill.id)
                    .map_err(tag_read_error)?;
                let known = self
                    .store()
                    .list_tags_with_counts()
                    .map_err(tag_read_error)?;
                let known_ids = known.iter().map(|tag| tag.id).collect::<HashSet<_>>();
                if tag_ids.iter().any(|id| !known_ids.contains(id)) {
                    return Err(invalid_argument("tag_ids", "one or more tags do not exist"));
                }
                self.store()
                    .set_skill_tags(&skill.id, &tag_ids)
                    .map_err(|_| ServiceError::internal("failed to set skill tags"))?;
                let after = self
                    .store()
                    .get_skill_tags(&skill.id)
                    .map_err(tag_read_error)?;
                Ok(skill_tag_outcome("set", &skill.id, before, after))
            }
            TagAction::Rename { tag, name } => {
                let current = self.resolve_tag(&tag)?;
                let name = self.available_tag_name(&name, Some(current.id))?;
                let changed = current.name != name;
                let renamed = self
                    .store()
                    .rename_tag(current.id, &name)
                    .map_err(|_| ServiceError::internal("failed to rename tag"))?;
                Ok(tag_outcome(
                    "rename",
                    Some(renamed.id),
                    Some(renamed.name),
                    None,
                    current.skill_count,
                    changed,
                ))
            }
            TagAction::Delete { .. } => unreachable!(),
        }
    }

    pub fn plan_remove(&self, selector: SkillSelector) -> Result<RemovePlan, ServiceError> {
        self.ensure_database_compatible()?;
        let skill = self.show_skill(selector)?;
        let snapshot = self.capture_remove(&skill.id)?;
        let id = plan_id(&snapshot)?;
        let plan = remove_plan(&id, &snapshot);
        self.library_plans
            .insert(id, LibraryPlanSnapshot::Remove(Box::new(snapshot)))?;
        Ok(plan)
    }

    pub fn remove(&self, request: RemoveRequest) -> Result<RemoveOutcome, ServiceError> {
        let snapshot = match self.library_plans.get(&request.plan_id)? {
            Some(LibraryPlanSnapshot::Remove(snapshot)) => *snapshot,
            _ => return Err(plan_stale()),
        };
        if !request.confirmed {
            return Err(ServiceError::new(
                ErrorCode::ConfirmationRequired,
                "removing a skill requires explicit confirmation",
                json!({ "skill_id": snapshot.skill.id }),
            ));
        }
        let _operation_lock = self.begin_write(OperationKind::Delete)?;
        validate_remove_paths(&snapshot.files)?;
        let fresh = self
            .capture_remove(&snapshot.skill.id)
            .map_err(map_remove_revalidation_error)?;
        if fresh != snapshot || plan_id(&fresh)? != request.plan_id {
            return Err(plan_stale());
        }
        let _device_sync_guard = if self
            .store()
            .get_device_sync_config()
            .map_err(|_| ServiceError::internal("failed to inspect device sync state"))?
            .is_some()
        {
            Some(
                crate::core::device_sync::try_lock_device_sync().map_err(|_| {
                    ServiceError::new(
                        ErrorCode::OperationBusy,
                        "another Skills Hub operation is already in progress",
                        json!({ "operation": "delete" }),
                    )
                })?,
            )
        } else {
            None
        };
        let item = RecycleBinService::new(self.store(), self.paths().recycle_bin_dir.clone())
            .archive(&snapshot.skill.id, DeletionSource::Manual, now_ms())
            .map_err(map_confirmed_archive_error)?;
        self.library_plans.remove(&request.plan_id);
        Ok(RemoveOutcome {
            skill_id: snapshot.skill.id,
            item,
        })
    }

    fn capture_adopt(
        &self,
        source: &Path,
        name_override: Option<String>,
        mode: AdoptMode,
    ) -> Result<AdoptSnapshot, ServiceError> {
        let central = resolve_central_repo_path(self.paths(), self.store())
            .map_err(|_| ServiceError::internal("failed to resolve the central skill library"))?;
        let central_root = path_for_comparison(&central)
            .map_err(|_| ServiceError::internal("failed to resolve the central skill library"))?;
        if mode == AdoptMode::Direct && !has_regular_skill_manifest(source)? {
            return Err(ServiceError::new(
                ErrorCode::InvalidSource,
                "SKILL_INVALID|missing_skill_md",
                json!({ "reason": "missing_skill_md" }),
            ));
        }
        let skills = self
            .store()
            .list_skills()
            .map_err(|_| ServiceError::internal("failed to inspect managed skills"))?;
        let targets = self
            .store()
            .list_all_skill_target_paths()
            .map_err(|_| ServiceError::internal("failed to inspect managed targets"))?;
        let mut scan =
            scan_adopt_directory(source, &central, &skills, &targets).map_err(|error| {
                if error.chain().any(|cause| {
                    cause.downcast_ref::<std::io::Error>().is_some()
                        || cause.downcast_ref::<walkdir::Error>().is_some()
                }) {
                    ServiceError::internal("failed to inspect the adopt source")
                } else {
                    ServiceError::new(
                        ErrorCode::InvalidSource,
                        "adopt source is not a safe skill directory",
                        json!({ "argument": "source" }),
                    )
                }
            })?;
        if let Some(name) = name_override.as_deref() {
            super::install::validate_skill_name(name)?;
            if scan.candidates.len() != 1 {
                return Err(invalid_argument(
                    "name",
                    "a custom name requires exactly one adopt candidate",
                ));
            }
            let name = name.trim();
            if central.join(name).exists() {
                return Err(ServiceError::new(
                    ErrorCode::TargetConflict,
                    "the central skill path already contains unmanaged content",
                    json!({
                        "path": central.join(name).to_string_lossy(),
                        "reason": "unmanaged_target"
                    }),
                ));
            }
            let collision = skills
                .iter()
                .any(|skill| skill.name.to_lowercase() == name.to_lowercase());
            if collision {
                return Err(invalid_argument(
                    "name",
                    "a managed skill already uses this name",
                ));
            }
            scan.candidates[0].name = name.to_string();
        } else {
            let managed_names = skills
                .iter()
                .map(|skill| skill.name.to_lowercase())
                .collect::<HashSet<_>>();
            let mut planned_names = HashSet::new();
            let mut candidates = Vec::new();
            for candidate in std::mem::take(&mut scan.candidates) {
                let key = candidate.name.to_lowercase();
                let reason = if managed_names.contains(&key) || !planned_names.insert(key) {
                    Some("managed_skill")
                } else if central.join(&candidate.name).exists() {
                    Some("central_conflict")
                } else {
                    None
                };
                if let Some(reason) = reason {
                    scan.excluded.push(AdoptScanExclusion {
                        path: candidate.path,
                        reason: reason.to_string(),
                    });
                } else {
                    candidates.push(candidate);
                }
            }
            scan.candidates = candidates;
        }
        let target_paths = scan
            .candidates
            .iter()
            .map(|candidate| central_root.join(&candidate.name))
            .collect::<Vec<_>>();
        Ok(AdoptSnapshot {
            operation: "adopt",
            mode,
            source_path: scan.root.clone(),
            name_override,
            central_root,
            target_paths,
            scan,
        })
    }

    fn capture_tag_delete(&self, tag_id: i64) -> Result<Option<TagDeleteSnapshot>, ServiceError> {
        let tag = self
            .store()
            .list_tags_with_counts()
            .map_err(tag_read_error)?
            .into_iter()
            .find(|tag| tag.id == tag_id);
        let Some(tag) = tag else {
            return Ok(None);
        };
        let skill_ids = self
            .store()
            .list_tag_skill_ids(tag_id)
            .map_err(tag_read_error)?;
        Ok(Some(TagDeleteSnapshot {
            operation: "tag_delete",
            tag_id,
            tag_name: tag.name,
            skill_ids,
        }))
    }

    fn capture_remove(&self, skill_id: &str) -> Result<RemoveSnapshot, ServiceError> {
        let skill = self
            .store()
            .get_skill_by_id(skill_id)
            .map_err(|_| ServiceError::internal("failed to inspect the skill"))?
            .ok_or_else(|| ServiceError::skill_not_found(skill_id))?;
        let mut tags = self
            .store()
            .get_skill_tags(skill_id)
            .map_err(tag_read_error)?
            .into_iter()
            .map(|tag| tag.name)
            .collect::<Vec<_>>();
        tags.sort_by_key(|name| name.to_lowercase());
        let mut targets = self
            .store()
            .list_skill_targets(skill_id)
            .map_err(|_| ServiceError::internal("failed to inspect skill targets"))?;
        targets.sort_by(|left, right| left.id.cmp(&right.id));
        let recycle = RecycleBinService::new(self.store(), self.paths().recycle_bin_dir.clone());
        recycle
            .validate_archive(&skill, &targets)
            .map_err(map_archive_error)?;
        let mut paths = vec![PathBuf::from(&skill.central_path)];
        paths.extend(
            targets
                .iter()
                .filter(|target| target.status != "disabled")
                .map(|target| PathBuf::from(&target.target_path)),
        );
        let files = paths
            .into_iter()
            .map(|path| {
                deployment_fingerprint(&path)
                    .map(|fingerprint| PathSnapshot {
                        path: path.to_string_lossy().into_owned(),
                        fingerprint,
                    })
                    .map_err(|_| ServiceError::internal("failed to inspect skill files"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(RemoveSnapshot {
            operation: "remove",
            skill,
            tags,
            targets,
            files,
        })
    }

    fn resolve_tag(&self, selector: &TagSelector) -> Result<TagWithCountRecord, ServiceError> {
        let tags = self
            .store()
            .list_tags_with_counts()
            .map_err(tag_read_error)?;
        let found = match selector {
            TagSelector::Id(id) => tags.into_iter().find(|tag| tag.id == *id),
            TagSelector::Name(name) => {
                let normalized = normalized_tag(name)?;
                tags.into_iter()
                    .find(|tag| tag.name.to_lowercase() == normalized.to_lowercase())
            }
        };
        found.ok_or_else(|| {
            ServiceError::new(
                ErrorCode::InvalidArgument,
                "tag was not found",
                json!({ "argument": "tag" }),
            )
        })
    }

    fn available_tag_name(
        &self,
        name: &str,
        current_id: Option<i64>,
    ) -> Result<String, ServiceError> {
        let normalized = normalized_tag(name)?;
        let collision = self
            .store()
            .list_tags_with_counts()
            .map_err(tag_read_error)?
            .into_iter()
            .any(|tag| {
                Some(tag.id) != current_id && tag.name.to_lowercase() == normalized.to_lowercase()
            });
        if collision {
            return Err(invalid_argument("name", "tag already exists"));
        }
        Ok(normalized)
    }

    fn canonical_tag_names(&self, names: Vec<String>) -> Result<Vec<String>, ServiceError> {
        let existing = self
            .store()
            .list_tags_with_counts()
            .map_err(tag_read_error)?;
        let mut seen = HashSet::new();
        let mut result = Vec::new();
        for name in names {
            let normalized = normalized_tag(&name)?;
            let key = normalized.to_lowercase();
            if !seen.insert(key.clone()) {
                continue;
            }
            result.push(
                existing
                    .iter()
                    .find(|tag| tag.name.to_lowercase() == key)
                    .map_or(normalized, |tag| tag.name.clone()),
            );
        }
        Ok(result)
    }

    fn mutate_skill_tag_names(
        &self,
        action: &str,
        selector: SkillSelector,
        requested: Vec<String>,
    ) -> Result<TagOutcome, ServiceError> {
        let skill = self.show_skill(selector)?;
        let before = self
            .store()
            .get_skill_tags(&skill.id)
            .map_err(tag_read_error)?;
        let requested = self.canonical_tag_names(requested)?;
        let requested_keys = requested
            .iter()
            .map(|name| name.to_lowercase())
            .collect::<HashSet<_>>();
        let next = match action {
            "add" => before
                .iter()
                .map(|tag| tag.name.clone())
                .chain(requested)
                .collect(),
            "remove" => before
                .iter()
                .filter(|tag| !requested_keys.contains(&tag.name.to_lowercase()))
                .map(|tag| tag.name.clone())
                .collect(),
            "set" => requested,
            _ => unreachable!(),
        };
        let next = self.canonical_tag_names(next)?;
        self.store()
            .set_skill_tag_names(&skill.id, &next)
            .map_err(|_| ServiceError::internal("failed to update skill tags"))?;
        let after = self
            .store()
            .get_skill_tags(&skill.id)
            .map_err(tag_read_error)?;
        Ok(skill_tag_outcome(action, &skill.id, before, after))
    }
}

fn adopt_plan(id: &str, snapshot: &AdoptSnapshot) -> AdoptPlan {
    AdoptPlan {
        id: id.to_string(),
        source_path: snapshot.source_path.to_string_lossy().into_owned(),
        candidates: snapshot
            .scan
            .candidates
            .iter()
            .zip(&snapshot.target_paths)
            .map(|(candidate, target)| adopt_candidate(candidate, target))
            .collect(),
        excluded: snapshot.scan.excluded.iter().map(adopt_exclusion).collect(),
    }
}

fn adopt_candidate(candidate: &AdoptScanCandidate, target: &Path) -> AdoptCandidate {
    AdoptCandidate {
        name: candidate.name.clone(),
        source_path: candidate.path.to_string_lossy().into_owned(),
        content_hash: candidate.content_hash.clone(),
        target_path: target.to_string_lossy().into_owned(),
    }
}

fn adopt_exclusion(exclusion: &AdoptScanExclusion) -> AdoptExclusion {
    AdoptExclusion {
        source_path: exclusion.path.to_string_lossy().into_owned(),
        reason: exclusion.reason.clone(),
    }
}

fn remove_plan(id: &str, snapshot: &RemoveSnapshot) -> RemovePlan {
    RemovePlan {
        id: id.to_string(),
        skill_id: snapshot.skill.id.clone(),
        skill_name: snapshot.skill.name.clone(),
        central_path: snapshot.skill.central_path.clone(),
        targets: snapshot
            .targets
            .iter()
            .map(|target| RemoveTarget {
                id: target.id.clone(),
                agent: target.tool.clone(),
                scope: target.scope.clone(),
                project_path: target.project_path.clone(),
                target_path: target.target_path.clone(),
            })
            .collect(),
        creates_recycle_bin_entry: true,
    }
}

fn plan_id(snapshot: &impl Serialize) -> Result<String, ServiceError> {
    let encoded = serde_json::to_vec(snapshot)
        .map_err(|_| ServiceError::internal("failed to create an immutable operation plan"))?;
    Ok(hex::encode(Sha256::digest(encoded)))
}

fn normalized_tag(name: &str) -> Result<String, ServiceError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(invalid_argument("name", "tag name must not be empty"));
    }
    Ok(name.to_string())
}

fn invalid_argument(argument: &str, message: &str) -> ServiceError {
    ServiceError::new(
        ErrorCode::InvalidArgument,
        message,
        json!({ "argument": argument }),
    )
}

fn has_regular_skill_manifest(path: &Path) -> Result<bool, ServiceError> {
    let stat = match std::fs::symlink_metadata(path.join("SKILL.md")) {
        Ok(metadata) => Ok(Some(
            metadata.is_file() && !metadata.file_type().is_symlink(),
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    };
    classify_direct_manifest_stat(stat)
}

pub(crate) fn classify_direct_manifest_stat(
    stat: std::io::Result<Option<bool>>,
) -> Result<bool, ServiceError> {
    match stat {
        Ok(Some(regular)) => Ok(regular),
        Ok(None) => Ok(false),
        Err(_) => Err(ServiceError::internal(
            "failed to inspect the adopt manifest",
        )),
    }
}

fn validate_planned_source(path: &Path) -> Result<(), ServiceError> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => Ok(()),
        Ok(_) => Err(plan_stale()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Err(plan_stale()),
        Err(_) => Err(ServiceError::internal("failed to inspect the adopt source")),
    }
}

fn map_adopt_revalidation_error(error: ServiceError) -> ServiceError {
    match error.code {
        ErrorCode::InvalidSource | ErrorCode::InvalidArgument | ErrorCode::TargetConflict => {
            plan_stale()
        }
        _ => error,
    }
}

pub(crate) fn map_batch_adopt_error(error: anyhow::Error) -> ServiceError {
    if error
        .chain()
        .any(|cause| cause.is::<crate::core::installer::AdoptPlanStaleError>())
    {
        plan_stale()
    } else {
        ServiceError::internal("failed to adopt local skills")
    }
}

fn validate_remove_paths(paths: &[PathSnapshot]) -> Result<(), ServiceError> {
    for expected in paths {
        let path = Path::new(&expected.path);
        match std::fs::symlink_metadata(path) {
            Ok(metadata) => {
                if let Some(fingerprint) = &expected.fingerprint {
                    let expected_directory = fingerprint.starts_with("dir:");
                    let expected_symlink = fingerprint.starts_with("link:");
                    if (expected_directory
                        && (!metadata.is_dir() || metadata.file_type().is_symlink()))
                        || (expected_symlink && !metadata.file_type().is_symlink())
                    {
                        return Err(plan_stale());
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if expected.fingerprint.is_some() {
                    return Err(plan_stale());
                }
            }
            Err(_) => return Err(ServiceError::internal("failed to inspect skill files")),
        }
    }
    Ok(())
}

fn map_remove_revalidation_error(error: ServiceError) -> ServiceError {
    match error.code {
        ErrorCode::SkillNotFound | ErrorCode::TargetConflict => plan_stale(),
        _ => error,
    }
}

fn plan_stale() -> ServiceError {
    ServiceError::new(
        ErrorCode::PlanStale,
        "the operation plan no longer matches current state",
        json!({}),
    )
}

fn tag_read_error(_: anyhow::Error) -> ServiceError {
    ServiceError::internal("failed to read tags")
}

fn tag_outcome(
    action: &str,
    id: Option<i64>,
    name: Option<String>,
    skill_id: Option<String>,
    affected_skill_count: i64,
    changed: bool,
) -> TagOutcome {
    TagOutcome {
        action: action.to_string(),
        tag: id.zip(name).map(|(id, name)| SkillTag { id, name }),
        skill_id,
        tags: Vec::new(),
        affected_skill_count,
        changed,
    }
}

fn skill_tag_outcome(
    action: &str,
    skill_id: &str,
    before: Vec<crate::core::skill_store::TagRecord>,
    after: Vec<crate::core::skill_store::TagRecord>,
) -> TagOutcome {
    let before_ids = before.iter().map(|tag| tag.id).collect::<Vec<_>>();
    let after_ids = after.iter().map(|tag| tag.id).collect::<Vec<_>>();
    TagOutcome {
        action: action.to_string(),
        tag: None,
        skill_id: Some(skill_id.to_string()),
        tags: after
            .into_iter()
            .map(|tag| SkillTag {
                id: tag.id,
                name: tag.name,
            })
            .collect(),
        affected_skill_count: i64::from(before_ids != after_ids),
        changed: before_ids != after_ids,
    }
}

fn map_archive_error(error: anyhow::Error) -> ServiceError {
    let message = error.to_string();
    if let Some(path) = message.strip_prefix("TARGET_MODIFIED|") {
        return ServiceError::new(
            ErrorCode::TargetConflict,
            "a managed target contains user changes",
            json!({ "path": path, "reason": "target_modified" }),
        );
    }
    if let Some(path) = message.strip_prefix("UNSAFE_TARGET|") {
        return ServiceError::new(
            ErrorCode::TargetConflict,
            "a target is not safe to remove",
            json!({ "path": path, "reason": "unmanaged_target" }),
        );
    }
    ServiceError::internal("failed to archive the skill in the recycle bin")
}

fn map_confirmed_archive_error(error: anyhow::Error) -> ServiceError {
    let message = error.to_string();
    if message.starts_with("TARGET_MODIFIED|")
        || message.starts_with("UNSAFE_TARGET|")
        || message.contains("Skill not found")
        || message.contains("Skill content is missing")
        || message.contains("unsafe central Skill path")
    {
        plan_stale()
    } else {
        ServiceError::internal("failed to archive the skill in the recycle bin")
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
