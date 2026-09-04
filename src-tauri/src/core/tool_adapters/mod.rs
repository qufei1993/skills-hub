use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::skill_store::{SkillStore, SkillTargetRecord};
use super::sync_engine::{
    path_is_protected_real_content, paths_overlap, remove_path_any,
    sync_dir_with_mode_with_overwrite, SyncMode,
};

pub const TOOL_CONFIG_SETTING: &str = "tool_config_v1";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToolId {
    Cursor,
    ClaudeCode,
    Codex,
    DeepSeekHarness,
    OpenCode,
    Antigravity,
    Amp,
    KimiCli,
    Augment,
    OpenClaw,
    Copaw,
    Cline,
    CodeBuddy,
    CodeWhale,
    WorkBuddy,
    CommandCode,
    Continue,
    Crush,
    Junie,
    IflowCli,
    KiroCli,
    Kode,
    McpJam,
    MistralVibe,
    Mux,
    OpenClaude,
    OpenHands,
    Pi,
    Qoder,
    QoderWork,
    QwenCode,
    Trae,
    TraeCn,
    Zencoder,
    Neovate,
    Pochi,
    AdaL,
    KiloCode,
    RooCode,
    Goose,
    GeminiCli,
    GithubCopilot,
    Clawdbot,
    Droid,
    Windsurf,
    Moltbot,
    HermesAgent,
}

impl ToolId {
    pub fn as_key(&self) -> &'static str {
        match self {
            ToolId::Cursor => "cursor",
            ToolId::ClaudeCode => "claude_code",
            ToolId::Codex => "codex",
            ToolId::DeepSeekHarness => "deepseek_harness",
            ToolId::OpenCode => "opencode",
            ToolId::Antigravity => "antigravity",
            ToolId::Amp => "amp",
            ToolId::KimiCli => "kimi_cli",
            ToolId::Augment => "augment",
            ToolId::OpenClaw => "openclaw",
            ToolId::Copaw => "copaw",
            ToolId::Cline => "cline",
            ToolId::CodeBuddy => "codebuddy",
            ToolId::CodeWhale => "codewhale",
            ToolId::WorkBuddy => "workbuddy",
            ToolId::CommandCode => "command_code",
            ToolId::Continue => "continue",
            ToolId::Crush => "crush",
            ToolId::Junie => "junie",
            ToolId::IflowCli => "iflow_cli",
            ToolId::KiroCli => "kiro_cli",
            ToolId::Kode => "kode",
            ToolId::McpJam => "mcpjam",
            ToolId::MistralVibe => "mistral_vibe",
            ToolId::Mux => "mux",
            ToolId::OpenClaude => "openclaude",
            ToolId::OpenHands => "openhands",
            ToolId::Pi => "pi",
            ToolId::Qoder => "qoder",
            ToolId::QoderWork => "qoderwork",
            ToolId::QwenCode => "qwen_code",
            ToolId::Trae => "trae",
            ToolId::TraeCn => "trae_cn",
            ToolId::Zencoder => "zencoder",
            ToolId::Neovate => "neovate",
            ToolId::Pochi => "pochi",
            ToolId::AdaL => "adal",
            ToolId::KiloCode => "kilo_code",
            ToolId::RooCode => "roo_code",
            ToolId::Goose => "goose",
            ToolId::GeminiCli => "gemini_cli",
            ToolId::GithubCopilot => "github_copilot",
            ToolId::Clawdbot => "clawdbot",
            ToolId::Droid => "droid",
            ToolId::Windsurf => "windsurf",
            ToolId::Moltbot => "moltbot",
            ToolId::HermesAgent => "hermes_agent",
        }
    }
}

#[derive(Clone, Debug)]
pub struct ToolAdapter {
    pub id: ToolId,
    pub display_name: &'static str,
    /// Global skill directory under user home (aligned with add-skill docs).
    pub relative_skills_dir: &'static str,
    /// Directory used to detect whether the tool is installed (aligned with add-skill docs).
    pub relative_detect_dir: &'static str,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CustomToolConfig {
    pub key: String,
    pub label: String,
    #[serde(default)]
    pub avatar: Option<String>,
    pub skills_dir: String,
    pub project_skills_dir: Option<String>,
    #[serde(default)]
    pub sync_mode: SyncMode,
    pub enabled: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ToolConfig {
    #[serde(default)]
    pub disabled_builtin_tools: Vec<String>,
    #[serde(default)]
    pub custom_tools: Vec<CustomToolConfig>,
}

#[derive(Clone, Debug)]
pub struct DetectedSkill {
    pub tool: ToolId,
    pub name: String,
    pub path: PathBuf,
    pub is_link: bool,
    pub link_target: Option<PathBuf>,
}

pub fn load_tool_config(store: &SkillStore) -> Result<ToolConfig> {
    let raw = store.get_setting(TOOL_CONFIG_SETTING)?;
    let config = raw
        .as_deref()
        .and_then(|value| serde_json::from_str::<ToolConfig>(value).ok())
        .unwrap_or_default();
    sanitize_tool_config(config)
}

pub fn save_tool_config(store: &SkillStore, config: ToolConfig) -> Result<ToolConfig> {
    let previous = load_tool_config(store)?;
    let config = sanitize_tool_config(config)?;
    ensure_enabled_custom_tool_dirs(&config)?;
    migrate_changed_custom_tool_targets(store, &previous, &config)?;
    store.set_setting(TOOL_CONFIG_SETTING, &serde_json::to_string(&config)?)?;
    Ok(config)
}

fn migrate_changed_custom_tool_targets(
    store: &SkillStore,
    previous: &ToolConfig,
    next: &ToolConfig,
) -> Result<()> {
    let previous_by_key = previous
        .custom_tools
        .iter()
        .map(|tool| (tool.key.as_str(), tool))
        .collect::<std::collections::HashMap<_, _>>();

    for next_tool in &next.custom_tools {
        let Some(previous_tool) = previous_by_key.get(next_tool.key.as_str()) else {
            continue;
        };
        if previous_tool.skills_dir == next_tool.skills_dir
            && previous_tool.project_skills_dir == next_tool.project_skills_dir
            && previous_tool.sync_mode == next_tool.sync_mode
        {
            continue;
        }

        for target in store.list_skill_targets_by_tool(&next_tool.key)? {
            if target.status == "disabled" {
                continue;
            }
            migrate_custom_tool_target(store, next_tool, &target)?;
        }
    }

    Ok(())
}

fn migrate_custom_tool_target(
    store: &SkillStore,
    tool: &CustomToolConfig,
    target: &SkillTargetRecord,
) -> Result<()> {
    let skill = store
        .get_skill_by_id(&target.skill_id)?
        .with_context(|| format!("skill not found for target {}", target.id))?;
    let source = PathBuf::from(&skill.central_path);
    if !source.is_dir() {
        anyhow::bail!("managed skill directory not found: {:?}", source);
    }

    let target_name = Path::new(&target.target_path)
        .file_name()
        .with_context(|| format!("invalid target path: {}", target.target_path))?;
    let root = if target.scope == "project" {
        let project_path = target
            .project_path
            .as_deref()
            .context("project target is missing its project path")?;
        let relative = tool.project_skills_dir.as_deref().with_context(|| {
            format!(
                "custom tool {} still has project Skills synced; set a project directory before saving",
                tool.label
            )
        })?;
        PathBuf::from(project_path).join(relative)
    } else {
        expand_custom_tool_path(&tool.skills_dir)?
    };
    let next_target = root.join(target_name);
    let previous_target = PathBuf::from(&target.target_path);
    let local_source = (skill.source_type == "local")
        .then_some(skill.source_ref.as_deref())
        .flatten()
        .filter(|path| !path.trim().is_empty())
        .map(PathBuf::from);
    if let Some(local_source) = local_source.as_ref() {
        if paths_overlap(&next_target, local_source)? {
            anyhow::bail!(
                "SKILL_TARGET_OVERLAPS_SOURCE|{}|custom tool target overlaps original local source",
                local_source.to_string_lossy()
            );
        }
    }
    let mut protected_paths = vec![source.clone()];
    if let Some(local_source) = local_source {
        protected_paths.push(local_source);
    }
    let previous_is_protected = path_is_protected_real_content(&previous_target, &protected_paths)?;
    let same_path = next_target == previous_target;
    let shared_target =
        store.is_skill_target_path_used_by_another_record(&target.target_path, &target.id)?;

    // A shared physical target must keep its current representation because changing it would
    // also mutate the other tool's live target. The selected mode still applies to future paths.
    if same_path && shared_target {
        return Ok(());
    }

    if !same_path && std::fs::symlink_metadata(&next_target).is_ok() {
        anyhow::bail!("new custom tool target already exists: {:?}", next_target);
    }

    let requested_mode = sync_mode_key(tool.sync_mode);
    let force_mode_recreate =
        same_path && tool.sync_mode != SyncMode::Auto && target.mode != requested_mode;
    if force_mode_recreate {
        if previous_is_protected {
            anyhow::bail!(
                "refusing to replace protected Skill source: {:?}",
                previous_target
            );
        }
        remove_path_any(&previous_target)
            .with_context(|| format!("remove old target {:?}", previous_target))?;
    }
    let outcome = sync_dir_with_mode_with_overwrite(
        tool.sync_mode,
        &source,
        &next_target,
        same_path && !force_mode_recreate,
    )
    .with_context(|| {
        format!(
            "migrate custom tool target {:?} -> {:?}",
            previous_target, next_target
        )
    });
    let outcome = match outcome {
        Ok(outcome) => outcome,
        Err(err) => {
            if force_mode_recreate {
                if let Some(previous_mode) = parse_sync_mode(&target.mode) {
                    let _ = sync_dir_with_mode_with_overwrite(
                        previous_mode,
                        &source,
                        &previous_target,
                        false,
                    );
                }
            }
            return Err(err);
        }
    };

    if !same_path && !shared_target && !previous_is_protected {
        if let Err(err) = remove_path_any(&previous_target) {
            let _ = remove_path_any(&next_target);
            return Err(err).with_context(|| format!("remove old target {:?}", previous_target));
        }
    }

    let migrated = SkillTargetRecord {
        id: target.id.clone(),
        skill_id: target.skill_id.clone(),
        tool: target.tool.clone(),
        scope: target.scope.clone(),
        project_path: target.project_path.clone(),
        target_path: outcome.target_path.to_string_lossy().to_string(),
        mode: sync_mode_key(outcome.mode_used).to_string(),
        status: "ok".to_string(),
        last_error: None,
        synced_at: Some(current_time_ms()),
    };
    store.upsert_skill_target(&migrated)?;

    Ok(())
}

fn sync_mode_key(mode: SyncMode) -> &'static str {
    match mode {
        SyncMode::Auto => "auto",
        SyncMode::Symlink => "symlink",
        SyncMode::Junction => "junction",
        SyncMode::Copy => "copy",
    }
}

fn parse_sync_mode(mode: &str) -> Option<SyncMode> {
    match mode {
        "auto" => Some(SyncMode::Auto),
        "symlink" => Some(SyncMode::Symlink),
        "junction" => Some(SyncMode::Junction),
        "copy" => Some(SyncMode::Copy),
        _ => None,
    }
}

fn current_time_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

pub fn is_builtin_tool_enabled(config: &ToolConfig, key: &str) -> bool {
    !config
        .disabled_builtin_tools
        .iter()
        .any(|disabled| disabled == key)
}

pub fn is_valid_custom_tool_key(key: &str) -> bool {
    let mut chars = key.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

fn expand_custom_tool_path(input: &str) -> Result<PathBuf> {
    let trimmed = input.trim();
    if trimmed == "~" {
        return dirs::home_dir().context("failed to resolve home directory");
    }
    if let Some(rest) = trimmed.strip_prefix("~/") {
        let home = dirs::home_dir().context("failed to resolve home directory")?;
        return Ok(home.join(rest));
    }
    Ok(PathBuf::from(trimmed))
}

fn ensure_enabled_custom_tool_dirs(config: &ToolConfig) -> Result<()> {
    for tool in &config.custom_tools {
        if !tool.enabled {
            continue;
        }
        let skills_dir = expand_custom_tool_path(&tool.skills_dir)?;
        std::fs::create_dir_all(&skills_dir)
            .with_context(|| format!("create custom tool skills dir {:?}", skills_dir))?;
    }
    Ok(())
}

fn sanitize_tool_config(mut config: ToolConfig) -> Result<ToolConfig> {
    let builtin_keys = default_tool_adapters()
        .into_iter()
        .map(|adapter| adapter.id.as_key().to_string())
        .collect::<std::collections::HashSet<_>>();

    config
        .disabled_builtin_tools
        .retain(|key| builtin_keys.contains(key));
    config.disabled_builtin_tools.sort();
    config.disabled_builtin_tools.dedup();

    let mut seen_custom_keys = std::collections::HashSet::new();
    let mut custom_tools = Vec::new();
    for mut tool in config.custom_tools {
        tool.key = tool.key.trim().to_string();
        tool.label = tool.label.trim().to_string();
        tool.avatar = tool
            .avatar
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        tool.skills_dir = tool.skills_dir.trim().to_string();
        tool.project_skills_dir = tool
            .project_skills_dir
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        if tool.key.is_empty() {
            anyhow::bail!("custom tool key is required");
        }
        if builtin_keys.contains(&tool.key) {
            anyhow::bail!(
                "custom tool key conflicts with a built-in tool: {}",
                tool.key
            );
        }
        if !is_valid_custom_tool_key(&tool.key) {
            anyhow::bail!("custom tool key contains invalid characters: {}", tool.key);
        }
        if !seen_custom_keys.insert(tool.key.clone()) {
            anyhow::bail!("duplicate custom tool key: {}", tool.key);
        }
        if tool.label.is_empty() {
            anyhow::bail!("custom tool name is required");
        }
        if tool.skills_dir.is_empty() {
            anyhow::bail!("custom tool skills directory is required");
        }
        if let Some(avatar) = &tool.avatar {
            if !avatar.starts_with("data:image/") {
                anyhow::bail!("custom tool avatar must be an image data URL");
            }
            if avatar.len() > 512 * 1024 {
                anyhow::bail!("custom tool avatar is too large");
            }
        }
        custom_tools.push(tool);
    }
    config.custom_tools = custom_tools;

    Ok(config)
}

pub fn default_tool_adapters() -> Vec<ToolAdapter> {
    vec![
        ToolAdapter {
            id: ToolId::Cursor,
            display_name: "Cursor",
            relative_skills_dir: ".cursor/skills",
            relative_detect_dir: ".cursor",
        },
        ToolAdapter {
            id: ToolId::ClaudeCode,
            display_name: "Claude Code",
            relative_skills_dir: ".claude/skills",
            relative_detect_dir: ".claude",
        },
        ToolAdapter {
            id: ToolId::Codex,
            display_name: "Codex",
            relative_skills_dir: ".codex/skills",
            relative_detect_dir: ".codex",
        },
        ToolAdapter {
            id: ToolId::DeepSeekHarness,
            display_name: "DeepSeek Harness",
            // DeepSeek Harness default DSH_HOME is ~/.dsh.
            relative_skills_dir: ".dsh/skills",
            relative_detect_dir: ".dsh",
        },
        ToolAdapter {
            id: ToolId::OpenCode,
            display_name: "OpenCode",
            // add-skill global path: ~/.config/opencode/skills/
            relative_skills_dir: ".config/opencode/skills",
            relative_detect_dir: ".config/opencode",
        },
        ToolAdapter {
            id: ToolId::Antigravity,
            display_name: "Antigravity",
            // Antigravity 2.0 global path: ~/.gemini/config/skills/
            relative_skills_dir: ".gemini/config/skills",
            relative_detect_dir: ".gemini/config",
        },
        ToolAdapter {
            id: ToolId::Amp,
            display_name: "Amp",
            // add-skill global path: ~/.config/agents/skills/
            relative_skills_dir: ".config/agents/skills",
            relative_detect_dir: ".config/agents",
        },
        ToolAdapter {
            id: ToolId::KimiCli,
            display_name: "Kimi Code CLI",
            // add-skill global path: ~/.config/agents/skills/
            // NOTE: Shares the same skills directory with Amp.
            relative_skills_dir: ".config/agents/skills",
            relative_detect_dir: ".config/agents",
        },
        ToolAdapter {
            id: ToolId::Augment,
            display_name: "Augment",
            // add-skill global path: ~/.augment/skills/
            relative_skills_dir: ".augment/skills",
            relative_detect_dir: ".augment",
        },
        ToolAdapter {
            id: ToolId::OpenClaw,
            display_name: "OpenClaw",
            // add-skill global path: ~/.openclaw/skills/
            relative_skills_dir: ".openclaw/skills",
            relative_detect_dir: ".openclaw",
        },
        ToolAdapter {
            id: ToolId::Copaw,
            display_name: "Copaw",
            // add-skill global path: ~/.copaw/skill_pool/
            relative_skills_dir: ".copaw/skill_pool",
            relative_detect_dir: ".copaw",
        },
        ToolAdapter {
            id: ToolId::Cline,
            display_name: "Cline",
            // add-skill global path: ~/.agents/skills/
            relative_skills_dir: ".agents/skills",
            relative_detect_dir: ".agents",
        },
        ToolAdapter {
            id: ToolId::CodeBuddy,
            display_name: "CodeBuddy",
            // add-skill global path: ~/.codebuddy/skills/
            relative_skills_dir: ".codebuddy/skills",
            relative_detect_dir: ".codebuddy",
        },
        ToolAdapter {
            id: ToolId::CodeWhale,
            display_name: "CodeWhale",
            // CodeWhale default global path: ~/.codewhale/skills/
            relative_skills_dir: ".codewhale/skills",
            relative_detect_dir: ".codewhale",
        },
        ToolAdapter {
            id: ToolId::WorkBuddy,
            display_name: "WorkBuddy",
            // add-skill global path: ~/.workbuddy/skills/
            relative_skills_dir: ".workbuddy/skills",
            relative_detect_dir: ".workbuddy",
        },
        ToolAdapter {
            id: ToolId::CommandCode,
            display_name: "Command Code",
            // add-skill global path: ~/.commandcode/skills/
            relative_skills_dir: ".commandcode/skills",
            relative_detect_dir: ".commandcode",
        },
        ToolAdapter {
            id: ToolId::Continue,
            display_name: "Continue",
            // add-skill global path: ~/.continue/skills/
            relative_skills_dir: ".continue/skills",
            relative_detect_dir: ".continue",
        },
        ToolAdapter {
            id: ToolId::Crush,
            display_name: "Crush",
            // add-skill global path: ~/.config/crush/skills/
            relative_skills_dir: ".config/crush/skills",
            relative_detect_dir: ".config/crush",
        },
        ToolAdapter {
            id: ToolId::Junie,
            display_name: "Junie",
            // add-skill global path: ~/.junie/skills/
            relative_skills_dir: ".junie/skills",
            relative_detect_dir: ".junie",
        },
        ToolAdapter {
            id: ToolId::IflowCli,
            display_name: "iFlow CLI",
            // add-skill global path: ~/.iflow/skills/
            relative_skills_dir: ".iflow/skills",
            relative_detect_dir: ".iflow",
        },
        ToolAdapter {
            id: ToolId::KiroCli,
            display_name: "Kiro CLI",
            // add-skill global path: ~/.kiro/skills/
            relative_skills_dir: ".kiro/skills",
            relative_detect_dir: ".kiro",
        },
        ToolAdapter {
            id: ToolId::Kode,
            display_name: "Kode",
            // add-skill global path: ~/.kode/skills/
            relative_skills_dir: ".kode/skills",
            relative_detect_dir: ".kode",
        },
        ToolAdapter {
            id: ToolId::McpJam,
            display_name: "MCPJam",
            // add-skill global path: ~/.mcpjam/skills/
            relative_skills_dir: ".mcpjam/skills",
            relative_detect_dir: ".mcpjam",
        },
        ToolAdapter {
            id: ToolId::MistralVibe,
            display_name: "Mistral Vibe",
            // add-skill global path: ~/.vibe/skills/
            relative_skills_dir: ".vibe/skills",
            relative_detect_dir: ".vibe",
        },
        ToolAdapter {
            id: ToolId::Mux,
            display_name: "Mux",
            // add-skill global path: ~/.mux/skills/
            relative_skills_dir: ".mux/skills",
            relative_detect_dir: ".mux",
        },
        ToolAdapter {
            id: ToolId::OpenClaude,
            display_name: "OpenClaude IDE",
            // add-skill global path: ~/.openclaude/skills/
            relative_skills_dir: ".openclaude/skills",
            relative_detect_dir: ".openclaude",
        },
        ToolAdapter {
            id: ToolId::OpenHands,
            display_name: "OpenHands",
            // add-skill global path: ~/.openhands/skills/
            relative_skills_dir: ".openhands/skills",
            relative_detect_dir: ".openhands",
        },
        ToolAdapter {
            id: ToolId::Pi,
            display_name: "Pi",
            // add-skill global path: ~/.pi/agent/skills/
            relative_skills_dir: ".pi/agent/skills",
            relative_detect_dir: ".pi",
        },
        ToolAdapter {
            id: ToolId::Qoder,
            display_name: "Qoder",
            // add-skill global path: ~/.qoder/skills/
            relative_skills_dir: ".qoder/skills",
            relative_detect_dir: ".qoder",
        },
        ToolAdapter {
            id: ToolId::QoderWork,
            display_name: "QoderWork",
            // add-skill global path: ~/.qoderwork/skills/
            relative_skills_dir: ".qoderwork/skills",
            relative_detect_dir: ".qoderwork",
        },
        ToolAdapter {
            id: ToolId::QwenCode,
            display_name: "Qwen Code",
            // add-skill global path: ~/.qwen/skills/
            relative_skills_dir: ".qwen/skills",
            relative_detect_dir: ".qwen",
        },
        ToolAdapter {
            id: ToolId::Trae,
            display_name: "Trae",
            // add-skill global path: ~/.trae/skills/
            relative_skills_dir: ".trae/skills",
            relative_detect_dir: ".trae",
        },
        ToolAdapter {
            id: ToolId::TraeCn,
            display_name: "Trae CN",
            // add-skill global path: ~/.trae-cn/skills/
            relative_skills_dir: ".trae-cn/skills",
            relative_detect_dir: ".trae-cn",
        },
        ToolAdapter {
            id: ToolId::Zencoder,
            display_name: "Zencoder",
            // add-skill global path: ~/.zencoder/skills/
            relative_skills_dir: ".zencoder/skills",
            relative_detect_dir: ".zencoder",
        },
        ToolAdapter {
            id: ToolId::Neovate,
            display_name: "Neovate",
            // add-skill global path: ~/.neovate/skills/
            relative_skills_dir: ".neovate/skills",
            relative_detect_dir: ".neovate",
        },
        ToolAdapter {
            id: ToolId::Pochi,
            display_name: "Pochi",
            // add-skill global path: ~/.pochi/skills/
            relative_skills_dir: ".pochi/skills",
            relative_detect_dir: ".pochi",
        },
        ToolAdapter {
            id: ToolId::AdaL,
            display_name: "AdaL",
            // add-skill global path: ~/.adal/skills/
            relative_skills_dir: ".adal/skills",
            relative_detect_dir: ".adal",
        },
        ToolAdapter {
            id: ToolId::KiloCode,
            display_name: "Kilo Code",
            // add-skill global path: ~/.kilocode/skills/
            relative_skills_dir: ".kilocode/skills",
            relative_detect_dir: ".kilocode",
        },
        ToolAdapter {
            id: ToolId::RooCode,
            display_name: "Roo Code",
            // add-skill global path: ~/.roo/skills/
            relative_skills_dir: ".roo/skills",
            relative_detect_dir: ".roo",
        },
        ToolAdapter {
            id: ToolId::Goose,
            display_name: "Goose",
            // add-skill global path: ~/.config/goose/skills/
            relative_skills_dir: ".config/goose/skills",
            relative_detect_dir: ".config/goose",
        },
        ToolAdapter {
            id: ToolId::GeminiCli,
            display_name: "Gemini CLI",
            // add-skill global path: ~/.gemini/skills/
            relative_skills_dir: ".gemini/skills",
            relative_detect_dir: ".gemini",
        },
        ToolAdapter {
            id: ToolId::GithubCopilot,
            display_name: "GitHub Copilot",
            // add-skill global path: ~/.copilot/skills/
            relative_skills_dir: ".copilot/skills",
            relative_detect_dir: ".copilot",
        },
        ToolAdapter {
            id: ToolId::Clawdbot,
            display_name: "Clawdbot",
            // add-skill global path: ~/.clawdbot/skills/
            relative_skills_dir: ".clawdbot/skills",
            relative_detect_dir: ".clawdbot",
        },
        ToolAdapter {
            id: ToolId::Droid,
            display_name: "Droid",
            // add-skill global path: ~/.factory/skills/
            relative_skills_dir: ".factory/skills",
            relative_detect_dir: ".factory",
        },
        ToolAdapter {
            id: ToolId::Windsurf,
            display_name: "Windsurf",
            // add-skill global path: ~/.codeium/windsurf/skills/
            relative_skills_dir: ".codeium/windsurf/skills",
            relative_detect_dir: ".codeium/windsurf",
        },
        ToolAdapter {
            id: ToolId::Moltbot,
            display_name: "MoltBot",
            // add-skill global path: ~/.moltbot/skills/
            relative_skills_dir: ".moltbot/skills",
            relative_detect_dir: ".moltbot",
        },
        ToolAdapter {
            id: ToolId::HermesAgent,
            display_name: "Hermes Agent",
            // Hermes stores managed skills under HERMES_HOME/skills; default HERMES_HOME is ~/.hermes.
            relative_skills_dir: ".hermes/skills",
            relative_detect_dir: ".hermes",
        },
    ]
}

/// Tools can share the same global skills directory (e.g. Amp and Kimi Code CLI).
/// Use this to coordinate UI warnings and avoid duplicate filesystem operations.
pub fn adapters_sharing_skills_dir(adapter: &ToolAdapter) -> Vec<ToolAdapter> {
    default_tool_adapters()
        .into_iter()
        .filter(|a| a.relative_skills_dir == adapter.relative_skills_dir)
        .collect()
}

pub fn adapters_sharing_project_skills_dir(adapter: &ToolAdapter) -> Vec<ToolAdapter> {
    let relative = project_relative_skills_dir(adapter);
    default_tool_adapters()
        .into_iter()
        .filter(|a| project_relative_skills_dir(a) == relative)
        .collect()
}

pub fn adapter_by_key(key: &str) -> Option<ToolAdapter> {
    default_tool_adapters()
        .into_iter()
        .find(|adapter| adapter.id.as_key() == key)
}

pub fn resolve_default_path(adapter: &ToolAdapter) -> Result<PathBuf> {
    let home = dirs::home_dir().context("failed to resolve home directory")?;
    Ok(home.join(adapter.relative_skills_dir))
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn resolve_project_path(adapter: &ToolAdapter, project_root: &Path) -> Result<PathBuf> {
    Ok(project_root.join(project_relative_skills_dir(adapter)))
}

pub fn supports_project_scope(adapter: &ToolAdapter) -> bool {
    !matches!(adapter.id, ToolId::HermesAgent | ToolId::WorkBuddy)
}

pub fn project_relative_skills_dir(adapter: &ToolAdapter) -> &'static str {
    match adapter.id {
        ToolId::Amp | ToolId::KimiCli => ".agents/skills",
        ToolId::Antigravity => ".agents/skills",
        ToolId::Augment => ".augment/skills",
        ToolId::ClaudeCode => ".claude/skills",
        ToolId::OpenClaw => "skills",
        ToolId::Cline => ".agents/skills",
        ToolId::CodeBuddy => ".codebuddy/skills",
        ToolId::CodeWhale => ".codewhale/skills",
        ToolId::WorkBuddy => ".workbuddy/skills",
        ToolId::Codex => ".agents/skills",
        ToolId::DeepSeekHarness => ".dsh/skills",
        ToolId::CommandCode => ".commandcode/skills",
        ToolId::Continue => ".continue/skills",
        ToolId::Crush => ".crush/skills",
        ToolId::Cursor => ".agents/skills",
        ToolId::Droid => ".factory/skills",
        ToolId::GeminiCli => ".agents/skills",
        ToolId::GithubCopilot => ".agents/skills",
        ToolId::Goose => ".goose/skills",
        ToolId::Junie => ".junie/skills",
        ToolId::IflowCli => ".iflow/skills",
        ToolId::KiloCode => ".kilocode/skills",
        ToolId::KiroCli => ".kiro/skills",
        ToolId::Kode => ".kode/skills",
        ToolId::McpJam => ".mcpjam/skills",
        ToolId::MistralVibe => ".vibe/skills",
        ToolId::Mux => ".mux/skills",
        ToolId::OpenCode => ".agents/skills",
        ToolId::OpenHands => ".openhands/skills",
        ToolId::Pi => ".pi/skills",
        ToolId::Qoder => ".qoder/skills",
        ToolId::QwenCode => ".qwen/skills",
        ToolId::RooCode => ".roo/skills",
        ToolId::Trae | ToolId::TraeCn => ".trae/skills",
        ToolId::Windsurf => ".windsurf/skills",
        ToolId::Zencoder => ".zencoder/skills",
        ToolId::Neovate => ".neovate/skills",
        ToolId::Pochi => ".pochi/skills",
        ToolId::AdaL => ".adal/skills",
        ToolId::Copaw
        | ToolId::OpenClaude
        | ToolId::QoderWork
        | ToolId::Clawdbot
        | ToolId::Moltbot
        | ToolId::HermesAgent => adapter.relative_skills_dir,
    }
}

pub fn resolve_detect_path(adapter: &ToolAdapter) -> Result<PathBuf> {
    let home = dirs::home_dir().context("failed to resolve home directory")?;
    Ok(home.join(adapter.relative_detect_dir))
}

pub fn is_tool_installed(adapter: &ToolAdapter) -> Result<bool> {
    Ok(resolve_detect_path(adapter)?.exists())
}

pub fn scan_tool_dir(tool: &ToolAdapter, dir: &Path) -> Result<Vec<DetectedSkill>> {
    let mut results = Vec::new();
    if !dir.exists() {
        return Ok(results);
    }

    let ignore_hint = "Application Support/com.tauri.dev/skills";

    for entry in std::fs::read_dir(dir).with_context(|| format!("read dir {:?}", dir))? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        let is_dir = file_type.is_dir() || (file_type.is_symlink() && path.is_dir());
        if !is_dir {
            continue;
        }

        let name = entry.file_name().to_string_lossy().to_string();
        if tool.id == ToolId::Codex && name == ".system" {
            continue;
        }
        let has_skill_file = std::fs::read_dir(&path)
            .ok()
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .any(|child| {
                child
                    .file_name()
                    .to_string_lossy()
                    .eq_ignore_ascii_case("SKILL.md")
                    && child.path().is_file()
            });
        if !has_skill_file {
            continue;
        }
        let (is_link, link_target) = detect_link(&path);
        if path.to_string_lossy().contains(ignore_hint)
            || link_target
                .as_ref()
                .map(|p| p.to_string_lossy().contains(ignore_hint))
                .unwrap_or(false)
        {
            continue;
        }
        results.push(DetectedSkill {
            tool: tool.id.clone(),
            name,
            path,
            is_link,
            link_target,
        });
    }

    Ok(results)
}

fn detect_link(path: &Path) -> (bool, Option<PathBuf>) {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            let target = std::fs::read_link(path).ok();
            (true, target)
        }
        _ => {
            let target = std::fs::read_link(path).ok();
            if target.is_some() {
                (true, target)
            } else {
                (false, None)
            }
        }
    }
}

#[cfg(test)]
#[path = "../tests/tool_adapters.rs"]
mod tests;
