use serde::{Deserialize, Serialize};

pub use crate::core::sync_engine::SyncMode;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub enum SkillSelector {
    Id(String),
    Name(String),
}

impl SkillSelector {
    pub fn value(&self) -> &str {
        match self {
            Self::Id(value) | Self::Name(value) => value,
        }
    }
}

impl From<String> for SkillSelector {
    fn from(value: String) -> Self {
        Self::Name(value)
    }
}

impl From<&str> for SkillSelector {
    fn from(value: &str) -> Self {
        Self::Name(value.to_string())
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct SkillSource {
    pub kind: String,
    pub reference: Option<String>,
    pub subpath: Option<String>,
    pub revision: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct SkillTag {
    pub id: i64,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct SkillTarget {
    pub tool: String,
    pub scope: String,
    pub project_path: Option<String>,
    pub target_path: String,
    pub mode: String,
    pub status: String,
    pub last_error: Option<String>,
    pub synced_at: Option<i64>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct Skill {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub source: SkillSource,
    pub central_path: String,
    pub content_hash: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub last_sync_at: Option<i64>,
    pub enabled: bool,
    pub content_status: String,
    pub source_error: Option<String>,
    pub source_checked_at: Option<i64>,
    pub tags: Vec<SkillTag>,
    pub targets: Vec<SkillTarget>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct SkillStatus {
    pub id: String,
    pub name: String,
    pub content_status: String,
    pub source_error: Option<String>,
    pub source_checked_at: Option<i64>,
    pub targets: Vec<SkillTarget>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct Agent {
    pub key: String,
    pub label: String,
    pub avatar: Option<String>,
    pub detected: bool,
    pub enabled: bool,
    pub is_custom: bool,
    pub skills_dir: String,
    pub project_skills_dir: String,
    pub supports_project_scope: bool,
    pub sync_mode: SyncMode,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct AgentList {
    pub agents: Vec<Agent>,
    pub installed: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct DoctorReport {
    pub database_status: String,
    pub database_path: String,
    pub profile: String,
    pub skill_count: usize,
    pub agent_count: usize,
    pub detected_agent_count: usize,
}
