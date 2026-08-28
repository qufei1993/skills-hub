use anyhow::Context;
use serde::{Deserialize, Serialize};
use tauri::State;

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
use crate::core::central_repo::{ensure_central_repo, resolve_central_repo_path};
use crate::core::content_hash::hash_dir;
use crate::core::featured_skills::{fetch_featured_skills, FeaturedSkill};
use crate::core::github_search::{search_github_repos, RepoSummary};
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
use crate::core::skill_store::{SkillStore, SkillTargetRecord};
use crate::core::skills_search::{
    search_skills_online as search_skills_online_core, OnlineSkillResult,
};
use crate::core::sync_engine::{
    copy_dir_recursive, sync_dir_for_tool_with_overwrite, sync_dir_hybrid,
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

fn format_anyhow_error(err: anyhow::Error) -> String {
    let first = err.to_string();
    // Frontend relies on these prefixes for special flows.
    if first.starts_with("MULTI_SKILLS|")
        || first.starts_with("TARGET_EXISTS|")
        || first.starts_with("TOOL_NOT_INSTALLED|")
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
    pub last_updated: usize,
    pub last_failed: usize,
    pub progress: AutoUpdateProgressSnapshot,
}

#[derive(Debug, Serialize)]
pub struct AutoUpdateRunResultDto {
    pub checked: usize,
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

#[tauri::command]
pub async fn set_central_repo_path(
    app: tauri::AppHandle,
    store: State<'_, SkillStore>,
    path: String,
) -> Result<String, String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let new_base = expand_home_path(&path)?;
        if !new_base.is_absolute() {
            anyhow::bail!("storage path must be absolute");
        }
        ensure_central_repo(&new_base)?;

        let current_base = resolve_central_repo_path(&app, &store)?;
        let skills = store.list_skills()?;
        if current_base == new_base {
            store.set_setting("central_repo_path", new_base.to_string_lossy().as_ref())?;
            return Ok::<_, anyhow::Error>(new_base.to_string_lossy().to_string());
        }

        if !skills.is_empty() {
            for skill in skills {
                let old_path = std::path::PathBuf::from(&skill.central_path);
                if !old_path.exists() {
                    anyhow::bail!("central path not found: {:?}", old_path);
                }
                let file_name = old_path
                    .file_name()
                    .ok_or_else(|| anyhow::anyhow!("invalid central path: {:?}", old_path))?;
                let new_path = new_base.join(file_name);
                if new_path.exists() {
                    anyhow::bail!("target path already exists: {:?}", new_path);
                }

                if let Err(err) = std::fs::rename(&old_path, &new_path) {
                    copy_dir_recursive(&old_path, &new_path)
                        .with_context(|| format!("copy {:?} -> {:?}", old_path, new_path))?;
                    std::fs::remove_dir_all(&old_path)
                        .with_context(|| format!("cleanup {:?}", old_path))?;
                    // Surface rename error in logs for troubleshooting.
                    eprintln!("rename failed, fallback used: {}", err);
                }

                let mut updated = skill.clone();
                updated.central_path = new_path.to_string_lossy().to_string();
                updated.updated_at = now_ms();
                store.upsert_skill(&updated)?;
            }
        }

        store.set_setting("central_repo_path", new_base.to_string_lossy().as_ref())?;
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

        if scope == "global" && !runtime_tool.installed {
            anyhow::bail!("TOOL_NOT_INSTALLED|{}", runtime_tool.key);
        }
        let tool_root = resolve_runtime_tool_root(&runtime_tool, project_root.as_deref())?;
        // Pre-check: ensure the skills directory is writable (fixes #20 — Windows OS error 5).
        if let Err(err) = std::fs::create_dir_all(&tool_root) {
            if err.kind() == std::io::ErrorKind::PermissionDenied {
                anyhow::bail!(
                    "TOOL_NOT_WRITABLE|{}|{}",
                    runtime_tool.label,
                    tool_root.to_string_lossy()
                );
            }
            anyhow::bail!("failed to create skills dir {:?}: {}", tool_root, err);
        }
        let target = tool_root.join(&name);
        let project_path_for_record = project_root
            .as_ref()
            .map(|path| path.to_string_lossy().to_string());
        if let Some(existing) =
            store.get_skill_target(&skillId, &tool, scope, project_path_for_record.as_deref())?
        {
            if existing.status != "disabled"
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
        let result = (if runtime_tool.is_custom {
            sync_dir_with_mode_with_overwrite(
                runtime_tool.sync_mode,
                sourcePath.as_ref(),
                &target,
                overwrite,
            )
        } else {
            sync_dir_for_tool_with_overwrite(&tool, sourcePath.as_ref(), &target, overwrite)
        })
        .map_err(|err| {
            let msg = err.to_string();
            if msg.contains("target already exists") {
                anyhow::anyhow!("TARGET_EXISTS|{}", target.to_string_lossy())
            } else if msg.contains("os error 5")
                || msg.contains("Access is denied")
                || msg.contains("Permission denied")
            {
                anyhow::anyhow!(
                    "TOOL_NOT_WRITABLE|{}|{}",
                    runtime_tool.label,
                    tool_root.to_string_lossy()
                )
            } else {
                anyhow::anyhow!(msg)
            }
        })?;

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
                    remove_path_any(&target.target_path).map_err(anyhow::Error::msg)?;
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
                    if let Err(err) = remove_path_any(&target.target_path) {
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
        let token = store.get_setting("github_token")?.unwrap_or_default();
        let proxy_url = get_github_proxy_url_core(&store)?;
        let token_opt = if token.is_empty() {
            None
        } else {
            Some(token.as_str())
        };
        search_github_repos(&query, limit, token_opt, &proxy_url)
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

#[tauri::command]
pub async fn get_github_token(store: State<'_, SkillStore>) -> Result<String, String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        Ok::<_, anyhow::Error>(store.get_setting("github_token")?.unwrap_or_default())
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(format_anyhow_error)
}

#[tauri::command]
pub async fn set_github_token(store: State<'_, SkillStore>, token: String) -> Result<(), String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let trimmed = token.trim();
        if trimmed.is_empty() {
            store.set_setting("github_token", "")?;
        } else {
            store.set_setting("github_token", trimmed)?;
        }
        Ok::<_, anyhow::Error>(())
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
    store: State<'_, SkillStore>,
    skillId: String,
) -> Result<(), String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        // 便于排查“按钮点了没反应”：确认前端确实触发了命令
        println!("[delete_managed_skill] skillId={}", skillId);

        // 先删除已同步到各工具目录的副本/软链接
        // 注意：如果先删 skills 行，会触发 skill_targets cascade，导致无法再拿到 target_path
        let targets = store.list_skill_targets(&skillId)?;

        let mut remove_failures: Vec<String> = Vec::new();
        for target in targets {
            if let Err(err) = remove_path_any(&target.target_path) {
                remove_failures.push(format!("{}: {}", target.target_path, err));
            }
        }

        let record = store.get_skill_by_id(&skillId)?;
        if let Some(skill) = record {
            let path = std::path::PathBuf::from(skill.central_path);
            if path.exists() {
                std::fs::remove_dir_all(&path)?;
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

fn remove_path_any(path: &str) -> Result<(), String> {
    let p = std::path::Path::new(path);
    // 用 symlink_metadata 而非 exists()：悬空链接（目标已删）也要清理掉
    let meta = match std::fs::symlink_metadata(p) {
        Ok(meta) => meta,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(format!("{path}: {err}")),
    };
    let ft = meta.file_type();

    // 删除链接本身：Windows junction 虽然 is_symlink()==true，但它是目录型
    // reparse point，remove_file（DeleteFileW）会报 os error 5，必须先试
    // remove_dir（RemoveDirectoryW 只移除链接，不会穿透到目标）
    if ft.is_symlink() {
        #[cfg(windows)]
        {
            if std::fs::remove_dir(p).is_ok() {
                return Ok(());
            }
        }
        std::fs::remove_file(p).map_err(|err| format!("{path}: {err}"))?;
        return Ok(());
    }

    if ft.is_dir() {
        std::fs::remove_dir_all(p).map_err(|err| format!("{path}: {err}"))?;
        return Ok(());
    }

    std::fs::remove_file(p).map_err(|err| format!("{path}: {err}"))?;
    Ok(())
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
        last_updated: config.last_updated,
        last_failed: config.last_failed,
        progress: config.progress,
    }
}

fn to_auto_update_run_result_dto(result: AutoUpdateRunResult) -> AutoUpdateRunResultDto {
    AutoUpdateRunResultDto {
        checked: result.checked,
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

fn get_managed_skills_impl(store: &SkillStore) -> Result<Vec<ManagedSkillDto>, String> {
    let skills = store.list_skills().map_err(|err| err.to_string())?;
    Ok(skills
        .into_iter()
        .map(|skill| {
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
                status: skill.status,
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

#[cfg(test)]
#[path = "tests/commands.rs"]
mod tests;
