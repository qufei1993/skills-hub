use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::Result as AnyResult;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::core::content_hash::{hash_dir, hash_dir_strict};
use crate::core::content_recovery::{compare_directories, FileChange};
use crate::core::skill_store::SkillRecord;
use crate::core::sync_engine::{
    deployment_fingerprint, path_for_comparison, paths_overlap, PreparedDeployment,
};

use super::deployment::{
    agent_root, target_location, DeploymentPlan, DeploymentRequest, DeploymentScope,
};
use super::error::{ErrorCode, ServiceError};
use super::operation_lock::OperationKind;
use super::skills_hub::SkillsHubService;
use super::types::SkillSelector;

type Result<T> = std::result::Result<T, ServiceError>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceConflictChoice {
    Central,
    Tool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolutionOption {
    pub targets: Vec<String>,
    pub blocked: Option<ResolutionBlock>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolutionBlock {
    pub code: String,
    pub path: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceConflictPreview {
    pub id: String,
    pub skill_id: String,
    pub name: String,
    pub central_path: String,
    pub source_path: String,
    pub changes: Vec<FileChange>,
    pub central: ResolutionOption,
    pub tool: ResolutionOption,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceConflictOutcome {
    pub id: String,
    pub skill_id: String,
    pub name: String,
    pub choice: SourceConflictChoice,
    pub targets: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct StoredPreview {
    preview: SourceConflictPreview,
    fingerprint: String,
}

struct Capture {
    record: SkillRecord,
    source: PathBuf,
    central_hash: String,
    source_hash: String,
    central: Result<Vec<DeploymentPlan>>,
    tool: Result<Vec<DeploymentPlan>>,
    fingerprint: String,
}

fn option(plans: &Result<Vec<DeploymentPlan>>) -> ResolutionOption {
    match plans {
        Ok(plans) => ResolutionOption {
            targets: plans
                .iter()
                .flat_map(|p| {
                    p.targets
                        .iter()
                        .map(|t| t.path.to_string_lossy().into_owned())
                })
                .collect(),
            blocked: None,
        },
        Err(error) => ResolutionOption {
            targets: vec![],
            blocked: Some(ResolutionBlock {
                code: error.code.as_str().into(),
                path: error.details["path"].as_str().map(str::to_string),
            }),
        },
    }
}

impl SkillsHubService {
    pub fn preview_source_conflict(&self, source_path: &str) -> Result<SourceConflictPreview> {
        let _lock = self.begin_write(OperationKind::Deploy)?;
        let capture = self.capture_source_conflict(source_path)?;
        let preview = SourceConflictPreview {
            id: Uuid::new_v4().to_string(),
            skill_id: capture.record.id.clone(),
            name: capture.record.name.clone(),
            central_path: capture.record.central_path.clone(),
            source_path: capture.source.to_string_lossy().into_owned(),
            changes: compare_directories(
                Path::new(&capture.record.central_path),
                &capture.source,
                true,
            )
            .map_err(inspect_error)?,
            central: option(&capture.central),
            tool: option(&capture.tool),
        };
        // Bind displayed text as well as metadata to the version being confirmed.
        if self.capture_source_conflict(source_path)?.fingerprint != capture.fingerprint {
            return Err(stale());
        }
        for prior in self
            .store()
            .workspace_list::<StoredPreview>("source-conflict-preview")
            .map_err(inspect_error)?
        {
            if prior.preview.skill_id == preview.skill_id {
                self.store()
                    .workspace_delete("source-conflict-preview", &prior.preview.id)
                    .map_err(inspect_error)?;
            }
        }
        self.store()
            .workspace_save(
                "source-conflict-preview",
                &preview.id,
                &StoredPreview {
                    preview: preview.clone(),
                    fingerprint: capture.fingerprint,
                },
            )
            .map_err(inspect_error)?;
        Ok(preview)
    }

    fn capture_source_conflict(&self, source_path: &str) -> Result<Capture> {
        self.ensure_database_compatible()?;
        let source = target_location(Path::new(source_path)).map_err(inspect_error)?;
        let skills = self.store().list_skills().map_err(inspect_error)?;
        let mut owners = skills.iter().filter(|skill| {
            skill
                .external_local_source()
                .is_some_and(|path| target_location(Path::new(path)).ok().as_ref() == Some(&source))
        });
        let record = owners.next().cloned().ok_or_else(stale)?;
        if owners.next().is_some() {
            return Err(blocked(&source, "shared_source"));
        }
        for path in [source.as_path(), Path::new(&record.central_path)] {
            let meta = std::fs::symlink_metadata(path).map_err(inspect_error)?;
            if !meta.is_dir() || meta.file_type().is_symlink() {
                return Err(stale());
            }
        }
        let source_hash = hash_dir_strict(&source).map_err(inspect_error)?;
        let central_hash =
            hash_dir_strict(Path::new(&record.central_path)).map_err(inspect_error)?;
        let agents = self.list_agents()?.agents;
        let mut source_requests = BTreeMap::<String, DeploymentRequest>::new();
        for agent in agents
            .iter()
            .filter(|agent| agent.enabled && agent.detected)
        {
            let global = agent_root(agent, None)
                .ok()
                .and_then(|root| target_location(&root.join(&record.name)).ok());
            let scope = if global.as_ref() == Some(&source) {
                Some(DeploymentScope::Global)
            } else if agent.supports_project_scope {
                let relative = Path::new(&agent.project_skills_dir);
                let count = relative.components().count();
                if count == 0
                    || relative.is_absolute()
                    || relative
                        .components()
                        .any(|part| !matches!(part, std::path::Component::Normal(_)))
                {
                    None
                } else {
                    source.ancestors().nth(count + 1).and_then(|root| {
                        (agent_root(agent, Some(root))
                            .ok()
                            .and_then(|path| target_location(&path.join(&record.name)).ok())
                            .as_ref()
                            == Some(&source))
                        .then(|| DeploymentScope::Project(root.to_path_buf()))
                    })
                }
            } else {
                None
            };
            if let Some(scope) = scope {
                let key = serde_json::to_string(&scope).map_err(inspect_error)?;
                let request = source_requests.entry(key).or_insert_with(|| {
                    let mut r = DeploymentRequest::global(record.id.clone(), [] as [&str; 0]);
                    r.scope = scope;
                    r
                });
                request.agents.push(agent.key.clone());
            }
        }
        if source_requests.len() != 1 {
            return Err(blocked(&source, "source_tool_unavailable"));
        }
        let rows = self
            .store()
            .list_skill_targets(&record.id)
            .map_err(inspect_error)?;
        let mut all_requests = source_requests.clone();
        for row in rows.iter().filter(|row| row.status != "disabled") {
            let scope = match row.scope.as_str() {
                "global" => DeploymentScope::Global,
                "project" => DeploymentScope::Project(PathBuf::from(
                    row.project_path.as_deref().ok_or_else(stale)?,
                )),
                _ => return Err(stale()),
            };
            let key = serde_json::to_string(&scope).map_err(inspect_error)?;
            let request = all_requests.entry(key).or_insert_with(|| {
                let mut r = DeploymentRequest::global(record.id.clone(), [] as [&str; 0]);
                r.scope = scope;
                r
            });
            if !request.agents.contains(&row.tool) {
                request.agents.push(row.tool.clone());
            }
        }
        let build = |requests: BTreeMap<String, DeploymentRequest>,
                     content: &Path|
         -> Result<Vec<DeploymentPlan>> {
            let manifest =
                std::fs::symlink_metadata(content.join("SKILL.md")).map_err(inspect_error)?;
            if !record.enabled || !manifest.is_file() || manifest.file_type().is_symlink() {
                return Err(blocked(content, "invalid_selected_content"));
            }
            if content != Path::new(&record.central_path) {
                for other in skills.iter().filter(|other| other.id != record.id) {
                    for protected in std::iter::once(other.central_path.as_str())
                        .chain(other.external_local_source())
                    {
                        if paths_overlap(Path::new(&record.central_path), Path::new(protected))
                            .map_err(inspect_error)?
                        {
                            return Err(blocked(Path::new(protected), "protected_central_overlap"));
                        }
                    }
                }
                for (_, path) in self
                    .store()
                    .list_all_skill_target_paths()
                    .map_err(inspect_error)?
                {
                    let location = target_location(Path::new(&path)).map_err(inspect_error)?;
                    let central = path_for_comparison(Path::new(&record.central_path))
                        .map_err(inspect_error)?;
                    // Compare the deployment's location, not the contents of its
                    // final symlink, which normally points to this central copy.
                    if location == central
                        || location.starts_with(&central)
                        || central.starts_with(&location)
                    {
                        return Err(blocked(&location, "protected_central_overlap"));
                    }
                }
            }
            requests
                .into_values()
                .map(|request| {
                    self.plan_source_conflict_deployment(
                        request,
                        self.show_skill(SkillSelector::Id(record.id.clone()))?,
                        content,
                    )
                })
                .collect()
        };
        let central = build(source_requests, Path::new(&record.central_path));
        let tool = build(all_requests, &source);
        let target_fingerprints = rows
            .iter()
            .filter(|row| row.status != "disabled")
            .map(|row| deployment_fingerprint(Path::new(&row.target_path)))
            .collect::<AnyResult<Vec<_>>>()
            .map_err(inspect_error)?;
        let baselines = rows
            .iter()
            .map(|row| {
                self.store()
                    .get_setting(&format!("device_sync.target_baseline.{}", row.id))
            })
            .collect::<AnyResult<Vec<_>>>()
            .map_err(inspect_error)?;
        let plan_state = |plans: &Result<Vec<DeploymentPlan>>| match plans {
            Ok(plans) => json!(plans
                .iter()
                .map(DeploymentPlan::workspace_fingerprint)
                .collect::<Vec<_>>()),
            Err(error) => json!(error),
        };
        let fingerprint = format!(
            "{:x}",
            Sha256::digest(
                serde_json::to_vec(&json!([
                    skills,
                    agents,
                    rows,
                    baselines,
                    target_fingerprints,
                    central_hash,
                    source_hash,
                    plan_state(&central),
                    plan_state(&tool)
                ]))
                .map_err(inspect_error)?
            )
        );
        Ok(Capture {
            record,
            source,
            central_hash,
            source_hash,
            central,
            tool,
            fingerprint,
        })
    }

    pub fn resolve_source_conflict(
        &self,
        id: &str,
        choice: SourceConflictChoice,
    ) -> Result<SourceConflictOutcome> {
        let _lock = self.begin_write(OperationKind::Deploy)?;
        if let Some(result) = self
            .store()
            .workspace_get::<SourceConflictOutcome>("source-conflict-result", id)
            .map_err(inspect_error)?
        {
            return if result.choice == choice {
                Ok(result)
            } else {
                Err(stale())
            };
        }
        let stored: StoredPreview = self
            .store()
            .workspace_get("source-conflict-preview", id)
            .map_err(inspect_error)?
            .ok_or_else(stale)?;
        let mut capture = self
            .capture_source_conflict(&stored.preview.source_path)
            .map_err(|_| stale())?;
        if capture.fingerprint != stored.fingerprint {
            return Err(stale());
        }
        let mut plans = match choice {
            SourceConflictChoice::Central => std::mem::replace(&mut capture.central, Ok(vec![]))?,
            SourceConflictChoice::Tool => std::mem::replace(&mut capture.tool, Ok(vec![]))?,
        };
        let before = capture.record;
        let central = PathBuf::from(&before.central_path);
        let selected_hash = match choice {
            SourceConflictChoice::Central => capture.central_hash.clone(),
            SourceConflictChoice::Tool => capture.source_hash.clone(),
        };
        let mut seen = HashSet::new();
        for target in plans.iter().flat_map(|plan| &plan.targets) {
            let location = target_location(&target.path).map_err(inspect_error)?;
            if !seen.insert(location.clone()) {
                return Err(blocked(&location, "overlapping_targets"));
            }
        }
        if self
            .capture_source_conflict(&stored.preview.source_path)?
            .fingerprint
            != stored.fingerprint
        {
            return Err(stale());
        }
        let mut central_replacement = None;
        let mut prepared: Vec<Vec<PreparedDeployment>> = vec![];
        let mut stage = "prepare";
        let outcome = SourceConflictOutcome {
            id: id.into(),
            skill_id: before.id.clone(),
            name: before.name.clone(),
            choice,
            targets: plans
                .iter()
                .flat_map(|p| {
                    p.targets
                        .iter()
                        .map(|t| t.path.to_string_lossy().into_owned())
                })
                .collect(),
        };
        let result = (|| -> AnyResult<()> {
            if choice == SourceConflictChoice::Tool {
                let replacement = PreparedDeployment::prepare_snapshot(
                    &capture.source,
                    &central,
                    Some(format!("dir:{}", capture.central_hash)),
                )?;
                central_replacement = Some(replacement);
                central_replacement.as_mut().unwrap().activate()?;
            }
            for plan in &plans {
                prepared.push(plan.prepare_targets()?);
            }
            stage = "activate";
            for group in &mut prepared {
                for replacement in group {
                    replacement.activate()?;
                }
            }
            let mut targets = vec![];
            for (plan, group) in plans.iter_mut().zip(&prepared) {
                targets.extend(plan.target_upserts(group)?);
            }
            let mut current = before.clone();
            current.source_ref = Some(current.central_path.clone());
            current.source_subpath = None;
            current.source_revision = None;
            current.content_hash = Some(hash_dir(&central)?);
            current.description = crate::core::installer::parse_skill_md(&central.join("SKILL.md"))
                .and_then(|(_, description)| description);
            current.status = "ok".into();
            current.updated_at = chrono::Utc::now().timestamp_millis();
            let verify = || -> AnyResult<()> {
                anyhow::ensure!(hash_dir_strict(&central)? == selected_hash, "PLAN_STALE");
                if let Some(replacement) = &central_replacement {
                    replacement.verify_unchanged()?;
                }
                for group in &prepared {
                    for replacement in group {
                        replacement.verify_unchanged()?;
                    }
                }
                Ok(())
            };
            verify()?;
            stage = "commit";
            self.store().commit_source_conflict(
                (&before, &current),
                &targets,
                id,
                &serde_json::to_value(&outcome)?,
                verify,
            )?;
            Ok(())
        })();
        if let Err(error) = result {
            let mut recovery = vec![];
            for group in prepared.iter_mut().rev() {
                for replacement in group.iter_mut().rev() {
                    if replacement.rollback().is_err() {
                        recovery.push(replacement.backup_path().map(Path::to_path_buf));
                    }
                }
            }
            if let Some(replacement) = &mut central_replacement {
                if replacement.rollback().is_err() {
                    recovery.push(replacement.backup_path().map(Path::to_path_buf));
                }
            }
            let stale_state = error.to_string().contains("PLAN_STALE")
                || error.to_string().starts_with("TARGET_MODIFIED|");
            return Err(ServiceError::new(
                if stale_state {
                    ErrorCode::PlanStale
                } else {
                    ErrorCode::InternalError
                },
                "source conflict resolution did not complete",
                json!({"stage":stage,"recovery":recovery}),
            ));
        }
        for group in &mut prepared {
            for replacement in group {
                replacement.commit();
            }
        }
        if let Some(replacement) = &mut central_replacement {
            replacement.commit();
        }
        Ok(outcome)
    }
}

fn inspect_error(error: impl std::fmt::Display) -> ServiceError {
    log::warn!("source conflict inspection failed: {error}");
    ServiceError::internal("failed to inspect the source conflict")
}
fn stale() -> ServiceError {
    ServiceError::new(
        ErrorCode::PlanStale,
        "content changed; review the conflict again",
        json!({}),
    )
}
fn blocked(path: &Path, reason: &str) -> ServiceError {
    ServiceError::new(
        ErrorCode::TargetConflict,
        "source conflict cannot be safely resolved",
        json!({"path":path,"reason":reason}),
    )
}
