use std::ffi::OsString;

use clap::{Arg, ArgAction, Command};

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
    MissingRequiredArguments,
    Usage,
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
            (Self::En, MessageKey::MissingRequiredArguments) => "Missing required arguments:",
            (Self::En, MessageKey::Usage) => "Usage: ",
            (Self::ZhCn, MessageKey::CommandCompleted) => "命令执行成功。",
            (Self::ZhCn, MessageKey::InvalidArguments) => "命令参数无效。",
            (Self::ZhCn, MessageKey::MissingRequiredArguments) => "缺少必需参数：",
            (Self::ZhCn, MessageKey::Usage) => "用法：",
            (Self::Ko, MessageKey::CommandCompleted) => "명령이 완료되었습니다.",
            (Self::Ko, MessageKey::InvalidArguments) => "명령 인수가 올바르지 않습니다.",
            (Self::Ko, MessageKey::MissingRequiredArguments) => "필수 인수가 없습니다:",
            (Self::Ko, MessageKey::Usage) => "사용법: ",
        }
    }

    pub fn localize_command(self, command: Command) -> Command {
        localize_command(command, self, "")
    }

    const fn help_template(self, has_positionals: bool, has_subcommands: bool) -> &'static str {
        match (self, has_positionals, has_subcommands) {
            (Self::En, true, true) => "{about-with-newline}\nUsage: {usage}\n\nArguments:\n{positionals}\n\nOptions:\n{options}\n\nCommands:\n{subcommands}",
            (Self::En, true, false) => "{about-with-newline}\nUsage: {usage}\n\nArguments:\n{positionals}\n\nOptions:\n{options}",
            (Self::En, false, true) => "{about-with-newline}\nUsage: {usage}\n\nOptions:\n{options}\n\nCommands:\n{subcommands}",
            (Self::En, false, false) => {
                "{about-with-newline}\nUsage: {usage}\n\nOptions:\n{options}"
            }
            (Self::ZhCn, true, true) => "{about-with-newline}\n用法：{usage}\n\n参数：\n{positionals}\n\n选项：\n{options}\n\n命令：\n{subcommands}",
            (Self::ZhCn, true, false) => "{about-with-newline}\n用法：{usage}\n\n参数：\n{positionals}\n\n选项：\n{options}",
            (Self::ZhCn, false, true) => "{about-with-newline}\n用法：{usage}\n\n选项：\n{options}\n\n命令：\n{subcommands}",
            (Self::ZhCn, false, false) => {
                "{about-with-newline}\n用法：{usage}\n\n选项：\n{options}"
            }
            (Self::Ko, true, true) => "{about-with-newline}\n사용법: {usage}\n\n인수:\n{positionals}\n\n옵션:\n{options}\n\n명령:\n{subcommands}",
            (Self::Ko, true, false) => "{about-with-newline}\n사용법: {usage}\n\n인수:\n{positionals}\n\n옵션:\n{options}",
            (Self::Ko, false, true) => "{about-with-newline}\n사용법: {usage}\n\n옵션:\n{options}\n\n명령:\n{subcommands}",
            (Self::Ko, false, false) => {
                "{about-with-newline}\n사용법: {usage}\n\n옵션:\n{options}"
            }
        }
    }

    const fn help_option(self) -> &'static str {
        match self {
            Self::En => "Print help",
            Self::ZhCn => "显示帮助",
            Self::Ko => "도움말 표시",
        }
    }

    const fn version_option(self) -> &'static str {
        match self {
            Self::En => "Print version",
            Self::ZhCn => "显示版本",
            Self::Ko => "버전 표시",
        }
    }

    fn command_about(self, path: &str) -> &'static str {
        match self {
            Self::En => english_command_about(path),
            Self::ZhCn => simplified_chinese_command_about(path),
            Self::Ko => korean_command_about(path),
        }
    }

    fn argument_help(self, path: &str, argument: &str) -> &'static str {
        match self {
            Self::En => english_argument_help(path, argument),
            Self::ZhCn => simplified_chinese_argument_help(path, argument),
            Self::Ko => korean_argument_help(path, argument),
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

fn localize_command(mut command: Command, locale: Locale, path: &str) -> Command {
    let has_positionals = command
        .get_arguments()
        .any(|argument| argument.get_long().is_none() && argument.get_short().is_none());
    let has_subcommands = command.get_subcommands().next().is_some();
    command = command
        .about(locale.command_about(path))
        .help_template(locale.help_template(has_positionals, has_subcommands))
        .disable_help_flag(true)
        .mut_args(|argument| {
            let id = argument.get_id().as_str().to_string();
            let argument = argument.help(locale.argument_help(path, &id));
            if id == "lang" {
                argument.hide_possible_values(true)
            } else {
                argument
            }
        })
        .arg(
            Arg::new("__localized_help")
                .short('h')
                .long("help")
                .help(locale.help_option())
                .action(ArgAction::Help),
        );

    if path.is_empty() {
        command = command.disable_version_flag(true).arg(
            Arg::new("__localized_version")
                .short('V')
                .long("version")
                .help(locale.version_option())
                .action(ArgAction::Version),
        );
    }

    let subcommands = command
        .get_subcommands()
        .map(|subcommand| subcommand.get_name().to_string())
        .collect::<Vec<_>>();
    for name in subcommands {
        let child_path = if path.is_empty() {
            name.clone()
        } else {
            format!("{path}.{name}")
        };
        command = command.mut_subcommand(&name, |subcommand| {
            localize_command(subcommand, locale, &child_path)
        });
    }
    command
}

fn english_command_about(path: &str) -> &'static str {
    match path {
        "" => "Manage Skills Hub from an agent or terminal",
        "skills" => "Find, install, deploy, update, tag, import, or remove Skills",
        "skills.list" => "List installed Skills",
        "skills.show" => "Show details for one Skill",
        "skills.search" => "Search for Skills",
        "skills.status" => "Show deployment and content status for one Skill",
        "skills.check" => "Check one or all Skills for updates",
        "skills.install" => "Install a Skill into the central library without deploying it",
        "skills.deploy" => "Deploy a Skill to one or more explicitly selected Agents",
        "skills.undeploy" => "Remove a managed deployment while keeping the central Skill",
        "skills.update" => "Update one or all installed Skills",
        "skills.adopt" => "Import existing Agent Skills into Skills Hub",
        "skills.tag" => "Manage Skill tags",
        "skills.tag.add" => "Add tags to a Skill",
        "skills.tag.remove" => "Remove tags from a Skill",
        "skills.tag.set" => "Replace all tags on a Skill",
        "skills.tag.list" => "List tags, optionally for one Skill",
        "skills.tag.rename" => "Rename a tag",
        "skills.tag.delete" => "Preview or delete a tag",
        "skills.remove" => "Preview or move a Skill to the Skills Hub recycle bin",
        "agents" => "Inspect supported Agents",
        "agents.list" => "List supported and detected Agents",
        "doctor" => "Inspect Skills Hub data and Agent detection status",
        "version" => "Print the Skills Hub CLI version",
        "setup" => "Install or remove the official skills-hub Skill for one Agent",
        "__bridge" => "Maintain the desktop CLI bridge",
        "__bridge.status" => "Inspect the desktop CLI bridge",
        _ => "Manage Skills Hub",
    }
}

fn simplified_chinese_command_about(path: &str) -> &'static str {
    match path {
        "" => "从 Agent 或终端管理 Skills Hub",
        "skills" => "查找、安装、部署、更新、标记、导入或移除 Skill",
        "skills.list" => "列出已安装的 Skill",
        "skills.show" => "显示一个 Skill 的详细信息",
        "skills.search" => "搜索 Skill",
        "skills.status" => "显示一个 Skill 的部署和内容状态",
        "skills.check" => "检查一个或全部 Skill 的更新",
        "skills.install" => "将 Skill 安装到中央库，但不部署到 Agent",
        "skills.deploy" => "将 Skill 部署到一个或多个明确指定的 Agent",
        "skills.undeploy" => "移除受管部署，但保留中央库中的 Skill",
        "skills.update" => "更新一个或全部已安装的 Skill",
        "skills.adopt" => "将 Agent 中已有的 Skill 导入 Skills Hub",
        "skills.tag" => "管理 Skill 标签",
        "skills.tag.add" => "为 Skill 添加标签",
        "skills.tag.remove" => "从 Skill 移除标签",
        "skills.tag.set" => "替换 Skill 的全部标签",
        "skills.tag.list" => "列出标签，也可只查看一个 Skill",
        "skills.tag.rename" => "重命名标签",
        "skills.tag.delete" => "预览或删除标签",
        "skills.remove" => "预览或将 Skill 移入 Skills Hub 回收站",
        "agents" => "检查支持的 Agent",
        "agents.list" => "列出支持和已检测到的 Agent",
        "doctor" => "检查 Skills Hub 数据和 Agent 检测状态",
        "version" => "显示 Skills Hub CLI 版本",
        "setup" => "为一个 Agent 安装或移除官方 skills-hub Skill",
        "__bridge" => "维护桌面 CLI bridge",
        "__bridge.status" => "检查桌面 CLI bridge",
        _ => "管理 Skills Hub",
    }
}

fn korean_command_about(path: &str) -> &'static str {
    match path {
        "" => "Agent 또는 터미널에서 Skills Hub를 관리합니다",
        "skills" => "Skill 검색, 설치, 배포, 업데이트, 태그, 가져오기 또는 제거",
        "skills.list" => "설치된 Skill 목록을 표시합니다",
        "skills.show" => "Skill 하나의 세부 정보를 표시합니다",
        "skills.search" => "Skill을 검색합니다",
        "skills.status" => "Skill 하나의 배포 및 콘텐츠 상태를 표시합니다",
        "skills.check" => "하나 또는 모든 Skill의 업데이트를 확인합니다",
        "skills.install" => "Agent에 배포하지 않고 중앙 라이브러리에 Skill을 설치합니다",
        "skills.deploy" => "Skill을 하나 이상의 명시적 Agent에 배포합니다",
        "skills.undeploy" => "중앙 Skill은 유지하고 관리되는 배포를 제거합니다",
        "skills.update" => "하나 또는 모든 설치된 Skill을 업데이트합니다",
        "skills.adopt" => "기존 Agent Skill을 Skills Hub로 가져옵니다",
        "skills.tag" => "Skill 태그를 관리합니다",
        "skills.tag.add" => "Skill에 태그를 추가합니다",
        "skills.tag.remove" => "Skill에서 태그를 제거합니다",
        "skills.tag.set" => "Skill의 모든 태그를 교체합니다",
        "skills.tag.list" => "태그 목록을 표시하며 Skill 하나로 제한할 수 있습니다",
        "skills.tag.rename" => "태그 이름을 변경합니다",
        "skills.tag.delete" => "태그 삭제를 미리 보거나 실행합니다",
        "skills.remove" => "Skill 제거를 미리 보거나 Skills Hub 휴지통으로 이동합니다",
        "agents" => "지원되는 Agent를 확인합니다",
        "agents.list" => "지원 및 감지된 Agent 목록을 표시합니다",
        "doctor" => "Skills Hub 데이터 및 Agent 감지 상태를 점검합니다",
        "version" => "Skills Hub CLI 버전을 표시합니다",
        "setup" => "Agent 하나에 공식 skills-hub Skill을 설치하거나 제거합니다",
        "__bridge" => "데스크톱 CLI bridge를 유지 관리합니다",
        "__bridge.status" => "데스크톱 CLI bridge를 확인합니다",
        _ => "Skills Hub 관리",
    }
}

fn english_argument_help(path: &str, argument: &str) -> &'static str {
    match (path, argument) {
        (_, "json") => "Write stable machine-readable JSON",
        (_, "lang") => "Select the human-output language: en, zh-CN, or ko",
        ("skills.list", "tag") => "Filter by tag; repeat for multiple tags",
        ("skills.list", "source") => "Filter by source type; repeat for multiple sources",
        ("skills.list", "agent") => "Filter by deployed Agent; repeat for multiple Agents",
        ("skills.list", "untagged") => "Show only Skills without tags",
        ("skills.list", "status") => "Filter by content status",
        ("skills.search", "query") => "Search query",
        ("skills.search", "limit") => "Maximum number of results",
        ("skills.install", "source") => {
            "Explicit local path, Git reference, or marketplace shorthand"
        }
        ("skills.install", "subpath") => "Skill path inside a repository",
        ("skills.deploy" | "skills.undeploy", "agent") => {
            "Specify at least one target Agent; repeat for multiple Agents"
        }
        ("skills.deploy" | "skills.undeploy", "project") => {
            "Deploy within this project instead of global scope"
        }
        ("skills.adopt", "source") => "Agent Skills directory to scan and import",
        ("setup", "agent") => "Agent that receives the official skills-hub Skill",
        ("setup", "remove") => "Remove the official Skill from the selected Agent",
        (_, "skill") => "Skill name or ID",
        (_, "all") => "Apply to all installed Skills",
        (_, "dry_run") => "Preview the operation without changing state",
        (_, "yes") => "Explicitly confirm the operation",
        (_, "tags") => "One or more tag names",
        (_, "tag") => "Tag name",
        (_, "old") => "Current tag name",
        (_, "new") => "New tag name",
        _ => "Command argument",
    }
}

fn simplified_chinese_argument_help(path: &str, argument: &str) -> &'static str {
    match (path, argument) {
        (_, "json") => "输出稳定的机器可读 JSON",
        (_, "lang") => "选择人类可读输出的语言：en、zh-CN 或 ko",
        ("skills.list", "tag") => "按标签筛选；可重复指定多个标签",
        ("skills.list", "source") => "按来源类型筛选；可重复指定多个来源",
        ("skills.list", "agent") => "按已部署 Agent 筛选；可重复指定多个 Agent",
        ("skills.list", "untagged") => "只显示没有标签的 Skill",
        ("skills.list", "status") => "按内容状态筛选",
        ("skills.search", "query") => "搜索关键词",
        ("skills.search", "limit") => "最多返回的结果数量",
        ("skills.install", "source") => "明确的本地路径、Git 引用或 marketplace 简写",
        ("skills.install", "subpath") => "仓库内的 Skill 路径",
        ("skills.deploy" | "skills.undeploy", "agent") => {
            "至少指定一个目标 Agent；可重复指定多个 Agent"
        }
        ("skills.deploy" | "skills.undeploy", "project") => "在该项目内部署，而不是使用全局范围",
        ("skills.adopt", "source") => "要扫描和导入的 Agent Skills 目录",
        ("setup", "agent") => "接收官方 skills-hub Skill 的 Agent",
        ("setup", "remove") => "从指定 Agent 移除官方 Skill",
        (_, "skill") => "Skill 名称或 ID",
        (_, "all") => "应用到全部已安装的 Skill",
        (_, "dry_run") => "预览操作，不修改任何状态",
        (_, "yes") => "明确确认该操作",
        (_, "tags") => "一个或多个标签名称",
        (_, "tag") => "标签名称",
        (_, "old") => "当前标签名称",
        (_, "new") => "新标签名称",
        _ => "命令参数",
    }
}

fn korean_argument_help(path: &str, argument: &str) -> &'static str {
    match (path, argument) {
        (_, "json") => "안정적인 기계 판독 가능 JSON 출력",
        (_, "lang") => "사람이 읽는 출력 언어 선택: en, zh-CN 또는 ko",
        ("skills.list", "tag") => "태그로 필터링하며 여러 태그에 반복 사용 가능",
        ("skills.list", "source") => "소스 유형으로 필터링하며 반복 사용 가능",
        ("skills.list", "agent") => "배포된 Agent로 필터링하며 반복 사용 가능",
        ("skills.list", "untagged") => "태그가 없는 Skill만 표시",
        ("skills.list", "status") => "콘텐츠 상태로 필터링",
        ("skills.search", "query") => "검색어",
        ("skills.search", "limit") => "최대 결과 수",
        ("skills.install", "source") => "명시적 로컬 경로, Git 참조 또는 marketplace 축약형",
        ("skills.install", "subpath") => "저장소 안의 Skill 경로",
        ("skills.deploy" | "skills.undeploy", "agent") => {
            "대상 Agent를 하나 이상 지정하며 여러 Agent에 반복 사용 가능"
        }
        ("skills.deploy" | "skills.undeploy", "project") => "전역 범위 대신 이 프로젝트 안에 배포",
        ("skills.adopt", "source") => "검색하고 가져올 Agent Skills 디렉터리",
        ("setup", "agent") => "공식 skills-hub Skill을 받을 Agent",
        ("setup", "remove") => "선택한 Agent에서 공식 Skill 제거",
        (_, "skill") => "배포할 Skill 이름 또는 ID",
        (_, "all") => "설치된 모든 Skill에 적용",
        (_, "dry_run") => "상태를 변경하지 않고 작업 미리 보기",
        (_, "yes") => "작업 명시적으로 확인",
        (_, "tags") => "하나 이상의 태그 이름",
        (_, "tag") => "태그 이름",
        (_, "old") => "현재 태그 이름",
        (_, "new") => "새 태그 이름",
        _ => "명령 인수",
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
