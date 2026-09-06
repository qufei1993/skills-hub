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
use crate::core::content_hash::hash_dir;
use crate::core::device_sync::credentials::{
    resolve_access_token, save_personal_access_token, CredentialStore, SystemCredentialStore,
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
use crate::core::installer::{
    import_existing_local_skill, install_git_skill, install_git_skill_from_selection,
    install_local_skill, install_local_skill_from_selection, list_git_skills, list_local_skills,
    update_managed_skill_from_source, GitSkillCandidate, InstallResult, LocalSkillCandidate,
};
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
use crate::core::skill_store::{SkillRecord, SkillStore, SkillTargetRecord};
use crate::core::skills_search::{
    search_skills_online as search_skills_online_core, OnlineSkillResult,
};
use crate::core::sync_engine::{
    copy_dir_recursive, path_is_protected_real_content, paths_overlap,
    remove_path_any as remove_path_any_core, sync_dir_for_tool_with_overwrite, sync_dir_hybrid,
    sync_dir_with_mode_with_overwrite, SyncMode,
};
use crate::core::system_scheduler::{
    current_scheduler_config, get_auto_update_task_status, install_auto_update_task,
    trigger_auto_update_task_now, uninstall_auto_update_task,
};
use crate::core::tool_adapters::{
    adapter_by_key, adapters_sharing_project_skills_dir, is_builtin_tool_enabled,
    is_tool_installed, load_tool_config, project_relative_skills_dir, resolve_default_path,
    save_tool_config, supports_project_scope, CustomToolConfig, ToolConfig,
};
use uuid::Uuid;

const RECENT_PROJECTS_SETTING: &str = "recent_projects_v1";
const DEVICE_SYNC_PENDING_OAUTH_SETTING: &str = "device_sync_pending_oauth_v1";
const DEVICE_SYNC_CREDENTIAL_CLEANUP_QUEUE_SETTING: &str =
    "device_sync_credential_cleanup_queue_v1";

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

#[derive(Clone, Debug)]
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
            supports_project_scope: supports_project_scope(&adapter),
            sync_mode: SyncMode::Auto,
        });
    }

    for custom in config.custom_tools {
        if !include_disabled && !custom.enabled {
            continue;
        }
        let skills_dir = expand_home_path(&custom.skills_dir)?;
        let supports_project_scope = custom.project_skills_dir.is_some();
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

fn runtime_tool_by_key(store: &SkillStore, key: &str) -> anyhow::Result<RuntimeTool> {
    runtime_tools(store, false)?
        .into_iter()
        .find(|tool| tool.key == key)
        .ok_or_else(|| anyhow::anyhow!("TOOL_NOT_INSTALLED|{}", key))
}

fn runtime_tools_sharing_dir(
    store: &SkillStore,
    selected: &RuntimeTool,
    scope: &str,
) -> anyhow::Result<Vec<RuntimeTool>> {
    let tools = runtime_tools(store, false)?;
    let shared = tools
        .into_iter()
        .filter(|tool| {
            tool.installed
                && if scope == "project" {
                    tool.project_skills_dir == selected.project_skills_dir
                } else {
                    tool.skills_dir == selected.skills_dir
                }
        })
        .collect::<Vec<_>>();
    Ok(shared)
}

fn resolve_runtime_tool_root(
    tool: &RuntimeTool,
    project_root: Option<&std::path::Path>,
) -> anyhow::Result<std::path::PathBuf> {
    if let Some(project_root) = project_root {
        if !tool.supports_project_scope {
            anyhow::bail!("PROJECT_SCOPE_UNSUPPORTED|{}", tool.key);
        }
        return Ok(project_root.join(&tool.project_skills_dir));
    }
    Ok(tool.skills_dir.clone())
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
pub async fn get_tool_status(store: State<'_, SkillStore>) -> Result<ToolStatusDto, String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut tools: Vec<ToolInfoDto> = Vec::new();
        let mut installed: Vec<String> = Vec::new();

        for tool in runtime_tools(&store, true)? {
            tools.push(ToolInfoDto {
                key: tool.key.clone(),
                label: tool.label,
                avatar: tool.avatar,
                installed: tool.installed,
                enabled: tool.enabled,
                is_custom: tool.is_custom,
                skills_dir: tool.skills_dir.to_string_lossy().to_string(),
                project_skills_dir: tool.project_skills_dir,
                supports_project_scope: tool.supports_project_scope,
                sync_mode: tool.sync_mode,
            });
            if tool.installed {
                installed.push(tool.key);
            }
        }

        installed.dedup();

        let prev: Vec<String> = store
            .get_setting("installed_tools_v1")?
            .and_then(|raw| serde_json::from_str::<Vec<String>>(&raw).ok())
            .unwrap_or_default();

        let prev_set: std::collections::HashSet<String> = prev.into_iter().collect();
        let newly_installed: Vec<String> = installed
            .iter()
            .filter(|k| !prev_set.contains(*k))
            .cloned()
            .collect();

        // Persist current set (best effort).
        let _ = store.set_setting(
            "installed_tools_v1",
            &serde_json::to_string(&installed).unwrap_or_else(|_| "[]".to_string()),
        );

        Ok::<_, anyhow::Error>(ToolStatusDto {
            tools,
            installed,
            newly_installed,
        })
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(format_anyhow_error)
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
        cleanup_git_cache_dirs(&app, std::time::Duration::from_secs(0))
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
        let path = resolve_central_repo_path(&app, &store)?;
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
        let current_base = resolve_central_repo_path(&app, &store)?;
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
        let new_base = expand_home_path(&path)?;
        if !new_base.is_absolute() {
            anyhow::bail!("storage path must be absolute");
        }
        let current_base = resolve_central_repo_path(&app, &store)?;
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
    app: tauri::AppHandle,
    store: State<'_, SkillStore>,
    sourcePath: String,
    name: Option<String>,
) -> Result<InstallResultDto, String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let result = install_local_skill(&app, &store, sourcePath.as_ref(), name)?;
        Ok::<_, anyhow::Error>(to_install_dto(result))
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(format_anyhow_error)
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn list_local_skills_cmd(basePath: String) -> Result<Vec<LocalSkillCandidate>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let path = std::path::PathBuf::from(basePath);
        list_local_skills(&path)
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(format_anyhow_error)
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn install_local_selection(
    app: tauri::AppHandle,
    store: State<'_, SkillStore>,
    basePath: String,
    subpath: String,
    name: Option<String>,
) -> Result<InstallResultDto, String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let base = std::path::PathBuf::from(basePath);
        let result =
            install_local_skill_from_selection(&app, &store, base.as_ref(), &subpath, name)?;
        Ok::<_, anyhow::Error>(to_install_dto(result))
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(format_anyhow_error)
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn install_git(
    app: tauri::AppHandle,
    store: State<'_, SkillStore>,
    cancel: State<'_, Arc<CancelToken>>,
    repoUrl: String,
    name: Option<String>,
) -> Result<InstallResultDto, String> {
    let store = store.inner().clone();
    cancel.reset();
    let cancel_token = Arc::clone(cancel.inner());
    tauri::async_runtime::spawn_blocking(move || {
        let result = install_git_skill(&app, &store, &repoUrl, name, Some(&cancel_token))?;
        Ok::<_, anyhow::Error>(to_install_dto(result))
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(format_anyhow_error)
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn list_git_skills_cmd(
    app: tauri::AppHandle,
    store: State<'_, SkillStore>,
    repoUrl: String,
) -> Result<Vec<GitSkillCandidate>, String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || list_git_skills(&app, &store, &repoUrl))
        .await
        .map_err(|err| err.to_string())?
        .map_err(format_anyhow_error)
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn install_git_selection(
    app: tauri::AppHandle,
    store: State<'_, SkillStore>,
    repoUrl: String,
    subpath: String,
    name: Option<String>,
) -> Result<InstallResultDto, String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let result = install_git_skill_from_selection(&app, &store, &repoUrl, &subpath, name)?;
        Ok::<_, anyhow::Error>(to_install_dto(result))
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(format_anyhow_error)
}

#[derive(Debug, Serialize)]
pub struct SyncResultDto {
    pub mode_used: String,
    pub target_path: String,
}

fn sync_mode_name(mode: SyncMode) -> &'static str {
    match mode {
        SyncMode::Auto => "auto",
        SyncMode::Symlink => "symlink",
        SyncMode::Junction => "junction",
        SyncMode::Copy => "copy",
    }
}

#[allow(clippy::too_many_arguments)]
fn record_skill_target_failure(
    store: &SkillStore,
    skill_id: &str,
    tool: &str,
    scope: &str,
    project_path: Option<&str>,
    target_path: &std::path::Path,
    requested_mode: SyncMode,
    error: &str,
) -> anyhow::Result<()> {
    let existing = store.get_skill_target(skill_id, tool, scope, project_path)?;
    let record = SkillTargetRecord {
        id: existing
            .as_ref()
            .map(|target| target.id.clone())
            .unwrap_or_else(|| Uuid::new_v4().to_string()),
        skill_id: skill_id.to_string(),
        tool: tool.to_string(),
        scope: scope.to_string(),
        project_path: project_path.map(str::to_string),
        target_path: target_path.to_string_lossy().to_string(),
        mode: existing
            .as_ref()
            .map(|target| target.mode.clone())
            .unwrap_or_else(|| sync_mode_name(requested_mode).to_string()),
        status: "error".to_string(),
        last_error: Some(error.to_string()),
        synced_at: existing.and_then(|target| target.synced_at),
    };
    store.upsert_skill_target(&record)
}

#[tauri::command]
pub async fn sync_skill_dir(
    source_path: String,
    target_path: String,
) -> Result<SyncResultDto, String> {
    tauri::async_runtime::spawn_blocking(move || {
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
    store: State<'_, SkillStore>,
    sourcePath: String,
    skillId: String,
    tool: String,
    name: String,
    overwrite: Option<bool>,
    overwriteIfSameContent: Option<bool>,
    scope: Option<String>,
    projectPath: Option<String>,
) -> Result<SyncResultDto, String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let runtime_tool = runtime_tool_by_key(&store, &tool)?;
        let scope = normalize_scope(scope.as_deref())?;
        if scope == "project" && !runtime_tool.supports_project_scope {
            anyhow::bail!("PROJECT_SCOPE_UNSUPPORTED|{}", runtime_tool.key);
        }
        let project_root = if scope == "project" {
            let raw = projectPath
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("projectPath is required for project scope"))?;
            let path = expand_home_path(raw)?;
            if !path.is_dir() {
                anyhow::bail!("projectPath must be an existing directory: {:?}", path);
            }
            Some(path)
        } else {
            None
        };

        let tool_root = resolve_runtime_tool_root(&runtime_tool, project_root.as_deref())?;
        let target = tool_root.join(&name);
        ensure_target_does_not_overlap_local_source(&store, &skillId, &target)?;
        let project_path_for_record = project_root
            .as_ref()
            .map(|path| path.to_string_lossy().to_string());
        if scope == "global" && !runtime_tool.installed {
            let error = format!("TOOL_NOT_INSTALLED|{}", runtime_tool.key);
            record_skill_target_failure(
                &store,
                &skillId,
                &tool,
                scope,
                project_path_for_record.as_deref(),
                &target,
                runtime_tool.sync_mode,
                &error,
            )?;
            anyhow::bail!(error);
        }
        // Pre-check: ensure the skills directory is writable (fixes #20 — Windows OS error 5).
        if let Err(err) = std::fs::create_dir_all(&tool_root) {
            let error = if err.kind() == std::io::ErrorKind::PermissionDenied {
                format!(
                    "TOOL_NOT_WRITABLE|{}|{}",
                    runtime_tool.label,
                    tool_root.to_string_lossy()
                )
            } else {
                format!("failed to create skills dir {:?}: {}", tool_root, err)
            };
            record_skill_target_failure(
                &store,
                &skillId,
                &tool,
                scope,
                project_path_for_record.as_deref(),
                &target,
                runtime_tool.sync_mode,
                &error,
            )?;
            anyhow::bail!(error);
        }
        if let Some(existing) =
            store.get_skill_target(&skillId, &tool, scope, project_path_for_record.as_deref())?
        {
            if existing.mode == "copy"
                && existing.target_path == target.to_string_lossy()
                && overwrite != Some(true)
            {
                let previous =
                    crate::core::content_hash::hash_dir_for_sync_conflict(sourcePath.as_ref())?;
                crate::core::tool_distribution::refresh_copy(
                    &store,
                    &skillId,
                    sourcePath.as_ref(),
                    &target,
                    Some(&previous),
                )?;
                return Ok(SyncResultDto {
                    mode_used: "copy".into(),
                    target_path: existing.target_path,
                });
            }
            if existing.status == "ok"
                && overwrite != Some(true)
                && existing.target_path == target.to_string_lossy()
                && target.exists()
            {
                return Ok::<_, anyhow::Error>(SyncResultDto {
                    mode_used: existing.mode,
                    target_path: existing.target_path,
                });
            }
        }
        let overwrite = overwrite.unwrap_or(false)
            || (overwriteIfSameContent.unwrap_or(false)
                && target_has_same_content(sourcePath.as_ref(), &target));
        let result = if runtime_tool.is_custom {
            sync_dir_with_mode_with_overwrite(
                runtime_tool.sync_mode,
                sourcePath.as_ref(),
                &target,
                overwrite,
            )
        } else {
            sync_dir_for_tool_with_overwrite(&tool, sourcePath.as_ref(), &target, overwrite)
        };
        let result = match result {
            Ok(result) => result,
            Err(err) => {
                let msg = err.to_string();
                let error = if msg.contains("target already exists") {
                    format!("TARGET_EXISTS|{}", target.to_string_lossy())
                } else if msg.contains("os error 5")
                    || msg.contains("Access is denied")
                    || msg.contains("Permission denied")
                {
                    format!(
                        "TOOL_NOT_WRITABLE|{}|{}",
                        runtime_tool.label,
                        tool_root.to_string_lossy()
                    )
                } else {
                    msg
                };
                record_skill_target_failure(
                    &store,
                    &skillId,
                    &tool,
                    scope,
                    project_path_for_record.as_deref(),
                    &target,
                    runtime_tool.sync_mode,
                    &error,
                )?;
                anyhow::bail!(error);
            }
        };

        // Some tools share the same skills directory; keep DB records consistent across them.
        let group = runtime_tools_sharing_dir(&store, &runtime_tool, scope)?;
        for a in group {
            let record = SkillTargetRecord {
                id: Uuid::new_v4().to_string(),
                skill_id: skillId.clone(),
                tool: a.key,
                scope: scope.to_string(),
                project_path: project_path_for_record.clone(),
                target_path: result.target_path.to_string_lossy().to_string(),
                mode: match result.mode_used {
                    SyncMode::Auto => "auto",
                    SyncMode::Symlink => "symlink",
                    SyncMode::Junction => "junction",
                    SyncMode::Copy => "copy",
                }
                .to_string(),
                status: "ok".to_string(),
                last_error: None,
                synced_at: Some(now_ms()),
            };
            store.upsert_skill_target(&record)?;
        }

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

fn target_has_same_content(source: &std::path::Path, target: &std::path::Path) -> bool {
    if !source.is_dir() || !target.is_dir() {
        return false;
    }
    match (hash_dir(source), hash_dir(target)) {
        (Ok(source_hash), Ok(target_hash)) => source_hash == target_hash,
        _ => false,
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
    store: State<'_, SkillStore>,
    skillId: String,
    tool: String,
    scope: Option<String>,
    projectPath: Option<String>,
) -> Result<(), String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let scope = normalize_scope(scope.as_deref())?;
        let project_path = if scope == "project" {
            let raw = projectPath
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("projectPath is required for project scope"))?;
            Some(expand_home_path(raw)?.to_string_lossy().to_string())
        } else {
            None
        };

        // Some tools share the same skills directory; unsync should update all of them.
        let group_tool_keys: Vec<String> =
            if let Ok(runtime_tool) = runtime_tool_by_key(&store, &tool) {
                runtime_tools_sharing_dir(&store, &runtime_tool, scope)?
                    .into_iter()
                    .map(|tool| tool.key)
                    .collect()
            } else if let Some(adapter) = adapter_by_key(&tool) {
                let group = if scope == "project" {
                    adapters_sharing_project_skills_dir(&adapter)
                } else {
                    crate::core::tool_adapters::adapters_sharing_skills_dir(&adapter)
                };
                // If none of the group tools are installed, do nothing (treat as already not effective).
                if scope == "global" {
                    let mut any_installed = false;
                    for a in &group {
                        if is_tool_installed(a)? {
                            any_installed = true;
                            break;
                        }
                    }
                    if !any_installed {
                        return Ok::<_, anyhow::Error>(());
                    }
                }
                group
                    .into_iter()
                    .map(|a| a.id.as_key().to_string())
                    .collect()
            } else {
                vec![tool.clone()]
            };

        // Remove filesystem target once (shared dir => shared target path).
        let mut removed = false;
        for k in &group_tool_keys {
            if let Some(target) =
                store.get_skill_target(&skillId, k, scope, project_path.as_deref())?
            {
                if !removed {
                    remove_skill_target_safely(&store, &skillId, &target.target_path)?;
                    removed = true;
                }
                store.delete_skill_target(&skillId, k, scope, project_path.as_deref())?;
            }
        }

        Ok::<_, anyhow::Error>(())
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(format_anyhow_error)
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn set_skill_enabled(
    store: State<'_, SkillStore>,
    skillId: String,
    enabled: bool,
) -> Result<(), String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
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
    app: tauri::AppHandle,
    store: State<'_, SkillStore>,
    skillId: String,
) -> Result<UpdateResultDto, String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let res = update_managed_skill_from_source(&app, &store, &skillId)?;
        Ok::<_, anyhow::Error>(UpdateResultDto {
            skill_id: res.skill_id,
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
    .map_err(format_anyhow_error)
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
    app: tauri::AppHandle,
    store: State<'_, SkillStore>,
    sourcePath: String,
    name: Option<String>,
) -> Result<InstallResultDto, String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let source = std::path::Path::new(&sourcePath);
        // Validate SKILL.md exists before importing (fixes #8: prevents importing
        // directories that were "discovered" but lack a valid SKILL.md).
        if !source.join("SKILL.md").exists() {
            anyhow::bail!("SKILL_INVALID|missing_skill_md");
        }
        let result = import_existing_local_skill(&app, &store, source, name)?;
        Ok::<_, anyhow::Error>(to_install_dto(result))
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(format_anyhow_error)
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

#[tauri::command]
pub fn get_managed_skills(store: State<'_, SkillStore>) -> Result<Vec<ManagedSkillDto>, String> {
    get_managed_skills_impl(store.inner())
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
pub fn create_tag(store: State<'_, SkillStore>, name: String) -> Result<TagDto, String> {
    store
        .create_tag(&name)
        .map(|tag| TagDto {
            id: tag.id,
            name: tag.name,
        })
        .map_err(format_anyhow_error)
}

#[tauri::command]
#[allow(non_snake_case)]
pub fn rename_tag(
    store: State<'_, SkillStore>,
    tagId: i64,
    name: String,
) -> Result<TagDto, String> {
    store
        .rename_tag(tagId, &name)
        .map(|tag| TagDto {
            id: tag.id,
            name: tag.name,
        })
        .map_err(format_anyhow_error)
}

#[tauri::command]
#[allow(non_snake_case)]
pub fn delete_tag(store: State<'_, SkillStore>, tagId: i64) -> Result<(), String> {
    store.delete_tag(tagId).map_err(format_anyhow_error)
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
    store: State<'_, SkillStore>,
    skillId: String,
    tagIds: Vec<i64>,
) -> Result<(), String> {
    store
        .set_skill_tags(&skillId, &tagIds)
        .map_err(format_anyhow_error)
}

#[tauri::command]
pub fn get_untagged_skill_ids(store: State<'_, SkillStore>) -> Result<Vec<String>, String> {
    store.list_untagged_skill_ids().map_err(format_anyhow_error)
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn delete_managed_skill(
    app: tauri::AppHandle,
    store: State<'_, SkillStore>,
    skillId: String,
) -> Result<(), String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _device_sync_guard = if store.get_device_sync_config()?.is_some() {
            Some(crate::core::device_sync::try_lock_device_sync()?)
        } else {
            None
        };
        // 便于排查“按钮点了没反应”：确认前端确实触发了命令
        println!("[delete_managed_skill] skillId={}", skillId);

        // 先删除已同步到各工具目录的副本/软链接
        // 注意：如果先删 skills 行，会触发 skill_targets cascade，导致无法再拿到 target_path
        let targets = store.list_skill_targets(&skillId)?;

        let mut remove_failures: Vec<String> = Vec::new();
        for target in targets {
            if let Err(err) = remove_skill_target_safely(&store, &skillId, &target.target_path) {
                remove_failures.push(format!("{}: {}", target.target_path, err));
            }
        }

        let record = store.get_skill_by_id(&skillId)?;
        if let Some(skill) = record {
            let path = std::path::PathBuf::from(&skill.central_path);
            let overlaps_local_source = match skill.external_local_source() {
                Some(source) => paths_overlap(&path, std::path::Path::new(source))?,
                None => false,
            };
            if path.exists() && !overlaps_local_source {
                if store.get_device_sync_config()?.is_some() {
                    let trash_id = Uuid::new_v4().to_string();
                    let trash_path = app
                        .path()
                        .app_data_dir()?
                        .join("device-sync")
                        .join("trash")
                        .join(&trash_id);
                    std::fs::create_dir_all(
                        trash_path.parent().context("trash path has no parent")?,
                    )?;
                    copy_dir_recursive(&path, &trash_path)
                        .context("copy deleted Skill to the recoverable app recycle bin")?;
                    if let Err(err) = remove_path_any_core(&path)
                        .context("move deleted Skill to the system recycle bin")
                    {
                        let _ = remove_path_any_core(&trash_path);
                        return Err(err);
                    }
                    let deleted_at = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as i64;
                    store.add_device_sync_trash(&TrashEntry {
                        id: trash_id,
                        skill_id: skill.id.clone(),
                        skill_name: skill.name.clone(),
                        trash_path: trash_path.to_string_lossy().to_string(),
                        deleted_at,
                        expires_at: deleted_at + 30 * 24 * 60 * 60 * 1000,
                    })?;
                } else {
                    remove_path_any_core(&path)?;
                }
            }
            store.delete_skill(&skillId)?;
        }

        if !remove_failures.is_empty() {
            anyhow::bail!(
                "已删除托管记录，但清理部分工具目录失败：\n- {}",
                remove_failures.join("\n- ")
            );
        }

        Ok::<_, anyhow::Error>(())
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(format_anyhow_error)
}

#[cfg(test)]
fn remove_path_any(path: &str) -> Result<(), String> {
    remove_path_any_core(std::path::Path::new(path)).map_err(|err| format!("{path}: {err:#}"))
}

fn to_install_dto(result: InstallResult) -> InstallResultDto {
    InstallResultDto {
        skill_id: result.skill_id,
        name: result.name,
        central_path: result.central_path.to_string_lossy().to_string(),
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

fn managed_skill_status(skill: &SkillRecord) -> String {
    if skill.status != "ok" {
        return skill.status.clone();
    }
    if skill.source_type != "local" {
        return skill.status.clone();
    }
    let source_exists = skill
        .source_ref
        .as_deref()
        .and_then(|source| expand_home_path(source).ok())
        .map(|source| source.exists())
        .unwrap_or(false);
    if source_exists {
        skill.status.clone()
    } else {
        "error".to_string()
    }
}

fn get_managed_skills_impl(store: &SkillStore) -> Result<Vec<ManagedSkillDto>, String> {
    let skills = store.list_skills().map_err(|err| err.to_string())?;
    let checks = store.source_checks().map_err(format_anyhow_error)?;
    Ok(skills
        .into_iter()
        .map(|skill| {
            let source_check = checks.get(&skill.id);
            let source_error = source_check.and_then(|check| check.0.clone());
            let status = if source_error.is_some() {
                "error".into()
            } else {
                managed_skill_status(&skill)
            };
            let targets = store
                .list_skill_targets(&skill.id)
                .unwrap_or_default()
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
                .collect();
            let tags = store
                .get_skill_tags(&skill.id)
                .unwrap_or_default()
                .into_iter()
                .map(|tag| TagDto {
                    id: tag.id,
                    name: tag.name,
                })
                .collect();

            ManagedSkillDto {
                source_error,
                source_checked_at: source_check.map(|check| check.1),
                id: skill.id,
                name: skill.name,
                description: skill.description,
                source_type: skill.source_type,
                source_ref: skill.source_ref,
                central_path: skill.central_path,
                created_at: skill.created_at,
                updated_at: skill.updated_at,
                last_sync_at: skill.last_sync_at,
                enabled: skill.enabled,
                status,
                tags,
                targets,
            }
        })
        .collect())
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
    store: State<'_, SkillStore>,
    query: String,
    limit: Option<u32>,
) -> Result<Vec<OnlineSkillDto>, String> {
    let store = store.inner().clone();
    let limit = limit.unwrap_or(20) as usize;
    tauri::async_runtime::spawn_blocking(move || {
        let proxy_url = get_github_proxy_url_core(&store)?;
        let results = search_skills_online_core(&query, limit, &proxy_url)?;
        Ok::<_, anyhow::Error>(results.into_iter().map(OnlineSkillDto::from).collect())
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(format_anyhow_error)
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
        if resolve_access_token(&credentials, key, usage)
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
pub async fn start_device_sync_oauth(providerId: ProviderId) -> Result<OAuthStartResult, String> {
    tauri::async_runtime::spawn_blocking(move || oauth::start(providerId))
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
        poll_device_sync_oauth_with(
            &store,
            &credentials,
            || oauth::poll(&flowId, &credentials),
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
    providerId: ProviderId,
    token: String,
) -> Result<ProviderAccount, String> {
    tauri::async_runtime::spawn_blocking(move || provider(providerId).validate_token(token.trim()))
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
        let provider = provider(providerId);
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
    credentialKey: Option<String>,
) -> Result<Vec<RemoteRepository>, String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let token = resolve_device_sync_token(&store, providerId, None, credentialKey)?;
        provider(providerId).list_repositories(&token)
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
        .list_device_sync_history(limit.unwrap_or(50).max(1))
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
    let workspace = app
        .path()
        .app_data_dir()
        .context("resolve device sync data directory")?
        .join("device-sync");
    let central = resolve_central_repo_path(app, store)?;
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
    resolve_access_token(
        &SystemCredentialStore,
        &key,
        &CredentialUsage::official(provider),
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
