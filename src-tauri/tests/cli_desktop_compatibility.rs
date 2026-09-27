#![cfg(debug_assertions)]

use app_lib::core::runtime_paths::{RuntimePaths, RuntimeProfile};
use app_lib::services::skills_hub::SkillsHubService;
use rusqlite::{Connection, OpenFlags};
use serde_json::Value;
use std::{fs, process::Command};
use tempfile::TempDir;

struct Fixture {
    root: TempDir,
    paths: RuntimePaths,
}

impl Fixture {
    fn stable() -> Self {
        let root = TempDir::new().unwrap();
        let paths = RuntimePaths::from_roots(
            RuntimeProfile::Test,
            root.path().join("home"),
            root.path().join("data"),
        );
        fs::create_dir_all(&paths.app_data_dir).unwrap();
        fs::create_dir_all(root.path().join("home/.codex")).unwrap();
        Connection::open(&paths.database_path)
            .unwrap()
            .execute_batch(include_str!("fixtures/v0.10.1-schema.sql"))
            .unwrap();
        Self { root, paths }
    }

    fn cli(&self, args: &[&str], expected_exit: i32) -> Value {
        let output = Command::new(env!("CARGO_BIN_EXE_skillshub-cli"))
            .env("SKILLSHUB_CLI_TEST_ROOT", self.root.path())
            .arg("--json")
            .args(args)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(expected_exit), "{output:?}");
        let bytes = if expected_exit == 0 {
            assert!(output.stderr.is_empty(), "{output:?}");
            output.stdout
        } else {
            assert!(output.stdout.is_empty(), "{output:?}");
            output.stderr
        };
        serde_json::from_slice(&bytes).unwrap()
    }
}

#[test]
fn stable_schema_survives_cli_install_tag_deploy_and_desktop_reopen() {
    let fixture = Fixture::stable();
    let source = fixture.root.path().join("compat-skill");
    fs::create_dir(&source).unwrap();
    fs::write(
        source.join("SKILL.md"),
        "---\nname: compat-skill\ndescription: Compatibility fixture\n---\nBody\n",
    )
    .unwrap();
    fixture.cli(&["skills", "install", source.to_str().unwrap()], 0);
    fixture.cli(
        &["skills", "tag", "add", "compat-skill", "compatibility"],
        0,
    );
    fixture.cli(&["skills", "deploy", "compat-skill", "--agent", "codex"], 0);

    // Reopen through the desktop's shared service, with exactly the CLI's RuntimePaths.
    let desktop = SkillsHubService::open(fixture.paths.clone()).unwrap();
    let skill = desktop.show_skill("compat-skill".into()).unwrap();
    assert_eq!(skill.source.kind, "local");
    assert_eq!(skill.tags[0].name, "compatibility");
    assert_eq!(skill.targets[0].scope, "global");
    assert_eq!(skill.targets[0].tool, "codex");
    assert_eq!(skill.targets[0].status, "ok");

    // Previous-stable-compatible reader: only v0.10.1 columns, no new service or migration.
    let stable = Connection::open_with_flags(
        &fixture.paths.database_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap();
    let version: i32 = stable
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, 6, "v0.10.1 must still accept the shared schema");
    let row: (String, String, String, String, String, String, String, Option<String>, String) = stable
        .query_row(
            "SELECT s.name,s.source_type,s.source_ref,s.description,t.name,d.tool,d.scope,d.project_path,d.target_path
             FROM skills s JOIN skill_tag_links l ON l.skill_id=s.id
             JOIN skill_tags t ON t.id=l.tag_id JOIN skill_targets d ON d.skill_id=s.id
             WHERE s.name='compat-skill' AND s.enabled=1",
            [],
            |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?,r.get(8)?)),
        )
        .unwrap();
    assert_eq!(row.0, "compat-skill");
    assert_eq!(row.1, "local");
    assert_eq!(
        std::path::Path::new(&row.2).canonicalize().unwrap(),
        source.canonicalize().unwrap()
    );
    assert_eq!(row.3, "Compatibility fixture");
    assert_eq!(row.4, "compatibility");
    assert_eq!(row.5, "codex");
    assert_eq!(row.6, "global");
    assert_eq!(row.7, None);
    assert_eq!(row.8, skill.targets[0].target_path);
    assert_eq!(
        fs::read_to_string(std::path::Path::new(&row.8).join("SKILL.md")).unwrap(),
        fs::read_to_string(source.join("SKILL.md")).unwrap()
    );
}

#[test]
fn unknown_newer_schema_rejects_cli_write_without_database_or_filesystem_mutation() {
    let fixture = Fixture::stable();
    let connection = Connection::open(&fixture.paths.database_path).unwrap();
    connection.pragma_update(None, "user_version", 99).unwrap();
    drop(connection);
    let before = fs::read(&fixture.paths.database_path).unwrap();
    let result = fixture.cli(&["skills", "tag", "add", "missing", "blocked"], 6);
    assert_eq!(result["code"], "INCOMPATIBLE_DATABASE");
    assert_eq!(fs::read(&fixture.paths.database_path).unwrap(), before);
    assert!(!fixture.paths.default_central_repo.exists());
    assert!(!fixture.root.path().join("home/.codex/skills").exists());
}
