use std::{fs, path::PathBuf};
use tempfile::TempDir;

use crate::core::runtime_paths::{RuntimePaths, RuntimeProfile};
use crate::services::agent_access::SetupAgentRequest;
use crate::services::error::ErrorCode;
use crate::services::operation_lock::{OperationKind, OperationLock};
use crate::services::skills_hub::SkillsHubService;

struct Fixture {
    home: TempDir,
    _data: TempDir,
    service: SkillsHubService,
}

impl Fixture {
    fn new() -> Self {
        let home = TempDir::new().unwrap();
        let data = TempDir::new().unwrap();
        fs::create_dir_all(home.path().join(".codex")).unwrap();
        fs::create_dir_all(home.path().join(".cursor")).unwrap();
        let paths = RuntimePaths::from_roots(RuntimeProfile::Test, home.path(), data.path());
        Self {
            service: SkillsHubService::open(paths).unwrap(),
            home,
            _data: data,
        }
    }

    fn target(&self, agent: &str) -> PathBuf {
        self.home.path().join(format!(".{agent}/skills/skills-hub"))
    }

    fn old_bundle(&self) {
        let bundled = self
            .service
            .prepare_bundled_install(
                "skills-hub",
                "---\nname: skills-hub\ndescription: Previous official skill\n---\nOld release\n",
                "0.0.1",
            )
            .unwrap();
        self.service
            .apply_bundled_install(bundled, || Ok(()))
            .unwrap();
        self.service
            .deploy(crate::services::deployment::DeploymentRequest::global(
                "skills-hub",
                ["cursor"],
            ))
            .unwrap();
    }
}

#[test]
fn agent_access_new_bundle_never_reclassifies_a_modified_target_as_trusted() {
    let f = Fixture::new();
    f.old_bundle();
    fs::write(
        f.target("cursor").join("SKILL.md"),
        super::super::agent_access::OFFICIAL_SKILL_MD,
    )
    .unwrap();
    let before = f.service.show_skill("skills-hub".into()).unwrap();
    let error = f
        .service
        .setup_agent_access(SetupAgentRequest::install("cursor"))
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::TargetConflict);
    assert_eq!(f.service.show_skill("skills-hub".into()).unwrap(), before);
}

#[test]
fn agent_access_upgrades_intact_library_without_redeploying_other_agents() {
    let f = Fixture::new();
    f.old_bundle();
    let old_copy = fs::read(f.target("cursor").join("SKILL.md")).unwrap();
    f.service
        .setup_agent_access(SetupAgentRequest::install("codex"))
        .unwrap();
    assert_eq!(
        fs::read(f.target("cursor").join("SKILL.md")).unwrap(),
        old_copy
    );
    assert_eq!(
        fs::read_to_string(f.target("codex").join("SKILL.md")).unwrap(),
        super::super::agent_access::OFFICIAL_SKILL_MD
    );
}

#[test]
fn agent_access_official_bundle_works_with_normal_check_and_update_workflows() {
    let f = Fixture::new();
    f.old_bundle();
    let check = f.service.check_updates("skills-hub".into()).unwrap();
    assert!(check.update_available);
    let updated = f.service.update("skills-hub".into()).unwrap();
    assert!(updated.changed);
    assert_eq!(
        fs::read_to_string(f.target("cursor").join("SKILL.md")).unwrap(),
        super::super::agent_access::OFFICIAL_SKILL_MD
    );
    assert!(
        !f.service
            .check_updates("skills-hub".into())
            .unwrap()
            .update_available
    );
    assert!(!f.service.update("skills-hub".into()).unwrap().changed);
}

#[test]
fn agent_access_bundled_copy_is_excluded_from_onboarding_discovery() {
    let f = Fixture::new();
    f.service
        .setup_agent_access(SetupAgentRequest::install("cursor"))
        .unwrap();
    let plan = crate::core::onboarding::build_onboarding_plan_for_runtime(
        f.service.paths(),
        f.service.store(),
        f.home.path(),
    )
    .unwrap();
    assert_eq!(plan.total_skills_found, 0);
    assert!(plan.groups.is_empty());
}

#[test]
fn agent_access_failed_deployment_commit_rolls_back_new_library_and_all_agents() {
    let f = Fixture::new();
    rusqlite::Connection::open(&f.service.paths().database_path).unwrap().execute_batch(
        "CREATE TRIGGER fail_setup_targets BEFORE INSERT ON skill_targets BEGIN SELECT RAISE(FAIL, 'fixture failure'); END;"
    ).unwrap();
    let mut request = SetupAgentRequest::install("codex");
    request.agents.push("cursor".into());
    let error = f.service.setup_agent_access(request).unwrap_err();
    assert_eq!(error.code, ErrorCode::InternalError);
    assert_eq!(error.details["bundled_rollback"]["files_restored"], true);
    assert_eq!(error.details["bundled_rollback"]["database_restored"], true);
    assert!(f.service.list_skills().unwrap().is_empty());
    assert!(!f
        .service
        .paths()
        .default_central_repo
        .join("skills-hub")
        .exists());
    assert!(!f.target("codex").exists());
    assert!(!f.target("cursor").exists());
}

#[test]
fn agent_access_failed_upgrade_restores_old_bundle_and_deployment() {
    let f = Fixture::new();
    f.old_bundle();
    let before = f.service.show_skill("skills-hub".into()).unwrap();
    let old_content = fs::read(f.target("cursor").join("SKILL.md")).unwrap();
    rusqlite::Connection::open(&f.service.paths().database_path).unwrap().execute_batch(
        "CREATE TRIGGER fail_setup_targets BEFORE INSERT ON skill_targets BEGIN SELECT RAISE(FAIL, 'fixture failure'); END;"
    ).unwrap();
    let error = f
        .service
        .setup_agent_access(SetupAgentRequest::install("cursor"))
        .unwrap_err();
    assert_eq!(error.details["bundled_rollback"]["files_restored"], true);
    assert_eq!(
        fs::read(PathBuf::from(&before.central_path).join("SKILL.md")).unwrap(),
        old_content
    );
    assert_eq!(
        fs::read(f.target("cursor").join("SKILL.md")).unwrap(),
        old_content
    );
    assert_eq!(
        f.service
            .show_skill("skills-hub".into())
            .unwrap()
            .content_hash,
        before.content_hash
    );
}

fn assert_bundle_rollback_recovery(upgrade: bool, database_failure: bool) {
    use crate::core::sync_engine::{set_deployment_race_hook, DeploymentRacePoint};
    let f = Fixture::new();
    if upgrade {
        f.old_bundle();
    }
    let central = f.service.paths().default_central_repo.join("skills-hub");
    let old_content = upgrade.then(|| fs::read(central.join("SKILL.md")).unwrap());
    if database_failure {
        rusqlite::Connection::open(&f.service.paths().database_path).unwrap().execute_batch(
            "CREATE TRIGGER fail_bundle BEFORE INSERT ON skills BEGIN SELECT RAISE(FAIL, 'raw-db-error-do-not-leak'); END;"
        ).unwrap();
    }
    let modified = central.clone();
    set_deployment_race_hook(
        if database_failure {
            DeploymentRacePoint::BundledBeforeCommit
        } else {
            DeploymentRacePoint::StagingWrite
        },
        move || {
            fs::write(
                modified.join("user-created.txt"),
                "private-user-content-do-not-leak",
            )
            .unwrap()
        },
    );
    let error = f
        .service
        .setup_agent_access(SetupAgentRequest::install("codex"))
        .unwrap_err();
    assert_eq!(
        error.code,
        if database_failure {
            ErrorCode::InternalError
        } else {
            ErrorCode::PlanStale
        }
    );
    let recovery = &error.details["bundled_rollback"];
    assert_eq!(recovery["reason"], "concurrent_content_preserved");
    assert_eq!(recovery["files_restored"], upgrade);
    let recovery_path = PathBuf::from(
        recovery["recovery_path"]
            .as_str()
            .expect("preserved content must have an actionable path"),
    );
    assert_eq!(
        fs::read_to_string(recovery_path.join("user-created.txt")).unwrap(),
        "private-user-content-do-not-leak"
    );
    assert_eq!(
        fs::read_to_string(recovery_path.join("SKILL.md")).unwrap(),
        super::super::agent_access::OFFICIAL_SKILL_MD
    );
    if let Some(old_content) = old_content {
        assert_eq!(fs::read(central.join("SKILL.md")).unwrap(), old_content);
        assert_ne!(recovery_path, central);
        assert!(!central.join("user-created.txt").exists());
        assert_eq!(
            f.service
                .show_skill("skills-hub".into())
                .unwrap()
                .source
                .revision
                .as_deref(),
            Some("0.0.1")
        );
    } else {
        assert_eq!(recovery_path, central);
        assert!(f.service.list_skills().unwrap().is_empty());
    }
    assert!(!f.target("codex").exists());
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    crate::cli::run_with_executor(
        ["skillshub-cli", "--json", "setup", "--agent", "codex"],
        &mut stdout,
        &mut stderr,
        |_| Err(error.clone()),
    );
    assert!(stdout.is_empty());
    let payload: serde_json::Value = serde_json::from_slice(&stderr).unwrap();
    assert_eq!(
        payload["details"]["bundled_rollback"]["recovery_path"],
        recovery["recovery_path"]
    );
    assert_eq!(
        payload["details"]["bundled_rollback"]["files_restored"],
        upgrade
    );
    assert_eq!(
        payload["details"]["bundled_rollback"]["reason"],
        "concurrent_content_preserved"
    );
    for secret in [
        "raw-db-error-do-not-leak",
        "private-user-content-do-not-leak",
    ] {
        assert!(!serde_json::to_string(&error).unwrap().contains(secret));
        assert!(!String::from_utf8_lossy(&stderr).contains(secret));
    }
}

#[test]
fn agent_access_new_install_reports_preserved_content_after_database_commit_failure() {
    assert_bundle_rollback_recovery(false, true);
}

#[test]
fn agent_access_upgrade_reports_restored_files_and_recovery_after_database_commit_failure() {
    assert_bundle_rollback_recovery(true, true);
}

#[test]
fn agent_access_new_install_reports_preserved_content_after_deployment_failure() {
    assert_bundle_rollback_recovery(false, false);
}

#[test]
fn agent_access_upgrade_reports_restored_files_and_recovery_after_deployment_failure() {
    assert_bundle_rollback_recovery(true, false);
}

#[test]
fn agent_access_existing_library_content_cannot_be_claimed_as_bundled() {
    let f = Fixture::new();
    let central = f.service.paths().default_central_repo.join("skills-hub");
    fs::create_dir_all(&central).unwrap();
    fs::write(central.join("SKILL.md"), "user library content").unwrap();
    let error = f
        .service
        .setup_agent_access(SetupAgentRequest::install("codex"))
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::TargetConflict);
    assert_eq!(
        fs::read_to_string(central.join("SKILL.md")).unwrap(),
        "user library content"
    );
    assert!(!f.target("codex").exists());
}

#[test]
fn agent_access_regular_updates_also_protect_modified_bundled_content() {
    for central in [true, false] {
        let f = Fixture::new();
        f.old_bundle();
        let target = if central {
            PathBuf::from(
                f.service
                    .show_skill("skills-hub".into())
                    .unwrap()
                    .central_path,
            )
        } else {
            f.target("cursor")
        };
        fs::write(target.join("SKILL.md"), "user edits").unwrap();
        assert_eq!(
            f.service.update("skills-hub".into()).unwrap_err().code,
            ErrorCode::TargetConflict
        );
        assert_eq!(
            fs::read_to_string(target.join("SKILL.md")).unwrap(),
            "user edits"
        );
    }
}

#[test]
fn agent_access_installs_bundled_source_only_to_explicit_agent_and_is_idempotent() {
    let f = Fixture::new();
    let first = f
        .service
        .setup_agent_access(SetupAgentRequest::install("codex"))
        .unwrap();
    assert!(first.deployed);
    assert!(f.target("codex").join("SKILL.md").is_file());
    assert!(!f.target("cursor").exists());
    let skill = f.service.show_skill("skills-hub".into()).unwrap();
    assert_eq!(skill.source.kind, "bundled");
    assert_eq!(
        skill.source.revision.as_deref(),
        Some(env!("CARGO_PKG_VERSION"))
    );
    let second = f
        .service
        .setup_agent_access(SetupAgentRequest::install("codex"))
        .unwrap();
    assert_eq!(first.skill_id, second.skill_id);
    assert_eq!(f.service.list_skills().unwrap().len(), 1);
    assert_eq!(
        f.service
            .show_skill("skills-hub".into())
            .unwrap()
            .targets
            .len(),
        1
    );
}

#[test]
fn agent_access_unmanaged_conflict_preserves_entire_batch() {
    let f = Fixture::new();
    fs::create_dir_all(f.target("cursor")).unwrap();
    fs::write(f.target("cursor").join("private.txt"), "keep").unwrap();
    let mut request = SetupAgentRequest::install("codex");
    request.agents.push("cursor".into());
    let error = f.service.setup_agent_access(request).unwrap_err();
    assert_eq!(error.code, ErrorCode::TargetConflict);
    assert_eq!(
        fs::read_to_string(f.target("cursor").join("private.txt")).unwrap(),
        "keep"
    );
    assert!(!f.target("codex").exists());
    assert!(f.service.list_skills().unwrap().is_empty());
}

#[test]
fn agent_access_modified_central_copy_is_never_overwritten() {
    let f = Fixture::new();
    f.service
        .setup_agent_access(SetupAgentRequest::install("codex"))
        .unwrap();
    let skill = f.service.show_skill("skills-hub".into()).unwrap();
    let manifest = PathBuf::from(skill.central_path).join("SKILL.md");
    fs::write(&manifest, "user changed this").unwrap();
    assert_eq!(
        f.service
            .setup_agent_access(SetupAgentRequest::install("cursor"))
            .unwrap_err()
            .code,
        ErrorCode::TargetConflict
    );
    assert_eq!(fs::read_to_string(manifest).unwrap(), "user changed this");
    assert!(!f.target("cursor").exists());
}

#[test]
fn agent_access_modified_deployed_copy_is_never_overwritten() {
    let f = Fixture::new();
    f.service
        .setup_agent_access(SetupAgentRequest::install("cursor"))
        .unwrap();
    let manifest = f.target("cursor").join("SKILL.md");
    fs::write(&manifest, "user changed this").unwrap();
    assert_eq!(
        f.service
            .setup_agent_access(SetupAgentRequest::install("cursor"))
            .unwrap_err()
            .code,
        ErrorCode::TargetConflict
    );
    assert_eq!(fs::read_to_string(manifest).unwrap(), "user changed this");
}

#[test]
fn agent_access_preview_and_unconfirmed_remove_do_not_mutate() {
    let f = Fixture::new();
    let mut preview = SetupAgentRequest::install("codex");
    preview.dry_run = true;
    f.service.setup_agent_access(preview).unwrap();
    assert!(f.service.list_skills().unwrap().is_empty());
    assert!(!f.target("codex").exists());
    f.service
        .setup_agent_access(SetupAgentRequest::install("codex"))
        .unwrap();
    let mut remove = SetupAgentRequest::install("codex");
    remove.remove = true;
    let error = f.service.setup_agent_access(remove.clone()).unwrap_err();
    assert_eq!(error.code, ErrorCode::ConfirmationRequired);
    assert!(error.details["plan"].is_object());
    assert!(f.target("codex").exists());
    remove.confirmed = true;
    f.service.setup_agent_access(remove).unwrap();
    assert!(!f.target("codex").exists());
    assert!(f.service.show_skill("skills-hub".into()).is_ok());
}

#[test]
fn agent_access_rejects_missing_intent_and_obeys_schema_before_operation_lock() {
    let f = Fixture::new();
    let mut request = SetupAgentRequest::install("codex");
    request.agents.clear();
    assert_eq!(
        f.service.setup_agent_access(request).unwrap_err().code,
        ErrorCode::InvalidArgument
    );
    let _lock = OperationLock::acquire(f.service.paths(), OperationKind::Install).unwrap();
    assert_eq!(
        f.service
            .setup_agent_access(SetupAgentRequest::install("codex"))
            .unwrap_err()
            .code,
        ErrorCode::OperationBusy
    );
    rusqlite::Connection::open(&f.service.paths().database_path)
        .unwrap()
        .execute_batch("PRAGMA user_version=999")
        .unwrap();
    assert_eq!(
        f.service
            .setup_agent_access(SetupAgentRequest::install("codex"))
            .unwrap_err()
            .code,
        ErrorCode::IncompatibleDatabase
    );
    assert!(!f.target("codex").exists());
}

#[test]
fn agent_access_status_is_read_only_and_reports_managed_deployments() {
    let f = Fixture::new();
    assert!(!f.service.agent_access_status().unwrap().installed);
    assert!(f.service.list_skills().unwrap().is_empty());
    f.service
        .setup_agent_access(SetupAgentRequest::install("codex"))
        .unwrap();
    let status = f.service.agent_access_status().unwrap();
    assert!(status.installed);
    assert!(status.deployed);
}

#[test]
fn agent_access_official_skill_covers_the_automation_safety_contract() {
    let content = fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../skills/skills-hub/SKILL.md"
    ))
    .unwrap_or_default();
    assert!(content.starts_with("---\nname: skills-hub\ndescription:"));
    for requirement in [
        "--json",
        "SHA-256",
        "PATH",
        "fail closed",
        "--dry-run",
        "--yes",
        "library",
        "explicit",
        "skills list",
        "skills show",
        "skills search",
        "skills status",
        "skills check",
        "skills install",
        "skills deploy",
        "skills undeploy",
        "skills update",
        "skills adopt",
        "skills tag",
        "skills remove",
        "agents list",
        "doctor",
        "version",
        "TARGET_CONFLICT",
        "UPDATE_HELD_BACK",
        "OPERATION_BUSY",
        "SQLite",
        "Token",
        "desktop",
        "--force",
    ] {
        assert!(
            content.contains(requirement),
            "missing automation safety requirement: {requirement}"
        );
    }
    for line in content.lines().filter(|line| line.starts_with("$CLI ")) {
        assert!(
            line.starts_with("$CLI --json "),
            "all CLI examples must emit JSON: {line}"
        );
    }
    assert!(content.lines().count() < 500);
}
