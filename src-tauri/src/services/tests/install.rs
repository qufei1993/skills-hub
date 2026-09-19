use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;

use rusqlite::Connection;
use serde_json::json;
use tempfile::TempDir;

use crate::core::cancel_token::CancelToken;
use crate::core::network_proxy::set_github_proxy_url;
use crate::core::runtime_paths::{RuntimePaths, RuntimeProfile};
use crate::core::skill_store::SkillTargetRecord;
use crate::services::error::ErrorCode;
use crate::services::install::InstallRequest;
use crate::services::operation_lock::{OperationKind, OperationLock};
use crate::services::skills_hub::SkillsHubService;

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

fn write_skill(dir: &Path, name: &str) {
    fs::create_dir_all(dir).unwrap();
    fs::write(
        dir.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: Test skill\n---\n\nBody\n"),
    )
    .unwrap();
}

fn init_git_repo(dir: &Path) {
    let repo = git2::Repository::init(dir).unwrap();
    let signature = git2::Signature::now("Skills Hub Test", "test@example.com").unwrap();
    let mut index = repo.index().unwrap();
    index
        .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
        .unwrap();
    let tree_id = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();
    repo.commit(Some("HEAD"), &signature, &signature, "initial", &tree, &[])
        .unwrap();
}

#[test]
fn local_install_is_visible_after_reopen_and_never_deploys() {
    let fixture = Fixture::new();
    let source = fixture.paths.app_data_dir.join("source-skill");
    write_skill(&source, "local-demo");
    let service = fixture.open();

    let installed = service.install(InstallRequest::local(&source)).unwrap();

    assert!(installed.targets.is_empty());
    let reopened = fixture.open();
    let shown = reopened.show_skill(installed.id.into()).unwrap();
    assert_eq!(shown.source.kind, "local");
    assert!(shown.targets.is_empty());
}

#[test]
fn source_parser_is_deterministic_and_does_not_guess_bare_local_names() {
    let local = InstallRequest::parse("./relative-skill").unwrap();
    let prefixed_local = InstallRequest::parse("local:relative-skill").unwrap();
    let git = InstallRequest::parse("git@example.com:owner/repo.git").unwrap();
    let marketplace = InstallRequest::parse("owner/repo").unwrap();
    let error = InstallRequest::parse("relative-skill").unwrap_err();

    assert!(local.source.is_local());
    assert!(prefixed_local.source.is_local());
    assert!(git.source.is_git());
    assert!(marketplace.source.is_git());
    assert_eq!(error.code, ErrorCode::InvalidSource);
}

#[test]
fn marketplace_shorthand_requires_a_complete_safe_ref_and_path() {
    for valid in [
        "owner/repo",
        "owner/repo.git",
        "owner/repo/tree/main/skills/demo",
        "owner/repo/blob/main/skills/demo/SKILL.md",
    ] {
        assert!(
            InstallRequest::parse(valid).is_ok(),
            "expected valid shorthand: {valid}"
        );
    }

    for invalid in [
        "owner/repo/tree",
        "owner/repo/tree/",
        "owner/repo/tree/main",
        "owner/repo/tree/main/",
        "owner/repo/tree//skills/demo",
        "owner/repo/tree/main/../demo",
        "owner/repo/blob/main/skills/demo?raw=1",
        "owner/repo/tree/main/skill name",
        "owner/..",
    ] {
        let error = InstallRequest::parse(invalid).unwrap_err();
        assert_eq!(
            error.code,
            ErrorCode::InvalidSource,
            "expected invalid shorthand: {invalid}"
        );
    }
}

#[test]
fn install_checks_database_compatibility_before_writing() {
    let fixture = Fixture::new();
    let source = fixture.paths.app_data_dir.join("future-source");
    write_skill(&source, "future-demo");
    let service = fixture.open();
    Connection::open(&fixture.paths.database_path)
        .unwrap()
        .execute_batch("PRAGMA user_version = 99;")
        .unwrap();

    let error = service.install(InstallRequest::local(&source)).unwrap_err();

    assert_eq!(error.code, ErrorCode::IncompatibleDatabase);
    assert!(!fixture
        .paths
        .default_central_repo
        .join("future-demo")
        .exists());
}

#[test]
fn install_uses_the_shared_operation_lock() {
    let fixture = Fixture::new();
    let source = fixture.paths.app_data_dir.join("busy-source");
    write_skill(&source, "busy-demo");
    let service = fixture.open();
    let _guard = OperationLock::acquire(&fixture.paths, OperationKind::Update).unwrap();

    let error = service.install(InstallRequest::local(&source)).unwrap_err();

    assert_eq!(error.code, ErrorCode::OperationBusy);
    assert_eq!(error.details, json!({ "operation": "install" }));
    assert!(!fixture
        .paths
        .default_central_repo
        .join("busy-demo")
        .exists());
}

#[test]
fn rejected_git_credentials_do_not_cross_the_service_error_boundary() {
    let fixture = Fixture::new();
    let service = fixture.open();
    let secret = "do-not-leak";

    let error = service
        .install(InstallRequest::git(format!(
            "https://user:{secret}@example.com/repo.git?token={secret}"
        )))
        .unwrap_err();

    assert_eq!(error.code, ErrorCode::InvalidSource);
    assert!(!serde_json::to_string(&error).unwrap().contains(secret));
}

#[test]
fn git_subpath_install_persists_the_selected_source_path() {
    let fixture = Fixture::new();
    let repo = fixture.paths.app_data_dir.join("git-source");
    write_skill(&repo.join("skills/alpha"), "alpha");
    write_skill(&repo.join("skills/beta"), "beta");
    init_git_repo(&repo);
    let repo_url = format!("file://{}", repo.display());
    let service = fixture.open();

    let installed = service
        .install(InstallRequest::git(repo_url).with_subpath("skills/beta"))
        .unwrap();
    let shown = service.show_skill(installed.id.into()).unwrap();

    assert_eq!(shown.name, "beta");
    assert_eq!(shown.source.kind, "git");
    assert_eq!(shown.source.subpath.as_deref(), Some("skills/beta"));
    assert!(shown.targets.is_empty());
}

#[test]
fn cancelled_git_candidate_discovery_does_not_install_the_single_candidate() {
    let fixture = Fixture::new();
    let repo = fixture.paths.app_data_dir.join("cancel-candidate-source");
    write_skill(&repo, "cancel-candidate");
    init_git_repo(&repo);
    let service = fixture.open();
    let cancel = CancelToken::new();
    cancel.cancel();

    let error = service
        .install_with_cancel(
            InstallRequest::git(format!("file://{}", repo.display())),
            Some(&cancel),
        )
        .unwrap_err();

    assert_eq!(error.details["legacy_category"], "cancelled");
    assert!(!fixture
        .paths
        .default_central_repo
        .join("cancel-candidate")
        .exists());
}

#[test]
fn cancelled_git_selection_stops_before_installing_the_selected_skill() {
    let fixture = Fixture::new();
    let repo = fixture.paths.app_data_dir.join("cancel-selection-source");
    write_skill(&repo.join("skills/selected"), "cancel-selection");
    init_git_repo(&repo);
    let service = fixture.open();
    let cancel = CancelToken::new();
    cancel.cancel();

    let error = service
        .install_with_cancel(
            InstallRequest::git(format!("file://{}", repo.display()))
                .with_subpath("skills/selected"),
            Some(&cancel),
        )
        .unwrap_err();

    assert_eq!(error.details["legacy_category"], "cancelled");
    assert!(!fixture
        .paths
        .default_central_repo
        .join("cancel-selection")
        .exists());
}

#[test]
fn install_rejects_a_subpath_that_escapes_the_declared_source() {
    let fixture = Fixture::new();
    let base = fixture.paths.app_data_dir.join("declared-source");
    let outside = fixture.paths.app_data_dir.join("outside-skill");
    fs::create_dir_all(&base).unwrap();
    write_skill(&outside, "outside");
    let service = fixture.open();

    let error = service
        .install(InstallRequest::local(&base).with_subpath("../outside-skill"))
        .unwrap_err();

    assert_eq!(error.code, ErrorCode::InvalidSource);
    assert!(!fixture.paths.default_central_repo.join("outside").exists());
}

#[test]
fn install_rejects_a_skill_name_that_escapes_the_central_library() {
    let fixture = Fixture::new();
    let source = fixture.paths.app_data_dir.join("malicious-name-source");
    write_skill(&source, "../escaped");
    let service = fixture.open();

    let error = service.install(InstallRequest::local(&source)).unwrap_err();

    assert_eq!(error.code, ErrorCode::InvalidArgument);
    assert!(!fixture
        .paths
        .default_central_repo
        .parent()
        .unwrap()
        .join("escaped")
        .exists());
}

#[test]
fn multi_skill_git_source_returns_safe_structured_candidates() {
    let fixture = Fixture::new();
    let repo = fixture.paths.app_data_dir.join("multi-source");
    write_skill(&repo.join("skills/alpha"), "alpha");
    write_skill(&repo.join("skills/beta"), "beta");
    init_git_repo(&repo);
    let service = fixture.open();

    let error = service
        .install(InstallRequest::git(format!("file://{}", repo.display())))
        .unwrap_err();

    assert_eq!(error.code, ErrorCode::MultiSkills);
    assert_eq!(
        error.details,
        json!({
            "candidates": [
                { "name": "alpha", "description": "Test skill", "subpath": "skills/alpha" },
                { "name": "beta", "description": "Test skill", "subpath": "skills/beta" }
            ]
        })
    );
    let serialized = serde_json::to_string(&error).unwrap();
    assert!(!serialized.contains(repo.to_string_lossy().as_ref()));
}

#[test]
fn multi_skill_local_source_keeps_the_complete_desktop_candidate_shape() {
    let fixture = Fixture::new();
    let source = fixture.paths.app_data_dir.join("multi-local-source");
    write_skill(&source.join("skills/alpha"), "alpha");
    write_skill(&source.join("skills/beta"), "beta");
    let service = fixture.open();

    let error = service.install(InstallRequest::local(&source)).unwrap_err();

    assert_eq!(error.code, ErrorCode::MultiSkills);
    assert_eq!(
        error.details,
        json!({
            "candidates": [
                {
                    "name": "alpha",
                    "description": "Test skill",
                    "subpath": "skills/alpha",
                    "valid": true,
                    "reason": null
                },
                {
                    "name": "beta",
                    "description": "Test skill",
                    "subpath": "skills/beta",
                    "valid": true,
                    "reason": null
                }
            ]
        })
    );
}

#[test]
fn duplicate_install_carries_only_the_safe_local_target_path() {
    let fixture = Fixture::new();
    let source = fixture.paths.app_data_dir.join("duplicate-source");
    write_skill(&source, "duplicate-demo");
    let service = fixture.open();
    service.install(InstallRequest::local(&source)).unwrap();

    let error = service.install(InstallRequest::local(&source)).unwrap_err();

    assert_eq!(error.code, ErrorCode::TargetConflict);
    assert_eq!(
        error.details,
        json!({
            "legacy_category": "skill_exists",
            "path": fixture.paths.default_central_repo.join("duplicate-demo"),
        })
    );
    assert!(!serde_json::to_string(&error)
        .unwrap()
        .contains(source.to_string_lossy().as_ref()));
}

#[test]
fn search_uses_the_saved_application_proxy() {
    let fixture = Fixture::new();
    let service = fixture.open();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let proxy_url = format!("http://{}", listener.local_addr().unwrap());
    set_github_proxy_url(service.store(), &proxy_url).unwrap();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = [0_u8; 2048];
        let count = stream.read(&mut request).unwrap();
        stream
            .write_all(
                b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            )
            .unwrap();
        String::from_utf8_lossy(&request[..count]).into_owned()
    });

    let error = service.search("proxy-boundary", 3).unwrap_err();
    let request = server.join().unwrap();

    assert_eq!(error.code, ErrorCode::NetworkError);
    assert!(request.starts_with("CONNECT skills.sh:443"));
    assert!(!serde_json::to_string(&error).unwrap().contains(&proxy_url));
}

#[test]
fn destructive_update_is_held_back_without_changing_the_library() {
    let fixture = Fixture::new();
    let source = fixture.paths.app_data_dir.join("update-source");
    write_skill(&source, "update-demo");
    fs::write(source.join("keep-me.txt"), "original").unwrap();
    let service = fixture.open();
    let installed = service.install(InstallRequest::local(&source)).unwrap();
    fs::remove_file(source.join("keep-me.txt")).unwrap();

    let check = service.check_updates(installed.id.clone().into()).unwrap();
    let error = service.update(installed.id.clone().into()).unwrap_err();

    assert!(check.update_available);
    assert!(check.held_back);
    assert_eq!(check.removal_count, 1);
    assert_eq!(error.code, ErrorCode::UpdateHeldBack);
    assert!(Path::new(&installed.central_path)
        .join("keep-me.txt")
        .exists());
    let unchanged = service.show_skill(installed.id.into()).unwrap();
    assert_eq!(unchanged.content_status, "ok");
    assert!(unchanged.source_error.is_none());
}

#[test]
fn update_is_held_back_when_a_deployed_copy_contains_a_removed_file() {
    let fixture = Fixture::new();
    let source = fixture.paths.app_data_dir.join("copy-source");
    write_skill(&source, "copy-demo");
    let service = fixture.open();
    let installed = service.install(InstallRequest::local(&source)).unwrap();
    let original_skill_md =
        fs::read_to_string(Path::new(&installed.central_path).join("SKILL.md")).unwrap();
    let target = fixture.paths.app_data_dir.join("deployed-copy");
    write_skill(&target, "copy-demo");
    fs::write(target.join("user-extra.txt"), "keep").unwrap();
    service
        .store()
        .upsert_skill_target(&SkillTargetRecord {
            id: "copy-target".to_string(),
            skill_id: installed.id.clone(),
            tool: "codex".to_string(),
            scope: "global".to_string(),
            project_path: None,
            target_path: target.to_string_lossy().into_owned(),
            mode: "copy".to_string(),
            status: "ok".to_string(),
            last_error: None,
            synced_at: None,
        })
        .unwrap();
    fs::write(
        source.join("SKILL.md"),
        "---\nname: copy-demo\ndescription: Updated\n---\n\nChanged\n",
    )
    .unwrap();

    let check = service.check_updates(installed.id.clone().into()).unwrap();
    let error = service.update(installed.id.clone().into()).unwrap_err();

    assert!(check.held_back);
    assert_eq!(check.removal_count, 1);
    assert_eq!(error.code, ErrorCode::UpdateHeldBack);
    assert_eq!(
        fs::read_to_string(Path::new(&installed.central_path).join("SKILL.md")).unwrap(),
        original_skill_md
    );
    assert!(target.join("user-extra.txt").exists());
}

#[test]
fn non_destructive_update_returns_the_shared_outcome() {
    let fixture = Fixture::new();
    let source = fixture.paths.app_data_dir.join("safe-update-source");
    write_skill(&source, "safe-update");
    let service = fixture.open();
    let installed = service.install(InstallRequest::local(&source)).unwrap();
    fs::write(source.join("new-file.txt"), "new").unwrap();

    let check = service.check_updates(installed.id.clone().into()).unwrap();
    let updated = service.update(installed.id.clone().into()).unwrap();

    assert!(check.update_available);
    assert!(!check.held_back);
    assert_eq!(updated.id, installed.id);
    assert!(updated.changed);
    assert!(Path::new(&installed.central_path)
        .join("new-file.txt")
        .exists());
}
