use rusqlite::Connection;
use serde_json::json;
use tempfile::TempDir;

use crate::core::runtime_paths::{RuntimePaths, RuntimeProfile};
use crate::core::skill_store::{SkillRecord, SkillTargetRecord};
use crate::core::sync_engine::SyncMode;
use crate::core::tool_adapters::{
    save_tool_config, CustomToolConfig, ToolConfig, TOOL_CONFIG_SETTING,
};
use crate::services::error::{ErrorCode, ServiceError};
use crate::services::operation_lock::{OperationKind, OperationLockError};
use crate::services::skills_hub::SkillsHubService;
use crate::services::types::SkillSelector;

struct Fixture {
    _home: TempDir,
    _data: TempDir,
    paths: RuntimePaths,
}

impl Fixture {
    fn new() -> Self {
        let home = TempDir::new().unwrap();
        let data = TempDir::new().unwrap();
        let paths = RuntimePaths::from_roots(RuntimeProfile::Test, home.path(), data.path());
        Self {
            _home: home,
            _data: data,
            paths,
        }
    }

    fn open(&self) -> SkillsHubService {
        SkillsHubService::open(self.paths.clone()).unwrap()
    }
}

fn skill(id: &str, name: &str, central_path: &str) -> SkillRecord {
    SkillRecord {
        id: id.to_string(),
        name: name.to_string(),
        description: Some(format!("{name} description")),
        source_type: "git".to_string(),
        source_ref: Some("https://example.com/example.git".to_string()),
        source_subpath: Some("skills/example".to_string()),
        source_revision: Some("main".to_string()),
        central_path: central_path.to_string(),
        content_hash: Some("sha256-content".to_string()),
        created_at: 10,
        updated_at: 20,
        last_sync_at: Some(30),
        last_seen_at: 20,
        enabled: true,
        status: "ok".to_string(),
    }
}

#[test]
fn selector_prefers_an_exact_id_before_case_insensitive_names() {
    let fixture = Fixture::new();
    let service = fixture.open();
    service
        .store()
        .upsert_skill(&skill("react", "Other", "/central/id"))
        .unwrap();
    service
        .store()
        .upsert_skill(&skill("other-id", "react", "/central/name"))
        .unwrap();

    let selected = service
        .show_skill(SkillSelector::Name("react".into()))
        .unwrap();

    assert_eq!(selected.id, "react");
    assert_eq!(selected.name, "Other");
}

#[test]
fn selector_rejects_ambiguous_case_insensitive_names_with_safe_candidates() {
    let fixture = Fixture::new();
    let service = fixture.open();
    service
        .store()
        .upsert_skill(&skill("react-one", "React", "/central/one"))
        .unwrap();
    service
        .store()
        .upsert_skill(&skill("react-two", "react", "/central/two"))
        .unwrap();

    let error = service
        .show_skill(SkillSelector::Name("REACT".into()))
        .unwrap_err();

    assert_eq!(error.code, ErrorCode::AmbiguousSkill);
    assert_eq!(error.details["candidates"].as_array().unwrap().len(), 2);
    assert_eq!(error.details["selector"], json!("REACT"));
}

#[test]
fn list_and_status_include_source_tags_targets_scope_and_content_state() {
    let fixture = Fixture::new();
    let service = fixture.open();
    service
        .store()
        .upsert_skill(&skill("demo", "Demo", "/central/demo"))
        .unwrap();
    let tag = service.store().create_tag("Frontend").unwrap();
    service.store().set_skill_tags("demo", &[tag.id]).unwrap();
    service
        .store()
        .upsert_skill_target(&SkillTargetRecord {
            id: "target-1".to_string(),
            skill_id: "demo".to_string(),
            tool: "codex".to_string(),
            scope: "project".to_string(),
            project_path: Some("/workspace/demo".to_string()),
            target_path: "/workspace/demo/.agents/skills/demo".to_string(),
            mode: "copy".to_string(),
            status: "ok".to_string(),
            last_error: None,
            synced_at: Some(40),
        })
        .unwrap();
    service
        .store()
        .set_setting("github_token", "must-not-leak")
        .unwrap();

    let listed = service.list_skills().unwrap();
    let status = service
        .skill_status(SkillSelector::Id("demo".into()))
        .unwrap();
    let serialized = serde_json::to_string(&listed).unwrap();

    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].source.kind, "git");
    assert_eq!(listed[0].source.subpath.as_deref(), Some("skills/example"));
    assert_eq!(listed[0].tags[0].name, "Frontend");
    assert_eq!(listed[0].targets[0].scope, "project");
    assert_eq!(
        listed[0].targets[0].project_path.as_deref(),
        Some("/workspace/demo")
    );
    assert_eq!(listed[0].content_status, "ok");
    assert_eq!(status.content_status, "ok");
    assert_eq!(status.targets.len(), 1);
    assert!(!serialized.contains("must-not-leak"));
    assert!(!serialized.contains("github_token"));
}

#[test]
fn source_dto_redacts_credentials_and_sensitive_query_values() {
    let fixture = Fixture::new();
    let service = fixture.open();
    let mut record = skill("private", "Private", "/central/private");
    record.source_ref =
        Some("https://user:password@example.com/private.git?token=top-secret&ref=main".to_string());
    service.store().upsert_skill(&record).unwrap();

    let shown = service
        .show_skill(SkillSelector::Id("private".into()))
        .unwrap();
    let serialized = serde_json::to_string(&shown).unwrap();

    assert!(!serialized.contains("user"));
    assert!(!serialized.contains("password"));
    assert!(!serialized.contains("top-secret"));
    assert!(serialized.contains("example.com"));
}

#[test]
fn list_agents_reports_detected_custom_agent_state() {
    let fixture = Fixture::new();
    let service = fixture.open();
    let custom_skills = fixture.paths.app_data_dir.join("custom-agent-skills");
    save_tool_config(
        service.store(),
        ToolConfig {
            disabled_builtin_tools: Vec::new(),
            custom_tools: vec![CustomToolConfig {
                key: "custom_agent".to_string(),
                label: "Custom Agent".to_string(),
                avatar: None,
                skills_dir: custom_skills.to_string_lossy().into_owned(),
                project_skills_dir: Some(".custom/skills".to_string()),
                sync_mode: SyncMode::Copy,
                enabled: true,
            }],
        },
    )
    .unwrap();

    let agents = service.list_agents().unwrap();
    let custom = agents
        .agents
        .iter()
        .find(|agent| agent.key == "custom_agent")
        .unwrap();

    assert!(custom.detected);
    assert!(custom.enabled);
    assert!(custom.supports_project_scope);
    assert!(agents.installed.contains(&"custom_agent".to_string()));
}

#[test]
fn built_in_agent_paths_use_the_runtime_home_root() {
    let fixture = Fixture::new();
    let service = fixture.open();

    let agents = service.list_agents().unwrap();
    let zcode = agents
        .agents
        .iter()
        .find(|agent| agent.key == "zcode")
        .unwrap();

    assert_eq!(
        zcode.skills_dir,
        fixture._home.path().join(".zcode/skills").to_string_lossy()
    );
}

#[test]
fn custom_agent_home_paths_use_the_runtime_home_root() {
    let fixture = Fixture::new();
    let service = fixture.open();
    let custom_skills = fixture._home.path().join(".custom-agent/skills");
    std::fs::create_dir_all(&custom_skills).unwrap();
    let config = ToolConfig {
        disabled_builtin_tools: Vec::new(),
        custom_tools: vec![CustomToolConfig {
            key: "home_custom".to_string(),
            label: "Home Custom".to_string(),
            avatar: None,
            skills_dir: "~/.custom-agent/skills".to_string(),
            project_skills_dir: None,
            sync_mode: SyncMode::Auto,
            enabled: true,
        }],
    };
    service
        .store()
        .set_setting(
            TOOL_CONFIG_SETTING,
            &serde_json::to_string(&config).unwrap(),
        )
        .unwrap();

    let agents = service.list_agents().unwrap();
    let custom = agents
        .agents
        .iter()
        .find(|agent| agent.key == "home_custom")
        .unwrap();

    assert!(custom.detected);
    assert_eq!(custom.skills_dir, custom_skills.to_string_lossy());
}

#[test]
fn future_schema_fails_closed_before_any_business_write() {
    let fixture = Fixture::new();
    std::fs::create_dir_all(&fixture.paths.app_data_dir).unwrap();
    let connection = Connection::open(&fixture.paths.database_path).unwrap();
    connection.pragma_update(None, "user_version", 99).unwrap();
    drop(connection);

    let service = fixture.open();
    let error = service.begin_write(OperationKind::Install).unwrap_err();
    let doctor = service.doctor().unwrap();

    assert_eq!(error.code, ErrorCode::IncompatibleDatabase);
    assert_eq!(error.details["found_version"], json!(99));
    assert_eq!(doctor.database_status, "incompatible");
    let connection = Connection::open(&fixture.paths.database_path).unwrap();
    assert_eq!(
        connection
            .query_row("PRAGMA user_version", [], |row| row.get::<_, i32>(0))
            .unwrap(),
        99
    );
}

#[test]
fn write_guard_rechecks_schema_after_service_open() {
    let fixture = Fixture::new();
    let service = fixture.open();
    let connection = Connection::open(&fixture.paths.database_path).unwrap();
    connection.pragma_update(None, "user_version", 99).unwrap();
    drop(connection);

    let error = service.begin_write(OperationKind::Update).unwrap_err();

    assert_eq!(error.code, ErrorCode::IncompatibleDatabase);
    assert_eq!(error.details["found_version"], json!(99));
}

#[test]
fn operation_busy_maps_to_stable_code_without_sensitive_details() {
    let error = ServiceError::from(OperationLockError::Busy(OperationKind::Update));

    assert_eq!(error.code, ErrorCode::OperationBusy);
    assert_eq!(error.details, json!({ "operation": "update" }));
}

#[test]
fn stable_error_codes_serialize_to_protocol_names() {
    let actual = ErrorCode::ALL
        .iter()
        .map(|code| {
            assert_eq!(serde_json::to_value(code).unwrap(), json!(code.as_str()));
            code.as_str()
        })
        .collect::<Vec<_>>();

    assert_eq!(
        actual,
        vec![
            "INVALID_ARGUMENT",
            "INVALID_SOURCE",
            "SKILL_NOT_FOUND",
            "AMBIGUOUS_SKILL",
            "MULTI_SKILLS",
            "AGENT_NOT_FOUND",
            "PROJECT_SCOPE_UNSUPPORTED",
            "TARGET_CONFLICT",
            "UPDATE_HELD_BACK",
            "CONFIRMATION_REQUIRED",
            "PLAN_STALE",
            "OPERATION_BUSY",
            "INCOMPATIBLE_DATABASE",
            "AUTH_REQUIRED",
            "NETWORK_ERROR",
            "INTERNAL_ERROR",
        ]
    );
}
