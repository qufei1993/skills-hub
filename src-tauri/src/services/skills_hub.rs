use std::path::{Path, PathBuf};

use rusqlite::Connection;
use serde_json::json;

use crate::core::runtime_paths::{open_store, RuntimePaths, RuntimeProfile};
use crate::core::skill_store::{IncompatibleDatabaseError, SkillRecord, SkillStore};
use crate::core::tool_adapters::{
    default_tool_adapters, is_builtin_tool_enabled, load_tool_config, project_relative_skills_dir,
    resolve_adapter_path_in_home, supports_project_scope,
};

use super::error::{ErrorCode, ServiceError};
use super::operation_lock::{OperationKind, OperationLock};
use super::types::{
    Agent, AgentList, DoctorReport, Skill, SkillSelector, SkillSource, SkillStatus, SkillTag,
    SkillTarget,
};

#[derive(Clone, Debug)]
pub struct SkillsHubService {
    paths: RuntimePaths,
    store: SkillStore,
    schema_version_at_open: Option<i32>,
    incompatible_schema_version: Option<i32>,
}

impl SkillsHubService {
    pub fn open(paths: RuntimePaths) -> Result<Self, ServiceError> {
        match open_store(&paths) {
            Ok(store) => Ok(Self::from_store(paths, store)),
            Err(error) => {
                if let Some(compatibility) = error.downcast_ref::<IncompatibleDatabaseError>() {
                    return Ok(Self {
                        store: SkillStore::new(paths.database_path.clone()),
                        paths,
                        schema_version_at_open: None,
                        incompatible_schema_version: Some(compatibility.found_version),
                    });
                }
                Err(ServiceError::internal(
                    "failed to open the Skills Hub database",
                ))
            }
        }
    }

    pub(crate) fn from_store(paths: RuntimePaths, store: SkillStore) -> Self {
        let schema_version_at_open = database_user_version(store.db_path()).ok();
        Self {
            paths,
            store,
            schema_version_at_open,
            incompatible_schema_version: None,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn store(&self) -> &SkillStore {
        &self.store
    }

    #[allow(dead_code)]
    pub(crate) fn begin_write(&self, kind: OperationKind) -> Result<OperationLock, ServiceError> {
        self.ensure_database_compatible()?;
        OperationLock::acquire(&self.paths, kind).map_err(Into::into)
    }

    pub fn list_skills(&self) -> Result<Vec<Skill>, ServiceError> {
        self.ensure_database_compatible()?;
        let records = self
            .store
            .list_skills()
            .map_err(|_| ServiceError::internal("failed to list managed skills"))?;
        let source_checks = self
            .store
            .source_checks()
            .map_err(|_| ServiceError::internal("failed to read skill source status"))?;

        records
            .into_iter()
            .map(|record| {
                let source_check = source_checks.get(&record.id);
                self.skill_from_record(record, source_check)
            })
            .collect()
    }

    pub fn show_skill(&self, selector: SkillSelector) -> Result<Skill, ServiceError> {
        self.ensure_database_compatible()?;
        let record = self.resolve_skill(&selector)?;
        let source_checks = self
            .store
            .source_checks()
            .map_err(|_| ServiceError::internal("failed to read skill source status"))?;
        let source_check = source_checks.get(&record.id);
        self.skill_from_record(record, source_check)
    }

    pub fn skill_status(&self, selector: SkillSelector) -> Result<SkillStatus, ServiceError> {
        let skill = self.show_skill(selector)?;
        Ok(SkillStatus {
            id: skill.id,
            name: skill.name,
            content_status: skill.content_status,
            source_error: skill.source_error,
            source_checked_at: skill.source_checked_at,
            targets: skill.targets,
        })
    }

    pub fn list_agents(&self) -> Result<AgentList, ServiceError> {
        self.ensure_database_compatible()?;
        let config = load_tool_config(&self.store)
            .map_err(|_| ServiceError::internal("failed to load Agent configuration"))?;
        let home_root = self
            .paths
            .default_central_repo
            .parent()
            .ok_or_else(|| ServiceError::internal("failed to resolve Agent paths"))?;
        let mut agents = Vec::new();

        for adapter in default_tool_adapters() {
            let enabled = is_builtin_tool_enabled(&config, adapter.id.as_key());
            let skills_dir = resolve_adapter_path_in_home(
                &adapter,
                home_root,
                adapter.relative_skills_dir,
                "skills",
            );
            let detected =
                resolve_adapter_path_in_home(&adapter, home_root, adapter.relative_detect_dir, "")
                    .exists();
            agents.push(Agent {
                key: adapter.id.as_key().to_string(),
                label: adapter.display_name.to_string(),
                avatar: None,
                detected,
                enabled,
                is_custom: false,
                skills_dir: skills_dir.to_string_lossy().into_owned(),
                project_skills_dir: project_relative_skills_dir(&adapter).to_string(),
                supports_project_scope: supports_project_scope(&adapter),
                sync_mode: Default::default(),
            });
        }

        for custom in config.custom_tools {
            let skills_dir = expand_home_path_in(&custom.skills_dir, home_root);
            agents.push(Agent {
                key: custom.key,
                label: custom.label,
                avatar: custom.avatar,
                detected: skills_dir.is_dir(),
                enabled: custom.enabled,
                is_custom: true,
                skills_dir: skills_dir.to_string_lossy().into_owned(),
                project_skills_dir: custom.project_skills_dir.clone().unwrap_or_default(),
                supports_project_scope: custom.project_skills_dir.is_some(),
                sync_mode: custom.sync_mode,
            });
        }

        let mut installed = agents
            .iter()
            .filter(|agent| agent.detected)
            .map(|agent| agent.key.clone())
            .collect::<Vec<_>>();
        installed.dedup();

        Ok(AgentList { agents, installed })
    }

    pub fn doctor(&self) -> Result<DoctorReport, ServiceError> {
        let profile = match self.paths.profile {
            RuntimeProfile::Production => "production",
            RuntimeProfile::Development => "development",
            RuntimeProfile::Test => "test",
        };
        if self.current_incompatible_schema_version()?.is_some() {
            return Ok(DoctorReport {
                database_status: "incompatible".to_string(),
                database_path: self.paths.database_path.to_string_lossy().into_owned(),
                profile: profile.to_string(),
                skill_count: 0,
                agent_count: 0,
                detected_agent_count: 0,
            });
        }

        let skill_count = self.list_skills()?.len();
        let agents = self.list_agents()?;
        let detected_agent_count = agents.agents.iter().filter(|agent| agent.detected).count();
        Ok(DoctorReport {
            database_status: "ok".to_string(),
            database_path: self.paths.database_path.to_string_lossy().into_owned(),
            profile: profile.to_string(),
            skill_count,
            agent_count: agents.agents.len(),
            detected_agent_count,
        })
    }

    fn ensure_database_compatible(&self) -> Result<(), ServiceError> {
        match self.current_incompatible_schema_version()? {
            Some(version) => Err(ServiceError::incompatible_database(version)),
            None => Ok(()),
        }
    }

    fn current_incompatible_schema_version(&self) -> Result<Option<i32>, ServiceError> {
        if let Some(version) = self.incompatible_schema_version {
            return Ok(Some(version));
        }
        let supported = self
            .schema_version_at_open
            .ok_or_else(|| ServiceError::internal("failed to inspect the database schema"))?;
        let current = database_user_version(&self.paths.database_path)
            .map_err(|_| ServiceError::internal("failed to inspect the database schema"))?;
        Ok((current > supported).then_some(current))
    }

    fn resolve_skill(&self, selector: &SkillSelector) -> Result<SkillRecord, ServiceError> {
        let value = selector.value().trim();
        if value.is_empty() {
            return Err(ServiceError::new(
                ErrorCode::InvalidArgument,
                "skill selector must not be empty",
                json!({ "argument": "skill" }),
            ));
        }

        if let Some(record) = self
            .store
            .get_skill_by_id(value)
            .map_err(|_| ServiceError::internal("failed to resolve the skill selector"))?
        {
            return Ok(record);
        }
        if matches!(selector, SkillSelector::Id(_)) {
            return Err(ServiceError::skill_not_found(value));
        }

        let normalized_name = value.to_lowercase();
        let mut candidates = self
            .store
            .list_skills()
            .map_err(|_| ServiceError::internal("failed to resolve the skill selector"))?
            .into_iter()
            .filter(|skill| skill.name.to_lowercase() == normalized_name)
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| left.id.cmp(&right.id));

        match candidates.len() {
            0 => Err(ServiceError::skill_not_found(value)),
            1 => Ok(candidates.remove(0)),
            _ => Err(ServiceError::new(
                ErrorCode::AmbiguousSkill,
                "multiple skills match this name",
                json!({
                    "selector": value,
                    "candidates": candidates
                        .into_iter()
                        .map(|skill| json!({ "id": skill.id, "name": skill.name }))
                        .collect::<Vec<_>>()
                }),
            )),
        }
    }

    fn skill_from_record(
        &self,
        record: SkillRecord,
        source_check: Option<&(Option<String>, i64)>,
    ) -> Result<Skill, ServiceError> {
        let targets = self
            .store
            .list_skill_targets(&record.id)
            .map_err(|_| ServiceError::internal("failed to read skill targets"))?
            .into_iter()
            .map(|target| SkillTarget {
                tool: target.tool,
                scope: target.scope,
                project_path: target.project_path,
                target_path: target.target_path,
                mode: target.mode,
                status: target.status,
                last_error: target
                    .last_error
                    .as_deref()
                    .map(crate::core::skill_issues::safe_output),
                synced_at: target.synced_at,
            })
            .collect();
        let tags = self
            .store
            .get_skill_tags(&record.id)
            .map_err(|_| ServiceError::internal("failed to read skill tags"))?
            .into_iter()
            .map(|tag| SkillTag {
                id: tag.id,
                name: tag.name,
            })
            .collect();
        let source_error = source_check
            .and_then(|check| check.0.as_deref())
            .map(crate::core::skill_issues::safe_output);
        let content_status = if source_error.is_some() {
            "error".to_string()
        } else {
            let home_root = self
                .paths
                .default_central_repo
                .parent()
                .ok_or_else(|| ServiceError::internal("failed to resolve the runtime home"))?;
            content_status_in(&record, home_root)
        };

        Ok(Skill {
            id: record.id,
            name: record.name,
            description: record.description,
            source: SkillSource {
                kind: record.source_type,
                reference: record.source_ref.as_deref().map(redact_source_reference),
                subpath: record.source_subpath,
                revision: record.source_revision,
            },
            central_path: record.central_path,
            content_hash: record.content_hash,
            created_at: record.created_at,
            updated_at: record.updated_at,
            last_sync_at: record.last_sync_at,
            enabled: record.enabled,
            content_status,
            source_error,
            source_checked_at: source_check.map(|check| check.1),
            tags,
            targets,
        })
    }
}

fn database_user_version(path: &Path) -> rusqlite::Result<i32> {
    Connection::open(path)?.query_row("PRAGMA user_version", [], |row| row.get(0))
}

#[cfg(test)]
pub(crate) fn content_status(skill: &SkillRecord) -> String {
    let home_root = dirs::home_dir().unwrap_or_default();
    content_status_in(skill, &home_root)
}

fn content_status_in(skill: &SkillRecord, home_root: &Path) -> String {
    if skill.status != "ok" || skill.source_type != "local" || skill.has_unbound_local_source() {
        return skill.status.clone();
    }
    let exists = skill
        .source_ref
        .as_deref()
        .map(|source| expand_home_path_in(source, home_root))
        .is_some_and(|source| source.exists());
    if exists {
        skill.status.clone()
    } else {
        "error".to_string()
    }
}

fn expand_home_path_in(input: &str, home_root: &Path) -> PathBuf {
    if input == "~" {
        return home_root.to_path_buf();
    }
    if let Some(rest) = input
        .strip_prefix("~/")
        .or_else(|| input.strip_prefix("~\\"))
    {
        return home_root.join(rest);
    }
    PathBuf::from(input)
}

fn redact_source_reference(reference: &str) -> String {
    let Ok(mut url) = reqwest::Url::parse(reference) else {
        return reference
            .split(['?', '#'])
            .next()
            .unwrap_or_default()
            .to_string();
    };
    if !url.username().is_empty() {
        let _ = url.set_username("");
    }
    if url.password().is_some() {
        let _ = url.set_password(None);
    }
    url.set_query(None);
    url.set_fragment(None);
    url.to_string()
}
