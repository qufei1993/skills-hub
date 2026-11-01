use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::operation_lock::OperationLockError;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    InvalidArgument,
    InvalidSource,
    SkillNotFound,
    AmbiguousSkill,
    MultiSkills,
    AgentNotFound,
    ProjectScopeUnsupported,
    TargetConflict,
    UpdateHeldBack,
    ConfirmationRequired,
    PlanStale,
    OperationBusy,
    IncompatibleDatabase,
    AuthRequired,
    NetworkError,
    InternalError,
}

impl ErrorCode {
    pub const ALL: [Self; 16] = [
        Self::InvalidArgument,
        Self::InvalidSource,
        Self::SkillNotFound,
        Self::AmbiguousSkill,
        Self::MultiSkills,
        Self::AgentNotFound,
        Self::ProjectScopeUnsupported,
        Self::TargetConflict,
        Self::UpdateHeldBack,
        Self::ConfirmationRequired,
        Self::PlanStale,
        Self::OperationBusy,
        Self::IncompatibleDatabase,
        Self::AuthRequired,
        Self::NetworkError,
        Self::InternalError,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidArgument => "INVALID_ARGUMENT",
            Self::InvalidSource => "INVALID_SOURCE",
            Self::SkillNotFound => "SKILL_NOT_FOUND",
            Self::AmbiguousSkill => "AMBIGUOUS_SKILL",
            Self::MultiSkills => "MULTI_SKILLS",
            Self::AgentNotFound => "AGENT_NOT_FOUND",
            Self::ProjectScopeUnsupported => "PROJECT_SCOPE_UNSUPPORTED",
            Self::TargetConflict => "TARGET_CONFLICT",
            Self::UpdateHeldBack => "UPDATE_HELD_BACK",
            Self::ConfirmationRequired => "CONFIRMATION_REQUIRED",
            Self::PlanStale => "PLAN_STALE",
            Self::OperationBusy => "OPERATION_BUSY",
            Self::IncompatibleDatabase => "INCOMPATIBLE_DATABASE",
            Self::AuthRequired => "AUTH_REQUIRED",
            Self::NetworkError => "NETWORK_ERROR",
            Self::InternalError => "INTERNAL_ERROR",
        }
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ServiceError {
    pub code: ErrorCode,
    pub message: String,
    pub details: Value,
}

impl ServiceError {
    pub fn new(code: ErrorCode, message: impl Into<String>, details: Value) -> Self {
        Self {
            code,
            message: message.into(),
            details,
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::InternalError, message, json!({}))
    }

    pub fn incompatible_database(found_version: i32) -> Self {
        Self::new(
            ErrorCode::IncompatibleDatabase,
            "database schema is newer than this Skills Hub version",
            json!({ "found_version": found_version }),
        )
    }

    pub fn skill_not_found(selector: &str) -> Self {
        Self::new(
            ErrorCode::SkillNotFound,
            "skill was not found",
            json!({ "selector": selector }),
        )
    }
}

impl fmt::Display for ServiceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for ServiceError {}

impl From<OperationLockError> for ServiceError {
    fn from(error: OperationLockError) -> Self {
        match error {
            OperationLockError::Busy(kind) => Self::new(
                ErrorCode::OperationBusy,
                "another Skills Hub operation is already in progress",
                json!({ "operation": kind.to_string() }),
            ),
            OperationLockError::Io(_) => {
                Self::internal("failed to acquire the Skills Hub operation lock")
            }
        }
    }
}
