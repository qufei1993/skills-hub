#![cfg(debug_assertions)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use app_lib::core::runtime_paths::{RuntimePaths, RuntimeProfile};
use app_lib::core::skill_store::SkillStore;
use app_lib::services::install::InstallRequest;
use app_lib::services::operation_lock::{OperationKind, OperationLock};
use app_lib::services::skills_hub::SkillsHubService;
use serde_json::Value;
use tempfile::TempDir;

struct Fixture {
    root: TempDir,
    paths: RuntimePaths,
}

impl Fixture {
    fn new() -> Self {
        let root = TempDir::new().unwrap();
        let paths = RuntimePaths::from_roots(
            RuntimeProfile::Test,
            root.path().join("home"),
            root.path().join("data"),
        );
        fs::create_dir_all(root.path().join("home/.codex")).unwrap();
        Self { root, paths }
    }

    fn service(&self) -> SkillsHubService {
        SkillsHubService::open(self.paths.clone()).unwrap()
    }

    fn store(&self) -> SkillStore {
        self.service();
        SkillStore::new(self.paths.database_path.clone())
    }

    fn source(&self, name: &str) -> PathBuf {
        let path = self.root.path().join("sources").join(name);
        write_skill(&path, name);
        path
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_skillshub-cli"))
            .env("SKILLSHUB_CLI_TEST_ROOT", self.root.path())
            .env("LANG", "en_US.UTF-8")
            .env_remove("LC_ALL")
            .env_remove("LC_MESSAGES")
            .args(args)
            .output()
            .unwrap()
    }

    fn json(&self, args: &[&str], code: Option<(&str, i32)>) -> Value {
        let mut arguments = vec!["--json"];
        arguments.extend_from_slice(args);
        let output = self.run(&arguments);
        let (payload, expected_exit) = if let Some((_, exit)) = code {
            assert!(output.stdout.is_empty(), "unexpected stdout: {:?}", output);
            (&output.stderr, exit)
        } else {
            assert!(output.stderr.is_empty(), "unexpected stderr: {:?}", output);
            (&output.stdout, 0)
        };
        assert_eq!(output.status.code(), Some(expected_exit), "{:?}", output);
        let value: Value = serde_json::from_slice(payload).unwrap();
        assert_eq!(value["ok"], code.is_none());
        let command_length = match args.first().copied() {
            Some("skills") if args.get(1) == Some(&"tag") => 3,
            Some("skills" | "agents") => 2,
            _ => 1,
        };
        assert_eq!(value["command"], args[..command_length].join("."));
        if let Some((code, _)) = code {
            assert_eq!(value.as_object().unwrap().len(), 5);
            assert_eq!(value["code"], code);
            assert!(value["message"].is_string());
            assert!(value["details"].is_object());
        } else {
            assert_eq!(value.as_object().unwrap().len(), 3);
            assert!(value.get("data").is_some());
        }
        value
    }

    fn install(&self, name: &str) -> Value {
        let source = self.source(name);
        self.json(&["skills", "install", source.to_str().unwrap()], None)["data"].clone()
    }

    fn copy_target(&self, skill: &app_lib::core::skill_store::SkillRecord) -> PathBuf {
        let target = self.root.path().join("copies").join(&skill.name);
        app_lib::core::sync_engine::copy_dir_recursive(Path::new(&skill.central_path), &target)
            .unwrap();
        let baseline = app_lib::core::content_hash::hash_dir_for_sync_conflict(&target).unwrap();
        let record = app_lib::core::skill_store::SkillTargetRecord {
            id: format!("copy-{}", skill.id),
            skill_id: skill.id.clone(),
            tool: "custom-copy".into(),
            scope: "global".into(),
            project_path: None,
            target_path: target.to_string_lossy().into_owned(),
            mode: "copy".into(),
            status: "ok".into(),
            last_error: None,
            synced_at: Some(1),
        };
        let store = self.store();
        store.upsert_skill_target(&record).unwrap();
        store
            .set_setting(
                &format!("device_sync.target_baseline.{}", record.id),
                &serde_json::to_string(&(record.target_path, baseline)).unwrap(),
            )
            .unwrap();
        target
    }
}

fn write_skill(path: &Path, name: &str) {
    fs::create_dir_all(path).unwrap();
    fs::write(
        path.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: Integration test\n---\nBody\n"),
    )
    .unwrap();
}

#[test]
fn cli_install_tag_deploy_reopens_the_same_desktop_service_state() {
    let fixture = Fixture::new();
    let installed = fixture.install("demo");
    assert_eq!(installed["targets"], serde_json::json!([]));
    assert!(!fixture.root.path().join("home/.codex/skills/demo").exists());
    fixture.json(&["skills", "tag", "add", "demo", "frontend"], None);
    let plan = fixture.json(
        &["skills", "deploy", "demo", "--agent", "codex", "--dry-run"],
        None,
    );
    assert_eq!(plan["data"]["targets"].as_array().unwrap().len(), 1);
    assert!(fixture
        .service()
        .show_skill("demo".into())
        .unwrap()
        .targets
        .is_empty());
    fixture.json(&["skills", "deploy", "demo", "--agent", "codex"], None);
    let shown = fixture.service().show_skill("demo".into()).unwrap();
    assert_eq!(shown.tags[0].name, "frontend");
    assert_eq!(shown.targets[0].tool, "codex");
    assert_eq!(shown.targets[0].scope, "global");
    assert_eq!(shown.source.kind, "local");
    assert!(shown.content_hash.is_some());
    assert!(Path::new(&shown.targets[0].target_path)
        .join("SKILL.md")
        .is_file());
    assert_eq!(
        fixture.json(&["skills", "show", &shown.id], None)["data"]["id"],
        shown.id
    );
    assert_eq!(
        fixture.json(&["skills", "status", "demo"], None)["data"]["targets"][0]["tool"],
        "codex"
    );
    assert_eq!(
        fixture.json(
            &["skills", "list", "--agent", "codex", "--status", "ok"],
            None
        )["data"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert!(
        fixture.json(&["skills", "list", "--agent", "cursor"], None)["data"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    fixture.json(
        &[
            "skills",
            "undeploy",
            "demo",
            "--agent",
            "codex",
            "--dry-run",
        ],
        None,
    );
    assert_eq!(
        fixture
            .service()
            .show_skill("demo".into())
            .unwrap()
            .targets
            .len(),
        1
    );
    fixture.json(&["skills", "undeploy", "demo", "--agent", "codex"], None);
    assert!(fixture
        .service()
        .show_skill("demo".into())
        .unwrap()
        .targets
        .is_empty());
    assert!(Path::new(&shown.central_path).is_dir());
}

#[test]
fn preview_and_confirmation_protect_remove_adopt_and_tag_delete() {
    let fixture = Fixture::new();
    fixture.install("demo");
    fixture.json(&["skills", "tag", "add", "demo", "frontend"], None);
    for arguments in [
        vec!["skills", "remove", "demo"],
        vec!["skills", "tag", "delete", "frontend"],
    ] {
        let error = fixture.json(&arguments, Some(("CONFIRMATION_REQUIRED", 4)));
        assert!(error["details"]["plan"].is_object());
        let mut preview = arguments.clone();
        preview.push("--dry-run");
        fixture.json(&preview, None);
    }
    assert_eq!(
        fixture
            .service()
            .show_skill("demo".into())
            .unwrap()
            .tags
            .len(),
        1
    );
    fixture.json(&["skills", "tag", "delete", "frontend", "--yes"], None);
    assert!(fixture
        .service()
        .show_skill("demo".into())
        .unwrap()
        .tags
        .is_empty());
    let removed = fixture.json(&["skills", "remove", "demo", "--yes"], None);
    assert!(
        Path::new(removed["data"]["item"]["trash_path"].as_str().unwrap())
            .join("SKILL.md")
            .is_file()
    );
    fixture.json(&["skills", "show", "demo"], Some(("SKILL_NOT_FOUND", 3)));

    let source = fixture.root.path().join("external");
    write_skill(&source.join("adopted"), "adopted");
    let args = ["skills", "adopt", source.to_str().unwrap()];
    let error = fixture.json(&args, Some(("CONFIRMATION_REQUIRED", 4)));
    assert_eq!(
        error["details"]["plan"]["candidates"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    fixture.json(&[args[0], args[1], args[2], "--dry-run"], None);
    assert!(fixture.service().list_skills().unwrap().is_empty());
    fixture.json(&[args[0], args[1], args[2], "--yes"], None);
    assert_eq!(
        fixture
            .service()
            .show_skill("adopted".into())
            .unwrap()
            .source
            .kind,
        "local"
    );
}

#[test]
fn multi_skill_conflict_and_scope_errors_have_stable_protocols() {
    let fixture = Fixture::new();
    fixture.source("alpha");
    fixture.source("beta");
    let container = fixture.root.path().join("sources");
    let error = fixture.json(
        &["skills", "install", container.to_str().unwrap()],
        Some(("MULTI_SKILLS", 3)),
    );
    assert_eq!(error["details"]["candidates"].as_array().unwrap().len(), 2);
    fixture.json(
        &[
            "skills",
            "install",
            container.to_str().unwrap(),
            "--subpath",
            "alpha",
        ],
        None,
    );
    let target = fixture.root.path().join("home/.codex/skills/alpha");
    write_skill(&target, "unmanaged");
    fixture.json(
        &["skills", "deploy", "alpha", "--agent", "codex"],
        Some(("TARGET_CONFLICT", 4)),
    );
    assert!(fs::read_to_string(target.join("SKILL.md"))
        .unwrap()
        .contains("unmanaged"));
    fixture.json(
        &["skills", "deploy", "alpha"],
        Some(("INVALID_ARGUMENT", 2)),
    );
    fixture.json(
        &["skills", "deploy", "alpha", "--agent", "unknown"],
        Some(("AGENT_NOT_FOUND", 3)),
    );
    let agents = fixture.service().list_agents().unwrap();
    let unsupported = agents
        .agents
        .iter()
        .find(|agent| !agent.supports_project_scope)
        .unwrap();
    fixture.json(
        &[
            "skills",
            "deploy",
            "alpha",
            "--agent",
            &unsupported.key,
            "--project",
            fixture.root.path().to_str().unwrap(),
        ],
        Some(("PROJECT_SCOPE_UNSUPPORTED", 4)),
    );
}

#[test]
fn ambiguous_names_require_ids_and_redact_stored_secrets() {
    let fixture = Fixture::new();
    let installed = fixture.install("demo");
    let store = fixture.store();
    let mut record = store
        .get_skill_by_id(installed["id"].as_str().unwrap())
        .unwrap()
        .unwrap();
    record.description = Some("Authorization: Bearer demo-secret-value".into());
    record.source_ref =
        Some("https://user:secret-password@example.com/repo?token=query-secret".into());
    store.upsert_skill(&record).unwrap();
    record.id = "duplicate-id".into();
    record.central_path = fixture
        .paths
        .default_central_repo
        .join("duplicate")
        .to_string_lossy()
        .into_owned();
    store.upsert_skill(&record).unwrap();
    fixture.json(&["skills", "show", "demo"], Some(("AMBIGUOUS_SKILL", 3)));
    let result = fixture.json(&["skills", "show", "duplicate-id"], None);
    let text = result.to_string();
    for secret in ["demo-secret-value", "secret-password", "query-secret"] {
        assert!(!text.contains(secret), "leaked {secret}");
    }
}

#[test]
fn writes_respect_desktop_lock_and_unknown_schema_while_diagnostics_work() {
    let fixture = Fixture::new();
    fixture.install("demo");
    let lock = OperationLock::acquire(&fixture.paths, OperationKind::DeviceSync).unwrap();
    fixture.json(
        &["skills", "tag", "add", "demo", "blocked"],
        Some(("OPERATION_BUSY", 5)),
    );
    drop(lock);
    fixture.json(&["skills", "tag", "add", "demo", "allowed"], None);
    rusqlite::Connection::open(&fixture.paths.database_path)
        .unwrap()
        .execute_batch("PRAGMA user_version=99;")
        .unwrap();
    fixture.json(
        &["skills", "tag", "add", "demo", "blocked"],
        Some(("INCOMPATIBLE_DATABASE", 6)),
    );
    assert_eq!(
        fixture.json(&["doctor"], None)["data"]["database_status"],
        "incompatible"
    );
    assert!(fixture.json(&["version"], None)["data"]["version"].is_string());
}

#[test]
fn destructive_update_is_held_back_and_safe_update_reopens() {
    let fixture = Fixture::new();
    let installed = fixture.install("demo");
    let central = Path::new(installed["central_path"].as_str().unwrap());
    fs::write(central.join("user-file.txt"), "keep").unwrap();
    fixture.json(&["skills", "update", "demo"], Some(("UPDATE_HELD_BACK", 4)));
    assert!(central.join("user-file.txt").exists());
    assert_eq!(
        fixture.json(&["skills", "check", "--all"], None)["data"][0]["held_back"],
        true
    );
    fs::remove_file(central.join("user-file.txt")).unwrap();
    let source = fixture.root.path().join("sources/demo/SKILL.md");
    fs::write(
        &source,
        "---\nname: demo\ndescription: Updated\n---\nUpdated body\n",
    )
    .unwrap();
    fixture.json(&["skills", "update", "--all"], None);
    assert!(fs::read_to_string(central.join("SKILL.md"))
        .unwrap()
        .contains("Updated body"));
}

#[test]
fn desktop_installs_are_visible_to_cli_filters_and_tag_workflows() {
    let fixture = Fixture::new();
    fixture
        .service()
        .install(InstallRequest::local(fixture.source("desktop")))
        .unwrap();
    fixture.install("other");
    fixture.json(
        &["skills", "tag", "set", "desktop", "First", "Second"],
        None,
    );
    fixture.json(&["skills", "tag", "remove", "desktop", "second"], None);
    fixture.json(&["skills", "tag", "rename", "first", "Renamed"], None);
    let tags = fixture.json(&["skills", "tag", "list", "desktop"], None);
    assert_eq!(tags["data"][0]["name"], "Renamed");
    let all_tags = fixture.json(&["skills", "tag", "list"], None);
    assert!(all_tags["data"]
        .as_array()
        .unwrap()
        .iter()
        .any(|tag| tag["name"] == "Renamed"));
    assert_eq!(
        fixture.json(
            &["skills", "list", "--tag", "renamed", "--source", "local"],
            None
        )["data"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        fixture.json(&["skills", "list", "--untagged"], None)["data"][0]["name"],
        "other"
    );
    fixture.json(&["agents", "list"], None);
    fixture.json(&["skills", "check", "desktop"], None);
    fixture.json(&["skills", "search", " "], Some(("INVALID_ARGUMENT", 2)));
}

#[test]
fn human_output_includes_results_and_localized_help_without_stream_noise() {
    let fixture = Fixture::new();
    fixture.install("demo");
    for (language, phrase) in [
        ("en", "Command completed"),
        ("zh-CN", "命令执行成功"),
        ("ko", "명령이 완료"),
    ] {
        let output = fixture.run(&["--lang", language, "skills", "show", "demo"]);
        assert!(output.status.success());
        assert!(output.stderr.is_empty());
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(!text.contains('\u{1b}'));
        assert!(text.contains(phrase));
        assert!(text.contains("demo"));
        let help = fixture.run(&["--lang", language, "skills", "--help"]);
        assert!(help.status.success());
        assert!(help.stderr.is_empty());
    }
}

#[test]
fn project_deployment_uses_the_requested_project_without_global_side_effects() {
    let fixture = Fixture::new();
    fixture.install("demo");
    let project = fixture.root.path().join("project");
    fs::create_dir(&project).unwrap();
    fixture.json(
        &[
            "skills",
            "deploy",
            "demo",
            "--agent",
            "codex",
            "--project",
            project.to_str().unwrap(),
        ],
        None,
    );
    let skill = fixture.service().show_skill("demo".into()).unwrap();
    assert_eq!(skill.targets[0].scope, "project");
    assert!(Path::new(&skill.targets[0].target_path).starts_with(project.canonicalize().unwrap()));
    assert!(!fixture.root.path().join("home/.codex/skills/demo").exists());
    fixture.json(
        &[
            "skills",
            "undeploy",
            "demo",
            "--agent",
            "codex",
            "--project",
            project.to_str().unwrap(),
        ],
        None,
    );
    assert!(fixture
        .service()
        .show_skill("demo".into())
        .unwrap()
        .targets
        .is_empty());
}

#[test]
fn batch_update_checks_every_skill_before_changing_any_skill() {
    let fixture = Fixture::new();
    let safe = fixture.install("safe");
    let held = fixture.install("held");
    let safe_central = Path::new(safe["central_path"].as_str().unwrap()).join("SKILL.md");
    let original = fs::read(&safe_central).unwrap();
    let before_skills = fixture.store().list_skills().unwrap();
    let targets = before_skills
        .iter()
        .map(|skill| fixture.copy_target(skill))
        .collect::<Vec<_>>();
    let before_targets = targets
        .iter()
        .map(|path| app_lib::core::content_hash::hash_dir_strict(path).unwrap())
        .collect::<Vec<_>>();
    fs::write(
        fixture.root.path().join("sources/safe/new.txt"),
        "new content",
    )
    .unwrap();
    fs::write(
        Path::new(held["central_path"].as_str().unwrap()).join("personal.txt"),
        "keep",
    )
    .unwrap();
    let before = fixture.service().list_skills().unwrap();
    fixture.json(
        &["skills", "update", "--all"],
        Some(("UPDATE_HELD_BACK", 4)),
    );
    assert_eq!(fs::read(&safe_central).unwrap(), original);
    assert!(!safe_central.parent().unwrap().join("new.txt").exists());
    assert_eq!(fixture.store().list_skills().unwrap(), before_skills);
    assert_eq!(fixture.service().list_skills().unwrap(), before);
    for (path, hash) in targets.iter().zip(before_targets) {
        assert_eq!(
            app_lib::core::content_hash::hash_dir_strict(path).unwrap(),
            hash
        );
    }
    assert_eq!(
        fs::read(Path::new(held["central_path"].as_str().unwrap()).join("personal.txt")).unwrap(),
        b"keep"
    );
}

#[test]
fn tag_filter_uses_the_same_unicode_case_folding_as_tag_mutations() {
    let fixture = Fixture::new();
    fixture.install("demo");
    fixture.json(&["skills", "tag", "add", "demo", "Äpfel"], None);
    let filtered = fixture.json(&["skills", "list", "--tag", "äpfel"], None);
    assert_eq!(filtered["data"].as_array().unwrap().len(), 1);
}

#[test]
fn batch_update_rejects_a_modified_copy_before_any_skill_or_metadata_changes() {
    let fixture = Fixture::new();
    fixture.install("first");
    fixture.install("second");
    let store = fixture.store();
    let mut skills = store.list_skills().unwrap();
    // Match the actual update traversal so a safe candidate precedes the conflict.
    let first = skills.remove(0);
    let second = skills.remove(0);
    for skill in [&first, &second] {
        fixture.copy_target(skill);
        fs::write(
            fixture
                .root
                .path()
                .join("sources")
                .join(&skill.name)
                .join("SKILL.md"),
            format!(
                "---\nname: {}\ndescription: Updated\n---\nUpdated content\n",
                skill.name
            ),
        )
        .unwrap();
    }
    let conflict_path = fixture.root.path().join("copies").join(&second.name);
    fs::write(
        conflict_path.join("SKILL.md"),
        "Personal edit, same filename",
    )
    .unwrap();
    let before_skills = store.list_skills().unwrap();
    let before_targets =
        [&first, &second].map(|skill| store.list_skill_targets(&skill.id).unwrap());
    let before_files = [&first, &second].map(|skill| {
        (
            fs::read(Path::new(&skill.central_path).join("SKILL.md")).unwrap(),
            fs::read(
                fixture
                    .root
                    .path()
                    .join("copies")
                    .join(&skill.name)
                    .join("SKILL.md"),
            )
            .unwrap(),
        )
    });
    let before_checks = serde_json::to_value(fixture.service().list_skills().unwrap()).unwrap();
    let result = fixture.json(&["skills", "update", "--all"], Some(("TARGET_CONFLICT", 4)));
    assert_eq!(result["details"]["skill_id"], second.id);
    assert_eq!(result["details"]["agent"], "custom-copy");
    assert_eq!(
        result["details"]["path"],
        conflict_path.to_string_lossy().as_ref()
    );
    assert_eq!(result["details"]["reason"], "modified_target");
    let reopened = fixture.store();
    assert_eq!(reopened.list_skills().unwrap(), before_skills);
    assert_eq!(
        serde_json::to_value(fixture.service().list_skills().unwrap()).unwrap(),
        before_checks
    );
    for (index, skill) in [&first, &second].iter().enumerate() {
        assert_eq!(
            reopened.list_skill_targets(&skill.id).unwrap(),
            before_targets[index]
        );
        assert_eq!(
            fs::read(Path::new(&skill.central_path).join("SKILL.md")).unwrap(),
            before_files[index].0
        );
        assert_eq!(
            fs::read(
                fixture
                    .root
                    .path()
                    .join("copies")
                    .join(&skill.name)
                    .join("SKILL.md")
            )
            .unwrap(),
            before_files[index].1
        );
    }
}

#[test]
fn batch_update_accepts_a_saved_copy_baseline_when_central_content_has_changed() {
    let fixture = Fixture::new();
    fixture.install("demo");
    let skill = fixture.store().list_skills().unwrap().remove(0);
    let target = fixture.copy_target(&skill);
    fs::write(
        Path::new(&skill.central_path).join("SKILL.md"),
        "Central changed since deployment",
    )
    .unwrap();
    fs::write(
        fixture.root.path().join("sources/demo/SKILL.md"),
        "---\nname: demo\ndescription: New\n---\nNew source\n",
    )
    .unwrap();
    let result = fixture.json(&["skills", "update", "--all"], None);
    assert_eq!(result["data"][0]["pending_targets"], serde_json::json!([]));
    assert!(fs::read_to_string(target.join("SKILL.md"))
        .unwrap()
        .contains("New source"));
}

#[cfg(unix)]
#[test]
fn batch_update_rejects_a_redirected_managed_link_before_mutation() {
    let fixture = Fixture::new();
    fixture.install("demo");
    fixture.json(&["skills", "deploy", "demo", "--agent", "codex"], None);
    let skill = fixture.service().show_skill("demo".into()).unwrap();
    let target = Path::new(&skill.targets[0].target_path);
    let unrelated = fixture.root.path().join("unrelated");
    write_skill(&unrelated, "unrelated");
    fs::remove_file(target).unwrap();
    std::os::unix::fs::symlink(&unrelated, target).unwrap();
    fs::write(
        fixture.root.path().join("sources/demo/new.txt"),
        "New source content",
    )
    .unwrap();
    let before = fixture.service().list_skills().unwrap();
    fixture.json(&["skills", "update", "--all"], Some(("TARGET_CONFLICT", 4)));
    assert_eq!(fixture.service().list_skills().unwrap(), before);
    assert!(!Path::new(&skill.central_path).join("new.txt").exists());
    assert!(!unrelated.join("new.txt").exists());
    assert_eq!(fs::read_link(target).unwrap(), unrelated);
}

#[test]
fn search_uses_saved_proxy_and_returns_network_error_on_stderr() {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    let fixture = Fixture::new();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    app_lib::core::network_proxy::set_github_proxy_url(
        &fixture.store(),
        &format!("http://{}", listener.local_addr().unwrap()),
    )
    .unwrap();
    listener.set_nonblocking(true).unwrap();
    let server = std::thread::spawn(move || {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    stream
                        .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                        .unwrap();
                    let mut request = [0_u8; 2048];
                    let count = stream.read(&mut request).unwrap();
                    stream.write_all(b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
                    return String::from_utf8_lossy(&request[..count]).into_owned();
                }
                Err(error)
                    if error.kind() == std::io::ErrorKind::WouldBlock
                        && std::time::Instant::now() < deadline =>
                {
                    std::thread::sleep(std::time::Duration::from_millis(10))
                }
                Err(error) => panic!("proxy was not used: {error}"),
            }
        }
    });
    fixture.json(
        &["skills", "search", "demo", "--limit", "3"],
        Some(("NETWORK_ERROR", 7)),
    );
    assert!(server.join().unwrap().starts_with("CONNECT skills.sh:443"));
}

#[test]
fn argument_and_service_errors_do_not_echo_secret_input() {
    let fixture = Fixture::new();
    for (args, expected) in [
        (
            vec!["skills", "install", "bare-name"],
            ("INVALID_SOURCE", 2),
        ),
        (
            vec!["skills", "show", "Authorization: Bearer super-secret"],
            ("SKILL_NOT_FOUND", 3),
        ),
        (
            vec!["skills", "deploy", "demo", "--token", "super-secret"],
            ("INVALID_ARGUMENT", 2),
        ),
        (
            vec![
                "skills",
                "install",
                "https://user:super-secret@example.com/repo.git",
            ],
            ("INVALID_SOURCE", 2),
        ),
    ] {
        let value = fixture.json(&args, Some(expected));
        assert!(!value.to_string().contains("super-secret"));
    }
    fixture.install("demo");
    let store = fixture.store();
    let mut record = store.list_skills().unwrap().remove(0);
    record.description = Some("Authorization: Bearer super-secret".into());
    store.upsert_skill(&record).unwrap();
    let output = fixture.run(&["skills", "show", "demo"]);
    assert!(output.status.success());
    assert!(!String::from_utf8(output.stdout)
        .unwrap()
        .contains("super-secret"));
}
