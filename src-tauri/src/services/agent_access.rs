use serde::Serialize;
use serde_json::json;
use std::path::Path;

use super::deployment::{DeploymentPlan, DeploymentRequest};
use super::error::{ErrorCode, ServiceError};
use super::operation_lock::OperationKind;
use super::skills_hub::SkillsHubService;
use super::types::{AgentList, Skill};

pub const OFFICIAL_SKILL_NAME: &str = "skills-hub";
pub use crate::core::installer::OFFICIAL_SKILL_MD;

#[derive(Clone, Debug)]
pub struct SetupAgentRequest {
    pub agents: Vec<String>,
    pub remove: bool,
    pub dry_run: bool,
    pub confirmed: bool,
}

impl SetupAgentRequest {
    pub fn install(agent: impl Into<String>) -> Self {
        Self {
            agents: vec![agent.into()],
            remove: false,
            dry_run: false,
            confirmed: false,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct AgentAccessStatus {
    pub installed: bool,
    pub deployed: bool,
    pub skill_id: Option<String>,
    pub bundled_version: String,
    pub installed_version: Option<String>,
    pub skill: Option<Skill>,
    pub agents: AgentList,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan: Option<DeploymentPlan>,
}

impl SkillsHubService {
    pub fn agent_access_status(&self) -> Result<AgentAccessStatus, ServiceError> {
        let skill = self.official_skill()?;
        Ok(AgentAccessStatus {
            installed: skill.is_some(),
            deployed: skill
                .as_ref()
                .is_some_and(|skill| !skill.targets.is_empty()),
            skill_id: skill.as_ref().map(|skill| skill.id.clone()),
            bundled_version: env!("CARGO_PKG_VERSION").into(),
            installed_version: skill
                .as_ref()
                .and_then(|skill| skill.source.revision.clone()),
            skill,
            agents: self.list_agents()?,
            plan: None,
        })
    }

    pub fn setup_agent_access(
        &self,
        request: SetupAgentRequest,
    ) -> Result<AgentAccessStatus, ServiceError> {
        self.ensure_database_compatible()?;
        if request.agents.is_empty() || request.agents.iter().any(|agent| agent.trim().is_empty()) {
            return Err(ServiceError::new(
                ErrorCode::InvalidArgument,
                "at least one explicit Agent is required",
                json!({"argument":"agent"}),
            ));
        }
        let _lock = if request.dry_run || (request.remove && !request.confirmed) {
            None
        } else {
            Some(self.begin_write(if request.remove {
                OperationKind::Undeploy
            } else {
                OperationKind::Install
            })?)
        };
        if request.remove {
            let skill = self
                .official_skill()?
                .ok_or_else(|| ServiceError::skill_not_found(OFFICIAL_SKILL_NAME))?;
            let plan = self.plan_undeploy(DeploymentRequest::global(skill.id, request.agents))?;
            if request.dry_run {
                let mut status = self.agent_access_status()?;
                status.plan = Some(plan);
                return Ok(status);
            }
            if !request.confirmed {
                return Err(ServiceError::new(
                    ErrorCode::ConfirmationRequired,
                    "confirm official Skill removal after reviewing the plan",
                    json!({"plan":plan}),
                ));
            }
            self.execute_deployment_plan(plan)?;
        } else {
            let bundled = self.prepare_bundled_install(
                OFFICIAL_SKILL_NAME,
                OFFICIAL_SKILL_MD,
                env!("CARGO_PKG_VERSION"),
            )?;
            let preview = self.skill_from_record(bundled.preview_record(), None)?;
            let deployment = DeploymentRequest::global(bundled.record.id.clone(), request.agents);
            let plan = self.plan_deployment_for_skill(deployment, false, preview)?;
            let central = crate::core::sync_engine::path_for_comparison(Path::new(
                &bundled.record.central_path,
            ))
            .map_err(|_| ServiceError::internal("failed to resolve bundled skill path"))?;
            if let Some(target) = plan.targets.iter().find(|target| {
                target.path.starts_with(&central) || central.starts_with(&target.path)
            }) {
                return Err(ServiceError::new(
                    ErrorCode::TargetConflict,
                    "Agent target overlaps the bundled library path",
                    json!({"path": target.path, "reason":"overlaps_skill_source"}),
                ));
            }
            if request.dry_run {
                let mut status = self.agent_access_status()?;
                status.plan = Some(plan);
                return Ok(status);
            }
            self.apply_bundled_install(bundled, || self.execute_bundled_deployment(plan))?;
        }
        self.agent_access_status()
    }

    fn official_skill(&self) -> Result<Option<Skill>, ServiceError> {
        match self.show_skill(OFFICIAL_SKILL_NAME.into()) {
            Ok(skill) if skill.source.kind == "bundled" => Ok(Some(skill)),
            Ok(skill) => Err(ServiceError::new(
                ErrorCode::TargetConflict,
                "the official Skill name belongs to another source",
                json!({"path":skill.central_path,"reason":"non_bundled_skill"}),
            )),
            Err(error) if error.code == ErrorCode::SkillNotFound => Ok(None),
            Err(error) => Err(error),
        }
    }
}
