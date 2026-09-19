use std::path::PathBuf;

use serde::Serialize;
use serde_json::json;

use super::deployment::{DeploymentOutcome, DeploymentPlan, DeploymentRequest};
use super::error::{ErrorCode, ServiceError};
use super::install::{UpdateCheck, UpdateOutcome};
use super::library::{
    AdoptOutcome, AdoptPlan, AdoptRequest, RemoveOutcome, RemovePlan, RemoveRequest, TagAction,
    TagDeletePlan, TagOutcome, TagSelector,
};
use super::operation_lock::OperationKind;
use super::skills_hub::SkillsHubService;
use super::types::{Skill, SkillSelector};

#[derive(Clone, Debug, Default)]
pub struct SkillFilter {
    pub tags: Vec<String>,
    pub sources: Vec<String>,
    pub agents: Vec<String>,
    pub untagged: bool,
    pub status: Option<String>,
}

#[derive(Clone, Debug)]
pub enum SkillSelection {
    One(SkillSelector),
    All,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum SelectionOutcome<T> {
    One(T),
    All(Vec<T>),
}

#[derive(Clone, Copy, Debug)]
pub struct Confirmation {
    pub dry_run: bool,
    pub confirmed: bool,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum PlannedOutcome<P, O> {
    Plan(P),
    Applied(O),
}

#[derive(Debug, Serialize)]
pub struct TagSummary {
    pub id: i64,
    pub name: String,
    pub skill_count: i64,
}

impl SkillsHubService {
    pub fn filtered_skills(&self, filter: SkillFilter) -> Result<Vec<Skill>, ServiceError> {
        Ok(self
            .list_skills()?
            .into_iter()
            .filter(|skill| {
                (!filter.untagged || skill.tags.is_empty())
                    && filter.tags.iter().all(|name| {
                        skill
                            .tags
                            .iter()
                            .any(|tag| tag.name.to_lowercase() == name.trim().to_lowercase())
                    })
                    && (filter.sources.is_empty() || filter.sources.contains(&skill.source.kind))
                    && filter
                        .agents
                        .iter()
                        .all(|agent| skill.targets.iter().any(|target| &target.tool == agent))
                    && filter
                        .status
                        .as_ref()
                        .map_or(true, |status| &skill.content_status == status)
            })
            .collect())
    }

    pub fn tags(&self, skill: Option<SkillSelector>) -> Result<Vec<TagSummary>, ServiceError> {
        self.ensure_database_compatible()?;
        if let Some(skill) = skill {
            return Ok(self
                .show_skill(skill)?
                .tags
                .into_iter()
                .map(|tag| TagSummary {
                    id: tag.id,
                    name: tag.name,
                    skill_count: 1,
                })
                .collect());
        }
        self.store()
            .list_tags_with_counts()
            .map(|tags| {
                tags.into_iter()
                    .map(|tag| TagSummary {
                        id: tag.id,
                        name: tag.name,
                        skill_count: tag.skill_count,
                    })
                    .collect()
            })
            .map_err(|_| ServiceError::internal("failed to list tags"))
    }

    pub fn check_selection(
        &self,
        selection: SkillSelection,
    ) -> Result<SelectionOutcome<UpdateCheck>, ServiceError> {
        match selection {
            SkillSelection::One(skill) => self.check_updates(skill).map(SelectionOutcome::One),
            SkillSelection::All => self
                .list_skills()?
                .into_iter()
                .map(|skill| self.check_updates(SkillSelector::Id(skill.id)))
                .collect::<Result<Vec<_>, _>>()
                .map(SelectionOutcome::All),
        }
    }

    pub fn update_selection(
        &self,
        selection: SkillSelection,
    ) -> Result<SelectionOutcome<UpdateOutcome>, ServiceError> {
        match selection {
            SkillSelection::One(skill) => self.update(skill).map(SelectionOutcome::One),
            SkillSelection::All => {
                let _lock = self.begin_write(OperationKind::Update)?;
                let skills = self.list_skills()?;
                for skill in &skills {
                    let check = self.check_updates(SkillSelector::Id(skill.id.clone()))?;
                    if check.held_back {
                        return Err(ServiceError::new(
                            ErrorCode::UpdateHeldBack,
                            "batch update would remove user files",
                            json!({"skill_id": skill.id, "removal_count": check.removal_count}),
                        ));
                    }
                }
                let mut updated = Vec::new();
                for skill in skills {
                    match self.update_under_lock(SkillSelector::Id(skill.id)) {
                        Ok(outcome) => updated.push(outcome),
                        Err(mut error) => {
                            error.details["completed"] = json!(updated);
                            return Err(error);
                        }
                    }
                }
                Ok(SelectionOutcome::All(updated))
            }
        }
    }

    pub fn deploy_workflow(
        &self,
        request: DeploymentRequest,
        dry_run: bool,
    ) -> Result<PlannedOutcome<DeploymentPlan, DeploymentOutcome>, ServiceError> {
        if dry_run {
            self.plan_deploy(request).map(PlannedOutcome::Plan)
        } else {
            self.deploy(request).map(PlannedOutcome::Applied)
        }
    }

    pub fn undeploy_workflow(
        &self,
        request: DeploymentRequest,
        dry_run: bool,
    ) -> Result<PlannedOutcome<DeploymentPlan, DeploymentOutcome>, ServiceError> {
        if dry_run {
            self.plan_undeploy(request).map(PlannedOutcome::Plan)
        } else {
            self.undeploy(request).map(PlannedOutcome::Applied)
        }
    }

    pub fn adopt_workflow(
        &self,
        source: PathBuf,
        confirmation: Confirmation,
    ) -> Result<PlannedOutcome<AdoptPlan, AdoptOutcome>, ServiceError> {
        let plan = self.plan_adopt(source)?;
        if confirmation.dry_run {
            return Ok(PlannedOutcome::Plan(plan));
        }
        self.adopt(AdoptRequest {
            plan_id: plan.id.clone(),
            confirmed: confirmation.confirmed,
        })
        .map(PlannedOutcome::Applied)
        .map_err(|error| include_confirmation_plan(error, &plan))
    }

    pub fn remove_workflow(
        &self,
        skill: SkillSelector,
        confirmation: Confirmation,
    ) -> Result<PlannedOutcome<RemovePlan, RemoveOutcome>, ServiceError> {
        let plan = self.plan_remove(skill)?;
        if confirmation.dry_run {
            return Ok(PlannedOutcome::Plan(plan));
        }
        self.remove(RemoveRequest {
            plan_id: plan.id.clone(),
            confirmed: confirmation.confirmed,
        })
        .map(PlannedOutcome::Applied)
        .map_err(|error| include_confirmation_plan(error, &plan))
    }

    pub fn delete_tag_workflow(
        &self,
        tag: TagSelector,
        confirmation: Confirmation,
    ) -> Result<PlannedOutcome<TagDeletePlan, TagOutcome>, ServiceError> {
        let plan = self.plan_tag_delete(tag)?;
        if confirmation.dry_run {
            return Ok(PlannedOutcome::Plan(plan));
        }
        self.apply_tag_action(TagAction::Delete {
            plan_id: plan.id.clone(),
            confirmed: confirmation.confirmed,
        })
        .map(PlannedOutcome::Applied)
        .map_err(|error| include_confirmation_plan(error, &plan))
    }
}

fn include_confirmation_plan(mut error: ServiceError, plan: &impl Serialize) -> ServiceError {
    if error.code == ErrorCode::ConfirmationRequired {
        error.details["plan"] = json!(plan);
    }
    error
}
