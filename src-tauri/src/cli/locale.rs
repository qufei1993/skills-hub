use std::ffi::OsString;

pub use super::args::Language;
use crate::services::error::ErrorCode;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Locale {
    En,
    ZhCn,
    Ko,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MessageKey {
    CommandCompleted,
    InvalidArguments,
}

impl Locale {
    pub fn resolve(language: Option<Language>) -> Self {
        Self::resolve_with(language, |name| std::env::var_os(name))
    }

    pub fn resolve_with<F>(language: Option<Language>, read_environment: F) -> Self
    where
        F: Fn(&str) -> Option<OsString>,
    {
        if let Some(language) = language {
            return language.into();
        }

        for name in ["LC_ALL", "LC_MESSAGES", "LANG"] {
            if let Some(locale) = read_environment(name)
                .as_deref()
                .and_then(Self::from_environment)
            {
                return locale;
            }
        }
        Self::En
    }

    pub const fn text(self, key: MessageKey) -> &'static str {
        match (self, key) {
            (Self::En, MessageKey::CommandCompleted) => "Command completed successfully.",
            (Self::En, MessageKey::InvalidArguments) => "The command arguments are invalid.",
            (Self::ZhCn, MessageKey::CommandCompleted) => "命令执行成功。",
            (Self::ZhCn, MessageKey::InvalidArguments) => "命令参数无效。",
            (Self::Ko, MessageKey::CommandCompleted) => "명령이 완료되었습니다.",
            (Self::Ko, MessageKey::InvalidArguments) => "명령 인수가 올바르지 않습니다.",
        }
    }

    pub const fn error_message(self, code: ErrorCode) -> &'static str {
        match self {
            Self::En => english_error(code),
            Self::ZhCn => simplified_chinese_error(code),
            Self::Ko => korean_error(code),
        }
    }

    fn from_environment(value: &std::ffi::OsStr) -> Option<Self> {
        let normalized = value
            .to_string_lossy()
            .split(['.', '@'])
            .next()
            .unwrap_or_default()
            .replace('_', "-")
            .to_ascii_lowercase();
        if normalized == "zh"
            || normalized.starts_with("zh-cn")
            || normalized.starts_with("zh-hans")
        {
            Some(Self::ZhCn)
        } else if normalized == "ko" || normalized.starts_with("ko-") {
            Some(Self::Ko)
        } else if normalized == "en" || normalized.starts_with("en-") {
            Some(Self::En)
        } else {
            None
        }
    }
}

impl From<Language> for Locale {
    fn from(value: Language) -> Self {
        match value {
            Language::En => Self::En,
            Language::ZhCn => Self::ZhCn,
            Language::Ko => Self::Ko,
        }
    }
}

const fn english_error(code: ErrorCode) -> &'static str {
    match code {
        ErrorCode::InvalidArgument => "The command arguments are invalid.",
        ErrorCode::InvalidSource => "The source is invalid or unsupported.",
        ErrorCode::SkillNotFound => "The Skill was not found.",
        ErrorCode::AmbiguousSkill => "Multiple Skills match this name. Use the full ID.",
        ErrorCode::MultiSkills => "The source contains multiple Skills. Choose one explicitly.",
        ErrorCode::AgentNotFound => "The Agent was not found.",
        ErrorCode::ProjectScopeUnsupported => "This Agent does not support project scope.",
        ErrorCode::TargetConflict => "The target contains content not managed by Skills Hub.",
        ErrorCode::UpdateHeldBack => "The update was held back to protect user files.",
        ErrorCode::ConfirmationRequired => "This operation requires explicit confirmation.",
        ErrorCode::PlanStale => "The preview is stale. Preview the operation again.",
        ErrorCode::OperationBusy => "Another Skills Hub operation is in progress. Try again later.",
        ErrorCode::IncompatibleDatabase => "The database version is incompatible with this CLI.",
        ErrorCode::AuthRequired => {
            "This source requires authorization configured in the desktop app."
        }
        ErrorCode::NetworkError => "The network or remote source request failed.",
        ErrorCode::InternalError => "Skills Hub could not complete the command.",
    }
}

const fn simplified_chinese_error(code: ErrorCode) -> &'static str {
    match code {
        ErrorCode::InvalidArgument => "命令参数无效。",
        ErrorCode::InvalidSource => "无法识别或不支持该来源。",
        ErrorCode::SkillNotFound => "未找到该 Skill。",
        ErrorCode::AmbiguousSkill => "匹配到多个 Skill，请使用完整 ID。",
        ErrorCode::MultiSkills => "来源包含多个 Skill，请明确选择一个。",
        ErrorCode::AgentNotFound => "未找到该 Agent。",
        ErrorCode::ProjectScopeUnsupported => "该 Agent 不支持项目范围。",
        ErrorCode::TargetConflict => "目标包含不受 Skills Hub 管理的内容。",
        ErrorCode::UpdateHeldBack => "为保护用户文件，已暂停更新。",
        ErrorCode::ConfirmationRequired => "此操作需要明确确认。",
        ErrorCode::PlanStale => "预览计划已过期，请重新预览。",
        ErrorCode::OperationBusy => "另一个 Skills Hub 操作正在进行中，请稍后重试。",
        ErrorCode::IncompatibleDatabase => "数据库版本与当前 CLI 不兼容。",
        ErrorCode::AuthRequired => "该来源需要授权，请在桌面端配置凭据。",
        ErrorCode::NetworkError => "网络或远程来源请求失败。",
        ErrorCode::InternalError => "Skills Hub 无法完成该命令。",
    }
}

const fn korean_error(code: ErrorCode) -> &'static str {
    match code {
        ErrorCode::InvalidArgument => "명령 인수가 올바르지 않습니다.",
        ErrorCode::InvalidSource => "소스가 올바르지 않거나 지원되지 않습니다.",
        ErrorCode::SkillNotFound => "Skill을 찾을 수 없습니다.",
        ErrorCode::AmbiguousSkill => "여러 Skill이 이 이름과 일치합니다. 전체 ID를 사용하세요.",
        ErrorCode::MultiSkills => "소스에 여러 Skill이 있습니다. 하나를 명시적으로 선택하세요.",
        ErrorCode::AgentNotFound => "Agent를 찾을 수 없습니다.",
        ErrorCode::ProjectScopeUnsupported => "이 Agent는 프로젝트 범위를 지원하지 않습니다.",
        ErrorCode::TargetConflict => "대상에 Skills Hub가 관리하지 않는 콘텐츠가 있습니다.",
        ErrorCode::UpdateHeldBack => "사용자 파일을 보호하기 위해 업데이트가 보류되었습니다.",
        ErrorCode::ConfirmationRequired => "이 작업은 명시적인 확인이 필요합니다.",
        ErrorCode::PlanStale => "미리 보기 계획이 오래되었습니다. 다시 미리 보세요.",
        ErrorCode::OperationBusy => "다른 Skills Hub 작업이 진행 중입니다. 나중에 다시 시도하세요.",
        ErrorCode::IncompatibleDatabase => "데이터베이스 버전이 이 CLI와 호환되지 않습니다.",
        ErrorCode::AuthRequired => "이 소스에는 데스크톱 앱에서 설정한 인증이 필요합니다.",
        ErrorCode::NetworkError => "네트워크 또는 원격 소스 요청이 실패했습니다.",
        ErrorCode::InternalError => "Skills Hub에서 명령을 완료하지 못했습니다.",
    }
}
