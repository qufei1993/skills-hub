use std::fs;

use crate::core::skill_store::{SkillRecord, SkillStore, SkillTargetRecord};
use crate::core::sync_engine::SyncMode;
use crate::core::tool_adapters::{
    adapter_by_key, adapters_sharing_project_skills_dir, adapters_sharing_skills_dir,
    default_tool_adapters, load_tool_config, project_relative_skills_dir,
    resolve_adapter_path_in_home_with_kimi_home, resolve_project_path, save_tool_config,
    scan_tool_dir, supports_project_scope, CustomToolConfig, ToolAdapter, ToolConfig, ToolId,
};

fn make_custom_tool(key: &str, label: &str, skills_dir: &str) -> CustomToolConfig {
    CustomToolConfig {
        key: key.to_string(),
        label: label.to_string(),
        avatar: None,
        skills_dir: skills_dir.to_string(),
        project_skills_dir: Some(".agents/skills".to_string()),
        sync_mode: SyncMode::Copy,
        enabled: true,
    }
}

#[test]
fn saving_changed_custom_tool_migrates_existing_targets_and_preserves_key() {
    let dir = tempfile::tempdir().unwrap();
    let store = SkillStore::new(dir.path().join("test.db"));
    store.ensure_schema().unwrap();

    let central = dir.path().join("central/skill-a");
    fs::create_dir_all(&central).unwrap();
    fs::write(central.join("SKILL.md"), "# Skill A").unwrap();
    let old_root = dir.path().join("old-tools");
    let new_root = dir.path().join("new-tools");

    save_tool_config(
        &store,
        ToolConfig {
            disabled_builtin_tools: Vec::new(),
            custom_tools: vec![make_custom_tool(
                "custom_casey",
                "Casey",
                old_root.to_string_lossy().as_ref(),
            )],
        },
    )
    .unwrap();

    store
        .upsert_skill(&SkillRecord {
            id: "skill-a".to_string(),
            name: "skill-a".to_string(),
            description: None,
            source_type: "local".to_string(),
            source_ref: None,
            source_subpath: None,
            source_revision: None,
            central_path: central.to_string_lossy().to_string(),
            content_hash: None,
            created_at: 1,
            updated_at: 1,
            last_sync_at: None,
            last_seen_at: 1,
            enabled: true,
            status: "ok".to_string(),
        })
        .unwrap();
    let old_target = old_root.join("skill-a");
    fs::create_dir_all(&old_target).unwrap();
    fs::copy(central.join("SKILL.md"), old_target.join("SKILL.md")).unwrap();
    store
        .upsert_skill_target(&SkillTargetRecord {
            id: "target-a".to_string(),
            skill_id: "skill-a".to_string(),
            tool: "custom_casey".to_string(),
            scope: "global".to_string(),
            project_path: None,
            target_path: old_target.to_string_lossy().to_string(),
            mode: "copy".to_string(),
            status: "ok".to_string(),
            last_error: None,
            synced_at: Some(1),
        })
        .unwrap();

    save_tool_config(
        &store,
        ToolConfig {
            disabled_builtin_tools: Vec::new(),
            custom_tools: vec![make_custom_tool(
                "custom_casey",
                "Casey Updated",
                new_root.to_string_lossy().as_ref(),
            )],
        },
    )
    .unwrap();

    let new_target = new_root.join("skill-a");
    assert!(!old_target.exists());
    assert_eq!(
        fs::read_to_string(new_target.join("SKILL.md")).unwrap(),
        "# Skill A"
    );
    let target = store
        .get_skill_target("skill-a", "custom_casey", "global", None)
        .unwrap()
        .unwrap();
    assert_eq!(target.target_path, new_target.to_string_lossy());
    assert_eq!(target.mode, "copy");
    let config = load_tool_config(&store).unwrap();
    assert_eq!(config.custom_tools[0].key, "custom_casey");
    assert_eq!(config.custom_tools[0].label, "Casey Updated");

    #[cfg(unix)]
    {
        let mut edited = make_custom_tool(
            "custom_casey",
            "Casey Updated",
            new_root.to_string_lossy().as_ref(),
        );
        edited.sync_mode = SyncMode::Symlink;
        save_tool_config(
            &store,
            ToolConfig {
                disabled_builtin_tools: Vec::new(),
                custom_tools: vec![edited],
            },
        )
        .unwrap();

        assert!(fs::symlink_metadata(&new_target)
            .unwrap()
            .file_type()
            .is_symlink());
        let target = store
            .get_skill_target("skill-a", "custom_casey", "global", None)
            .unwrap()
            .unwrap();
        assert_eq!(target.mode, "symlink");
    }
}

#[test]
fn changing_custom_tool_directory_to_local_source_returns_safe_error_and_preserves_data() {
    let dir = tempfile::tempdir().unwrap();
    let store = SkillStore::new(dir.path().join("test.db"));
    store.ensure_schema().unwrap();

    let original = dir.path().join("source/skill-a");
    fs::create_dir_all(&original).unwrap();
    fs::write(original.join("source-marker.txt"), "original").unwrap();
    let central = dir.path().join("central/skill-a");
    fs::create_dir_all(&central).unwrap();
    fs::write(central.join("SKILL.md"), "# Skill A").unwrap();
    let old_root = dir.path().join("old-tools");

    save_tool_config(
        &store,
        ToolConfig {
            disabled_builtin_tools: Vec::new(),
            custom_tools: vec![make_custom_tool(
                "custom_casey",
                "Casey",
                old_root.to_string_lossy().as_ref(),
            )],
        },
    )
    .unwrap();

    store
        .upsert_skill(&SkillRecord {
            id: "skill-a".to_string(),
            name: "skill-a".to_string(),
            description: None,
            source_type: "local".to_string(),
            source_ref: Some(original.to_string_lossy().to_string()),
            source_subpath: None,
            source_revision: None,
            central_path: central.to_string_lossy().to_string(),
            content_hash: None,
            created_at: 1,
            updated_at: 1,
            last_sync_at: None,
            last_seen_at: 1,
            enabled: true,
            status: "ok".to_string(),
        })
        .unwrap();
    let old_target = old_root.join("skill-a");
    fs::create_dir_all(&old_target).unwrap();
    fs::write(old_target.join("SKILL.md"), "# Skill A").unwrap();
    store
        .upsert_skill_target(&SkillTargetRecord {
            id: "target-a".to_string(),
            skill_id: "skill-a".to_string(),
            tool: "custom_casey".to_string(),
            scope: "global".to_string(),
            project_path: None,
            target_path: old_target.to_string_lossy().to_string(),
            mode: "copy".to_string(),
            status: "ok".to_string(),
            last_error: None,
            synced_at: Some(1),
        })
        .unwrap();

    let err = save_tool_config(
        &store,
        ToolConfig {
            disabled_builtin_tools: Vec::new(),
            custom_tools: vec![make_custom_tool(
                "custom_casey",
                "Casey",
                original.parent().unwrap().to_string_lossy().as_ref(),
            )],
        },
    )
    .unwrap_err();

    assert!(err.to_string().starts_with("SKILL_TARGET_OVERLAPS_SOURCE|"));
    assert_eq!(
        fs::read_to_string(original.join("source-marker.txt")).unwrap(),
        "original"
    );
    assert!(old_target.join("SKILL.md").is_file());
    let config = load_tool_config(&store).unwrap();
    assert_eq!(
        config.custom_tools[0].skills_dir,
        old_root.to_string_lossy()
    );
}

#[test]
fn legacy_custom_tool_config_defaults_avatar_and_sync_mode() {
    let config: ToolConfig = serde_json::from_str(
        r#"{
          "disabled_builtin_tools": [],
          "custom_tools": [{
            "key": "custom_legacy",
            "label": "Legacy",
            "skills_dir": "~/.legacy/skills",
            "project_skills_dir": null,
            "enabled": true
          }]
        }"#,
    )
    .unwrap();

    let tool = &config.custom_tools[0];
    assert_eq!(tool.avatar, None);
    assert_eq!(tool.sync_mode, SyncMode::Auto);
}

#[test]
fn adapter_by_key_finds_known_tool() {
    let a = adapter_by_key("codex").unwrap();
    assert_eq!(a.id, ToolId::Codex);
}

#[test]
fn builtin_tool_count_matches_v090_documentation() {
    assert_eq!(default_tool_adapters().len(), 47);
}

#[test]
fn adapter_by_key_finds_new_tools() {
    assert!(adapter_by_key("kimi_cli").is_some());
    assert!(adapter_by_key("augment").is_some());
    assert!(adapter_by_key("openclaw").is_some());
    assert!(adapter_by_key("command_code").is_some());
    assert!(adapter_by_key("qwen_code").is_some());
    assert!(adapter_by_key("deepseek_harness").is_some());
    assert!(adapter_by_key("hermes_agent").is_some());
    assert!(adapter_by_key("workbuddy").is_some());
}

#[test]
fn deepseek_harness_adapter_uses_native_skill_dirs() {
    let deepseek_harness = adapter_by_key("deepseek_harness").unwrap();

    assert_eq!(deepseek_harness.id, ToolId::DeepSeekHarness);
    assert_eq!(deepseek_harness.relative_skills_dir, ".dsh/skills");
    assert_eq!(deepseek_harness.relative_detect_dir, ".dsh");
    assert_eq!(
        project_relative_skills_dir(&deepseek_harness),
        ".dsh/skills"
    );
    assert!(supports_project_scope(&deepseek_harness));
}

#[test]
fn deepseek_harness_scan_discovers_directory_skill() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("release-check")).unwrap();
    fs::write(
        dir.path().join("release-check/SKILL.md"),
        "---\nname: release-check\ndescription: Release check\n---\n",
    )
    .unwrap();

    let deepseek_harness = adapter_by_key("deepseek_harness").unwrap();
    let detected = scan_tool_dir(&deepseek_harness, dir.path()).unwrap();

    assert_eq!(detected.len(), 1);
    assert_eq!(detected[0].tool, ToolId::DeepSeekHarness);
    assert_eq!(detected[0].name, "release-check");
    assert_eq!(detected[0].path, dir.path().join("release-check"));
}

#[test]
fn codewhale_adapter_uses_tool_specific_skill_dirs() {
    let codewhale = adapter_by_key("codewhale").unwrap();

    assert_eq!(codewhale.id, ToolId::CodeWhale);
    assert_eq!(codewhale.relative_skills_dir, ".codewhale/skills");
    assert_eq!(codewhale.relative_detect_dir, ".codewhale");
    assert_eq!(project_relative_skills_dir(&codewhale), ".codewhale/skills");
    assert!(supports_project_scope(&codewhale));
}

#[test]
fn antigravity_adapter_uses_current_global_skill_dir() {
    let antigravity = adapter_by_key("antigravity").unwrap();

    assert_eq!(antigravity.id, ToolId::Antigravity);
    assert_eq!(antigravity.relative_skills_dir, ".gemini/config/skills");
    assert_eq!(antigravity.relative_detect_dir, ".gemini/config");
    assert_eq!(project_relative_skills_dir(&antigravity), ".agents/skills");
    assert!(supports_project_scope(&antigravity));
}

#[test]
fn kimi_adapter_uses_kimi_code_directories() {
    let kimi = adapter_by_key("kimi_cli").unwrap();

    assert_eq!(kimi.relative_skills_dir, ".kimi-code/skills");
    assert_eq!(kimi.relative_detect_dir, ".kimi-code");
    assert_eq!(project_relative_skills_dir(&kimi), ".kimi-code/skills");
}

#[test]
fn kimi_adapter_honors_kimi_code_home() {
    let home = tempfile::tempdir().unwrap();
    let kimi_home = tempfile::tempdir().unwrap();
    let kimi = adapter_by_key("kimi_cli").unwrap();
    assert_eq!(
        resolve_adapter_path_in_home_with_kimi_home(
            &kimi,
            home.path(),
            kimi.relative_skills_dir,
            "skills",
            Some(kimi_home.path()),
        ),
        kimi_home.path().join("skills")
    );
    assert_eq!(
        resolve_adapter_path_in_home_with_kimi_home(
            &kimi,
            home.path(),
            kimi.relative_detect_dir,
            "",
            Some(kimi_home.path()),
        ),
        kimi_home.path()
    );
}

#[test]
fn adapters_sharing_skills_dir_keeps_amp_separate_from_kimi() {
    let amp = adapter_by_key("amp").unwrap();
    let group = adapters_sharing_skills_dir(&amp);
    let keys: std::collections::HashSet<&'static str> =
        group.into_iter().map(|a| a.id.as_key()).collect();
    assert!(keys.contains("amp"));
    assert!(!keys.contains("kimi_cli"));
}

#[test]
fn project_relative_skills_dir_maps_supported_agents() {
    let shared_agents = [
        ("cursor", ".agents/skills"),
        ("codex", ".agents/skills"),
        ("opencode", ".agents/skills"),
        ("gemini_cli", ".agents/skills"),
        ("github_copilot", ".agents/skills"),
        ("amp", ".agents/skills"),
        ("antigravity", ".agents/skills"),
        ("cline", ".agents/skills"),
    ];

    for (key, expected) in shared_agents {
        let adapter = adapter_by_key(key).unwrap();
        assert_eq!(project_relative_skills_dir(&adapter), expected, "{key}");
        assert!(supports_project_scope(&adapter), "{key}");
    }

    let kimi = adapter_by_key("kimi_cli").unwrap();
    assert_eq!(project_relative_skills_dir(&kimi), ".kimi-code/skills");

    let claude = adapter_by_key("claude_code").unwrap();
    assert_eq!(project_relative_skills_dir(&claude), ".claude/skills");

    let openclaw = adapter_by_key("openclaw").unwrap();
    assert_eq!(project_relative_skills_dir(&openclaw), "skills");

    let windsurf = adapter_by_key("windsurf").unwrap();
    assert_eq!(project_relative_skills_dir(&windsurf), ".windsurf/skills");

    let qwen = adapter_by_key("qwen_code").unwrap();
    assert_eq!(project_relative_skills_dir(&qwen), ".qwen/skills");

    let hermes = adapter_by_key("hermes_agent").unwrap();
    assert_eq!(project_relative_skills_dir(&hermes), ".hermes/skills");
    assert!(!supports_project_scope(&hermes));

    let workbuddy = adapter_by_key("workbuddy").unwrap();
    assert_eq!(project_relative_skills_dir(&workbuddy), ".workbuddy/skills");
    assert!(!supports_project_scope(&workbuddy));
}

#[test]
fn project_path_resolution_uses_project_specific_mapping() {
    let dir = tempfile::tempdir().unwrap();
    let amp = adapter_by_key("amp").unwrap();
    let opencode = adapter_by_key("opencode").unwrap();
    let openclaw = adapter_by_key("openclaw").unwrap();

    assert_eq!(
        resolve_project_path(&amp, dir.path()).unwrap(),
        dir.path().join(".agents/skills")
    );
    assert_eq!(
        resolve_project_path(&opencode, dir.path()).unwrap(),
        dir.path().join(".agents/skills")
    );
    assert_eq!(
        resolve_project_path(&openclaw, dir.path()).unwrap(),
        dir.path().join("skills")
    );
}

#[test]
fn adapters_sharing_project_skills_dir_groups_agents_tools() {
    let cursor = adapter_by_key("cursor").unwrap();
    let group = adapters_sharing_project_skills_dir(&cursor);
    let keys: std::collections::HashSet<&'static str> =
        group.into_iter().map(|a| a.id.as_key()).collect();

    assert!(keys.contains("cursor"));
    assert!(keys.contains("codex"));
    assert!(keys.contains("opencode"));
    assert!(keys.contains("gemini_cli"));
    assert!(keys.contains("github_copilot"));
    assert!(keys.contains("amp"));
    assert!(!keys.contains("kimi_cli"));
    assert!(keys.contains("antigravity"));
    assert!(keys.contains("cline"));
    assert!(!keys.contains("claude_code"));
    assert!(!keys.contains("windsurf"));
}

#[test]
fn scan_tool_dir_skips_codex_system_and_includes_symlink_dir() {
    let dir = tempfile::tempdir().unwrap();

    fs::create_dir_all(dir.path().join("a")).unwrap();
    fs::write(dir.path().join("a/SKILL.md"), "# Skill A").unwrap();
    fs::create_dir_all(dir.path().join(".system")).unwrap();
    fs::write(dir.path().join(".system/SKILL.md"), "# System Skill").unwrap();
    fs::write(dir.path().join("not-a-dir"), b"x").unwrap();

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(dir.path().join("a"), dir.path().join("link-a")).unwrap();
    }

    let tool = ToolAdapter {
        id: ToolId::Codex,
        display_name: "Codex",
        relative_skills_dir: "ignored",
        relative_detect_dir: "ignored",
    };

    let out = scan_tool_dir(&tool, dir.path()).unwrap();
    let names: Vec<String> = out.iter().map(|s| s.name.clone()).collect();

    assert!(names.contains(&"a".to_string()));
    assert!(!names.contains(&".system".to_string()));

    #[cfg(unix)]
    {
        let link = out.iter().find(|s| s.name == "link-a").unwrap();
        assert!(link.is_link);
        assert!(link.link_target.is_some());
    }
}

#[test]
fn scan_tool_dir_skips_app_support_path() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir
        .path()
        .join("Library/Application Support/com.tauri.dev/skills");
    std::fs::create_dir_all(root.join("foo")).unwrap();
    std::fs::write(root.join("foo/SKILL.md"), "# Ignored Skill").unwrap();

    let tool = ToolAdapter {
        id: ToolId::Cursor,
        display_name: "Cursor",
        relative_skills_dir: "ignored",
        relative_detect_dir: "ignored",
    };

    let out = scan_tool_dir(&tool, &root).unwrap();
    assert!(out.is_empty());
}
