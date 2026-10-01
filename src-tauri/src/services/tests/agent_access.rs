use std::{fs, path::PathBuf};
use tempfile::TempDir;

use crate::core::runtime_paths::{RuntimePaths, RuntimeProfile};
use crate::services::agent_access::SetupAgentRequest;
use crate::services::error::ErrorCode;
use crate::services::operation_lock::{OperationKind, OperationLock};
use crate::services::skills_hub::SkillsHubService;

#[test]
fn one_click_management_ignores_unavailable_and_disabled_historical_targets() {
    use crate::core::tool_adapters::{save_tool_config, ToolConfig};
    use crate::services::agent_access::OfficialSkillState;
    for disabled in [false, true] {
        let f = Fixture::new();
        f.service.enable_ai_management().unwrap();
        let detached = f.home.path().join("cursor-detached");
        fs::rename(f.home.path().join(".cursor"), &detached).unwrap();
        if disabled {
            fs::create_dir(f.home.path().join(".cursor")).unwrap();
            save_tool_config(
                f.service.store(),
                ToolConfig {
                    disabled_builtin_tools: vec!["cursor".into()],
                    ..Default::default()
                },
            )
            .unwrap();
        }
        for _ in 0..2 {
            let status = f.service.enable_ai_management().unwrap();
            assert!(matches!(status.official_state, OfficialSkillState::Healthy));
            let historical = status
                .health
                .iter()
                .find(|agent| agent.agent == "cursor")
                .unwrap();
            assert!(historical.needs_repair);
            assert!(!f.target("cursor").exists());
            assert!(detached.join("skills/manage-skills-hub/SKILL.md").exists());
        }
    }
}

#[test]
fn one_click_management_installs_once_and_syncs_detected_enabled_tools() {
    let f = Fixture::new();
    let status = f.service.enable_ai_management().unwrap();
    assert!(status.installed);
    assert!(f.target("codex").join("SKILL.md").exists());
    assert!(f.target("cursor").join("SKILL.md").exists());
    assert_eq!(f.service.list_skills().unwrap().len(), 1);
    f.service.enable_ai_management().unwrap();
    assert_eq!(f.service.list_skills().unwrap().len(), 1);
}

#[test]
fn one_click_management_without_detected_tools_does_not_install() {
    let f = Fixture::new();
    fs::remove_dir(f.home.path().join(".codex")).unwrap();
    fs::remove_dir(f.home.path().join(".cursor")).unwrap();
    assert!(f.service.enable_ai_management().is_err());
    assert!(f.service.list_skills().unwrap().is_empty());
}

#[test]
fn one_click_management_rejects_shared_directory_scope_expansion_before_install() {
    use crate::core::sync_engine::SyncMode;
    use crate::core::tool_adapters::{save_tool_config, CustomToolConfig, ToolConfig};
    let f = Fixture::new();
    let root = f.home.path().join("shared-tools");
    fs::create_dir_all(&root).unwrap();
    save_tool_config(
        f.service.store(),
        ToolConfig {
            disabled_builtin_tools: vec![],
            custom_tools: [("custom_active", true), ("custom_disabled", false)]
                .into_iter()
                .map(|(key, enabled)| CustomToolConfig {
                    key: key.into(),
                    label: key.into(),
                    avatar: None,
                    skills_dir: root.to_string_lossy().into_owned(),
                    project_skills_dir: None,
                    sync_mode: SyncMode::Copy,
                    enabled,
                })
                .collect(),
        },
    )
    .unwrap();
    let error = f.service.enable_ai_management().unwrap_err();
    assert_eq!(error.code, ErrorCode::TargetConflict);
    assert_eq!(error.details["reason"], "shared_directory_scope_expansion");
    assert_eq!(
        error.details["affected_agents"],
        serde_json::json!(["custom_active", "custom_disabled"])
    );
    assert!(f.service.list_skills().unwrap().is_empty());
    assert!(!root.join("manage-skills-hub").exists());
    assert!(!f.target("codex").exists());
    assert!(!f.target("cursor").exists());
}

#[test]
fn one_click_management_respects_disabled_tools_and_preserves_conflicting_content() {
    use crate::core::tool_adapters::{ToolConfig, TOOL_CONFIG_SETTING};
    let f = Fixture::new();
    f.service
        .store()
        .set_setting(
            TOOL_CONFIG_SETTING,
            &serde_json::to_string(&ToolConfig {
                disabled_builtin_tools: vec!["cursor".into()],
                custom_tools: vec![],
            })
            .unwrap(),
        )
        .unwrap();
    f.service.enable_ai_management().unwrap();
    assert!(f.target("codex").exists());
    assert!(!f.target("cursor").exists());

    let conflict = Fixture::new();
    fs::create_dir_all(conflict.target("cursor")).unwrap();
    fs::write(conflict.target("cursor").join("SKILL.md"), "user owned").unwrap();
    assert!(conflict.service.enable_ai_management().is_err());
    assert!(conflict.service.list_skills().unwrap().is_empty());
    assert!(!conflict.target("codex").exists());
    assert_eq!(
        fs::read_to_string(conflict.target("cursor").join("SKILL.md")).unwrap(),
        "user owned"
    );
}

#[test]
fn agent_access_status_reads_actual_copy_health_without_changing_files_or_records() {
    let f = Fixture::new();
    f.service
        .setup_agent_access(SetupAgentRequest::install("cursor"))
        .unwrap();
    let before = f.service.show_skill("manage-skills-hub".into()).unwrap();
    fs::write(f.target("cursor").join("SKILL.md"), "user edit").unwrap();
    let status = f.service.agent_access_status().unwrap();
    let health = status
        .health
        .iter()
        .find(|agent| agent.agent == "cursor")
        .unwrap();
    assert!(health.needs_repair);
    assert_eq!(
        health.reason,
        Some(super::super::agent_access::AgentAccessReason::TargetModified)
    );
    assert_eq!(
        f.service.show_skill("manage-skills-hub".into()).unwrap(),
        before
    );
    assert_eq!(
        fs::read_to_string(f.target("cursor").join("SKILL.md")).unwrap(),
        "user edit"
    );
    fs::remove_dir_all(f.target("cursor")).unwrap();
    let status = f.service.agent_access_status().unwrap();
    assert_eq!(
        status
            .health
            .iter()
            .find(|a| a.agent == "cursor")
            .unwrap()
            .reason,
        Some(super::super::agent_access::AgentAccessReason::TargetMissing)
    );
}

#[test]
fn agent_access_status_reports_central_damage() {
    let f = Fixture::new();
    f.service
        .setup_agent_access(SetupAgentRequest::install("codex"))
        .unwrap();
    let central = f
        .service
        .show_skill("manage-skills-hub".into())
        .unwrap()
        .central_path;
    fs::write(PathBuf::from(&central).join("SKILL.md"), "modified bundle").unwrap();
    assert_eq!(
        f.service.agent_access_status().unwrap().central_reason,
        Some(super::super::agent_access::AgentAccessReason::CentralModified)
    );
    fs::remove_dir_all(central).unwrap();
    assert_eq!(
        f.service.agent_access_status().unwrap().central_reason,
        Some(super::super::agent_access::AgentAccessReason::CentralMissing)
    );
}

#[test]
fn agent_access_status_reports_saved_errors_and_target_path_ownership_without_writes() {
    use super::super::agent_access::AgentAccessReason;
    let f = Fixture::new();
    f.service
        .setup_agent_access(SetupAgentRequest::install("cursor"))
        .unwrap();
    let db = rusqlite::Connection::open(&f.service.paths().database_path).unwrap();
    db.execute(
        "UPDATE skill_targets SET status='error' WHERE tool='cursor'",
        [],
    )
    .unwrap();
    let status = f.service.agent_access_status().unwrap();
    assert_eq!(
        status
            .health
            .iter()
            .find(|a| a.agent == "cursor")
            .unwrap()
            .reason,
        Some(AgentAccessReason::RecordError)
    );
    db.execute(
        "UPDATE skill_targets SET status='ok', target_path=?1 WHERE tool='cursor'",
        [f.home
            .path()
            .join("outside/manage-skills-hub")
            .to_str()
            .unwrap()],
    )
    .unwrap();
    let status = f.service.agent_access_status().unwrap();
    assert_eq!(
        status
            .health
            .iter()
            .find(|a| a.agent == "cursor")
            .unwrap()
            .reason,
        Some(AgentAccessReason::TargetOwnership)
    );
    assert!(f.target("cursor").is_dir());
}

#[cfg(unix)]
#[test]
fn agent_access_status_accepts_owned_symlinks_but_rejects_redirected_and_broken_links() {
    let f = Fixture::new();
    f.service
        .setup_agent_access(SetupAgentRequest::install("codex"))
        .unwrap();
    let central = f
        .service
        .show_skill("manage-skills-hub".into())
        .unwrap()
        .central_path;
    fs::remove_dir_all(f.target("codex")).unwrap();
    std::os::unix::fs::symlink(&central, f.target("codex")).unwrap();
    assert!(
        !f.service
            .agent_access_status()
            .unwrap()
            .health
            .iter()
            .find(|a| a.agent == "codex")
            .unwrap()
            .needs_repair
    );
    fs::remove_file(f.target("codex")).unwrap();
    let outside = f.home.path().join("outside");
    fs::create_dir(&outside).unwrap();
    fs::copy(
        PathBuf::from(&central).join("SKILL.md"),
        outside.join("SKILL.md"),
    )
    .unwrap();
    std::os::unix::fs::symlink(&outside, f.target("codex")).unwrap();
    assert_eq!(
        f.service
            .agent_access_status()
            .unwrap()
            .health
            .iter()
            .find(|a| a.agent == "codex")
            .unwrap()
            .reason,
        Some(super::super::agent_access::AgentAccessReason::TargetOwnership)
    );
    fs::remove_dir_all(outside).unwrap();
    assert!(
        f.service
            .agent_access_status()
            .unwrap()
            .health
            .iter()
            .find(|a| a.agent == "codex")
            .unwrap()
            .needs_repair
    );
}

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
        self.home
            .path()
            .join(format!(".{agent}/skills/manage-skills-hub"))
    }

    fn old_bundle(&self) {
        let bundled = self
            .service
            .prepare_bundled_install(
                "manage-skills-hub",
                "---\nname: manage-skills-hub\ndescription: Previous official skill\n---\nOld release\n",
                "0.0.1",
            )
            .unwrap();
        self.service
            .apply_bundled_install(bundled, || Ok(()))
            .unwrap();
        self.service
            .deploy(crate::services::deployment::DeploymentRequest::global(
                "manage-skills-hub",
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
    let before = f.service.show_skill("manage-skills-hub".into()).unwrap();
    let error = f
        .service
        .setup_agent_access(SetupAgentRequest::install("cursor"))
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::TargetConflict);
    assert_eq!(
        f.service.show_skill("manage-skills-hub".into()).unwrap(),
        before
    );
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
fn automatic_management_update_keeps_existing_targets_only() {
    let f = Fixture::new();
    f.old_bundle();
    let result = f.service.refresh_installed_ai_management().unwrap();
    assert!(result);
    assert_eq!(
        f.service
            .agent_access_status()
            .unwrap()
            .installed_version
            .as_deref(),
        Some(env!("CARGO_PKG_VERSION"))
    );
    assert!(f.target("cursor").exists());
    assert!(!f.target("codex").exists());
}

#[test]
fn automatic_management_update_does_not_install_for_unenrolled_users() {
    let f = Fixture::new();
    assert!(!f.service.refresh_installed_ai_management().unwrap());
    assert!(f.service.list_skills().unwrap().is_empty());
}

#[test]
fn automatic_management_update_requires_an_enabled_deployment() {
    let f = Fixture::new();
    assert!(!f
        .service
        .agent_access_status()
        .unwrap()
        .auto_update_eligible());
    f.old_bundle();
    let status = f.service.agent_access_status().unwrap();
    assert!(status.auto_update_eligible());
    f.service
        .store()
        .set_skill_enabled(status.skill_id.as_deref().unwrap(), false)
        .unwrap();
    assert!(!f
        .service
        .agent_access_status()
        .unwrap()
        .auto_update_eligible());

    let other = Fixture::new();
    let bundled = other
        .service
        .prepare_bundled_install(
            "manage-skills-hub",
            super::super::agent_access::OFFICIAL_SKILL_MD,
            env!("CARGO_PKG_VERSION"),
        )
        .unwrap();
    other
        .service
        .apply_bundled_install(bundled, || Ok(()))
        .unwrap();
    assert!(!other
        .service
        .agent_access_status()
        .unwrap()
        .auto_update_eligible());
}

#[test]
fn automatic_management_update_refreshes_changed_bundle_at_same_version() {
    let f = Fixture::new();
    let bundled = f
        .service
        .prepare_bundled_install(
            "manage-skills-hub",
            "---\nname: manage-skills-hub\n---\nPrevious content\n",
            env!("CARGO_PKG_VERSION"),
        )
        .unwrap();
    f.service.apply_bundled_install(bundled, || Ok(())).unwrap();
    assert!(f.service.refresh_installed_ai_management().unwrap());
    assert_eq!(
        fs::read_to_string(
            f.service
                .show_skill("manage-skills-hub".into())
                .unwrap()
                .central_path
                + "/SKILL.md"
        )
        .unwrap(),
        super::super::agent_access::OFFICIAL_SKILL_MD
    );
    assert!(!f.target("codex").exists());
}

#[test]
fn agent_access_official_bundle_works_with_normal_check_and_update_workflows() {
    let f = Fixture::new();
    f.old_bundle();
    let check = f.service.check_updates("manage-skills-hub".into()).unwrap();
    assert!(check.update_available);
    let updated = f.service.update("manage-skills-hub".into()).unwrap();
    assert!(updated.changed);
    assert_eq!(
        fs::read_to_string(f.target("cursor").join("SKILL.md")).unwrap(),
        super::super::agent_access::OFFICIAL_SKILL_MD
    );
    assert!(
        !f.service
            .check_updates("manage-skills-hub".into())
            .unwrap()
            .update_available
    );
    assert!(
        !f.service
            .update("manage-skills-hub".into())
            .unwrap()
            .changed
    );
}

#[test]
fn agent_access_update_skips_tools_missing_from_runtime_home() {
    let f = Fixture::new();
    f.old_bundle();
    let previous_target = fs::read(f.target("cursor").join("SKILL.md")).unwrap();
    let detached = f.home.path().join("cursor-detached");
    fs::rename(f.home.path().join(".cursor"), &detached).unwrap();
    let updated = f.service.update("manage-skills-hub".into()).unwrap();
    assert!(updated.changed);
    let record = f.service.show_skill("manage-skills-hub".into()).unwrap();
    assert!(record.targets.iter().all(|target| target.status != "error"));
    assert_eq!(
        fs::read(detached.join("skills/manage-skills-hub/SKILL.md")).unwrap(),
        previous_target
    );
    assert!(!f.home.path().join(".cursor").exists());
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
        .join("manage-skills-hub")
        .exists());
    assert!(!f.target("codex").exists());
    assert!(!f.target("cursor").exists());
}

#[test]
fn agent_access_failed_upgrade_restores_old_bundle_and_deployment() {
    let f = Fixture::new();
    f.old_bundle();
    let before = f.service.show_skill("manage-skills-hub".into()).unwrap();
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
            .show_skill("manage-skills-hub".into())
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
    let central = f
        .service
        .paths()
        .default_central_repo
        .join("manage-skills-hub");
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
                .show_skill("manage-skills-hub".into())
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
fn agent_access_name_conflicts_return_safe_status_without_claiming_user_skills() {
    for kind in ["local", "git"] {
        let f = Fixture::new();
        f.service
            .setup_agent_access(SetupAgentRequest::install("cursor"))
            .unwrap();
        let db = rusqlite::Connection::open(&f.service.paths().database_path).unwrap();
        let source = if kind == "git" {
            "https://user:private-token@example.test/skills.git?token=private-query#private-fragment"
        } else {
            "/local/user-skill"
        };
        db.execute(
            "UPDATE skills SET source_type=?1, source_ref=?2 WHERE name='manage-skills-hub'",
            rusqlite::params![kind, source],
        )
        .unwrap();
        let before = f.service.show_skill("manage-skills-hub".into()).unwrap();
        let status = f
            .service
            .agent_access_status()
            .expect("name conflicts must not fail status reads");
        let json = serde_json::to_value(&status).unwrap();
        assert_eq!(json["official_state"], "name_conflict");
        assert_eq!(json["conflict"]["sourceKind"], kind);
        assert_eq!(json["conflict"]["centralPath"], before.central_path);
        assert!(!status.installed);
        assert!(!status.deployed);
        assert!(status.skill.is_none());
        assert!(status
            .agents
            .agents
            .iter()
            .any(|agent| agent.key == "cursor" && agent.detected));
        assert!(status.health.iter().all(|agent| !agent.deployed));
        let serialized = serde_json::to_string(&status).unwrap();
        for secret in [
            "private-token",
            "private-query",
            "private-fragment",
            "example.test",
        ] {
            assert!(!serialized.contains(secret));
        }
        for remove in [false, true] {
            let mut request = SetupAgentRequest::install("cursor");
            request.remove = remove;
            request.confirmed = true;
            assert_eq!(
                f.service.setup_agent_access(request).unwrap_err().code,
                ErrorCode::TargetConflict
            );
        }
        assert_eq!(
            f.service.show_skill("manage-skills-hub".into()).unwrap(),
            before
        );
        assert!(f.target("cursor").join("SKILL.md").is_file());
    }
}

#[test]
fn agent_access_existing_library_content_cannot_be_claimed_as_bundled() {
    let f = Fixture::new();
    let central = f
        .service
        .paths()
        .default_central_repo
        .join("manage-skills-hub");
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
                    .show_skill("manage-skills-hub".into())
                    .unwrap()
                    .central_path,
            )
        } else {
            f.target("cursor")
        };
        fs::write(target.join("SKILL.md"), "user edits").unwrap();
        assert_eq!(
            f.service
                .update("manage-skills-hub".into())
                .unwrap_err()
                .code,
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
    let skill = f.service.show_skill("manage-skills-hub".into()).unwrap();
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
            .show_skill("manage-skills-hub".into())
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
fn agent_access_modified_managed_official_central_copy_is_replaced() {
    for missing_hash in [false, true] {
        let f = Fixture::new();
        f.service.enable_ai_management().unwrap();
        let skill = f.service.show_skill("manage-skills-hub".into()).unwrap();
        let manifest = PathBuf::from(&skill.central_path).join("SKILL.md");
        fs::write(&manifest, "previous development content").unwrap();
        if missing_hash {
            let mut record = f
                .service
                .store()
                .get_skill_by_id(&skill.id)
                .unwrap()
                .unwrap();
            record.content_hash = None;
            f.service.store().upsert_skill(&record).unwrap();
        }
        f.service.enable_ai_management().unwrap();
        assert_eq!(
            fs::read_to_string(manifest).unwrap(),
            super::super::agent_access::OFFICIAL_SKILL_MD
        );
        assert_eq!(
            f.service.agent_access_status().unwrap().central_reason,
            None
        );
    }
}

#[test]
fn official_content_update_status_is_read_only_and_detects_same_version_changes() {
    let f = Fixture::new();
    let bundled = f
        .service
        .prepare_bundled_install(
            "manage-skills-hub",
            "---\nname: manage-skills-hub\n---\nOld development content\n",
            env!("CARGO_PKG_VERSION"),
        )
        .unwrap();
    f.service.apply_bundled_install(bundled, || Ok(())).unwrap();
    let record = f.service.store().list_skills().unwrap().pop().unwrap();
    let before = fs::read(PathBuf::from(&record.central_path).join("SKILL.md")).unwrap();
    for _ in 0..2 {
        let status = f.service.agent_access_status().unwrap();
        assert_eq!(status.central_reason, None);
        assert_eq!(
            serde_json::to_value(status).unwrap()["skill_update_available"],
            true
        );
        assert_eq!(
            f.service
                .store()
                .get_skill_by_id(&record.id)
                .unwrap()
                .unwrap(),
            record
        );
        assert_eq!(
            fs::read(PathBuf::from(&record.central_path).join("SKILL.md")).unwrap(),
            before
        );
    }
    f.service.enable_ai_management().unwrap();
    assert_eq!(
        serde_json::to_value(f.service.agent_access_status().unwrap()).unwrap()
            ["skill_update_available"],
        false
    );
}

#[test]
fn official_central_recovery_rolls_back_modified_bytes_and_original_record() {
    let f = Fixture::new();
    f.service.enable_ai_management().unwrap();
    let original = f.service.store().list_skills().unwrap().pop().unwrap();
    let manifest = PathBuf::from(&original.central_path).join("SKILL.md");
    fs::write(&manifest, "development edits").unwrap();
    let bundled = f
        .service
        .prepare_bundled_install(
            "manage-skills-hub",
            super::super::agent_access::OFFICIAL_SKILL_MD,
            env!("CARGO_PKG_VERSION"),
        )
        .unwrap();
    let result: Result<(), _> = f.service.apply_bundled_install(bundled, || {
        Err(crate::services::error::ServiceError::internal(
            "deployment failed",
        ))
    });
    assert!(result.is_err());
    assert_eq!(fs::read_to_string(manifest).unwrap(), "development edits");
    assert_eq!(
        f.service
            .store()
            .get_skill_by_id(&original.id)
            .unwrap()
            .unwrap(),
        original
    );
}

#[test]
fn official_recovery_rejects_a_record_outside_the_owned_library() {
    let f = Fixture::new();
    f.service.enable_ai_management().unwrap();
    let mut record = f.service.store().list_skills().unwrap().pop().unwrap();
    let outside = f.home.path().join("outside/manage-skills-hub");
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("SKILL.md"), "user content").unwrap();
    record.central_path = outside.to_string_lossy().into_owned();
    f.service.store().upsert_skill(&record).unwrap();
    assert!(f.service.enable_ai_management().is_err());
    assert_eq!(
        fs::read_to_string(outside.join("SKILL.md")).unwrap(),
        "user content"
    );
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
    assert!(f.service.show_skill("manage-skills-hub".into()).is_ok());
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
        "/../skills/manage-skills-hub/SKILL.md"
    ))
    .unwrap_or_default();
    assert!(content.starts_with("---\nname: manage-skills-hub\ndescription:"));
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

fn legacy_device_sync_bundle(f: &Fixture) -> crate::core::skill_store::SkillRecord {
    use crate::core::device_sync::manifest::export_library;
    let mut record = f.service.store().list_skills().unwrap().remove(0);
    let exported = tempfile::tempdir().unwrap();
    let manifest = export_library(f.service.store(), exported.path()).unwrap();
    record.content_hash = Some(manifest.skills[&record.id].content_hash.clone());
    f.service.store().upsert_skill(&record).unwrap();
    record
}

#[test]
fn one_click_management_recovers_legacy_device_sync_hash_without_preflight_writes() {
    let f = Fixture::new();
    f.service.enable_ai_management().unwrap();
    let id = f.service.store().list_skills().unwrap()[0].id.clone();
    f.service
        .store()
        .set_skill_tag_names(&id, &["official".into(), "management".into()])
        .unwrap();
    let original = legacy_device_sync_bundle(&f);
    let schema_before = rusqlite::Connection::open(f.service.store().db_path())
        .unwrap()
        .pragma_query_value(None, "user_version", |row| row.get::<_, i32>(0))
        .unwrap();
    f.service.preflight_ai_management().unwrap();
    assert_eq!(
        f.service
            .store()
            .get_skill_by_id(&original.id)
            .unwrap()
            .unwrap()
            .content_hash,
        original.content_hash
    );
    let status = f.service.enable_ai_management().unwrap();
    assert_eq!(status.central_reason, None);
    assert!(status.health.iter().all(|health| !health.needs_repair));
    let repaired = f
        .service
        .store()
        .get_skill_by_id(&original.id)
        .unwrap()
        .unwrap();
    assert_eq!(repaired.id, original.id);
    assert_eq!(repaired.created_at, original.created_at);
    assert_eq!(
        repaired.content_hash,
        Some(
            crate::core::content_hash::hash_dir_strict(std::path::Path::new(
                &repaired.central_path
            ))
            .unwrap()
        )
    );
    assert_ne!(repaired.content_hash, original.content_hash);
    assert_eq!(
        f.service
            .store()
            .get_skill_tags(&id)
            .unwrap()
            .into_iter()
            .map(|tag| tag.name)
            .collect::<Vec<_>>(),
        ["management", "official"]
    );
    assert_eq!(
        rusqlite::Connection::open(f.service.store().db_path())
            .unwrap()
            .pragma_query_value(None, "user_version", |row| row.get::<_, i32>(0))
            .unwrap(),
        schema_before
    );
    f.service.enable_ai_management().unwrap();
    assert_eq!(f.service.store().list_skills().unwrap().len(), 1);
}

#[test]
fn official_skill_update_recovers_legacy_device_sync_hash_and_upgrades_old_content() {
    let f = Fixture::new();
    f.old_bundle();
    let original = legacy_device_sync_bundle(&f);
    assert!(f.service.refresh_installed_ai_management().unwrap());
    assert_eq!(
        fs::read_to_string(PathBuf::from(original.central_path).join("SKILL.md")).unwrap(),
        super::super::agent_access::OFFICIAL_SKILL_MD
    );
    assert_eq!(
        f.service.agent_access_status().unwrap().central_reason,
        None
    );
    assert!(!f.target("codex").exists());
}

#[test]
fn legacy_device_sync_official_recovery_replaces_owned_regular_content() {
    for change in ["edited", "extra_file", "extra_directory", "wrong_hash"] {
        let f = Fixture::new();
        f.service.enable_ai_management().unwrap();
        let mut original = legacy_device_sync_bundle(&f);
        let central = PathBuf::from(&original.central_path);
        match change {
            "edited" => fs::write(central.join("SKILL.md"), "user edit").unwrap(),
            "extra_file" => fs::write(central.join("notes.txt"), "user notes").unwrap(),
            "extra_directory" => fs::create_dir(central.join("notes")).unwrap(),
            "wrong_hash" => {
                original.content_hash = Some("unknown-baseline".into());
                f.service.store().upsert_skill(&original).unwrap();
            }
            _ => unreachable!(),
        }
        let before = crate::core::content_hash::hash_dir_strict(&central).unwrap();
        f.service.preflight_ai_management().unwrap();
        assert!(
            f.service.refresh_installed_ai_management().is_err(),
            "{change}"
        );
        assert_eq!(
            crate::core::content_hash::hash_dir_strict(&central).unwrap(),
            before
        );
        assert_eq!(
            f.service
                .store()
                .get_skill_by_id(&original.id)
                .unwrap()
                .unwrap()
                .content_hash,
            original.content_hash
        );
        f.service.enable_ai_management().unwrap();
        assert_eq!(
            fs::read_to_string(central.join("SKILL.md")).unwrap(),
            super::super::agent_access::OFFICIAL_SKILL_MD
        );
        assert!(!central.join("notes.txt").exists());
        assert!(!central.join("notes").exists());
    }
}

#[cfg(unix)]
#[test]
fn legacy_device_sync_recovery_rejects_skill_symlinks() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new();
    f.service.enable_ai_management().unwrap();
    let original = legacy_device_sync_bundle(&f);
    let file = PathBuf::from(&original.central_path).join("SKILL.md");
    let external = f.home.path().join("external.md");
    fs::rename(&file, &external).unwrap();
    symlink(&external, &file).unwrap();
    let original_bytes = fs::read(&external).unwrap();
    assert!(f.service.enable_ai_management().is_err());
    assert!(fs::symlink_metadata(&file)
        .unwrap()
        .file_type()
        .is_symlink());
    assert_eq!(fs::read(external).unwrap(), original_bytes);
    assert_eq!(
        f.service
            .store()
            .get_skill_by_id(&original.id)
            .unwrap()
            .unwrap()
            .content_hash,
        original.content_hash
    );
}

#[cfg(unix)]
#[test]
fn official_recovery_replaces_owned_regular_executable_files() {
    use std::os::unix::fs::PermissionsExt;
    for executable_manifest in [false, true] {
        let f = Fixture::new();
        f.service.enable_ai_management().unwrap();
        let original = legacy_device_sync_bundle(&f);
        let central = PathBuf::from(&original.central_path);
        let file = if executable_manifest {
            central.join("SKILL.md")
        } else {
            let script = central.join("old-helper.sh");
            fs::write(&script, "#!/bin/sh\necho old-development-helper\n").unwrap();
            script
        };
        fs::set_permissions(&file, fs::Permissions::from_mode(0o755)).unwrap();
        f.service.enable_ai_management().unwrap();
        assert_eq!(
            fs::read_to_string(central.join("SKILL.md")).unwrap(),
            super::super::agent_access::OFFICIAL_SKILL_MD
        );
        assert!(!central.join("old-helper.sh").exists());
        assert_eq!(
            f.service.agent_access_status().unwrap().central_reason,
            None
        );
    }
}

#[test]
fn legacy_device_sync_recovery_rolls_back_original_record_when_deployment_fails() {
    let f = Fixture::new();
    f.service.enable_ai_management().unwrap();
    let original = legacy_device_sync_bundle(&f);
    let before = fs::read(PathBuf::from(&original.central_path).join("SKILL.md")).unwrap();
    let bundled = f
        .service
        .prepare_bundled_install(
            "manage-skills-hub",
            super::super::agent_access::OFFICIAL_SKILL_MD,
            env!("CARGO_PKG_VERSION"),
        )
        .unwrap();
    let result: Result<(), _> = f.service.apply_bundled_install(bundled, || {
        Err(crate::services::error::ServiceError::internal(
            "deployment failed",
        ))
    });
    assert!(result.is_err());
    assert_eq!(
        f.service
            .store()
            .get_skill_by_id(&original.id)
            .unwrap()
            .unwrap()
            .content_hash,
        original.content_hash
    );
    assert_eq!(
        fs::read(PathBuf::from(&original.central_path).join("SKILL.md")).unwrap(),
        before
    );
}

#[test]
fn official_recovery_preserves_the_disabled_skill_setting() {
    let f = Fixture::new();
    f.old_bundle();
    let mut record = f.service.store().list_skills().unwrap().pop().unwrap();
    record.enabled = false;
    f.service.store().upsert_skill(&record).unwrap();
    fs::write(
        PathBuf::from(&record.central_path).join("SKILL.md"),
        "development edits",
    )
    .unwrap();
    let status = f.service.enable_ai_management().unwrap();
    assert!(!status.skill.unwrap().enabled);
    assert!(!status.skill_update_available);
}

#[test]
fn official_recovery_rejects_paths_shared_with_another_managed_skill() {
    let f = Fixture::new();
    f.service.enable_ai_management().unwrap();
    let mut record = f.service.store().list_skills().unwrap().pop().unwrap();
    record.id = "another-skill".into();
    record.name = "another-skill".into();
    record.source_type = "local".into();
    let manifest = PathBuf::from(&record.central_path).join("SKILL.md");
    record.central_path = PathBuf::from(&record.central_path)
        .join("another-skill")
        .to_string_lossy()
        .into_owned();
    fs::create_dir(&record.central_path).unwrap();
    f.service.store().upsert_skill(&record).unwrap();
    fs::write(&manifest, "shared content").unwrap();
    assert!(f.service.enable_ai_management().is_err());
    assert_eq!(fs::read_to_string(manifest).unwrap(), "shared content");
}

#[cfg(unix)]
#[test]
fn official_recovery_rejects_a_symlinked_central_directory() {
    let f = Fixture::new();
    f.service.enable_ai_management().unwrap();
    let record = f.service.store().list_skills().unwrap().pop().unwrap();
    let central = PathBuf::from(&record.central_path);
    let external = f.home.path().join("external-official");
    fs::rename(&central, &external).unwrap();
    std::os::unix::fs::symlink(&external, &central).unwrap();
    assert_eq!(
        f.service.agent_access_status().unwrap().central_reason,
        Some(super::super::agent_access::AgentAccessReason::CentralUnsafePath)
    );
    assert!(f.service.enable_ai_management().is_err());
    assert!(fs::symlink_metadata(central)
        .unwrap()
        .file_type()
        .is_symlink());
    assert_eq!(
        fs::read_to_string(external.join("SKILL.md")).unwrap(),
        super::super::agent_access::OFFICIAL_SKILL_MD
    );
}

#[test]
fn official_status_distinguishes_content_edits_from_bundle_updates() {
    let f = Fixture::new();
    f.service.enable_ai_management().unwrap();
    let record = f.service.store().list_skills().unwrap().pop().unwrap();
    fs::write(
        PathBuf::from(&record.central_path).join("SKILL.md"),
        "local edits",
    )
    .unwrap();
    let status = f.service.agent_access_status().unwrap();
    assert_eq!(
        status.central_reason,
        Some(super::super::agent_access::AgentAccessReason::CentralModified)
    );
    assert!(!status.skill_update_available);
}
