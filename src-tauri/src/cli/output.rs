use std::io::{self, Write};

use serde::Serialize;
use serde_json::Value;

use super::locale::{Locale, MessageKey};
use super::sanitize::sanitize_payload;
use crate::services::error::{ErrorCode, ServiceError};

#[derive(Clone, Debug)]
pub struct CommandSuccess {
    pub command: &'static str,
    pub data: Value,
    pub message: MessageKey,
}

impl CommandSuccess {
    pub const fn new(command: &'static str, data: Value, message: MessageKey) -> Self {
        Self {
            command,
            data,
            message,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum JsonEnvelope<T> {
    Success {
        ok: bool,
        command: &'static str,
        data: T,
    },
    Failure {
        ok: bool,
        command: &'static str,
        code: ErrorCode,
        message: String,
        details: Value,
    },
}

pub const fn exit_code_for_error(code: ErrorCode) -> u8 {
    match code {
        ErrorCode::InvalidArgument | ErrorCode::InvalidSource => 2,
        ErrorCode::SkillNotFound
        | ErrorCode::AmbiguousSkill
        | ErrorCode::MultiSkills
        | ErrorCode::AgentNotFound => 3,
        ErrorCode::ProjectScopeUnsupported
        | ErrorCode::TargetConflict
        | ErrorCode::UpdateHeldBack
        | ErrorCode::ConfirmationRequired
        | ErrorCode::PlanStale => 4,
        ErrorCode::OperationBusy => 5,
        ErrorCode::IncompatibleDatabase => 6,
        ErrorCode::AuthRequired | ErrorCode::NetworkError => 7,
        ErrorCode::InternalError => 10,
    }
}

pub fn write_success(
    writer: &mut impl Write,
    success: &CommandSuccess,
    locale: Locale,
    json: bool,
) -> io::Result<()> {
    let data = sanitize_payload(&success.data);
    if json {
        let envelope = JsonEnvelope::Success {
            ok: true,
            command: success.command,
            data,
        };
        serde_json::to_writer(&mut *writer, &envelope)?;
        writeln!(writer)
    } else {
        writeln!(writer, "{}", locale.text(success.message))
    }
}

pub fn write_failure(
    writer: &mut impl Write,
    command: &'static str,
    error: &ServiceError,
    locale: Locale,
    json: bool,
) -> io::Result<()> {
    let message = locale.error_message(error.code);
    let details = sanitize_payload(&error.details);
    if json {
        let envelope = JsonEnvelope::<Value>::Failure {
            ok: false,
            command,
            code: error.code,
            message: message.to_string(),
            details,
        };
        serde_json::to_writer(&mut *writer, &envelope)?;
        writeln!(writer)
    } else {
        writeln!(writer, "{}: {message}", error.code)
    }
}
