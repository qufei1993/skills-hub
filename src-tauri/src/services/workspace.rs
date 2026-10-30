use std::collections::HashSet;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::core::content_hash::hash_dir_strict;
use crate::core::skill_store::SkillTargetRecord;
use crate::core::sync_engine::deployment_fingerprint;
pub use crate::core::workspace::{Preset, PresetMember};

use super::deployment::{DeploymentPlan, DeploymentRequest, DeploymentScope, DeploymentTarget};
use super::error::{ErrorCode, ServiceError};
use super::operation_lock::OperationKind;
use super::skills_hub::SkillsHubService;
use super::types::SkillSelector;

type Result<T> = std::result::Result<T, ServiceError>;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplicationRequest {
    pub preset_id: Option<String>,
    pub members: Vec<PresetMember>,
    pub scopes: Vec<DeploymentScope>,
    #[serde(default)]
    pub remove: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplicationItem {
    pub skill_id: String,
    pub name: String,
    pub tools: Vec<String>,
    pub scope: DeploymentScope,
    pub targets: Vec<DeploymentTarget>,
    pub action: String,
    pub status: String,
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conflict: Option<ApplicationConflict>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ApplicationConflict {
    pub agent: String,
    pub path: String,
    pub reason: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplicationRecord {
    pub id: String,
    pub preset: Option<Preset>,
    pub created_at: i64,
    pub confirmed: bool,
    pub remove: bool,
    pub items: Vec<ApplicationItem>,
    /// Response-only count of receipts still available for an undo review.
    /// Protected targets are included; only the explicit undo preview checks safety.
    #[serde(skip_deserializing, skip_serializing_if = "Option::is_none")]
    pub undo_target_count: Option<usize>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct Receipt {
    target: SkillTargetRecord,
    fingerprint: String,
    source_hash: String,
    #[serde(default)]
    undone: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct Journal {
    record: ApplicationRecord,
    expected: Vec<Option<String>>,
    receipts: Vec<Vec<Receipt>>,
}

impl Journal {
    fn into_record(mut self) -> ApplicationRecord {
        self.record.undo_target_count = Some(if self.record.remove {
            0
        } else {
            self.receipts
                .iter()
                .zip(&self.record.items)
                .filter(|(_, item)| item.status != "undone")
                .flat_map(|(receipts, _)| receipts)
                .filter(|receipt| !receipt.undone)
                .count()
        });
        self.record
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UndoItem {
    pub target_id: String,
    pub skill_id: String,
    pub path: String,
    pub tool: String,
    pub scope: DeploymentScope,
    pub can_remove: bool,
    pub reason: String,
}

impl SkillsHubService {
    pub fn list_presets(&self) -> Result<Vec<Preset>> {
        self.ensure_database_compatible()?;
        self.store().workspace_list("preset").map_err(storage)
    }

    pub fn save_preset(&self, mut preset: Preset) -> Result<Preset> {
        let _lock = self.begin_write(OperationKind::Deploy)?;
        preset.name = preset.name.trim().to_string();
        if preset.name.is_empty() || preset.members.is_empty() {
            return Err(invalid("name and at least one member are required"));
        }
        let all = self.list_presets()?;
        if all
            .iter()
            .any(|p| p.id != preset.id && p.name.to_lowercase() == preset.name.to_lowercase())
        {
            return Err(invalid("preset name already exists"));
        }
        let existing = all.iter().find(|p| p.id == preset.id);
        if existing.map(|p| p.revision).unwrap_or(0) != preset.revision {
            return Err(stale());
        }
        self.validate_members(&preset.members)?;
        if preset.id.is_empty() {
            preset.id = Uuid::new_v4().to_string();
        }
        preset.revision += 1;
        self.store()
            .workspace_save("preset", &preset.id, &preset)
            .map_err(storage)?;
        Ok(preset)
    }

    pub fn delete_preset(&self, id: &str, revision: u64) -> Result<()> {
        let _lock = self.begin_write(OperationKind::Delete)?;
        let current: Preset = self
            .store()
            .workspace_get("preset", id)
            .map_err(storage)?
            .ok_or_else(stale)?;
        if current.revision != revision {
            return Err(stale());
        }
        self.store().workspace_delete("preset", id).map_err(storage)
    }

    fn validate_members(&self, members: &[PresetMember]) -> Result<()> {
        let agents = self.list_agents()?.agents;
        let mut ids = HashSet::new();
        for member in members {
            if member.skill_id.is_empty()
                || !ids.insert(&member.skill_id)
                || member.tools.is_empty()
                || member.tools.iter().collect::<HashSet<_>>().len() != member.tools.len()
                || member
                    .tools
                    .iter()
                    .any(|key| !agents.iter().any(|agent| &agent.key == key))
            {
                return Err(invalid(
                    "members require unique identities and explicit registered tools",
                ));
            }
        }
        Ok(())
    }

    pub fn preview_application(&self, request: ApplicationRequest) -> Result<ApplicationRecord> {
        let _lock = self.begin_write(OperationKind::Deploy)?;
        if request.members.is_empty()
            || request.scopes.is_empty()
            || request
                .scopes
                .iter()
                .any(|s| matches!(s, DeploymentScope::Global))
                && request.scopes.len() > 1
        {
            return Err(invalid("choose global scope or one or more projects"));
        }
        self.validate_members(&request.members)?;
        let preset = request
            .preset_id
            .as_ref()
            .map(|id| {
                self.store()
                    .workspace_get::<Preset>("preset", id)
                    .map_err(storage)?
                    .ok_or_else(stale)
            })
            .transpose()?;
        let mut journal = Journal {
            record: ApplicationRecord {
                id: Uuid::new_v4().to_string(),
                preset,
                created_at: chrono::Utc::now().timestamp_millis(),
                confirmed: false,
                remove: request.remove,
                items: vec![],
                undo_target_count: None,
            },
            expected: vec![],
            receipts: vec![],
        };
        let mut seen = HashSet::new();
        for scope in &request.scopes {
            for member in &request.members {
                let name = self
                    .show_skill(SkillSelector::Id(member.skill_id.clone()))
                    .map(|s| s.name)
                    .unwrap_or_else(|_| member.skill_id.clone());
                let mut item = ApplicationItem {
                    skill_id: member.skill_id.clone(),
                    name,
                    tools: member.tools.clone(),
                    scope: scope.clone(),
                    targets: vec![],
                    action: "blocked".into(),
                    status: "pending".into(),
                    reason: None,
                    conflict: None,
                };
                let fingerprint = match self.application_plan(&item, request.remove) {
                    Ok(plan) => {
                        item.scope = plan.scope.clone();
                        let identity = format!(
                            "{}:{}",
                            member.skill_id,
                            serde_json::to_string(&plan.scope).map_err(|_| stale())?
                        );
                        if !seen.insert(identity) {
                            continue;
                        }
                        item.action = self.application_action(&plan)?;
                        item.targets = plan.targets.clone();
                        Some(plan.workspace_fingerprint())
                    }
                    Err(error) => {
                        item.status = "blocked".into();
                        item.reason = Some(error.code.as_str().into());
                        item.conflict = application_conflict(&error);
                        None
                    }
                };
                journal.record.items.push(item);
                journal.expected.push(fingerprint);
                journal.receipts.push(vec![]);
            }
        }
        self.save_journal(&journal)?;
        Ok(journal.into_record())
    }

    fn application_plan(&self, item: &ApplicationItem, remove: bool) -> Result<DeploymentPlan> {
        let skill = self.show_skill(SkillSelector::Id(item.skill_id.clone()))?;
        if !remove && !skill.enabled {
            return Err(invalid("skill is disabled; enable it explicitly first"));
        }
        let request = DeploymentRequest {
            skill: SkillSelector::Id(item.skill_id.clone()),
            agents: item.tools.clone(),
            scope: item.scope.clone(),
            overwrite: false,
            overwrite_if_same_content: false,
        };
        if remove {
            self.plan_undeploy(request)
        } else {
            self.plan_deploy(request)
        }
    }

    fn application_action(&self, plan: &DeploymentPlan) -> Result<String> {
        if plan.undeploy {
            return Ok("remove".into());
        }
        let rows = self
            .store()
            .list_skill_targets(&plan.skill_id)
            .map_err(storage)?;
        if plan.targets.iter().all(|t| {
            t.exists
                && t.agents.iter().all(|agent| {
                    rows.iter().any(|r| {
                        r.tool == *agent
                            && r.target_path == t.path.to_string_lossy()
                            && r.status == "ok"
                    })
                })
        }) {
            Ok("existing".into())
        } else if plan.targets.iter().any(|t| {
            rows.iter()
                .any(|r| r.target_path == t.path.to_string_lossy())
        }) {
            Ok("repair".into())
        } else {
            Ok("add".into())
        }
    }

    fn save_journal(&self, journal: &Journal) -> Result<()> {
        self.store()
            .workspace_save("application", &journal.record.id, journal)
            .map_err(storage)
    }

    fn journal(&self, id: &str) -> Result<Journal> {
        self.store()
            .workspace_get("application", id)
            .map_err(storage)?
            .ok_or_else(stale)
    }

    pub fn application_history(&self) -> Result<Vec<ApplicationRecord>> {
        self.ensure_database_compatible()?;
        if self
            .store()
            .workspace_applications_need_pruning()
            .map_err(storage)?
        {
            match self.begin_write(OperationKind::Delete) {
                Ok(_lock) => self
                    .store()
                    .workspace_prune_applications()
                    .map_err(storage)?,
                // Retention can wait for the next read/write. Viewing history
                // must remain available while auto-update or the CLI is active.
                Err(error) if error.code == ErrorCode::OperationBusy => {}
                Err(error) => return Err(error),
            }
        }
        Ok(self
            .store()
            .workspace_list::<Journal>("application")
            .map_err(storage)?
            .into_iter()
            .filter(|j| j.record.confirmed)
            .map(Journal::into_record)
            .collect())
    }

    pub fn discard_application(&self, id: &str) -> Result<()> {
        let _lock = self.begin_write(OperationKind::Delete)?;
        let Some(journal) = self
            .store()
            .workspace_get::<Journal>("application", id)
            .map_err(storage)?
        else {
            return Ok(());
        };
        // A delayed close callback must never remove a confirmed application or
        // evidence of a write, including an execution whose response was lost.
        if journal.record.confirmed
            || journal
                .record
                .items
                .iter()
                .any(|item| !matches!(item.status.as_str(), "pending" | "blocked"))
            || journal.receipts.iter().any(|receipts| !receipts.is_empty())
        {
            return Ok(());
        }
        self.store()
            .workspace_delete_application(id)
            .map_err(storage)
    }

    pub fn recheck_application(&self, id: &str) -> Result<ApplicationRecord> {
        let _lock = self.begin_write(OperationKind::Deploy)?;
        let mut journal = self.journal(id)?;
        for i in 0..journal.record.items.len() {
            if matches!(journal.record.items[i].status.as_str(), "done" | "undone") {
                continue;
            }
            match self.application_plan(&journal.record.items[i], journal.record.remove) {
                Ok(plan) => {
                    journal.record.items[i].scope = plan.scope.clone();
                    journal.expected[i] = Some(plan.workspace_fingerprint());
                    journal.record.items[i].targets = plan.targets.clone();
                    journal.record.items[i].action = self.application_action(&plan)?;
                    journal.record.items[i].status = "pending".into();
                    journal.record.items[i].reason = None;
                    journal.record.items[i].conflict = None;
                }
                Err(error) => {
                    journal.expected[i] = None;
                    journal.record.items[i].action = "blocked".into();
                    journal.record.items[i].status = "blocked".into();
                    journal.record.items[i].reason = Some(error.code.as_str().into());
                    journal.record.items[i].conflict = application_conflict(&error);
                    journal.record.items[i].targets.clear();
                }
            }
        }
        self.save_journal(&journal)?;
        Ok(journal.into_record())
    }

    pub fn execute_application(&self, id: &str) -> Result<ApplicationRecord> {
        let _lock = self.begin_write(OperationKind::Deploy)?;
        let mut journal = self.journal(id)?;
        // Validate every pending item before changing any item. Plans are server-owned.
        let mut plans = Vec::new();
        for (i, item) in journal.record.items.iter().enumerate() {
            if matches!(
                item.status.as_str(),
                "done" | "undone" | "blocked" | "interrupted"
            ) {
                plans.push(None);
                continue;
            }
            if item.status == "running" {
                plans.push(None);
                continue;
            }
            let plan = self
                .application_plan(item, journal.record.remove)
                .map_err(|_| stale())?;
            if Some(plan.workspace_fingerprint()) != journal.expected[i] {
                return Err(stale());
            }
            plans.push(Some(plan));
        }
        journal.record.confirmed = true;
        self.save_journal(&journal)?;
        for (i, plan) in plans.into_iter().enumerate() {
            if journal.record.items[i].status == "running" {
                journal.record.items[i].status = "interrupted".into();
                journal.record.items[i].reason = Some("INTERRUPTED_RECHECK".into());
                journal.record.items[i].conflict = None;
                self.save_journal(&journal)?;
                continue;
            }
            let Some(plan) = plan else {
                continue;
            };
            let before = self
                .store()
                .list_skill_targets(&plan.skill_id)
                .map_err(storage)?;
            journal.record.items[i].status = "running".into();
            journal.record.items[i].reason = None;
            journal.record.items[i].conflict = None;
            self.save_journal(&journal)?;
            let result = if journal.record.items[i].action == "existing" {
                // Explicit reuse is still an operation, and protects previous additions.
                Ok(())
            } else {
                self.execute_deployment_plan(plan).map(|_| ())
            };
            match result {
                Ok(()) => {
                    let item = &journal.record.items[i];
                    if !journal.record.remove {
                        let skill = self.show_skill(SkillSelector::Id(item.skill_id.clone()))?;
                        let hash =
                            hash_dir_strict(Path::new(&skill.central_path)).map_err(storage)?;
                        for target in self
                            .store()
                            .list_skill_targets(&item.skill_id)
                            .map_err(storage)?
                        {
                            let same_scope = match &item.scope {
                                DeploymentScope::Global => {
                                    target.scope == "global" && target.project_path.is_none()
                                }
                                DeploymentScope::Project(path) => {
                                    target.scope == "project"
                                        && target.project_path.as_deref() == path.to_str()
                                }
                            };
                            if same_scope
                                && item.targets.iter().any(|t| {
                                    t.path.to_string_lossy() == target.target_path
                                        && t.agents.contains(&target.tool)
                                })
                            {
                                self.store()
                                    .workspace_save("target-use", &target.id, &journal.record.id)
                                    .map_err(storage)?;
                                if !before.iter().any(|old| old.id == target.id) {
                                    journal.receipts[i].push(Receipt {
                                        fingerprint: target_fingerprint(Path::new(
                                            &target.target_path,
                                        ))?,
                                        target,
                                        source_hash: hash.clone(),
                                        undone: false,
                                    });
                                }
                            }
                        }
                    }
                    journal.record.items[i].status = "done".into();
                    journal.record.items[i].reason = None;
                    journal.record.items[i].conflict = None;
                }
                Err(error) => {
                    journal.record.items[i].status = "failed".into();
                    journal.record.items[i].reason = Some(error.code.as_str().into());
                    journal.record.items[i].conflict = application_conflict(&error);
                }
            }
            self.save_journal(&journal)?;
        }
        Ok(journal.into_record())
    }

    fn application_undo_items(&self, id: &str) -> Result<Vec<UndoItem>> {
        self.ensure_database_compatible()?;
        let journal = self.journal(id)?;
        let mut result = vec![];
        for (index, receipt) in journal
            .receipts
            .iter()
            .enumerate()
            .flat_map(|(index, receipts)| receipts.iter().map(move |receipt| (index, receipt)))
        {
            let row = &receipt.target;
            let scope = row
                .project_path
                .as_ref()
                .map(|p| DeploymentScope::Project(p.into()))
                .unwrap_or_default();
            let reason = if receipt.undone || journal.record.items[index].status == "undone" {
                Some("alreadyUndone".into())
            } else {
                self.undo_block_reason(id, receipt)?
            };
            result.push(UndoItem {
                target_id: row.id.clone(),
                skill_id: row.skill_id.clone(),
                path: row.target_path.clone(),
                tool: row.tool.clone(),
                scope,
                can_remove: reason.is_none(),
                reason: reason.unwrap_or_else(|| "newTarget".into()),
            });
        }
        Ok(result)
    }

    pub fn preview_application_undo(&self, id: &str) -> Result<Vec<UndoItem>> {
        let _lock = self.begin_write(OperationKind::Undeploy)?;
        let result = self.application_undo_items(id)?;
        self.store()
            .workspace_save("undo-preview", id, &result)
            .map_err(storage)?;
        Ok(result)
    }

    fn undo_block_reason(&self, id: &str, receipt: &Receipt) -> Result<Option<String>> {
        let rows = self
            .store()
            .list_skill_targets(&receipt.target.skill_id)
            .map_err(storage)?;
        if !rows.iter().any(|r| r == &receipt.target) {
            return Ok(Some("targetChanged".into()));
        }
        let last_use: Option<String> = self
            .store()
            .workspace_get("target-use", &receipt.target.id)
            .map_err(storage)?;
        if last_use.as_deref() != Some(id) {
            return Ok(Some("laterUse".into()));
        }
        let skill = match self.show_skill(SkillSelector::Id(receipt.target.skill_id.clone())) {
            Ok(skill) => skill,
            Err(_) => return Ok(Some("targetChanged".into())),
        };
        if hash_dir_strict(Path::new(&skill.central_path))
            .ok()
            .as_ref()
            != Some(&receipt.source_hash)
            || target_fingerprint(Path::new(&receipt.target.target_path))
                .ok()
                .as_ref()
                != Some(&receipt.fingerprint)
        {
            return Ok(Some("contentChanged".into()));
        }
        Ok(None)
    }

    pub fn undo_application(&self, id: &str) -> Result<Vec<UndoItem>> {
        let _lock = self.begin_write(OperationKind::Undeploy)?;
        let expected: Vec<UndoItem> = self
            .store()
            .workspace_get("undo-preview", id)
            .map_err(storage)?
            .ok_or_else(stale)?;
        let mut journal = self.journal(id)?;
        let current = self.application_undo_items(id)?;
        if serde_json::to_value(&current).map_err(|_| stale())?
            != serde_json::to_value(&expected).map_err(|_| stale())?
        {
            return Err(stale());
        }
        // Group shared paths: removing a logical reference must preserve other users.
        for item in &current {
            if !item.can_remove {
                continue;
            }
            let rows = self
                .store()
                .list_skill_targets(&item.skill_id)
                .map_err(storage)?;
            let Some(row) = rows.iter().find(|r| r.id == item.target_id) else {
                continue;
            };
            if rows
                .iter()
                .any(|other| other.id != row.id && other.target_path == row.target_path)
            {
                self.store()
                    .commit_deployment_targets(&[], std::slice::from_ref(row), || Ok(()))
                    .map_err(storage)?;
            } else {
                let request = DeploymentRequest {
                    skill: SkillSelector::Id(item.skill_id.clone()),
                    agents: vec![item.tool.clone()],
                    scope: item.scope.clone(),
                    overwrite: false,
                    overwrite_if_same_content: false,
                };
                let plan = self.plan_undeploy(request)?;
                self.execute_deployment_plan(plan)?;
            }
            for (index, receipts) in journal.receipts.iter_mut().enumerate() {
                if let Some(receipt) = receipts.iter_mut().find(|r| r.target.id == item.target_id) {
                    receipt.undone = true;
                    if receipts.iter().all(|receipt| receipt.undone) {
                        journal.record.items[index].status = "undone".into();
                    }
                }
            }
            // Persist each completed removal before a later target can fail.
            self.save_journal(&journal)?;
        }
        self.store()
            .workspace_delete("undo-preview", id)
            .map_err(storage)?;
        Ok(current)
    }
}

fn application_conflict(error: &ServiceError) -> Option<ApplicationConflict> {
    if error.code != ErrorCode::TargetConflict {
        return None;
    }
    let field = |key| {
        error
            .details
            .get(key)?
            .as_str()
            .filter(|value| !value.trim().is_empty())
            .map(str::to_owned)
    };
    Some(ApplicationConflict {
        agent: field("agent")?,
        path: field("path")?,
        reason: field("reason")?,
    })
}

fn target_fingerprint(path: &Path) -> Result<String> {
    let meta = std::fs::symlink_metadata(path).map_err(storage)?;
    #[cfg(unix)]
    let identity = {
        use std::os::unix::fs::MetadataExt;
        format!("{}:{}", meta.dev(), meta.ino())
    };
    #[cfg(not(unix))]
    let identity = format!("{:?}", meta.created().map_err(storage)?);
    Ok(format!(
        "{}:{:?}:{:?}",
        identity,
        meta.modified().map_err(storage)?,
        deployment_fingerprint(path).map_err(storage)?
    ))
}
fn storage(_: impl std::fmt::Display) -> ServiceError {
    ServiceError::internal(
        "workspace state could not be saved or inspected; retry after checking storage",
    )
}
fn invalid(message: &str) -> ServiceError {
    ServiceError::new(ErrorCode::InvalidArgument, message, json!({}))
}
fn stale() -> ServiceError {
    ServiceError::new(
        ErrorCode::PlanStale,
        "state changed; preview the operation again",
        json!({}),
    )
}

impl SkillsHubService {
    pub fn preview_content(
        &self,
        id: &str,
        restore: bool,
    ) -> Result<crate::core::content_recovery::ContentPreview> {
        let _lock = self.begin_write(OperationKind::Update)?;
        crate::core::content_recovery::preview(self.paths(), self.store(), id, restore)
            .map_err(content_error)
    }
    pub fn recoverable_skills(&self) -> Result<Vec<String>> {
        self.ensure_database_compatible()?;
        Ok(self
            .store()
            .workspace_list::<crate::core::content_recovery::Recovery>("content-recovery")
            .map_err(storage)?
            .into_iter()
            .map(|r| r.skill_id)
            .collect())
    }
    pub fn execute_content(&self, id: &str) -> Result<serde_json::Value> {
        let _lock = self.begin_write(OperationKind::Update)?;
        let plan: crate::core::content_recovery::ContentPreview = self
            .store()
            .workspace_get("content-preview", id)
            .map_err(storage)?
            .ok_or_else(stale)?;
        if plan.restore {
            let pending = crate::core::content_recovery::restore(self.paths(), self.store(), id)
                .map_err(content_error)?;
            Ok(json!({"changed":true,"pending_targets":pending}))
        } else {
            let result =
                crate::core::installer::update_from_preview(self.paths(), self.store(), id)
                    .map_err(content_error)?;
            Ok(json!({"changed":result.changed,"pending_targets":result.pending_targets}))
        }
    }
}
pub(super) fn content_error(error: anyhow::Error) -> ServiceError {
    if let Some(conflict) = error.downcast_ref::<crate::core::installer::UpdateTargetConflict>() {
        return ServiceError::new(
            ErrorCode::TargetConflict,
            "a managed update target has changed",
            json!({
                "skill_id": conflict.skill_id, "agent": conflict.agent,
                "path": conflict.path, "reason": conflict.reason,
            }),
        );
    }
    let message = error.to_string();
    if message.contains("PLAN_STALE") {
        stale()
    } else if message.contains("MODIFIED") {
        ServiceError::new(
            ErrorCode::TargetConflict,
            "content changed; preserve edits before updating or restoring",
            json!({}),
        )
    } else {
        ServiceError::new(
            ErrorCode::InvalidSource,
            "source or recovery content is unavailable; check the source and try again",
            json!({}),
        )
    }
}

impl SkillsHubService {
    pub fn preview_source_repair(
        &self,
        id: &str,
        kind: &str,
        reference: &str,
        subpath: Option<String>,
    ) -> Result<crate::core::content_recovery::SourceRepairPlan> {
        let _lock = self.begin_write(OperationKind::Update)?;
        crate::core::content_recovery::preview_source_repair(
            self.paths(),
            self.store(),
            id,
            kind,
            reference,
            subpath,
        )
        .map_err(content_error)
    }
    pub fn apply_source_repair(&self, id: &str) -> Result<()> {
        let _lock = self.begin_write(OperationKind::Update)?;
        crate::core::content_recovery::apply_source_repair(self.paths(), self.store(), id)
            .map_err(content_error)
    }
}
