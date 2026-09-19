use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::core::content_hash::{hash_dir_for_sync_conflict, hash_dir_strict};
use crate::core::skill_store::SkillTargetRecord;
use crate::core::sync_engine::{
    deployment_fingerprint, path_for_comparison, DeploymentParentSnapshot, PreparedDeployment,
    SyncMode,
};

use super::error::{ErrorCode, ServiceError};
use super::operation_lock::OperationKind;
use super::skills_hub::SkillsHubService;
use super::types::{Agent, SkillSelector};

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "path", rename_all = "lowercase")]
pub enum DeploymentScope {
    #[default]
    Global,
    Project(PathBuf),
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct DeploymentRequest {
    pub skill: SkillSelector,
    pub agents: Vec<String>,
    pub scope: DeploymentScope,
    #[serde(skip)]
    pub(crate) overwrite: bool,
    #[serde(skip)]
    pub(crate) overwrite_if_same_content: bool,
}

impl DeploymentRequest {
    pub fn global<S, I, A>(skill: S, agents: I) -> Self
    where
        S: Into<SkillSelector>,
        I: IntoIterator<Item = A>,
        A: Into<String>,
    {
        Self {
            skill: skill.into(),
            agents: agents.into_iter().map(Into::into).collect(),
            scope: DeploymentScope::Global,
            overwrite: false,
            overwrite_if_same_content: false,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct DeploymentTarget {
    pub agents: Vec<String>,
    pub path: PathBuf,
    pub mode: SyncMode,
    pub exists: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct DeploymentPlan {
    pub skill_id: String,
    pub scope: DeploymentScope,
    pub targets: Vec<DeploymentTarget>,
    pub undeploy: bool,
    #[serde(skip)]
    request: DeploymentRequest,
    #[serde(skip)]
    snapshot: Value,
    #[serde(skip)]
    source: PathBuf,
    #[serde(skip)]
    rows: Vec<SkillTargetRecord>,
    #[serde(skip)]
    fingerprints: Vec<Option<String>>,
    #[serde(skip)]
    parent_snapshots: Vec<DeploymentParentSnapshot>,
}

#[derive(Clone, Debug, Serialize)]
pub struct DeploymentOutcome {
    pub skill_id: String,
    pub scope: DeploymentScope,
    pub targets: Vec<DeploymentTarget>,
    pub undeploy: bool,
}

impl SkillsHubService {
    pub fn plan_deploy(&self, request: DeploymentRequest) -> Result<DeploymentPlan, ServiceError> {
        self.build_deployment_plan(request, false)
    }

    pub fn plan_undeploy(
        &self,
        request: DeploymentRequest,
    ) -> Result<DeploymentPlan, ServiceError> {
        self.build_deployment_plan(request, true)
    }

    pub fn deploy(&self, request: DeploymentRequest) -> Result<DeploymentOutcome, ServiceError> {
        let _lock = self.begin_write(OperationKind::Deploy)?;
        let plan = self.build_deployment_plan(request, false)?;
        self.execute_deployment_plan(plan)
    }

    pub fn undeploy(&self, request: DeploymentRequest) -> Result<DeploymentOutcome, ServiceError> {
        let _lock = self.begin_write(OperationKind::Undeploy)?;
        let plan = self.build_deployment_plan(request, true)?;
        self.execute_deployment_plan(plan)
    }

    pub fn apply_deployment_plan(
        &self,
        plan: DeploymentPlan,
    ) -> Result<DeploymentOutcome, ServiceError> {
        self.ensure_database_compatible()?;
        if plan.snapshot["undeploy"].as_bool() != Some(plan.undeploy) {
            return Err(stale());
        }
        let _lock = self.begin_write(if plan.undeploy {
            OperationKind::Undeploy
        } else {
            OperationKind::Deploy
        })?;
        let current = self
            .build_deployment_plan(plan.request.clone(), plan.undeploy)
            .map_err(|_| stale())?;
        if current.snapshot != plan.snapshot
            || current.targets != plan.targets
            || current.skill_id != plan.skill_id
            || current.scope != plan.scope
        {
            return Err(stale());
        }
        self.execute_deployment_plan(current)
    }

    fn build_deployment_plan(
        &self,
        request: DeploymentRequest,
        undeploy: bool,
    ) -> Result<DeploymentPlan, ServiceError> {
        self.ensure_database_compatible()?;
        if request.agents.is_empty() || request.agents.iter().any(|agent| agent.trim().is_empty()) {
            return Err(ServiceError::new(
                ErrorCode::InvalidArgument,
                "at least one explicit Agent is required",
                json!({"argument":"agent"}),
            ));
        }
        let skill = self.show_skill(request.skill.clone())?;
        if Path::new(&skill.name).components().count() != 1
            || !matches!(
                Path::new(&skill.name).components().next(),
                Some(Component::Normal(_))
            )
        {
            return Err(ServiceError::new(
                ErrorCode::InvalidArgument,
                "skill name is not a safe directory name",
                json!({"argument":"skill"}),
            ));
        }
        let source = PathBuf::from(&skill.central_path);
        let source_hash = hash_dir_strict(&source)
            .map_err(|_| ServiceError::internal("failed to inspect the central skill"))?;
        let conflict_hash = hash_dir_for_sync_conflict(&source)
            .map_err(|_| ServiceError::internal("failed to inspect the central skill"))?;
        let agents = self.list_agents()?.agents;
        let project = match &request.scope {
            DeploymentScope::Global => None,
            DeploymentScope::Project(path) => {
                let home = self
                    .paths()
                    .default_central_repo
                    .parent()
                    .ok_or_else(|| ServiceError::internal("runtime home unavailable"))?;
                let path = if path == Path::new("~") {
                    home.to_path_buf()
                } else if let Ok(rest) = path.strip_prefix("~") {
                    home.join(rest)
                } else {
                    path.clone()
                };
                if !path.is_dir() {
                    return Err(ServiceError::new(
                        ErrorCode::InvalidArgument,
                        "project must be an existing directory",
                        json!({"argument":"project"}),
                    ));
                }
                Some(
                    std::fs::canonicalize(path)
                        .map_err(|_| ServiceError::internal("failed to resolve project"))?,
                )
            }
        };
        let scope_name = if project.is_some() {
            "project"
        } else {
            "global"
        };
        let project_string = project
            .as_ref()
            .map(|path| path.to_string_lossy().into_owned());
        let scope = project
            .clone()
            .map(DeploymentScope::Project)
            .unwrap_or_default();
        let records = self
            .store()
            .list_skill_targets(&skill.id)
            .map_err(|_| ServiceError::internal("failed to read deployment records"))?;
        let mut groups: BTreeMap<PathBuf, Vec<Agent>> = BTreeMap::new();
        for key in &request.agents {
            let agent = agents
                .iter()
                .find(|agent| &agent.key == key)
                .ok_or_else(|| {
                    ServiceError::new(
                        ErrorCode::AgentNotFound,
                        "Agent not found",
                        json!({"agent":key}),
                    )
                })?;
            if project.is_some() && !agent.supports_project_scope {
                return Err(ServiceError::new(
                    ErrorCode::ProjectScopeUnsupported,
                    "Agent does not support project scope",
                    json!({"agent":key,"reason":"project_scope_unsupported"}),
                ));
            }
            if !undeploy && (!agent.enabled || (project.is_none() && !agent.detected)) {
                return Err(ServiceError::new(
                    ErrorCode::AgentNotFound,
                    "Agent is not installed or enabled",
                    json!({"agent":key,"reason":"not_installed"}),
                ));
            }
            let root = agent_root(agent, project.as_deref())?;
            let path = root.join(&skill.name);
            let group = groups.entry(path).or_default();
            for other in &agents {
                if (other.key == *key
                    || (other.enabled
                        && other.detected
                        && (project.is_none() || other.supports_project_scope)
                        && agent_root(other, project.as_deref())? == root))
                    && !group.iter().any(|existing| existing.key == other.key)
                {
                    group.push(other.clone());
                }
            }
        }
        let all_skills = self
            .store()
            .list_skills()
            .map_err(|_| ServiceError::internal("failed to inspect protected skill paths"))?;
        let mut all_records = Vec::new();
        for other in &all_skills {
            all_records.extend(
                self.store()
                    .list_skill_targets(&other.id)
                    .map_err(|_| ServiceError::internal("failed to inspect target ownership"))?,
            );
        }
        let mut targets: Vec<DeploymentTarget> = Vec::new();
        let mut fingerprints = Vec::new();
        let mut rows = Vec::new();
        let mut baseline_snapshot = Vec::new();
        for (path, mut group) in groups {
            group.sort_by(|a, b| a.key.cmp(&b.key));
            let group_key = group[0].key.clone();
            let key = group_key.as_str();
            let physical =
                target_location(&path).map_err(|_| conflict(key, &path, "path_resolution"))?;
            if project
                .as_ref()
                .is_some_and(|root| !physical.starts_with(root))
            {
                return Err(conflict(key, &path, "project_path_escape"));
            }
            for previous in &targets {
                let other = target_location(&previous.path)
                    .map_err(|_| conflict(key, &path, "path_resolution"))?;
                if physical == other || physical.starts_with(&other) || other.starts_with(&physical)
                {
                    return Err(conflict(key, &path, "overlapping_targets"));
                }
            }
            for protected in &all_skills {
                for protected_path in std::iter::once(protected.central_path.as_str())
                    .chain(protected.external_local_source())
                {
                    let protected_path = path_for_comparison(Path::new(protected_path))
                        .map_err(|_| conflict(key, &path, "path_resolution"))?;
                    if physical == protected_path
                        || physical.starts_with(&protected_path)
                        || protected_path.starts_with(&physical)
                    {
                        return Err(conflict(key, &path, "overlaps_skill_source"));
                    }
                }
            }
            for other in &all_records {
                let other_location = target_location(Path::new(&other.target_path))
                    .map_err(|_| conflict(key, &path, "path_resolution"))?;
                if (other.skill_id != skill.id
                    || other.scope != scope_name
                    || !same_project(other.project_path.as_deref(), project_string.as_deref()))
                    && (physical == other_location
                        || physical.starts_with(&other_location)
                        || other_location.starts_with(&physical))
                {
                    return Err(conflict(key, &path, "target_owned_elsewhere"));
                }
            }
            check_parent_writable(&path).map_err(|_| conflict(key, &path, "not_writable"))?;
            let fingerprint = deployment_fingerprint(&path)
                .map_err(|_| conflict(key, &path, "invalid_target"))?;
            let matching: Vec<_> = records
                .iter()
                .filter(|record| {
                    target_location(Path::new(&record.target_path))
                        .ok()
                        .as_ref()
                        == Some(&physical)
                        && record.scope == scope_name
                        && same_project(record.project_path.as_deref(), project_string.as_deref())
                })
                .cloned()
                .collect();
            for record in &matching {
                let saved = self
                    .store()
                    .get_setting(&format!("device_sync.target_baseline.{}", record.id))
                    .map_err(|_| ServiceError::internal("failed to inspect target baseline"))?;
                baseline_snapshot.push((record.id.clone(), saved));
                if let Some(agent) = agents.iter().find(|agent| agent.key == record.tool) {
                    if !group.iter().any(|a| a.key == agent.key) {
                        group.push(agent.clone());
                    }
                }
            }
            if fingerprint.is_some() {
                let managed = matching
                    .iter()
                    .any(|record| record.status == "ok" || record.synced_at.is_some());
                let linked = std::fs::read_link(&path).ok().is_some_and(|link| {
                    let link = if link.is_absolute() {
                        link
                    } else {
                        path.parent().unwrap_or(Path::new(".")).join(link)
                    };
                    path_for_comparison(&link).ok() == path_for_comparison(&source).ok()
                });
                let actual = if std::fs::symlink_metadata(&path)
                    .is_ok_and(|meta| !meta.file_type().is_symlink())
                {
                    hash_dir_for_sync_conflict(&path).ok()
                } else {
                    None
                };
                let baseline_matches = baseline_snapshot.iter().any(|(id, value)| {
                    matching.iter().any(|record| &record.id == id)
                        && value.as_deref().is_some_and(|value| {
                            crate::core::tool_distribution::matches_saved_target_baseline(
                                value,
                                &path,
                                actual.as_deref(),
                            )
                        })
                });
                let same_content = actual.as_ref() == Some(&conflict_hash);
                let trusted = managed && (linked || baseline_matches || same_content);
                let explicit_overwrite = !undeploy
                    && (request.overwrite || (request.overwrite_if_same_content && same_content));
                if !trusted && !explicit_overwrite {
                    return Err(conflict(
                        key,
                        &path,
                        if managed {
                            "target_modified"
                        } else {
                            "unmanaged_target"
                        },
                    ));
                }
            }
            for agent in &group {
                if let Some(old) = records.iter().find(|r| {
                    r.tool == agent.key
                        && r.scope == scope_name
                        && same_project(r.project_path.as_deref(), project_string.as_deref())
                }) {
                    if target_location(Path::new(&old.target_path)).ok().as_ref() != Some(&physical)
                    {
                        return Err(conflict(&agent.key, &path, "registered_path_changed"));
                    }
                }
            }
            if undeploy {
                rows.extend(matching);
            }
            let mut mode = SyncMode::Auto;
            for agent in &group {
                let required = if agent.key == "cursor" {
                    SyncMode::Copy
                } else {
                    agent.sync_mode
                };
                if required != SyncMode::Auto {
                    if !undeploy && mode != SyncMode::Auto && mode != required {
                        return Err(conflict(&agent.key, &path, "incompatible_shared_modes"));
                    }
                    mode = required;
                }
            }
            let mut keys = group.into_iter().map(|agent| agent.key).collect::<Vec<_>>();
            keys.sort();
            keys.dedup();
            targets.push(DeploymentTarget {
                agents: keys,
                path,
                mode,
                exists: fingerprint.is_some(),
            });
            fingerprints.push(fingerprint);
        }
        let locations = targets
            .iter()
            .map(|target| target_location(&target.path))
            .collect::<anyhow::Result<Vec<_>>>()
            .map_err(|_| ServiceError::internal("failed to inspect deployment paths"))?;
        let parent_snapshots = targets
            .iter()
            .map(|target| DeploymentParentSnapshot::capture(&target.path, project.as_deref()))
            .collect::<anyhow::Result<Vec<_>>>()
            .map_err(|_| stale())?;
        let snapshot = json!({"undeploy":undeploy,"parents":parent_snapshots,"skill":skill,"source_hash":source_hash,"agents":agents,"records":all_records,"baselines":baseline_snapshot,"fingerprints":fingerprints,"locations":locations});
        if !undeploy {
            rows = records;
        }
        Ok(DeploymentPlan {
            skill_id: skill.id,
            scope,
            targets,
            undeploy,
            request,
            snapshot,
            source,
            rows,
            fingerprints,
            parent_snapshots,
        })
    }

    fn execute_deployment_plan(
        &self,
        mut plan: DeploymentPlan,
    ) -> Result<DeploymentOutcome, ServiceError> {
        let mut prepared = Vec::new();
        let result = (|| -> anyhow::Result<()> {
            for ((target, expected), parents) in plan
                .targets
                .iter()
                .zip(&plan.fingerprints)
                .zip(&plan.parent_snapshots)
            {
                prepared.push(PreparedDeployment::prepare_in(
                    (!plan.undeploy).then_some(plan.source.as_path()),
                    &target.path,
                    target.mode,
                    expected.clone(),
                    parents.clone(),
                )?);
            }
            for replacement in &mut prepared {
                replacement.activate()?;
            }
            let mut upserts = Vec::new();
            let scope = if matches!(plan.scope, DeploymentScope::Global) {
                "global"
            } else {
                "project"
            };
            let project_path = match &plan.scope {
                DeploymentScope::Global => None,
                DeploymentScope::Project(path) => Some(path.to_string_lossy().into_owned()),
            };
            if !plan.undeploy {
                for (target, replacement) in plan.targets.iter_mut().zip(&prepared) {
                    let hash = hash_dir_for_sync_conflict(&target.path)?;
                    target.mode = replacement.mode;
                    for agent in &target.agents {
                        let id = plan
                            .rows
                            .iter()
                            .find(|row| {
                                row.tool == *agent
                                    && row.scope == scope
                                    && same_project(
                                        row.project_path.as_deref(),
                                        project_path.as_deref(),
                                    )
                            })
                            .map(|row| row.id.clone())
                            .unwrap_or_else(|| Uuid::new_v4().to_string());
                        upserts.push((
                            SkillTargetRecord {
                                id,
                                skill_id: plan.skill_id.clone(),
                                tool: agent.clone(),
                                scope: scope.into(),
                                project_path: project_path.clone(),
                                target_path: target.path.to_string_lossy().into_owned(),
                                mode: mode_name(target.mode).into(),
                                status: "ok".into(),
                                last_error: None,
                                synced_at: Some(
                                    std::time::SystemTime::now()
                                        .duration_since(std::time::UNIX_EPOCH)
                                        .unwrap_or_default()
                                        .as_millis() as i64,
                                ),
                            },
                            hash.clone(),
                        ));
                    }
                }
            }
            anyhow::ensure!(
                Some(hash_dir_strict(&plan.source)?.as_str())
                    == plan.snapshot["source_hash"].as_str(),
                "PLAN_STALE"
            );
            for replacement in &prepared {
                replacement.verify_unchanged()?;
            }
            self.store().commit_deployment_targets(
                &upserts,
                if plan.undeploy { &plan.rows } else { &[] },
                || {
                    for replacement in &prepared {
                        replacement.verify_unchanged()?;
                    }
                    anyhow::ensure!(
                        Some(hash_dir_strict(&plan.source)?.as_str())
                            == plan.snapshot["source_hash"].as_str(),
                        "PLAN_STALE"
                    );
                    Ok(())
                },
            )?;
            Ok(())
        })();
        if let Err(error) = result {
            let mut recovery = Vec::new();
            for (target, replacement) in plan.targets.iter().zip(prepared.iter_mut()).rev() {
                if replacement.rollback().is_err() {
                    recovery.push(json!({"agents":target.agents,"path":target.path,"backup":replacement.backup_path(),"reason":"rollback_conflict"}));
                }
            }
            return Err(ServiceError::new(
                if error.to_string().contains("PLAN_STALE") {
                    ErrorCode::PlanStale
                } else {
                    ErrorCode::InternalError
                },
                "deployment failed; original targets were retained",
                json!({"recovery":recovery}),
            ));
        }
        for replacement in &mut prepared {
            replacement.commit();
        }
        Ok(DeploymentOutcome {
            skill_id: plan.skill_id,
            scope: plan.scope,
            targets: plan.targets,
            undeploy: plan.undeploy,
        })
    }
}

fn agent_root(agent: &Agent, project: Option<&Path>) -> Result<PathBuf, ServiceError> {
    if let Some(project) = project {
        let relative = Path::new(&agent.project_skills_dir);
        if relative.as_os_str().is_empty()
            || relative.is_absolute()
            || relative
                .components()
                .any(|part| !matches!(part, Component::Normal(_)))
        {
            return Err(conflict(&agent.key, relative, "invalid_project_path"));
        }
        path_for_comparison(&project.join(relative))
            .map_err(|_| conflict(&agent.key, relative, "path_resolution"))
    } else {
        path_for_comparison(Path::new(&agent.skills_dir))
            .map_err(|_| conflict(&agent.key, Path::new(&agent.skills_dir), "path_resolution"))
    }
}

fn same_project(first: Option<&str>, second: Option<&str>) -> bool {
    match (first, second) {
        (None, None) => true,
        (Some(first), Some(second)) => {
            first == second
                || matches!((path_for_comparison(Path::new(first)), path_for_comparison(Path::new(second))), (Ok(a), Ok(b)) if a == b)
        }
        _ => false,
    }
}

fn target_location(path: &Path) -> anyhow::Result<PathBuf> {
    Ok(path_for_comparison(
        path.parent()
            .ok_or_else(|| anyhow::anyhow!("target has no parent"))?,
    )?
    .join(
        path.file_name()
            .ok_or_else(|| anyhow::anyhow!("target has no name"))?,
    ))
}

fn check_parent_writable(path: &Path) -> anyhow::Result<()> {
    let mut parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("target has no parent"))?;
    while !parent.exists() {
        parent = parent
            .parent()
            .ok_or_else(|| anyhow::anyhow!("target has no ancestor"))?;
    }
    let metadata = std::fs::metadata(parent)?;
    anyhow::ensure!(metadata.is_dir(), "target parent is not a directory");
    #[cfg(not(windows))]
    anyhow::ensure!(
        !metadata.permissions().readonly(),
        "target parent is not writable"
    );
    #[cfg(unix)]
    {
        use std::ffi::{c_char, c_int, CString};
        use std::os::unix::ffi::OsStrExt;
        use std::os::unix::fs::PermissionsExt;
        extern "C" {
            fn access(path: *const c_char, mode: c_int) -> c_int;
        }
        anyhow::ensure!(
            metadata.permissions().mode() & 0o222 != 0
                && metadata.permissions().mode() & 0o111 != 0,
            "target parent is not writable"
        );
        let path = CString::new(parent.as_os_str().as_bytes())?;
        // POSIX W_OK | X_OK checks the current user's permissions and ACL without a write probe.
        anyhow::ensure!(
            unsafe { access(path.as_ptr(), 3) } == 0,
            "target parent is not writable"
        );
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // Open only the existing parent with add-directory, delete-child, and traverse access.
        std::fs::OpenOptions::new()
            .access_mode(0x0004 | 0x0040 | 0x0020)
            .custom_flags(0x02000000)
            .open(parent)?;
    }
    Ok(())
}

fn conflict(agent: &str, path: &Path, reason: &str) -> ServiceError {
    ServiceError::new(
        ErrorCode::TargetConflict,
        "Agent target cannot be safely changed",
        json!({"agent":agent,"path":path,"reason":reason}),
    )
}

fn stale() -> ServiceError {
    ServiceError::new(
        ErrorCode::PlanStale,
        "deployment plan is stale; create a new plan",
        json!({"reason":"state_changed"}),
    )
}

pub(crate) fn mode_name(mode: SyncMode) -> &'static str {
    match mode {
        SyncMode::Auto => "auto",
        SyncMode::Symlink => "symlink",
        SyncMode::Junction => "junction",
        SyncMode::Copy => "copy",
    }
}
