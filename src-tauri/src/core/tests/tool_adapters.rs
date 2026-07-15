use std::fs;

use crate::core::tool_adapters::{
    adapter_by_key, adapters_sharing_project_skills_dir, adapters_sharing_skills_dir,
    project_relative_skills_dir, resolve_project_path, scan_tool_dir, supports_project_scope,
    ToolAdapter, ToolId,
};

#[test]
fn adapter_by_key_finds_known_tool() {
    let a = adapter_by_key("codex").unwrap();
    assert_eq!(a.id, ToolId::Codex);
}

#[test]
fn adapter_by_key_finds_new_tools() {
    assert!(adapter_by_key("kimi_cli").is_some());
    assert!(adapter_by_key("augment").is_some());
    assert!(adapter_by_key("openclaw").is_some());
    assert!(adapter_by_key("command_code").is_some());
    assert!(adapter_by_key("qwen_code").is_some());
    assert!(adapter_by_key("hermes_agent").is_some());
    assert!(adapter_by_key("workbuddy").is_some());
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
fn adapters_sharing_skills_dir_groups_amp_and_kimi() {
    let amp = adapter_by_key("amp").unwrap();
    let group = adapters_sharing_skills_dir(&amp);
    let keys: std::collections::HashSet<&'static str> =
        group.into_iter().map(|a| a.id.as_key()).collect();
    assert!(keys.contains("amp"));
    assert!(keys.contains("kimi_cli"));
}

#[test]
fn project_relative_skills_dir_maps_supported_agents() {
    let shared_agents = [
        ("cursor", ".agents/skills"),
        ("codex", ".agents/skills"),
        ("opencode", ".agents/skills"),
        ("github_copilot", ".agents/skills"),
        ("amp", ".agents/skills"),
        ("kimi_cli", ".agents/skills"),
        ("antigravity", ".agents/skills"),
        ("cline", ".agents/skills"),
    ];

    for (key, expected) in shared_agents {
        let adapter = adapter_by_key(key).unwrap();
        assert_eq!(project_relative_skills_dir(&adapter), expected, "{key}");
        assert!(supports_project_scope(&adapter), "{key}");
    }

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
    assert!(keys.contains("github_copilot"));
    assert!(keys.contains("amp"));
    assert!(keys.contains("kimi_cli"));
    assert!(keys.contains("antigravity"));
    assert!(keys.contains("cline"));
    assert!(!keys.contains("claude_code"));
    assert!(!keys.contains("windsurf"));
}

#[test]
fn scan_tool_dir_skips_codex_system_and_includes_symlink_dir() {
    let dir = tempfile::tempdir().unwrap();

    fs::create_dir_all(dir.path().join("a")).unwrap();
    fs::create_dir_all(dir.path().join(".system")).unwrap();
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

    let tool = ToolAdapter {
        id: ToolId::Cursor,
        display_name: "Cursor",
        relative_skills_dir: "ignored",
        relative_detect_dir: "ignored",
    };

    let out = scan_tool_dir(&tool, &root).unwrap();
    assert!(out.is_empty());
}
