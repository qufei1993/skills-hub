use anyhow::Context;
use serde::{Deserialize, Serialize};
use tauri::{Manager, State};

use std::sync::Arc;

use crate::core::auto_update::{
    get_auto_update_config as get_auto_update_config_core, record_auto_update_triggered,
    run_auto_update_now as run_auto_update_now_core,
    set_auto_update_config as set_auto_update_config_core, AutoUpdateConfig,
    AutoUpdateIntervalUnit, AutoUpdateProgressSnapshot, AutoUpdateRunResult, AutoUpdateSchedule,
    AutoUpdateScheduleType,
};
use crate::core::cache_cleanup::{
    cleanup_git_cache_dirs, get_git_cache_cleanup_days as get_git_cache_cleanup_days_core,
    get_git_cache_ttl_secs as get_git_cache_ttl_secs_core,
    set_git_cache_cleanup_days as set_git_cache_cleanup_days_core,
    set_git_cache_ttl_secs as set_git_cache_ttl_secs_core,
};
use crate::core::cancel_token::CancelToken;
use crate::core::central_repo::{
    ensure_central_repo, plan_central_repo_migration, resolve_central_repo_path,
    validate_central_repo_path_change, CentralRepoMigrationItem,
};
#[cfg(test)]
use crate::core::device_sync::credentials::resolve_access_token;
use crate::core::device_sync::credentials::{
    resolve_access_token_with_proxy, save_personal_access_token, CredentialStore,
    SystemCredentialStore,
};
use crate::core::device_sync::oauth;
use crate::core::device_sync::providers::provider;
use crate::core::device_sync::types::{
    ConflictResolution, CredentialUsage, DeviceSyncConfig, DeviceSyncDevice, OAuthPollResult,
    OAuthProviderAvailability, OAuthStartResult, PendingOAuthAuthorization, ProviderAccount,
    ProviderId, RemoteRepository, SyncChangeSummary, SyncConflict, SyncHistoryEntry, SyncRunResult,
    SyncStatus, TrashEntry,
};
use crate::core::device_sync::DeviceSyncService;
use crate::core::featured_skills::{fetch_featured_skills, FeaturedSkill};
use crate::core::github_search::{search_github_repos, RepoSummary};
use crate::core::github_token::{
    has_github_token, resolve_github_token, set_github_token as set_github_token_core,
    SystemGithubTokenStore,
};
use crate::core::installer::{GitSkillCandidate, LocalSkillCandidate};
use crate::core::network_proxy::{
    app_http_client, get_github_proxy_config as get_github_proxy_config_core,
    get_github_proxy_url as get_github_proxy_url_core,
    set_github_proxy_config as set_github_proxy_config_core,
    set_github_proxy_url as set_github_proxy_url_core, GithubProxyConfig,
};
use crate::core::onboarding::{
    build_onboarding_plan, get_discovery_scan_settings as get_discovery_scan_settings_core,
    save_discovery_scan_config, DiscoveryScanConfig, DiscoveryScanSettings, OnboardingPlan,
};
use crate::core::recycle_bin::{RecycleBinItem, RecycleBinService};
#[cfg(test)]
use crate::core::skill_store::{SkillRecord, SkillTargetRecord};
use crate::core::skill_store::{SkillStore, DEVICE_SYNC_HISTORY_LIMIT};
use crate::core::skills_search::OnlineSkillResult;
use crate::core::sync_engine::{
    copy_dir_recursive, path_is_protected_real_content, paths_overlap,
    remove_path_any as remove_path_any_core, sync_dir_hybrid, sync_dir_with_mode_with_overwrite,
    SyncMode,
};
use crate::core::system_scheduler::{
    current_scheduler_config, get_auto_update_task_status, install_auto_update_task,
    trigger_auto_update_task_now, uninstall_auto_update_task,
};
use crate::core::tool_adapters::{
    is_builtin_tool_enabled, is_tool_installed, load_tool_config, project_relative_skills_dir,
    resolve_default_path, save_tool_config, supports_project_scope, CustomToolConfig, ToolConfig,
};
use crate::services::install::{InstallOutcome, InstallRequest};
use crate::services::library::{AdoptRequest, RemoveRequest, TagAction, TagSelector};
use crate::services::operation_lock::{OperationKind, OperationLock};
use crate::services::skills_hub::SkillsHubService;
use crate::services::types::{Agent as ServiceAgent, Skill as ServiceSkill};
use uuid::Uuid;

const RECENT_PROJECTS_SETTING: &str = "recent_projects_v1";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentAccessAgentDto {
    key: String,
    label: String,
    detected: bool,
    enabled: bool,
    deployed: bool,
    needs_repair: bool,
    reason: Option<crate::services::agent_access::AgentAccessReason>,
    path: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentAccessStatusDto {
    skill_id: Option<String>,
    skill_enabled: bool,
    terminal_ready: bool,
    official_state: crate::services::agent_access::OfficialSkillState,
    conflict: Option<crate::services::agent_access::OfficialSkillConflict>,
    bridge: crate::core::cli_bridge::CliBridgeStatus,
    bundled_version: String,
    installed_version: Option<String>,
    installed: bool,
    central_reason: Option<crate::services::agent_access::AgentAccessReason>,
    agents: Vec<AgentAccessAgentDto>,
}

fn agent_access_dto(
    status: crate::services::agent_access::AgentAccessStatus,
    bridge: crate::core::cli_bridge::CliBridgeStatus,
    terminal_ready: bool,
) -> AgentAccessStatusDto {
    let agents = status
        .agents
        .agents
        .into_iter()
        .map(|agent| {
            let health = status
                .health
                .iter()
                .find(|health| health.agent == agent.key);
            AgentAccessAgentDto {
                deployed: health.is_some_and(|health| health.deployed),
                needs_repair: health.is_some_and(|health| health.needs_repair),
                reason: health.and_then(|health| health.reason),
                key: agent.key,
                label: agent.label,
                detected: agent.detected,
                enabled: agent.enabled,
                path: agent.skills_dir,
            }
        })
        .collect();
    AgentAccessStatusDto {
        skill_enabled: status.skill.as_ref().is_some_and(|skill| skill.enabled),
        terminal_ready,
        skill_id: status.skill_id,
        official_state: status.official_state,
        conflict: status.conflict,
        bridge,
        bundled_version: status.bundled_version,
        installed_version: status.installed_version,
        installed: status.installed,
        central_reason: status.central_reason,
        agents,
    }
}

fn enable_ai_management_impl(
    service: &SkillsHubService,
    source: &std::path::Path,
) -> Result<AgentAccessStatusDto, String> {
    use crate::core::cli_bridge::{publish_bundled_cli_bridge, CliBridgeHealth};
    let bridge = publish_bundled_cli_bridge(source, &service.paths().cli_bridge_dir);
    if bridge.status != CliBridgeHealth::Valid {
        return Err("CLI_UNAVAILABLE".into());
    }
    let status = service
        .enable_ai_management()
        .map_err(format_service_error)?;
    crate::core::cli_terminal::configure(
        service
            .paths()
            .default_central_repo
            .parent()
            .ok_or("CLI_TERMINAL_UNAVAILABLE")?,
        &service.paths().cli_bridge_dir,
    )
    .map_err(|_| "CLI_TERMINAL_UNAVAILABLE".to_string())?;
    Ok(agent_access_dto(status, bridge, true))
}

#[tauri::command]
pub async fn enable_ai_management(
    service: State<'_, SkillsHubService>,
) -> Result<AgentAccessStatusDto, String> {
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let source = tauri::utils::platform::current_exe()
            .ok()
            .and_then(|exe| {
                exe.parent()
                    .map(|dir| dir.join(crate::core::cli_bridge::BINARY_NAME))
            })
            .ok_or_else(|| "CLI_UNAVAILABLE".to_string())?;
        enable_ai_management_impl(&service, &source)
    })
    .await
    .map_err(|_| "INTERNAL_ERROR".to_string())?
}

#[tauri::command]
pub async fn get_agent_access_status(
    service: State<'_, SkillsHubService>,
) -> Result<AgentAccessStatusDto, String> {
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let status = service
            .agent_access_status()
            .map_err(format_service_error)?;
        Ok(agent_access_dto(
            status,
            crate::core::cli_bridge::bundled_cli_bridge_status(&service.paths().cli_bridge_dir),
            service
                .paths()
                .default_central_repo
                .parent()
                .is_some_and(|home| {
                    crate::core::cli_terminal::configured(home, &service.paths().cli_bridge_dir)
                }),
        ))
    })
    .await
    .map_err(|_| "INTERNAL_ERROR".to_string())?
}

fn set_agent_access_impl(
    service: &SkillsHubService,
    agent: String,
    action: String,
    confirmed: bool,
) -> Result<crate::services::agent_access::AgentAccessStatus, String> {
    use crate::services::agent_access::SetupAgentRequest;
    let request = match action.as_str() {
        "install" | "repair" => SetupAgentRequest::install(agent),
        "remove" => SetupAgentRequest {
            agents: vec![agent],
            remove: true,
            confirmed,
            dry_run: false,
        },
        _ => return Err("INVALID_ARGUMENT".to_string()),
    };
    service.setup_agent_access(request).map_err(|error| {
        if error.details["reason"].as_str() == Some("target_modified") {
            "TARGET_MODIFIED".to_string()
        } else {
            error.code.as_str().to_string()
        }
    })
}

#[tauri::command]
pub async fn set_agent_access(
    service: State<'_, SkillsHubService>,
    agent: String,
    action: String,
    confirmed: Option<bool>,
) -> Result<AgentAccessStatusDto, String> {
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let status = set_agent_access_impl(&service, agent, action, confirmed.unwrap_or(false))?;
        Ok(agent_access_dto(
            status,
            crate::core::cli_bridge::bundled_cli_bridge_status(&service.paths().cli_bridge_dir),
            service
                .paths()
                .default_central_repo
                .parent()
                .is_some_and(|home| {
                    crate::core::cli_terminal::configured(home, &service.paths().cli_bridge_dir)
                }),
        ))
    })
    .await
    .map_err(|_| "INTERNAL_ERROR".to_string())?
}
const DEVICE_SYNC_PENDING_OAUTH_SETTING: &str = "device_sync_pending_oauth_v1";
const DEVICE_SYNC_CREDENTIAL_CLEANUP_QUEUE_SETTING: &str =
    "device_sync_credential_cleanup_queue_v1";

fn acquire_operation_lock<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    kind: OperationKind,
) -> anyhow::Result<OperationLock> {
    let paths = crate::runtime_paths_for_tauri(app)?;
    OperationLock::acquire(&paths, kind).map_err(Into::into)
}

fn oauth_proxy_url(store: &SkillStore, _provider_id: ProviderId) -> anyhow::Result<String> {
    get_github_proxy_url_core(store)
}

fn format_anyhow_error(err: anyhow::Error) -> String {
    let first = err.to_string();
    // Frontend relies on these prefixes for special flows.
    if first.starts_with("MULTI_SKILLS|")
        || first.starts_with("TARGET_EXISTS|")
        || first.starts_with("TOOL_NOT_INSTALLED|")
        || first.starts_with("TOOL_NOT_WRITABLE|")
        || first.starts_with("UNSAFE_STORAGE_PATH|")
        || first.starts_with("STORAGE_MIGRATION_CONFIRMATION_REQUIRED|")
        || first.starts_with("SKILL_TARGET_OVERLAPS_SOURCE|")
        || first.starts_with("TARGET_MODIFIED|")
        || first.starts_with("UPDATE_IN_PROGRESS|")
        || first.starts_with("CENTRAL_MODIFIED|")
        || first.starts_with("ROLLBACK_CONFLICT|")
    {
        return first;
    }

    // Include the full error chain (causes), not just the top context.
    let mut full = format!("{:#}", err);

    // Redact noisy temp paths from clone context (we care about the cause, not the dest).
    // Example: `clone https://... into "/Users/.../skills-hub-git-<uuid>"`
    if let Some(head) = full.lines().next() {
        if head.starts_with("clone ") {
            if let Some(pos) = head.find(" into ") {
                let head_redacted = format!("{} (已省略临时目录)", &head[..pos]);
                let rest: String = full.lines().skip(1).collect::<Vec<_>>().join("\n");
                full = if rest.is_empty() {
                    head_redacted
                } else {
                    format!("{}\n{}", head_redacted, rest)
                };
            }
        }
    }

    let root = err.root_cause().to_string();
    let lower = full.to_lowercase();

    // Heuristic-friendly messaging for GitHub clone failures.
    if lower.contains("github.com")
        && (lower.contains("clone ") || lower.contains("remote") || lower.contains("fetch"))
    {
        if lower.contains("securetransport") {
            return format!(
        "无法从 GitHub 拉取仓库：TLS/证书校验失败（macOS SecureTransport）。\n\n建议：\n- 检查网络/代理是否拦截 HTTPS\n- 如在公司网络，可能需要安装公司根证书或使用可信代理\n- 也可在终端确认 `git clone {}` 是否可用\n\n详细：{}",
        "https://github.com/<owner>/<repo>",
        root
      );
        }
        let hint = if lower.contains("authentication")
            || lower.contains("permission denied")
            || lower.contains("credentials")
        {
            "无法访问该仓库：可能是私有仓库/权限不足/需要鉴权。"
        } else if lower.contains("not found") {
            "仓库不存在或无权限访问（GitHub 返回 not found）。"
        } else if lower.contains("failed to resolve")
            || lower.contains("could not resolve")
            || lower.contains("dns")
        {
            "无法解析 GitHub 域名（DNS）。请检查网络/代理。"
        } else if lower.contains("timed out") || lower.contains("timeout") {
            "连接 GitHub 超时。请检查网络/代理。"
        } else if lower.contains("connection refused") || lower.contains("connection reset") {
            "连接 GitHub 失败（连接被拒绝/重置）。请检查网络/代理。"
        } else {
            "无法从 GitHub 拉取仓库。请检查网络/代理，或稍后重试。"
        };

        return format!("{}\n\n详细：{}", hint, root);
    }

    full
}

fn format_service_error(error: crate::services::error::ServiceError) -> String {
    match error.code {
        crate::services::error::ErrorCode::MultiSkills => {
            let candidates = error
                .details
                .get("candidates")
                .cloned()
                .unwrap_or_else(|| serde_json::json!([]));
            format!(
                "MULTI_SKILLS|{}",
                serde_json::to_string(&candidates).unwrap_or_else(|_| "[]".to_string())
            )
        }
        crate::services::error::ErrorCode::UpdateHeldBack => {
            let removal_count = error.details["removal_count"].as_u64().unwrap_or(0);
            format!("UPDATE_HELD_BACK|{removal_count}")
        }
        crate::services::error::ErrorCode::TargetConflict => {
            match (
                error.details["reason"].as_str(),
                error.details["path"].as_str(),
            ) {
                (Some("shared_directory_scope_expansion"), _) => {
                    "SHARED_DIRECTORY_SCOPE_EXPANSION".into()
                }
                (Some("target_modified"), Some(path)) => format!("TARGET_MODIFIED|{path}"),
                (_, Some(path)) => format!("TARGET_EXISTS|{path}"),
                _ => format_anyhow_error(anyhow::anyhow!(error.message)),
            }
        }
        _ => match error.details["legacy_category"].as_str() {
            Some("github_auth") => format_anyhow_error(anyhow::anyhow!(
                "git clone https://github.com/<owner>/<repo> failed: authentication failed"
            )),
            Some("github_network") => format_anyhow_error(anyhow::anyhow!(
                "git clone https://github.com/<owner>/<repo> failed"
            )),
            Some("github_tls") => format_anyhow_error(anyhow::anyhow!(
                "git clone https://github.com/<owner>/<repo> failed: SecureTransport certificate error"
            )),
            Some("github_not_found") => format_anyhow_error(anyhow::anyhow!(
                "git clone https://github.com/<owner>/<repo> failed: repository not found"
            )),
            Some("github_dns") => format_anyhow_error(anyhow::anyhow!(
                "git clone https://github.com/<owner>/<repo> failed: could not resolve host"
            )),
            Some("github_timeout") => format_anyhow_error(anyhow::anyhow!(
                "git clone https://github.com/<owner>/<repo> failed: connection timed out"
            )),
            Some("github_connection") => format_anyhow_error(anyhow::anyhow!(
                "git clone https://github.com/<owner>/<repo> failed: connection refused"
            )),
            Some("github_rate_limited") => {
                "GitHub API 频率限制已触发。可在设置中配置 GitHub Token 以提升限额。"
                    .to_string()
            }
            Some("cancelled") => "CANCELLED|操作已被用户取消。".to_string(),
            _ => format_anyhow_error(anyhow::anyhow!(error.message)),
        },
    }
}

#[derive(Debug, Serialize)]
pub struct ToolInfoDto {
    pub key: String,
    pub label: String,
    pub avatar: Option<String>,
    pub installed: bool,
    pub enabled: bool,
    pub is_custom: bool,
    pub skills_dir: String,
    pub project_skills_dir: String,
    pub supports_project_scope: bool,
    pub sync_mode: SyncMode,
}

#[derive(Debug, Serialize)]
pub struct ToolStatusDto {
    pub tools: Vec<ToolInfoDto>,
    pub installed: Vec<String>,
    pub newly_installed: Vec<String>,
}

impl From<ServiceAgent> for ToolInfoDto {
    fn from(agent: ServiceAgent) -> Self {
        Self {
            key: agent.key,
            label: agent.label,
            avatar: agent.avatar,
            installed: agent.detected,
            enabled: agent.enabled,
            is_custom: agent.is_custom,
            skills_dir: agent.skills_dir,
            project_skills_dir: agent.project_skills_dir,
            supports_project_scope: agent.supports_project_scope,
            sync_mode: agent.sync_mode,
        }
    }
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
struct RuntimeTool {
    key: String,
    label: String,
    avatar: Option<String>,
    installed: bool,
    enabled: bool,
    is_custom: bool,
    skills_dir: std::path::PathBuf,
    project_skills_dir: String,
    supports_project_scope: bool,
    sync_mode: SyncMode,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ToolConfigDto {
    pub disabled_builtin_tools: Vec<String>,
    pub custom_tools: Vec<CustomToolConfigDto>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CustomToolConfigDto {
    pub key: String,
    pub label: String,
    pub avatar: Option<String>,
    pub skills_dir: String,
    pub project_skills_dir: Option<String>,
    pub sync_mode: SyncMode,
    pub enabled: bool,
}

impl From<ToolConfig> for ToolConfigDto {
    fn from(config: ToolConfig) -> Self {
        Self {
            disabled_builtin_tools: config.disabled_builtin_tools,
            custom_tools: config
                .custom_tools
                .into_iter()
                .map(|tool| CustomToolConfigDto {
                    key: tool.key,
                    label: tool.label,
                    avatar: tool.avatar,
                    skills_dir: tool.skills_dir,
                    project_skills_dir: tool.project_skills_dir,
                    sync_mode: tool.sync_mode,
                    enabled: tool.enabled,
                })
                .collect(),
        }
    }
}

impl From<ToolConfigDto> for ToolConfig {
    fn from(config: ToolConfigDto) -> Self {
        Self {
            disabled_builtin_tools: config.disabled_builtin_tools,
            custom_tools: config
                .custom_tools
                .into_iter()
                .map(|tool| CustomToolConfig {
                    key: tool.key,
                    label: tool.label,
                    avatar: tool.avatar,
                    skills_dir: tool.skills_dir,
                    project_skills_dir: tool.project_skills_dir,
                    sync_mode: tool.sync_mode,
                    enabled: tool.enabled,
                })
                .collect(),
        }
    }
}

fn runtime_tools(store: &SkillStore, include_disabled: bool) -> anyhow::Result<Vec<RuntimeTool>> {
    let config = load_tool_config(store)?;
    let mut tools = Vec::new();

    for adapter in crate::core::tool_adapters::default_tool_adapters() {
        let enabled = is_builtin_tool_enabled(&config, adapter.id.as_key());
        if !include_disabled && !enabled {
            continue;
        }
        let detected = is_tool_installed(&adapter)?;
        tools.push(RuntimeTool {
            key: adapter.id.as_key().to_string(),
            label: adapter.display_name.to_string(),
            avatar: None,
            installed: enabled && detected,
            enabled,
            is_custom: false,
            skills_dir: resolve_default_path(&adapter)?,
            project_skills_dir: project_relative_skills_dir(&adapter).to_string(),
            supports_project_scope: cfg!(any(target_os = "macos", target_os = "linux"))
                && supports_project_scope(&adapter),
            sync_mode: SyncMode::Auto,
        });
    }

    for custom in config.custom_tools {
        if !include_disabled && !custom.enabled {
            continue;
        }
        let skills_dir = expand_home_path(&custom.skills_dir)?;
        let supports_project_scope = cfg!(any(target_os = "macos", target_os = "linux"))
            && custom.project_skills_dir.is_some();
        let detected = skills_dir.is_dir();
        tools.push(RuntimeTool {
            key: custom.key,
            label: custom.label,
            avatar: custom.avatar,
            installed: custom.enabled && detected,
            enabled: custom.enabled,
            is_custom: true,
            skills_dir,
            project_skills_dir: custom.project_skills_dir.unwrap_or_default(),
            supports_project_scope,
            sync_mode: custom.sync_mode,
        });
    }

    Ok(tools)
}

#[tauri::command]
pub async fn get_tool_config(store: State<'_, SkillStore>) -> Result<ToolConfigDto, String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || load_tool_config(&store).map(ToolConfigDto::from))
        .await
        .map_err(|err| err.to_string())?
        .map_err(format_anyhow_error)
}

#[tauri::command]
pub async fn set_tool_config(
    store: State<'_, SkillStore>,
    config: ToolConfigDto,
) -> Result<ToolConfigDto, String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        save_tool_config(&store, config.into()).map(ToolConfigDto::from)
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(format_anyhow_error)
}

#[tauri::command]
pub async fn get_tool_status(
    service: State<'_, SkillsHubService>,
    store: State<'_, SkillStore>,
) -> Result<ToolStatusDto, String> {
    let service = service.inner().clone();
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        service
            .list_agents()
            .map(|agents| desktop_tool_status(&store, agents))
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(format_service_error)
}

fn desktop_tool_status(
    store: &SkillStore,
    agents: crate::services::types::AgentList,
) -> ToolStatusDto {
    let serialized = serde_json::to_string(&agents.installed).unwrap_or_else(|_| "[]".to_string());
    let previous = store
        .replace_setting("installed_tools_v1", &serialized)
        .ok()
        .flatten()
        .and_then(|raw| serde_json::from_str::<std::collections::HashSet<String>>(&raw).ok())
        .unwrap_or_default();
    let newly_installed = agents
        .installed
        .iter()
        .filter(|key| !previous.contains(*key))
        .cloned()
        .collect();
    ToolStatusDto {
        tools: agents.agents.into_iter().map(ToolInfoDto::from).collect(),
        installed: agents.installed,
        newly_installed,
    }
}

#[tauri::command]
pub async fn get_onboarding_plan(
    app: tauri::AppHandle,
    store: State<'_, SkillStore>,
) -> Result<OnboardingPlan, String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || build_onboarding_plan(&app, &store))
        .await
        .map_err(|err| err.to_string())?
        .map_err(format_anyhow_error)
}

#[tauri::command]
pub async fn get_discovery_scan_settings(
    store: State<'_, SkillStore>,
) -> Result<DiscoveryScanSettings, String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || get_discovery_scan_settings_core(&store))
        .await
        .map_err(|err| err.to_string())?
        .map_err(format_anyhow_error)
}

#[tauri::command]
pub async fn set_discovery_scan_config(
    store: State<'_, SkillStore>,
    config: DiscoveryScanConfig,
) -> Result<DiscoveryScanSettings, String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        save_discovery_scan_config(&store, config)?;
        get_discovery_scan_settings_core(&store)
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(format_anyhow_error)
}

#[tauri::command]
pub async fn get_git_cache_cleanup_days(store: State<'_, SkillStore>) -> Result<i64, String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        Ok::<_, anyhow::Error>(get_git_cache_cleanup_days_core(&store))
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(format_anyhow_error)
}

#[tauri::command]
pub async fn set_git_cache_cleanup_days(
    store: State<'_, SkillStore>,
    days: i64,
) -> Result<i64, String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || set_git_cache_cleanup_days_core(&store, days))
        .await
        .map_err(|err| err.to_string())?
        .map_err(format_anyhow_error)
}

#[tauri::command]
pub async fn clear_git_cache_now(app: tauri::AppHandle) -> Result<usize, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let paths = crate::runtime_paths_for_tauri(&app)?;
        cleanup_git_cache_dirs(&paths, std::time::Duration::from_secs(0))
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(format_anyhow_error)
}

#[tauri::command]
pub async fn get_git_cache_ttl_secs(store: State<'_, SkillStore>) -> Result<i64, String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        Ok::<_, anyhow::Error>(get_git_cache_ttl_secs_core(&store))
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(format_anyhow_error)
}

#[tauri::command]
pub async fn set_git_cache_ttl_secs(
    store: State<'_, SkillStore>,
    secs: i64,
) -> Result<i64, String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || set_git_cache_ttl_secs_core(&store, secs))
        .await
        .map_err(|err| err.to_string())?
        .map_err(format_anyhow_error)
}

#[derive(Debug, Serialize)]
pub struct AutoUpdateConfigDto {
    pub enabled: bool,
    pub interval_hours: i64,
    pub schedule_type: String,
    pub interval_value: i64,
    pub interval_unit: String,
    pub daily_time: String,
    pub local_skill_count: usize,
    pub protected_local_skill_count: usize,
    pub task_registered: bool,
    pub task_status_detail: String,
    pub last_run_at: Option<i64>,
    pub last_started_at: Option<i64>,
    pub last_finished_at: Option<i64>,
    pub last_status: Option<String>,
    pub last_error: Option<String>,
    pub last_checked: usize,
    pub last_unchanged: usize,
    pub last_updated: usize,
    pub last_failed: usize,
    pub progress: AutoUpdateProgressSnapshot,
}

#[derive(Debug, Serialize)]
pub struct AutoUpdateRuntimeDto {
    pub local_skill_count: usize,
    pub protected_local_skill_count: usize,
    pub last_run_at: Option<i64>,
    pub last_started_at: Option<i64>,
    pub last_finished_at: Option<i64>,
    pub last_status: Option<String>,
    pub last_error: Option<String>,
    pub last_checked: usize,
    pub last_unchanged: usize,
    pub last_updated: usize,
    pub last_failed: usize,
    pub progress: AutoUpdateProgressSnapshot,
}

#[derive(Debug, Serialize)]
pub struct AutoUpdateRunResultDto {
    pub checked: usize,
    pub unchanged: usize,
    pub updated: usize,
    pub failed: usize,
    pub errors: Vec<String>,
    pub progress: AutoUpdateProgressSnapshot,
}

#[derive(Debug, Serialize)]
pub struct GithubProxyConfigDto {
    pub enabled: bool,
    pub port: u16,
    pub url: String,
    pub auto_detected: bool,
}

#[tauri::command]
pub async fn get_auto_update_config(
    store: State<'_, SkillStore>,
) -> Result<AutoUpdateConfigDto, String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        get_auto_update_config_core(&store).map(to_auto_update_config_dto)
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(format_anyhow_error)
}

#[tauri::command]
pub async fn get_auto_update_runtime(
    store: State<'_, SkillStore>,
) -> Result<AutoUpdateRuntimeDto, String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        get_auto_update_config_core(&store).map(to_auto_update_runtime_dto)
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(format_anyhow_error)
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn set_auto_update_config(
    store: State<'_, SkillStore>,
    enabled: bool,
    intervalHours: i64,
    scheduleType: Option<String>,
    intervalValue: Option<i64>,
    intervalUnit: Option<String>,
    dailyTime: Option<String>,
) -> Result<AutoUpdateConfigDto, String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let schedule = build_auto_update_schedule(
            intervalHours,
            scheduleType.as_deref(),
            intervalValue,
            intervalUnit.as_deref(),
            dailyTime.as_deref(),
        )?;
        if enabled {
            let scheduler_config = current_scheduler_config(schedule.clone())?;
            install_auto_update_task(&scheduler_config)?;
        } else {
            uninstall_auto_update_task()?;
        }

        let existing = get_auto_update_config_core(&store)?;
        let saved = set_auto_update_config_core(
            &store,
            AutoUpdateConfig {
                enabled,
                interval_hours: intervalHours,
                schedule,
                local_skill_count: existing.local_skill_count,
                protected_local_skill_count: existing.protected_local_skill_count,
                last_run_at: existing.last_run_at,
                last_started_at: existing.last_started_at,
                last_finished_at: existing.last_finished_at,
                last_status: existing.last_status,
                last_error: existing.last_error,
                last_checked: existing.last_checked,
                last_unchanged: existing.last_unchanged,
                last_updated: existing.last_updated,
                last_failed: existing.last_failed,
                progress: existing.progress,
            },
        )?;
        Ok::<_, anyhow::Error>(to_auto_update_config_dto(saved))
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(format_anyhow_error)
}

fn build_auto_update_schedule(
    legacy_interval_hours: i64,
    schedule_type: Option<&str>,
    interval_value: Option<i64>,
    interval_unit: Option<&str>,
    daily_time: Option<&str>,
) -> anyhow::Result<AutoUpdateSchedule> {
    let schedule_type = match schedule_type.unwrap_or("interval") {
        "daily" => AutoUpdateScheduleType::Daily,
        "interval" => AutoUpdateScheduleType::Interval,
        other => anyhow::bail!("unsupported auto update schedule type: {other}"),
    };
    let interval_unit = match interval_unit.unwrap_or("hours") {
        "minutes" => AutoUpdateIntervalUnit::Minutes,
        "hours" => AutoUpdateIntervalUnit::Hours,
        other => anyhow::bail!("unsupported auto update interval unit: {other}"),
    };
    let schedule = AutoUpdateSchedule {
        schedule_type,
        interval_value: interval_value.unwrap_or(legacy_interval_hours),
        interval_unit,
        daily_time: daily_time.unwrap_or("03:00").to_string(),
    };
    match schedule.schedule_type {
        AutoUpdateScheduleType::Interval => {
            let minutes = schedule.interval_minutes();
            if !(15..=24 * 30 * 60).contains(&minutes) {
                anyhow::bail!("interval minutes must be between 15 and 43200");
            }
        }
        AutoUpdateScheduleType::Daily => {
            let Some((hour, minute)) = schedule.daily_time.split_once(':') else {
                anyhow::bail!("daily time must use HH:mm format");
            };
            if hour.len() != 2 || minute.len() != 2 {
                anyhow::bail!("daily time must use HH:mm format");
            }
            let hour = hour.parse::<u8>().context("parse daily schedule hour")?;
            let minute = minute
                .parse::<u8>()
                .context("parse daily schedule minute")?;
            if hour > 23 || minute > 59 {
                anyhow::bail!("daily time must use HH:mm format");
            }
        }
    }
    Ok(schedule)
}

#[tauri::command]
pub async fn run_auto_update_now(
    app: tauri::AppHandle,
    store: State<'_, SkillStore>,
) -> Result<AutoUpdateRunResultDto, String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _operation_lock = acquire_operation_lock(&app, OperationKind::AutoUpdate)?;
        run_auto_update_now_core(&app, &store).map(to_auto_update_run_result_dto)
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(format_anyhow_error)
}

#[tauri::command]
pub async fn trigger_auto_update_task_now_cmd(store: State<'_, SkillStore>) -> Result<(), String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let config = get_auto_update_config_core(&store)?;
        let scheduler_config = current_scheduler_config(config.schedule)?;
        install_auto_update_task(&scheduler_config)?;
        record_auto_update_triggered(&store)?;
        trigger_auto_update_task_now()
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(format_anyhow_error)
}

#[derive(Debug, Serialize)]
pub struct InstallResultDto {
    pub skill_id: String,
    pub name: String,
    pub central_path: String,
    pub content_hash: Option<String>,
}

fn expand_home_path(input: &str) -> Result<std::path::PathBuf, anyhow::Error> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        anyhow::bail!("storage path is empty");
    }
    if trimmed == "~" {
        let home = dirs::home_dir().context("failed to resolve home directory")?;
        return Ok(home);
    }
    if let Some(stripped) = trimmed.strip_prefix("~/") {
        let home = dirs::home_dir().context("failed to resolve home directory")?;
        return Ok(home.join(stripped));
    }
    Ok(std::path::PathBuf::from(trimmed))
}

fn normalize_scope(scope: Option<&str>) -> Result<&'static str, anyhow::Error> {
    match scope.unwrap_or("global") {
        "global" => Ok("global"),
        "project" => Ok("project"),
        other => anyhow::bail!("invalid scope: {}", other),
    }
}

#[tauri::command]
pub async fn get_recent_projects(store: State<'_, SkillStore>) -> Result<Vec<String>, String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || get_recent_projects_impl(&store))
        .await
        .map_err(|err| err.to_string())?
        .map_err(format_anyhow_error)
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn save_recent_project(
    store: State<'_, SkillStore>,
    projectPath: String,
) -> Result<Vec<String>, String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || save_recent_project_impl(&store, &projectPath))
        .await
        .map_err(|err| err.to_string())?
        .map_err(format_anyhow_error)
}

fn get_recent_projects_impl(store: &SkillStore) -> Result<Vec<String>, anyhow::Error> {
    let projects = store
        .get_setting(RECENT_PROJECTS_SETTING)?
        .and_then(|raw| serde_json::from_str::<Vec<String>>(&raw).ok())
        .unwrap_or_default();
    Ok(projects)
}

fn save_recent_project_impl(
    store: &SkillStore,
    project_path: &str,
) -> Result<Vec<String>, anyhow::Error> {
    let path = expand_home_path(project_path)?;
    if !path.is_dir() {
        anyhow::bail!("projectPath must be an existing directory: {:?}", path);
    }
    let normalized = path.to_string_lossy().to_string();
    let mut projects = get_recent_projects_impl(store)?;
    projects.retain(|item| item != &normalized);
    projects.insert(0, normalized);
    projects.truncate(8);
    store.set_setting(
        RECENT_PROJECTS_SETTING,
        &serde_json::to_string(&projects).unwrap_or_else(|_| "[]".to_string()),
    )?;
    Ok(projects)
}

#[tauri::command]
pub async fn get_central_repo_path(
    app: tauri::AppHandle,
    store: State<'_, SkillStore>,
) -> Result<String, String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let paths = crate::runtime_paths_for_tauri(&app)?;
        let path = resolve_central_repo_path(&paths, &store)?;
        ensure_central_repo(&path)?;
        Ok::<_, anyhow::Error>(path.to_string_lossy().to_string())
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(format_anyhow_error)
}

#[derive(Debug, Serialize)]
pub struct StoragePathChangePreviewDto {
    pub current_path: String,
    pub new_path: String,
    pub skill_count: usize,
}

#[derive(Clone, Debug)]
struct StorageLinkMigration {
    mode: SyncMode,
    old_source: std::path::PathBuf,
    new_source: std::path::PathBuf,
    target: std::path::PathBuf,
}

fn recycle_new_storage_copies(plan: &[CentralRepoMigrationItem]) {
    for item in plan {
        if std::fs::symlink_metadata(&item.new_path).is_ok() {
            if let Err(err) = remove_path_any_core(&item.new_path) {
                eprintln!(
                    "failed to recycle incomplete storage copy {:?}: {err:#}",
                    item.new_path
                );
            }
        }
    }
}

fn rollback_central_repo_migration(
    plan: &[CentralRepoMigrationItem],
    links: &[StorageLinkMigration],
    attempted_link_count: usize,
) {
    let mut links_restored = true;
    for link in links[..attempted_link_count].iter().rev() {
        if let Err(err) =
            sync_dir_with_mode_with_overwrite(link.mode, &link.old_source, &link.target, true)
        {
            links_restored = false;
            eprintln!("failed to restore Skill link {:?}: {err:#}", link.target);
        }
    }
    if links_restored {
        recycle_new_storage_copies(plan);
    } else {
        eprintln!("keeping new storage copies because one or more links could not be restored");
    }
}

fn storage_path_change_plan(
    store: &SkillStore,
    current_base: &std::path::Path,
    new_base: &std::path::Path,
) -> anyhow::Result<Vec<CentralRepoMigrationItem>> {
    let skills = store.list_skills()?;
    let mut tool_roots = runtime_tools(store, true)?
        .into_iter()
        .map(|tool| tool.skills_dir)
        .collect::<Vec<_>>();
    for (_, target_path) in store.list_all_skill_target_paths()? {
        if let Some(parent) = std::path::Path::new(&target_path).parent() {
            tool_roots.push(parent.to_path_buf());
        }
    }
    let local_sources = skills
        .iter()
        .filter_map(|skill| skill.external_local_source())
        .map(std::path::PathBuf::from)
        .collect::<Vec<_>>();
    validate_central_repo_path_change(current_base, new_base, &tool_roots, &local_sources)?;
    plan_central_repo_migration(&skills, new_base)
}

#[tauri::command]
pub async fn preview_central_repo_path_change(
    app: tauri::AppHandle,
    store: State<'_, SkillStore>,
    path: String,
) -> Result<StoragePathChangePreviewDto, String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let new_base = expand_home_path(&path)?;
        if !new_base.is_absolute() {
            anyhow::bail!("storage path must be absolute");
        }
        let paths = crate::runtime_paths_for_tauri(&app)?;
        let current_base = resolve_central_repo_path(&paths, &store)?;
        let skill_count = if current_base == new_base {
            0
        } else {
            storage_path_change_plan(&store, &current_base, &new_base)?.len()
        };
        Ok::<_, anyhow::Error>(StoragePathChangePreviewDto {
            current_path: current_base.to_string_lossy().to_string(),
            new_path: new_base.to_string_lossy().to_string(),
            skill_count,
        })
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(format_anyhow_error)
}

#[tauri::command]
pub async fn set_central_repo_path(
    app: tauri::AppHandle,
    store: State<'_, SkillStore>,
    path: String,
    confirmed: Option<bool>,
) -> Result<String, String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _operation_lock = acquire_operation_lock(&app, OperationKind::StorageMigration)?;
        let new_base = expand_home_path(&path)?;
        if !new_base.is_absolute() {
            anyhow::bail!("storage path must be absolute");
        }
        let paths = crate::runtime_paths_for_tauri(&app)?;
        let current_base = resolve_central_repo_path(&paths, &store)?;
        if current_base == new_base {
            store.set_setting("central_repo_path", new_base.to_string_lossy().as_ref())?;
            return Ok::<_, anyhow::Error>(new_base.to_string_lossy().to_string());
        }

        let plan = storage_path_change_plan(&store, &current_base, &new_base)?;
        if !plan.is_empty() && confirmed != Some(true) {
            anyhow::bail!("STORAGE_MIGRATION_CONFIRMATION_REQUIRED|{}", plan.len());
        }
        ensure_central_repo(&new_base)?;

        let mut links = Vec::new();
        for item in &plan {
            let protected_paths = skill_protected_paths(&store, &item.skill.id)?;
            for target in store.list_skill_targets(&item.skill.id)? {
                if target.status == "disabled" {
                    continue;
                }
                let mode = match target.mode.as_str() {
                    "symlink" => Some(SyncMode::Symlink),
                    "junction" => Some(SyncMode::Junction),
                    _ => None,
                };
                if let Some(mode) = mode {
                    let target_path = std::path::PathBuf::from(&target.target_path);
                    ensure_target_does_not_overlap_local_source(
                        &store,
                        &item.skill.id,
                        &target_path,
                    )?;
                    if path_is_protected_real_content(&target_path, &protected_paths)? {
                        anyhow::bail!(
                            "refusing to replace protected Skill path during storage migration: {:?}",
                            target_path
                        );
                    }
                    links.push(StorageLinkMigration {
                        mode,
                        old_source: item.old_path.clone(),
                        new_source: item.new_path.clone(),
                        target: target_path,
                    });
                }
            }
        }

        for item in &plan {
            if let Err(err) = copy_dir_recursive(&item.old_path, &item.new_path)
                .with_context(|| format!("copy {:?} -> {:?}", item.old_path, item.new_path))
            {
                recycle_new_storage_copies(&plan);
                return Err(err);
            }
        }

        for (index, link) in links.iter().enumerate() {
            if let Err(err) = sync_dir_with_mode_with_overwrite(
                link.mode,
                &link.new_source,
                &link.target,
                true,
            )
            .with_context(|| format!("refresh moved Skill target {:?}", link.target))
            {
                rollback_central_repo_migration(&plan, &links, index + 1);
                return Err(err);
            }
        }

        let updated_at = now_ms();
        let updates = plan
            .iter()
            .map(|item| {
                (
                    item.skill.id.clone(),
                    item.new_path.to_string_lossy().to_string(),
                    updated_at,
                )
            })
            .collect::<Vec<_>>();
        if let Err(err) = store.commit_central_repo_migration(
            &updates,
            new_base.to_string_lossy().as_ref(),
        ) {
            rollback_central_repo_migration(&plan, &links, links.len());
            return Err(err);
        }

        for item in &plan {
            if let Err(err) = remove_path_any_core(&item.old_path) {
                eprintln!(
                    "storage migration succeeded but old path could not be recycled {:?}: {err:#}",
                    item.old_path
                );
            }
        }
        Ok::<_, anyhow::Error>(new_base.to_string_lossy().to_string())
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(format_anyhow_error)
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn install_local(
    service: State<'_, SkillsHubService>,
    sourcePath: String,
    name: Option<String>,
) -> Result<InstallResultDto, String> {
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        service
            .install(InstallRequest::local(sourcePath).with_name(name))
            .map(to_service_install_dto)
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(format_service_error)
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn list_local_skills_cmd(
    service: State<'_, SkillsHubService>,
    basePath: String,
) -> Result<Vec<LocalSkillCandidate>, String> {
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || service.local_install_candidates(basePath))
        .await
        .map_err(|err| err.to_string())?
        .map_err(format_service_error)
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn install_local_selection(
    service: State<'_, SkillsHubService>,
    basePath: String,
    subpath: String,
    name: Option<String>,
) -> Result<InstallResultDto, String> {
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        service
            .install(
                InstallRequest::local(basePath)
                    .with_subpath(subpath)
                    .with_name(name),
            )
            .map(to_service_install_dto)
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(format_service_error)
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn install_git(
    service: State<'_, SkillsHubService>,
    cancel: State<'_, Arc<CancelToken>>,
    repoUrl: String,
    name: Option<String>,
) -> Result<InstallResultDto, String> {
    let service = service.inner().clone();
    cancel.reset();
    let cancel_token = Arc::clone(cancel.inner());
    tauri::async_runtime::spawn_blocking(move || {
        service
            .install_with_cancel(
                InstallRequest::git(repoUrl).with_name(name),
                Some(&cancel_token),
            )
            .map(to_service_install_dto)
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(format_service_error)
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn list_git_skills_cmd(
    service: State<'_, SkillsHubService>,
    cancel: State<'_, Arc<CancelToken>>,
    repoUrl: String,
) -> Result<Vec<GitSkillCandidate>, String> {
    let service = service.inner().clone();
    cancel.reset();
    let cancel_token = Arc::clone(cancel.inner());
    tauri::async_runtime::spawn_blocking(move || {
        service.git_install_candidates_with_cancel(&repoUrl, Some(&cancel_token))
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(format_service_error)
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn install_git_selection(
    service: State<'_, SkillsHubService>,
    cancel: State<'_, Arc<CancelToken>>,
    repoUrl: String,
    subpath: String,
    name: Option<String>,
) -> Result<InstallResultDto, String> {
    let service = service.inner().clone();
    cancel.reset();
    let cancel_token = Arc::clone(cancel.inner());
    tauri::async_runtime::spawn_blocking(move || {
        service
            .install_with_cancel(
                InstallRequest::git(repoUrl)
                    .with_subpath(subpath)
                    .with_name(name),
                Some(&cancel_token),
            )
            .map(to_service_install_dto)
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(format_service_error)
}

#[derive(Debug, Serialize)]
pub struct SyncResultDto {
    pub mode_used: String,
    pub target_path: String,
}

#[tauri::command]
pub async fn sync_skill_dir(
    app: tauri::AppHandle,
    source_path: String,
    target_path: String,
) -> Result<SyncResultDto, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _operation_lock = acquire_operation_lock(&app, OperationKind::Deploy)?;
        let result = sync_dir_hybrid(source_path.as_ref(), target_path.as_ref())?;
        Ok::<_, anyhow::Error>(SyncResultDto {
            mode_used: match result.mode_used {
                SyncMode::Auto => "auto",
                SyncMode::Symlink => "symlink",
                SyncMode::Junction => "junction",
                SyncMode::Copy => "copy",
            }
            .to_string(),
            target_path: result.target_path.to_string_lossy().to_string(),
        })
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(format_anyhow_error)
}

#[tauri::command]
#[allow(non_snake_case)]
#[allow(clippy::too_many_arguments)]
pub async fn sync_skill_to_tool(
    service: State<'_, SkillsHubService>,
    sourcePath: String,
    skillId: String,
    tool: String,
    name: String,
    overwrite: Option<bool>,
    overwriteIfSameContent: Option<bool>,
    scope: Option<String>,
    projectPath: Option<String>,
) -> Result<SyncResultDto, String> {
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let skill = service
            .show_skill(skillId.clone().into())
            .map_err(format_service_error)?;
        if skill.central_path != sourcePath || skill.name != name {
            return Err("PLAN_STALE|skill source or name changed".to_string());
        }
        let mut request = desktop_deployment_request(skillId, tool, scope, projectPath)?;
        request.overwrite = overwrite.unwrap_or(false);
        request.overwrite_if_same_content = overwriteIfSameContent.unwrap_or(false);
        let outcome = service.deploy(request).map_err(format_deployment_error)?;
        let target = outcome
            .targets
            .first()
            .ok_or_else(|| "deployment returned no target".to_string())?;
        Ok(SyncResultDto {
            mode_used: crate::services::deployment::mode_name(target.mode).to_string(),
            target_path: target.path.to_string_lossy().into_owned(),
        })
    })
    .await
    .map_err(|error| error.to_string())?
}

fn desktop_deployment_request(
    skill_id: String,
    tool: String,
    scope: Option<String>,
    project_path: Option<String>,
) -> Result<crate::services::deployment::DeploymentRequest, String> {
    let mut request = crate::services::deployment::DeploymentRequest::global(
        crate::services::types::SkillSelector::Id(skill_id),
        [tool],
    );
    if normalize_scope(scope.as_deref()).map_err(format_anyhow_error)? == "project" {
        request.scope = crate::services::deployment::DeploymentScope::Project(
            project_path
                .ok_or_else(|| "projectPath is required for project scope".to_string())?
                .into(),
        );
    }
    Ok(request)
}

fn format_deployment_error(error: crate::services::error::ServiceError) -> String {
    use crate::services::error::ErrorCode;
    let agent = error.details["agent"].as_str().unwrap_or_default();
    let path = error.details["path"].as_str().unwrap_or_default();
    match error.code {
        ErrorCode::AgentNotFound => format!("TOOL_NOT_INSTALLED|{agent}"),
        ErrorCode::ProjectScopeUnsupported => format!("PROJECT_SCOPE_UNSUPPORTED|{agent}"),
        ErrorCode::TargetConflict if error.details["reason"] == "not_writable" => {
            format!("TOOL_NOT_WRITABLE|{agent}|{path}")
        }
        ErrorCode::TargetConflict if error.details["reason"] == "overlaps_skill_source" => format!(
            "SKILL_TARGET_OVERLAPS_SOURCE|{path}|sync target overlaps original local source"
        ),
        ErrorCode::PlanStale => "PLAN_STALE|deployment state changed".to_string(),
        _ => format_service_error(error),
    }
}

fn skill_protected_paths(
    store: &SkillStore,
    skill_id: &str,
) -> anyhow::Result<Vec<std::path::PathBuf>> {
    let Some(skill) = store.get_skill_by_id(skill_id)? else {
        return Ok(Vec::new());
    };
    let mut paths = vec![std::path::PathBuf::from(&skill.central_path)];
    if let Some(source) = skill.external_local_source() {
        paths.push(std::path::PathBuf::from(source));
    }
    Ok(paths)
}

fn ensure_target_does_not_overlap_local_source(
    store: &SkillStore,
    skill_id: &str,
    target: &std::path::Path,
) -> anyhow::Result<()> {
    let Some(skill) = store.get_skill_by_id(skill_id)? else {
        return Ok(());
    };
    if let Some(source) = skill.external_local_source() {
        let source = std::path::PathBuf::from(source);
        if paths_overlap(target, &source)? {
            anyhow::bail!(
                "SKILL_TARGET_OVERLAPS_SOURCE|{}|sync target overlaps original local source",
                source.to_string_lossy()
            );
        }
    }
    Ok(())
}

fn remove_skill_target_safely(
    store: &SkillStore,
    skill_id: &str,
    target: &str,
) -> anyhow::Result<()> {
    let path = std::path::Path::new(target);
    let protected_paths = skill_protected_paths(store, skill_id)?;
    if path_is_protected_real_content(path, &protected_paths)? {
        return Ok(());
    }
    remove_path_any_core(path)
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn unsync_skill_from_tool(
    service: State<'_, SkillsHubService>,
    skillId: String,
    tool: String,
    scope: Option<String>,
    projectPath: Option<String>,
) -> Result<(), String> {
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let request = desktop_deployment_request(skillId, tool, scope, projectPath)?;
        service
            .undeploy(request)
            .map(|_| ())
            .map_err(format_deployment_error)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn set_skill_enabled(
    app: tauri::AppHandle,
    store: State<'_, SkillStore>,
    skillId: String,
    enabled: bool,
) -> Result<(), String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let operation_kind = if enabled {
            OperationKind::Deploy
        } else {
            OperationKind::Undeploy
        };
        let _operation_lock = acquire_operation_lock(&app, operation_kind)?;
        if !enabled {
            let targets = store.list_skill_targets(&skillId)?;
            let mut remove_failures: Vec<String> = Vec::new();
            for target in targets {
                if target.status != "disabled" {
                    if let Err(err) =
                        remove_skill_target_safely(&store, &skillId, &target.target_path)
                    {
                        remove_failures.push(format!("{}: {}", target.target_path, err));
                    }
                }
                store.update_skill_target_status(
                    &skillId,
                    &target.tool,
                    &target.scope,
                    target.project_path.as_deref(),
                    "disabled",
                )?;
            }
            store.set_skill_enabled(&skillId, false)?;
            if !remove_failures.is_empty() {
                anyhow::bail!(
                    "已停用 Skill，但清理部分工具目录失败：\n- {}",
                    remove_failures.join("\n- ")
                );
            }
            return Ok::<_, anyhow::Error>(());
        }

        store.set_skill_enabled(&skillId, true)?;
        Ok::<_, anyhow::Error>(())
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(format_anyhow_error)
}

#[derive(Debug, Serialize)]
pub struct UpdateResultDto {
    pub skill_id: String,
    pub name: String,
    pub content_hash: Option<String>,
    pub source_revision: Option<String>,
    pub updated_targets: Vec<String>,
    pub pending_targets: Vec<String>,
    pub changed: bool,
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn update_managed_skill(
    service: State<'_, SkillsHubService>,
    skillId: String,
) -> Result<UpdateResultDto, String> {
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        service.update(skillId.into()).map(|res| UpdateResultDto {
            skill_id: res.id,
            name: res.name,
            content_hash: res.content_hash,
            source_revision: res.source_revision,
            updated_targets: res.updated_targets,
            pending_targets: res.pending_targets,
            changed: res.changed,
        })
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(format_service_error)
}

#[tauri::command]
pub async fn search_github(
    store: State<'_, SkillStore>,
    query: String,
    limit: Option<u32>,
) -> Result<Vec<RepoSummary>, String> {
    let store = store.inner().clone();
    let limit = limit.unwrap_or(10) as usize;
    tauri::async_runtime::spawn_blocking(move || {
        let proxy_url = get_github_proxy_url_core(&store)?;
        let credentials = SystemGithubTokenStore;
        let token = resolve_github_token(&store, &credentials)?;
        search_github_repos(&query, limit, token.as_deref(), &proxy_url)
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(format_anyhow_error)
}

#[derive(Debug, Deserialize)]
struct GithubReleaseApiResponse {
    body: Option<String>,
}

#[tauri::command]
pub async fn get_github_release_notes(
    store: State<'_, SkillStore>,
    version: String,
) -> Result<Option<String>, String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let proxy_url = get_github_proxy_url_core(&store)?;
        let tag = format!("v{}", version.trim().trim_start_matches('v'));
        let url = format!(
            "https://api.github.com/repos/qufei1993/skills-hub/releases/tags/{}",
            urlencoding::encode(&tag)
        );
        let client = app_http_client(&proxy_url, Some(20))?;
        let response = client
            .get(url)
            .header("User-Agent", "skills-hub")
            .header("Accept", "application/vnd.github+json")
            .send()
            .context("GitHub release notes request failed")?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        let response = response
            .error_for_status()
            .context("GitHub release notes returned error")?;
        let result: GithubReleaseApiResponse = response
            .json()
            .context("parse GitHub release notes response")?;
        Ok(result.body)
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(format_anyhow_error)
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct GithubTokenStatusDto {
    pub has_token: bool,
}

fn get_github_token_status_impl(
    store: &SkillStore,
    credentials: &dyn CredentialStore,
) -> anyhow::Result<GithubTokenStatusDto> {
    Ok(GithubTokenStatusDto {
        has_token: has_github_token(store, credentials)?,
    })
}

#[tauri::command]
pub async fn get_github_token_status(
    store: State<'_, SkillStore>,
) -> Result<GithubTokenStatusDto, String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        get_github_token_status_impl(&store, &SystemGithubTokenStore)
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(format_anyhow_error)
}

#[tauri::command]
pub async fn set_github_token(store: State<'_, SkillStore>, token: String) -> Result<(), String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        set_github_token_core(&store, &SystemGithubTokenStore, &token)
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(format_anyhow_error)
}

#[tauri::command]
pub async fn get_github_proxy_config(
    store: State<'_, SkillStore>,
) -> Result<GithubProxyConfigDto, String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        get_github_proxy_config_core(&store).map(to_github_proxy_config_dto)
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(format_anyhow_error)
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn set_github_proxy_config(
    store: State<'_, SkillStore>,
    enabled: bool,
    port: u16,
) -> Result<GithubProxyConfigDto, String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        set_github_proxy_config_core(&store, enabled, port).map(to_github_proxy_config_dto)
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(format_anyhow_error)
}

#[tauri::command]
pub async fn get_github_proxy_url(store: State<'_, SkillStore>) -> Result<String, String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || get_github_proxy_url_core(&store))
        .await
        .map_err(|err| err.to_string())?
        .map_err(format_anyhow_error)
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn set_github_proxy_url(
    store: State<'_, SkillStore>,
    proxyUrl: String,
) -> Result<String, String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || set_github_proxy_url_core(&store, &proxyUrl))
        .await
        .map_err(|err| err.to_string())?
        .map_err(format_anyhow_error)
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn import_existing_skill(
    service: State<'_, SkillsHubService>,
    sourcePath: String,
    name: Option<String>,
) -> Result<InstallResultDto, String> {
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let plan = service.plan_adopt_direct_with_name(&sourcePath, name)?;
        let mut outcome = service.adopt(AdoptRequest::confirmed(plan.id))?;
        let adopted = outcome.adopted.pop().ok_or_else(|| {
            crate::services::error::ServiceError::internal("failed to import the selected skill")
        })?;
        Ok::<_, crate::services::error::ServiceError>(to_service_install_dto(adopted))
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(format_service_error)
}

#[derive(Debug, Serialize)]
pub struct ManagedSkillDto {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub source_type: String,
    pub source_ref: Option<String>,
    pub central_path: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub last_sync_at: Option<i64>,
    pub enabled: bool,
    pub status: String,
    pub source_error: Option<String>,
    pub source_checked_at: Option<i64>,
    pub tags: Vec<TagDto>,
    pub targets: Vec<SkillTargetDto>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TagDto {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct TagWithCountDto {
    pub id: i64,
    pub name: String,
    pub skill_count: i64,
    pub updated_at: i64,
}

#[derive(Debug, Serialize)]
pub struct SkillTargetDto {
    pub tool: String,
    pub scope: String,
    pub project_path: Option<String>,
    pub mode: String,
    pub status: String,
    pub last_error: Option<String>,
    pub target_path: String,
    pub synced_at: Option<i64>,
}

impl From<ServiceSkill> for ManagedSkillDto {
    fn from(skill: ServiceSkill) -> Self {
        Self {
            id: skill.id,
            name: skill.name,
            description: skill.description,
            source_type: skill.source.kind,
            source_ref: skill.source.reference,
            central_path: skill.central_path,
            created_at: skill.created_at,
            updated_at: skill.updated_at,
            last_sync_at: skill.last_sync_at,
            enabled: skill.enabled,
            status: skill.content_status,
            source_error: skill.source_error,
            source_checked_at: skill.source_checked_at,
            tags: skill
                .tags
                .into_iter()
                .map(|tag| TagDto {
                    id: tag.id,
                    name: tag.name,
                })
                .collect(),
            targets: skill
                .targets
                .into_iter()
                .map(|target| SkillTargetDto {
                    tool: target.tool,
                    scope: target.scope,
                    project_path: target.project_path,
                    mode: target.mode,
                    status: target.status,
                    last_error: target.last_error,
                    target_path: target.target_path,
                    synced_at: target.synced_at,
                })
                .collect(),
        }
    }
}

#[tauri::command]
pub fn get_managed_skills(
    service: State<'_, SkillsHubService>,
) -> Result<Vec<ManagedSkillDto>, String> {
    service
        .list_skills()
        .map(|skills| skills.into_iter().map(ManagedSkillDto::from).collect())
        .map_err(format_service_error)
}

#[tauri::command]
pub fn get_tags(store: State<'_, SkillStore>) -> Result<Vec<TagWithCountDto>, String> {
    store
        .list_tags_with_counts()
        .map(|tags| {
            tags.into_iter()
                .map(|tag| TagWithCountDto {
                    id: tag.id,
                    name: tag.name,
                    skill_count: tag.skill_count,
                    updated_at: tag.updated_at,
                })
                .collect()
        })
        .map_err(format_anyhow_error)
}

#[tauri::command]
#[allow(non_snake_case)]
pub fn create_tag(service: State<'_, SkillsHubService>, name: String) -> Result<TagDto, String> {
    service
        .apply_tag_action(TagAction::Create { name })
        .and_then(|outcome| {
            outcome.tag.ok_or_else(|| {
                crate::services::error::ServiceError::internal("created tag is missing")
            })
        })
        .map(|tag| TagDto {
            id: tag.id,
            name: tag.name,
        })
        .map_err(format_service_error)
}

#[tauri::command]
#[allow(non_snake_case)]
pub fn rename_tag(
    service: State<'_, SkillsHubService>,
    tagId: i64,
    name: String,
) -> Result<TagDto, String> {
    service
        .apply_tag_action(TagAction::Rename {
            tag: TagSelector::Id(tagId),
            name,
        })
        .and_then(|outcome| {
            outcome.tag.ok_or_else(|| {
                crate::services::error::ServiceError::internal("renamed tag is missing")
            })
        })
        .map(|tag| TagDto {
            id: tag.id,
            name: tag.name,
        })
        .map_err(format_service_error)
}

#[tauri::command]
#[allow(non_snake_case)]
pub fn delete_tag(service: State<'_, SkillsHubService>, tagId: i64) -> Result<(), String> {
    service
        .plan_tag_delete(TagSelector::Id(tagId))
        .and_then(|plan| {
            service.apply_tag_action(TagAction::Delete {
                plan_id: plan.id,
                confirmed: true,
            })
        })
        .map(|_| ())
        .map_err(format_service_error)
}

#[tauri::command]
#[allow(non_snake_case)]
pub fn get_skill_tags(
    store: State<'_, SkillStore>,
    skillId: String,
) -> Result<Vec<TagDto>, String> {
    store
        .get_skill_tags(&skillId)
        .map(|tags| {
            tags.into_iter()
                .map(|tag| TagDto {
                    id: tag.id,
                    name: tag.name,
                })
                .collect()
        })
        .map_err(format_anyhow_error)
}

#[tauri::command]
#[allow(non_snake_case)]
pub fn set_skill_tags(
    service: State<'_, SkillsHubService>,
    skillId: String,
    tagIds: Vec<i64>,
) -> Result<(), String> {
    service
        .apply_tag_action(TagAction::SetIds {
            skill: crate::services::types::SkillSelector::Id(skillId),
            tag_ids: tagIds,
        })
        .map(|_| ())
        .map_err(format_service_error)
}

#[tauri::command]
pub fn get_untagged_skill_ids(store: State<'_, SkillStore>) -> Result<Vec<String>, String> {
    store.list_untagged_skill_ids().map_err(format_anyhow_error)
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn delete_managed_skill(
    service: State<'_, SkillsHubService>,
    skillId: String,
) -> Result<(), String> {
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let plan = match service.plan_remove(crate::services::types::SkillSelector::Id(skillId)) {
            Ok(plan) => plan,
            Err(error) if error.code == crate::services::error::ErrorCode::SkillNotFound => {
                return Ok(());
            }
            Err(error) => return Err(error),
        };
        service.remove(RemoveRequest::confirmed(plan.id))?;
        Ok::<_, crate::services::error::ServiceError>(())
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(format_service_error)
}

#[cfg(test)]
fn remove_path_any(path: &str) -> Result<(), String> {
    remove_path_any_core(std::path::Path::new(path)).map_err(|err| format!("{path}: {err:#}"))
}

fn to_service_install_dto(result: InstallOutcome) -> InstallResultDto {
    InstallResultDto {
        skill_id: result.id,
        name: result.name,
        central_path: result.central_path,
        content_hash: result.content_hash,
    }
}

fn to_auto_update_config_dto(mut config: AutoUpdateConfig) -> AutoUpdateConfigDto {
    let task_status = get_auto_update_task_status();
    if config.last_status.as_deref() == Some("running")
        && task_status.detail.contains("state = not running")
    {
        config.last_status = Some("stopped".to_string());
    }
    AutoUpdateConfigDto {
        enabled: config.enabled,
        interval_hours: config.interval_hours,
        schedule_type: match config.schedule.schedule_type {
            AutoUpdateScheduleType::Interval => "interval".to_string(),
            AutoUpdateScheduleType::Daily => "daily".to_string(),
        },
        interval_value: config.schedule.interval_value,
        interval_unit: match config.schedule.interval_unit {
            AutoUpdateIntervalUnit::Minutes => "minutes".to_string(),
            AutoUpdateIntervalUnit::Hours => "hours".to_string(),
        },
        daily_time: config.schedule.daily_time,
        local_skill_count: config.local_skill_count,
        protected_local_skill_count: config.protected_local_skill_count,
        task_registered: task_status.registered,
        task_status_detail: task_status.detail,
        last_run_at: config.last_run_at,
        last_started_at: config.last_started_at,
        last_finished_at: config.last_finished_at,
        last_status: config.last_status,
        last_error: config.last_error,
        last_checked: config.last_checked,
        last_unchanged: config.last_unchanged,
        last_updated: config.last_updated,
        last_failed: config.last_failed,
        progress: config.progress,
    }
}

fn to_auto_update_runtime_dto(config: AutoUpdateConfig) -> AutoUpdateRuntimeDto {
    AutoUpdateRuntimeDto {
        local_skill_count: config.local_skill_count,
        protected_local_skill_count: config.protected_local_skill_count,
        last_run_at: config.last_run_at,
        last_started_at: config.last_started_at,
        last_finished_at: config.last_finished_at,
        last_status: config.last_status,
        last_error: config.last_error,
        last_checked: config.last_checked,
        last_unchanged: config.last_unchanged,
        last_updated: config.last_updated,
        last_failed: config.last_failed,
        progress: config.progress,
    }
}

fn to_auto_update_run_result_dto(result: AutoUpdateRunResult) -> AutoUpdateRunResultDto {
    AutoUpdateRunResultDto {
        checked: result.checked,
        unchanged: result.unchanged,
        updated: result.updated,
        failed: result.failed,
        errors: result.errors,
        progress: result.progress,
    }
}

fn to_github_proxy_config_dto(config: GithubProxyConfig) -> GithubProxyConfigDto {
    GithubProxyConfigDto {
        enabled: config.enabled,
        port: config.port,
        url: config.url,
        auto_detected: config.auto_detected,
    }
}

fn now_ms() -> i64 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    now.as_millis() as i64
}

#[cfg(test)]
fn managed_skill_status(skill: &SkillRecord) -> String {
    crate::services::skills_hub::content_status(skill)
}

#[cfg(test)]
fn get_managed_skills_impl(store: &SkillStore) -> Result<Vec<ManagedSkillDto>, String> {
    let app_data_dir = store
        .db_path()
        .parent()
        .ok_or_else(|| "test database has no parent".to_string())?;
    let paths = crate::core::runtime_paths::RuntimePaths::from_tauri(
        crate::core::runtime_paths::RuntimeProfile::Test,
        app_data_dir,
        app_data_dir,
        app_data_dir,
    );
    SkillsHubService::from_store(paths, store.clone())
        .list_skills()
        .map(|skills| skills.into_iter().map(ManagedSkillDto::from).collect())
        .map_err(format_service_error)
}

#[derive(Debug, Serialize)]
pub struct FeaturedSkillDto {
    pub slug: String,
    pub name: String,
    pub summary: String,
    pub downloads: u64,
    pub stars: u64,
    pub source_url: String,
}

impl From<FeaturedSkill> for FeaturedSkillDto {
    fn from(s: FeaturedSkill) -> Self {
        Self {
            slug: s.slug,
            name: s.name,
            summary: s.summary,
            downloads: s.downloads,
            stars: s.stars,
            source_url: s.source_url,
        }
    }
}

#[tauri::command]
pub async fn get_featured_skills(
    store: State<'_, SkillStore>,
) -> Result<Vec<FeaturedSkillDto>, String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let skills = fetch_featured_skills(&store)?;
        Ok::<_, anyhow::Error>(skills.into_iter().map(FeaturedSkillDto::from).collect())
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(format_anyhow_error)
}

#[derive(Debug, Serialize)]
pub struct OnlineSkillDto {
    pub name: String,
    pub installs: u64,
    pub source: String,
    pub source_url: String,
}

impl From<OnlineSkillResult> for OnlineSkillDto {
    fn from(r: OnlineSkillResult) -> Self {
        Self {
            name: r.name,
            installs: r.installs,
            source: r.source,
            source_url: r.source_url,
        }
    }
}

#[tauri::command]
pub async fn search_skills_online(
    service: State<'_, SkillsHubService>,
    query: String,
    limit: Option<u32>,
) -> Result<Vec<OnlineSkillDto>, String> {
    let service = service.inner().clone();
    let limit = limit.unwrap_or(20) as usize;
    tauri::async_runtime::spawn_blocking(move || {
        service
            .search(&query, limit)
            .map(|results| results.into_iter().map(OnlineSkillDto::from).collect())
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(format_service_error)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillFileEntry {
    pub path: String,
    pub size: u64,
}

#[tauri::command]
pub async fn list_skill_files(central_path: String) -> Result<Vec<SkillFileEntry>, String> {
    let path = std::path::PathBuf::from(&central_path);
    tauri::async_runtime::spawn_blocking(move || {
        let entries = crate::core::skill_files::list_files(&path)?;
        Ok::<_, anyhow::Error>(
            entries
                .into_iter()
                .map(|e| SkillFileEntry {
                    path: e.path,
                    size: e.size,
                })
                .collect(),
        )
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(format_anyhow_error)
}

#[tauri::command]
pub async fn read_skill_file(central_path: String, file_path: String) -> Result<String, String> {
    let base = std::path::PathBuf::from(&central_path);
    tauri::async_runtime::spawn_blocking(move || {
        crate::core::skill_files::read_file(&base, &file_path)
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(format_anyhow_error)
}

#[tauri::command]
pub fn cancel_current_operation(cancel: State<'_, Arc<CancelToken>>) -> Result<(), String> {
    cancel.cancel();
    Ok(())
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DeviceSyncConfigDto {
    pub visibility: crate::core::device_sync::types::RepositoryVisibility,
    pub public_upload_confirmed: bool,
    pub provider: ProviderId,
    pub remote_url: String,
    pub branch: String,
    pub username: Option<String>,
    pub auto_check: bool,
    pub auto_sync: bool,
    pub auto_sync_schedule: Option<crate::core::device_sync::scheduler::SyncSchedule>,
    pub has_credential: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SaveDeviceSyncConfigInput {
    #[serde(default)]
    pub visibility: crate::core::device_sync::types::RepositoryVisibility,
    #[serde(default)]
    pub public_upload_confirmed: bool,
    pub provider: ProviderId,
    pub remote_url: String,
    pub branch: String,
    pub username: Option<String>,
    pub token: Option<String>,
    pub credential_key: Option<String>,
    pub auto_check: bool,
    pub auto_sync: bool,
    #[serde(default)]
    pub auto_sync_schedule: Option<crate::core::device_sync::scheduler::SyncSchedule>,
}

#[tauri::command]
pub fn get_device_sync_config(
    store: State<'_, SkillStore>,
) -> Result<Option<DeviceSyncConfigDto>, String> {
    store
        .get_device_sync_config()
        .map(|config| {
            config.map(|item| DeviceSyncConfigDto {
                visibility: item.visibility,
                public_upload_confirmed: item.public_upload_confirmed,
                provider: item.provider,
                remote_url: item.remote_url,
                branch: item.branch,
                username: item.username,
                auto_check: item.auto_check,
                auto_sync: item.auto_sync && item.auto_sync_schedule.is_some(),
                auto_sync_schedule: item.auto_sync_schedule,
                has_credential: item.credential_key.is_some(),
            })
        })
        .map_err(format_anyhow_error)
}

#[tauri::command]
pub fn save_device_sync_config(
    store: State<'_, SkillStore>,
    config: SaveDeviceSyncConfigInput,
) -> Result<DeviceSyncConfigDto, String> {
    let _sync_guard =
        crate::core::device_sync::try_lock_device_sync().map_err(format_anyhow_error)?;
    if config.auto_sync && config.auto_sync_schedule.is_none() {
        return Err("choose an automatic sync schedule".to_string());
    }
    if let Some(schedule) = &config.auto_sync_schedule {
        schedule.validate().map_err(format_anyhow_error)?;
    }
    if config.remote_url.trim().is_empty() {
        return Err("device sync repository URL is empty".to_string());
    }
    validate_device_sync_remote(&config.remote_url)?;
    let branch = if config.branch.trim().is_empty() {
        "main"
    } else {
        config.branch.trim()
    };
    if !git2::Reference::is_valid_name(&format!("refs/heads/{branch}")) {
        return Err("invalid device sync branch name".to_string());
    }
    let credentials = SystemCredentialStore;
    let previous = store
        .get_device_sync_config()
        .map_err(format_anyhow_error)?;
    let same_repository = previous.as_ref().is_some_and(|item| {
        item.provider == config.provider
            && item.remote_url == config.remote_url.trim()
            && item.branch == branch
    });
    let requested_credential_key = config
        .credential_key
        .filter(|value| !value.trim().is_empty());
    let remote_usage = CredentialUsage::from_https_remote(config.provider, &config.remote_url).ok();
    if let Some(key) = requested_credential_key.as_deref() {
        let usage = remote_usage
            .as_ref()
            .ok_or_else(|| "token authentication requires an HTTPS repository URL".to_string())?;
        let proxy_url = get_github_proxy_url_core(&store).map_err(format_anyhow_error)?;
        if resolve_access_token_with_proxy(&credentials, key, usage, &proxy_url)
            .map_err(format_anyhow_error)?
            .is_none()
        {
            return Err("OAuth authorization is no longer available; sign in again".to_string());
        }
    }
    let credential_key = requested_credential_key.or_else(|| {
        remote_usage
            .as_ref()
            .and_then(|usage| inherited_device_sync_credential(previous.as_ref(), usage))
    });
    let manual_token = config
        .token
        .as_deref()
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(str::to_string);
    let saved = DeviceSyncConfig {
        visibility: config.visibility,
        public_upload_confirmed: config.public_upload_confirmed
            && config.visibility == crate::core::device_sync::types::RepositoryVisibility::Public,
        provider: config.provider,
        remote_url: config.remote_url.trim().to_string(),
        branch: branch.to_string(),
        username: config.username.filter(|value| !value.trim().is_empty()),
        credential_key,
        auto_check: config.auto_check,
        auto_sync: config.auto_sync,
        auto_sync_schedule: config.auto_sync_schedule,
        last_synced_commit: if same_repository {
            previous
                .as_ref()
                .and_then(|item| item.last_synced_commit.clone())
        } else {
            None
        },
    };
    let persist_config = |candidate: &DeviceSyncConfig| -> anyhow::Result<()> {
        if previous.is_some() && !same_repository {
            store.clear_device_sync_repository_state()?;
        }
        store.save_device_sync_config(candidate)
    };
    let replaced_credential_key = previous
        .as_ref()
        .and_then(|item| item.credential_key.as_deref())
        .filter(|old_key| {
            manual_token.is_some() || Some(*old_key) != saved.credential_key.as_deref()
        });
    let saved = persist_device_sync_credential_replacement_with(
        &store,
        &credentials,
        replaced_credential_key,
        || {
            if let Some(token) = manual_token.as_deref() {
                let usage = remote_usage
                    .as_ref()
                    .context("token authentication requires an HTTPS repository URL")?;
                persist_config_with_staged_personal_access_token(
                    &store,
                    &credentials,
                    usage,
                    token,
                    saved,
                    persist_config,
                )
            } else {
                persist_config(&saved)?;
                Ok(saved)
            }
        },
    )
    .map_err(format_anyhow_error)?;
    if load_pending_oauth(&store)
        .map_err(format_anyhow_error)?
        .is_some()
    {
        clear_pending_oauth_with_credentials(&store, &credentials, true)
            .map_err(format_anyhow_error)?;
    }
    Ok(DeviceSyncConfigDto {
        provider: saved.provider,
        remote_url: saved.remote_url,
        branch: saved.branch,
        username: saved.username,
        auto_check: saved.auto_check,
        auto_sync: saved.auto_sync,
        auto_sync_schedule: saved.auto_sync_schedule,
        has_credential: saved.credential_key.is_some(),
        visibility: saved.visibility,
        public_upload_confirmed: saved.public_upload_confirmed,
    })
}

#[tauri::command]
pub fn get_device_sync_oauth_availability() -> Vec<OAuthProviderAvailability> {
    oauth::availability()
}

#[tauri::command]
pub fn get_device_sync_pending_oauth(
    store: State<'_, SkillStore>,
) -> Result<Option<PendingOAuthAuthorization>, String> {
    load_pending_oauth(&store).map_err(format_anyhow_error)
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn start_device_sync_oauth(
    store: State<'_, SkillStore>,
    providerId: ProviderId,
) -> Result<OAuthStartResult, String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let proxy_url = oauth_proxy_url(&store, providerId)?;
        oauth::start(providerId, &proxy_url)
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(format_anyhow_error)
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn poll_device_sync_oauth(
    store: State<'_, SkillStore>,
    flowId: String,
) -> Result<OAuthPollResult, String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let credentials = SystemCredentialStore;
        let proxy_url = get_github_proxy_url_core(&store)?;
        poll_device_sync_oauth_with(
            &store,
            &credentials,
            || oauth::poll(&flowId, &credentials, &proxy_url),
            |pending| save_pending_oauth(&store, pending),
        )
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(format_anyhow_error)
}

#[tauri::command]
pub fn clear_device_sync_pending_oauth(store: State<'_, SkillStore>) -> Result<(), String> {
    let _sync_guard =
        crate::core::device_sync::try_lock_device_sync().map_err(format_anyhow_error)?;
    clear_pending_oauth(&store, true).map_err(format_anyhow_error)
}

#[tauri::command]
#[allow(non_snake_case)]
pub fn cancel_device_sync_oauth(flowId: String) {
    oauth::cancel(&flowId);
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn validate_device_sync_account(
    store: State<'_, SkillStore>,
    providerId: ProviderId,
    token: String,
) -> Result<ProviderAccount, String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let proxy_url = get_github_proxy_url_core(&store)?;
        provider(providerId, &proxy_url)?.validate_token(token.trim())
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(format_anyhow_error)
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn create_device_sync_repository(
    store: State<'_, SkillStore>,
    providerId: ProviderId,
    token: Option<String>,
    credentialKey: Option<String>,
    name: String,
) -> Result<RemoteRepository, String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let token = resolve_device_sync_token(&store, providerId, token, credentialKey)?;
        let proxy_url = get_github_proxy_url_core(&store)?;
        let provider = provider(providerId, &proxy_url)?;
        provider.validate_token(token.trim())?;
        provider.create_private_repository(token.trim(), &name)
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(format_anyhow_error)
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn list_device_sync_repositories(
    store: State<'_, SkillStore>,
    providerId: ProviderId,
    token: Option<String>,
    credentialKey: Option<String>,
) -> Result<Vec<RemoteRepository>, String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let token = resolve_device_sync_token(&store, providerId, token, credentialKey)?;
        let proxy_url = get_github_proxy_url_core(&store)?;
        provider(providerId, &proxy_url)?.list_repositories(&token)
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(format_anyhow_error)
}

#[tauri::command]
pub fn get_device_sync_status(
    app: tauri::AppHandle,
    store: State<'_, SkillStore>,
) -> Result<SyncStatus, String> {
    let (workspace, central) = device_sync_paths(&app, &store).map_err(format_anyhow_error)?;
    let credentials = SystemCredentialStore;
    let service = DeviceSyncService::new(&store, &credentials, workspace, central);
    let mut status = service.status().map_err(format_anyhow_error)?;
    let runtime = app
        .try_state::<crate::core::device_sync::scheduler::SchedulerRuntime>()
        .map(|state| state.inner().clone())
        .unwrap_or_default();
    status.schedule_status = Some(
        runtime
            .status(&store, status.conflict_count > 0, status.is_running)
            .map_err(format_anyhow_error)?,
    );
    Ok(status)
}

#[tauri::command]
pub async fn check_device_sync(
    app: tauri::AppHandle,
    store: State<'_, SkillStore>,
) -> Result<SyncChangeSummary, String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _operation_lock = acquire_operation_lock(&app, OperationKind::DeviceSync)?;
        let (workspace, central) = device_sync_paths(&app, &store)?;
        let credentials = SystemCredentialStore;
        DeviceSyncService::new(&store, &credentials, workspace, central).check()
    })
    .await
    .map_err(|_| "DEVICE_SYNC_FAILURE_unknown".to_string())?
    .map_err(crate::core::device_sync::errors::format_error)
}

#[tauri::command]
pub async fn run_device_sync(
    app: tauri::AppHandle,
    store: State<'_, SkillStore>,
) -> Result<SyncRunResult, String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _operation_lock = acquire_operation_lock(&app, OperationKind::DeviceSync)?;
        let (workspace, central) = device_sync_paths(&app, &store)?;
        let credentials = SystemCredentialStore;
        DeviceSyncService::new(&store, &credentials, workspace, central).sync()
    })
    .await
    .map_err(|_| "DEVICE_SYNC_FAILURE_unknown".to_string())?
    .map_err(crate::core::device_sync::errors::format_error)
}

#[tauri::command]
pub fn get_device_sync_history(
    store: State<'_, SkillStore>,
    limit: Option<usize>,
) -> Result<Vec<SyncHistoryEntry>, String> {
    store
        .list_device_sync_history(limit.unwrap_or(50).clamp(1, DEVICE_SYNC_HISTORY_LIMIT))
        .map_err(format_anyhow_error)
}

#[tauri::command]
pub fn get_device_sync_devices(
    app: tauri::AppHandle,
    store: State<'_, SkillStore>,
) -> Result<Vec<DeviceSyncDevice>, String> {
    let (workspace, central) = device_sync_paths(&app, &store).map_err(format_anyhow_error)?;
    let credentials = SystemCredentialStore;
    DeviceSyncService::new(&store, &credentials, workspace, central)
        .devices()
        .map_err(format_anyhow_error)
}

#[tauri::command]
#[allow(non_snake_case)]
pub fn set_device_sync_device_alias(
    store: State<'_, SkillStore>,
    deviceId: String,
    alias: Option<String>,
) -> Result<(), String> {
    store
        .set_device_sync_device_alias(&deviceId, alias.as_deref())
        .map_err(format_anyhow_error)
}

#[tauri::command]
pub fn get_device_sync_conflicts(
    store: State<'_, SkillStore>,
) -> Result<Vec<SyncConflict>, String> {
    store
        .list_device_sync_conflicts()
        .map_err(format_anyhow_error)
}

#[tauri::command]
pub fn get_device_sync_trash(store: State<'_, SkillStore>) -> Result<Vec<TrashEntry>, String> {
    store.list_device_sync_trash().map_err(format_anyhow_error)
}

#[derive(Serialize)]
pub struct RecycleBinLocationsDto {
    pub manual_backup: String,
    pub sync_backup: String,
}

#[tauri::command]
pub fn get_recycle_bin_locations(app: tauri::AppHandle) -> Result<RecycleBinLocationsDto, String> {
    let root = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?;
    Ok(RecycleBinLocationsDto {
        manual_backup: root.join("recycle-bin").to_string_lossy().into_owned(),
        sync_backup: root
            .join("device-sync")
            .join("trash")
            .to_string_lossy()
            .into_owned(),
    })
}

#[tauri::command]
pub fn get_recycle_bin_items(
    app: tauri::AppHandle,
    store: State<'_, SkillStore>,
) -> Result<Vec<RecycleBinItem>, String> {
    let root = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?
        .join("recycle-bin");
    RecycleBinService::new(&store, root)
        .list()
        .map_err(format_anyhow_error)
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn restore_recycle_bin_item(
    app: tauri::AppHandle,
    store: State<'_, SkillStore>,
    trashId: String,
    targetIds: Vec<String>,
) -> Result<(), String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _operation_lock = acquire_operation_lock(&app, OperationKind::Restore)?;
        let root = app.path().app_data_dir()?.join("recycle-bin");
        let service = RecycleBinService::new(&store, root);
        if service.has_snapshot(&trashId)? {
            let _device_sync_guard = if store.get_device_sync_config()?.is_some() {
                Some(crate::core::device_sync::try_lock_device_sync()?)
            } else {
                None
            };
            service.restore_with_targets(&trashId, Some(&targetIds))
        } else {
            let (workspace, central) = device_sync_paths(&app, &store)?;
            let credentials = SystemCredentialStore;
            DeviceSyncService::new(&store, &credentials, workspace, central).restore_trash(&trashId)
        }
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(format_anyhow_error)
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn delete_recycle_bin_item(
    app: tauri::AppHandle,
    store: State<'_, SkillStore>,
    trashId: String,
) -> Result<(), String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _operation_lock = acquire_operation_lock(&app, OperationKind::Delete)?;
        let root = app.path().app_data_dir()?.join("recycle-bin");
        RecycleBinService::new(&store, root).delete_permanently(&trashId)
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(format_anyhow_error)
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn clear_recycle_bin(
    app: tauri::AppHandle,
    store: State<'_, SkillStore>,
    trashIds: Vec<String>,
) -> Result<usize, String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _operation_lock = acquire_operation_lock(&app, OperationKind::Delete)?;
        let root = app.path().app_data_dir()?.join("recycle-bin");
        RecycleBinService::new(&store, root).clear(&trashIds)
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(format_anyhow_error)
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn resolve_device_sync_conflict(
    app: tauri::AppHandle,
    store: State<'_, SkillStore>,
    conflictId: String,
    resolution: ConflictResolution,
) -> Result<(), String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _operation_lock = acquire_operation_lock(&app, OperationKind::DeviceSync)?;
        let (workspace, central) = device_sync_paths(&app, &store)?;
        let credentials = SystemCredentialStore;
        DeviceSyncService::new(&store, &credentials, workspace, central)
            .resolve_conflict(&conflictId, resolution)
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(format_anyhow_error)
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn restore_device_sync_trash(
    app: tauri::AppHandle,
    store: State<'_, SkillStore>,
    trashId: String,
) -> Result<(), String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _operation_lock = acquire_operation_lock(&app, OperationKind::Restore)?;
        let (workspace, central) = device_sync_paths(&app, &store)?;
        let credentials = SystemCredentialStore;
        DeviceSyncService::new(&store, &credentials, workspace, central).restore_trash(&trashId)
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(format_anyhow_error)
}

#[tauri::command]
pub fn disconnect_device_sync(store: State<'_, SkillStore>) -> Result<(), String> {
    let _sync_guard =
        crate::core::device_sync::try_lock_device_sync().map_err(format_anyhow_error)?;
    disconnect_device_sync_with_credentials(&store, &SystemCredentialStore)
        .map_err(format_anyhow_error)
}

fn disconnect_device_sync_with_credentials(
    store: &SkillStore,
    credentials: &dyn CredentialStore,
) -> anyhow::Result<()> {
    retry_queued_credential_cleanup(store, credentials)?;
    if let Some(key) = store
        .get_device_sync_config()?
        .and_then(|config| config.credential_key)
    {
        enqueue_credential_cleanup(store, &key)?;
    }
    clear_pending_oauth_with_credentials(store, credentials, true)?;
    store.clear_device_sync_repository_state()?;
    store.clear_device_sync_config()?;
    retry_queued_credential_cleanup(store, credentials)
}

fn load_pending_oauth(store: &SkillStore) -> anyhow::Result<Option<PendingOAuthAuthorization>> {
    store
        .get_setting(DEVICE_SYNC_PENDING_OAUTH_SETTING)?
        .map(|value| serde_json::from_str(&value).context("decode pending OAuth authorization"))
        .transpose()
}

fn save_pending_oauth(
    store: &SkillStore,
    pending: &PendingOAuthAuthorization,
) -> anyhow::Result<()> {
    store.set_setting(
        DEVICE_SYNC_PENDING_OAUTH_SETTING,
        &serde_json::to_string(pending)?,
    )
}

pub(crate) fn poll_device_sync_oauth_with<P, S>(
    store: &SkillStore,
    credentials: &dyn CredentialStore,
    poll: P,
    save: S,
) -> anyhow::Result<OAuthPollResult>
where
    P: FnOnce() -> anyhow::Result<OAuthPollResult>,
    S: FnOnce(&PendingOAuthAuthorization) -> anyhow::Result<()>,
{
    let _sync_guard = crate::core::device_sync::try_lock_device_sync()?;
    retry_queued_credential_cleanup(store, credentials)?;
    let result = poll()?;
    persist_pending_oauth_result_with(store, credentials, &result, save)?;
    Ok(result)
}

fn clear_pending_oauth(store: &SkillStore, delete_credential: bool) -> anyhow::Result<()> {
    clear_pending_oauth_with_credentials(store, &SystemCredentialStore, delete_credential)
}

fn clear_pending_oauth_with_credentials(
    store: &SkillStore,
    credentials: &dyn CredentialStore,
    delete_credential: bool,
) -> anyhow::Result<()> {
    let pending = load_pending_oauth(store)?;
    let delete_key = if delete_credential {
        pending
            .as_ref()
            .map(|pending| {
                credential_key_is_active(store, &pending.credential_key)
                    .map(|active| (!active).then(|| pending.credential_key.clone()))
            })
            .transpose()?
            .flatten()
    } else {
        None
    };
    if let Some(key) = delete_key.as_deref() {
        enqueue_credential_cleanup(store, key)?;
    }
    store.delete_setting(DEVICE_SYNC_PENDING_OAUTH_SETTING)?;
    retry_queued_credential_cleanup(store, credentials)?;
    Ok(())
}

fn load_credential_cleanup_queue(store: &SkillStore) -> anyhow::Result<Vec<String>> {
    let mut keys = store
        .get_setting(DEVICE_SYNC_CREDENTIAL_CLEANUP_QUEUE_SETTING)?
        .map(|value| {
            serde_json::from_str::<Vec<String>>(&value).context("decode credential cleanup queue")
        })
        .transpose()?
        .unwrap_or_default();
    keys.retain(|key| !key.trim().is_empty());
    keys.sort();
    keys.dedup();
    Ok(keys)
}

fn save_credential_cleanup_queue(store: &SkillStore, keys: &[String]) -> anyhow::Result<()> {
    if keys.is_empty() {
        store.delete_setting(DEVICE_SYNC_CREDENTIAL_CLEANUP_QUEUE_SETTING)
    } else {
        store.set_setting(
            DEVICE_SYNC_CREDENTIAL_CLEANUP_QUEUE_SETTING,
            &serde_json::to_string(keys)?,
        )
    }
}

fn enqueue_credential_cleanup(store: &SkillStore, key: &str) -> anyhow::Result<()> {
    let mut keys = load_credential_cleanup_queue(store)?;
    if !keys.iter().any(|queued| queued == key) {
        keys.push(key.to_string());
        keys.sort();
        save_credential_cleanup_queue(store, &keys)?;
    }
    Ok(())
}

fn dequeue_credential_cleanup(store: &SkillStore, key: &str) -> anyhow::Result<()> {
    let mut keys = load_credential_cleanup_queue(store)?;
    keys.retain(|queued| queued != key);
    save_credential_cleanup_queue(store, &keys)
}

fn persist_device_sync_credential_replacement_with<T, F>(
    store: &SkillStore,
    credentials: &dyn CredentialStore,
    replaced_credential_key: Option<&str>,
    persist: F,
) -> anyhow::Result<T>
where
    F: FnOnce() -> anyhow::Result<T>,
{
    retry_queued_credential_cleanup(store, credentials)?;
    if let Some(key) = replaced_credential_key {
        enqueue_credential_cleanup(store, key)?;
    }
    let value = persist()?;
    retry_queued_credential_cleanup(store, credentials)?;
    Ok(value)
}

pub(crate) fn retry_queued_credential_cleanup(
    store: &SkillStore,
    credentials: &dyn CredentialStore,
) -> anyhow::Result<()> {
    let keys = load_credential_cleanup_queue(store)?;
    let ownership = keys
        .iter()
        .map(|key| credential_key_is_owned(store, key).map(|owned| (key, owned)))
        .collect::<anyhow::Result<Vec<_>>>()?;
    let mut remaining = Vec::new();
    let mut first_error = None;
    for (key, owned) in ownership {
        if owned {
            remaining.push(key.clone());
            continue;
        }
        if let Err(err) = credentials.delete(key) {
            remaining.push(key.clone());
            if first_error.is_none() {
                first_error = Some(err);
            }
        }
    }
    save_credential_cleanup_queue(store, &remaining)?;
    if let Some(err) = first_error {
        return Err(err).context("delete queued device sync credential");
    }
    Ok(())
}

fn persist_pending_oauth_result_with<F>(
    store: &SkillStore,
    credentials: &dyn CredentialStore,
    result: &OAuthPollResult,
    save: F,
) -> anyhow::Result<()>
where
    F: FnOnce(&PendingOAuthAuthorization) -> anyhow::Result<()>,
{
    let (Some(credential_key), Some(account)) =
        (result.credential_key.as_ref(), result.account.as_ref())
    else {
        return retry_queued_credential_cleanup(store, credentials);
    };
    let previous = match load_pending_oauth(store) {
        Ok(previous) => previous,
        Err(err) => {
            return defer_credential_cleanup_after_failure(store, credentials, credential_key, err);
        }
    };
    let previous_key = previous
        .as_ref()
        .map(|pending| pending.credential_key.as_str())
        .filter(|previous_key| *previous_key != credential_key);
    if let Some(previous_key) = previous_key {
        let previous_is_active = match credential_key_is_active(store, previous_key) {
            Ok(active) => active,
            Err(err) => {
                return defer_credential_cleanup_after_failure(
                    store,
                    credentials,
                    credential_key,
                    err,
                );
            }
        };
        if !previous_is_active {
            if let Err(err) = enqueue_credential_cleanup(store, previous_key) {
                return defer_credential_cleanup_after_failure(
                    store,
                    credentials,
                    credential_key,
                    err,
                );
            }
        }
    }
    let pending = PendingOAuthAuthorization {
        provider: result.provider,
        credential_key: credential_key.clone(),
        account: account.clone(),
    };
    if let Err(err) = save(&pending) {
        return defer_credential_cleanup_after_failure(store, credentials, credential_key, err);
    }
    retry_queued_credential_cleanup(store, credentials)
}

fn defer_credential_cleanup_after_failure<T>(
    store: &SkillStore,
    credentials: &dyn CredentialStore,
    key: &str,
    primary_error: anyhow::Error,
) -> anyhow::Result<T> {
    let cleanup_result = match enqueue_credential_cleanup(store, key) {
        Ok(()) => retry_queued_credential_cleanup(store, credentials),
        Err(queue_error) => match credentials.delete(key) {
            Ok(()) => Ok(()),
            Err(delete_error) => Err(delete_error).with_context(|| {
                format!(
                    "persist cleanup intent after {queue_error:#} and delete uncommitted credential"
                )
            }),
        },
    };
    match cleanup_result {
        Ok(()) => Err(primary_error),
        Err(cleanup_error) => Err(cleanup_error).with_context(|| {
            format!("operation failed and credential cleanup was deferred: {primary_error:#}")
        }),
    }
}

fn credential_key_is_active(store: &SkillStore, key: &str) -> anyhow::Result<bool> {
    Ok(store
        .get_device_sync_config()?
        .and_then(|config| config.credential_key)
        .as_deref()
        == Some(key))
}

fn credential_key_is_owned(store: &SkillStore, key: &str) -> anyhow::Result<bool> {
    if credential_key_is_active(store, key)? {
        return Ok(true);
    }
    Ok(load_pending_oauth(store)?
        .map(|pending| pending.credential_key == key)
        .unwrap_or(false))
}

fn persist_config_with_staged_personal_access_token<F>(
    store: &SkillStore,
    credentials: &dyn CredentialStore,
    usage: &CredentialUsage,
    token: &str,
    mut config: DeviceSyncConfig,
    save: F,
) -> anyhow::Result<DeviceSyncConfig>
where
    F: FnOnce(&DeviceSyncConfig) -> anyhow::Result<()>,
{
    let staged_key = Uuid::new_v4().to_string();
    enqueue_credential_cleanup(store, &staged_key)?;
    if let Err(err) = save_personal_access_token(credentials, &staged_key, usage, token) {
        return defer_credential_cleanup_after_failure(store, credentials, &staged_key, err);
    }
    config.credential_key = Some(staged_key.clone());
    if let Err(err) = save(&config) {
        return defer_credential_cleanup_after_failure(store, credentials, &staged_key, err);
    }
    dequeue_credential_cleanup(store, &staged_key)?;
    retry_queued_credential_cleanup(store, credentials)?;
    Ok(config)
}

fn device_sync_paths(
    app: &tauri::AppHandle,
    store: &SkillStore,
) -> anyhow::Result<(std::path::PathBuf, std::path::PathBuf)> {
    let paths = crate::runtime_paths_for_tauri(app)?;
    let workspace = paths.app_data_dir.join("device-sync");
    let central = resolve_central_repo_path(&paths, store)?;
    Ok((workspace, central))
}

fn resolve_device_sync_token(
    store: &SkillStore,
    provider: ProviderId,
    token: Option<String>,
    credential_key: Option<String>,
) -> anyhow::Result<String> {
    if let Some(token) = token.filter(|value| !value.trim().is_empty()) {
        return Ok(token.trim().to_string());
    }
    let key = credential_key
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            store
                .get_device_sync_config()
                .ok()
                .flatten()
                .filter(|config| config.provider == provider)
                .and_then(|config| config.credential_key)
        })
        .context("sign in or provide an access token first")?;
    let proxy_url = get_github_proxy_url_core(store)?;
    resolve_access_token_with_proxy(
        &SystemCredentialStore,
        &key,
        &CredentialUsage::official(provider),
        &proxy_url,
    )
    .context("read saved device sync authorization")?
    .context("saved authorization is unavailable; sign in again")
}

fn inherited_device_sync_credential(
    previous: Option<&DeviceSyncConfig>,
    expected_usage: &CredentialUsage,
) -> Option<String> {
    let previous = previous?;
    let previous_usage =
        CredentialUsage::from_https_remote(previous.provider, &previous.remote_url).ok()?;
    (previous_usage == *expected_usage)
        .then(|| previous.credential_key.clone())
        .flatten()
}

fn validate_device_sync_remote(value: &str) -> Result<(), String> {
    let value = value.trim();
    if value.contains(['\n', '\r']) || value.contains('?') || value.contains('#') {
        return Err("device sync repository URL contains unsupported characters".to_string());
    }
    if let Some(rest) = value.strip_prefix("https://") {
        let authority = rest.split('/').next().unwrap_or_default();
        if authority.contains('@') {
            return Err("do not include credentials in the repository URL".to_string());
        }
        return Ok(());
    }
    if value.starts_with("ssh://") || value.starts_with("git@") {
        return Ok(());
    }
    Err("use an HTTPS or SSH repository URL".to_string())
}

#[cfg(test)]
#[path = "tests/commands.rs"]
mod tests;
