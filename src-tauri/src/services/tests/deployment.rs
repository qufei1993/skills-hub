use std::fs;
use std::path::PathBuf;

use rusqlite::Connection;
use tempfile::TempDir;

use crate::core::runtime_paths::{RuntimePaths, RuntimeProfile};
use crate::services::deployment::{DeploymentRequest, DeploymentScope};
use crate::services::error::ErrorCode;
use crate::services::install::InstallRequest;
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
        let paths = RuntimePaths::from_roots(RuntimeProfile::Test, home.path(), data.path());
        let service = SkillsHubService::open(paths).unwrap();
        let source = data.path().join("source");
        fs::create_dir_all(&source).unwrap();
        fs::write(
            source.join("SKILL.md"),
            "---\nname: demo\ndescription: Demo\n---\nBody\n",
        )
        .unwrap();
        service.install(InstallRequest::local(source)).unwrap();
        fs::create_dir_all(home.path().join(".codex")).unwrap();
        fs::create_dir_all(home.path().join(".cursor")).unwrap();
        Self {
            home,
            _data: data,
            service,
        }
    }

    fn target(&self, agent: &str) -> PathBuf {
        self.home.path().join(format!(".{agent}/skills/demo"))
    }

    fn rows(&self) -> usize {
        self.service
            .show_skill("demo".into())
            .unwrap()
            .targets
            .len()
    }
}

#[test]
fn conflict_in_second_agent_leaves_first_agent_and_database_unchanged() {
    let f = Fixture::new();
    fs::create_dir_all(f.target("cursor")).unwrap();
    fs::write(f.target("cursor").join("user.txt"), "keep").unwrap();
    let error = f
        .service
        .deploy(DeploymentRequest::global("demo", ["codex", "cursor"]))
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::TargetConflict);
    assert_eq!(error.details["agent"], "cursor");
    assert!(!f.target("codex").exists());
    assert_eq!(f.rows(), 0);
}

#[test]
fn dry_run_has_no_filesystem_or_database_effects() {
    let f = Fixture::new();
    let plan = f
        .service
        .plan_deploy(DeploymentRequest::global("demo", ["codex", "cursor"]))
        .unwrap();
    assert_eq!(plan.targets.len(), 2);
    assert!(!f.home.path().join(".codex/skills").exists());
    assert!(!f.home.path().join(".cursor/skills").exists());
    assert_eq!(f.rows(), 0);
}

#[test]
fn deployment_requires_explicit_agents() {
    let f = Fixture::new();
    assert_eq!(
        f.service
            .deploy(DeploymentRequest::global("demo", [] as [&str; 0]))
            .unwrap_err()
            .code,
        ErrorCode::InvalidArgument
    );
}

#[test]
fn stale_plan_cannot_overwrite_new_target() {
    let f = Fixture::new();
    let plan = f
        .service
        .plan_deploy(DeploymentRequest::global("demo", ["codex", "cursor"]))
        .unwrap();
    fs::create_dir_all(f.target("cursor")).unwrap();
    fs::write(f.target("cursor").join("user.txt"), "keep").unwrap();
    assert_eq!(
        f.service.apply_deployment_plan(plan).unwrap_err().code,
        ErrorCode::PlanStale
    );
    assert!(!f.target("codex").exists());
    assert_eq!(f.rows(), 0);
}

#[test]
fn database_failure_rolls_back_all_activated_targets() {
    let f = Fixture::new();
    Connection::open(f.service.paths().database_path.clone()).unwrap().execute_batch(
        "CREATE TRIGGER reject_deploy BEFORE INSERT ON skill_targets WHEN NEW.tool='cursor' BEGIN SELECT RAISE(FAIL, 'injected failure'); END;"
    ).unwrap();
    assert!(f
        .service
        .deploy(DeploymentRequest::global("demo", ["codex", "cursor"]))
        .is_err());
    assert!(!f.target("codex").exists());
    assert!(!f.target("cursor").exists());
    assert_eq!(f.rows(), 0);
}

#[test]
fn successful_deploy_and_undeploy_preserve_central_copy() {
    let f = Fixture::new();
    f.service
        .deploy(DeploymentRequest::global("demo", ["codex", "cursor"]))
        .unwrap();
    assert_eq!(f.rows(), 2);
    assert!(f.target("cursor").join("SKILL.md").is_file());
    f.service
        .undeploy(DeploymentRequest::global("demo", ["codex", "cursor"]))
        .unwrap();
    assert_eq!(f.rows(), 0);
    assert!(!f.target("codex").exists());
    assert!(!f.target("cursor").exists());
    assert!(
        PathBuf::from(f.service.show_skill("demo".into()).unwrap().central_path)
            .join("SKILL.md")
            .is_file()
    );
}

#[test]
fn undeploy_refuses_modified_copy_without_removing_other_agents() {
    let f = Fixture::new();
    f.service
        .deploy(DeploymentRequest::global("demo", ["codex", "cursor"]))
        .unwrap();
    fs::write(f.target("cursor").join("user.txt"), "keep").unwrap();
    assert_eq!(
        f.service
            .undeploy(DeploymentRequest::global("demo", ["codex", "cursor"]))
            .unwrap_err()
            .code,
        ErrorCode::TargetConflict
    );
    assert!(f.target("codex").exists());
    assert_eq!(f.rows(), 2);
}

#[test]
fn undeploy_database_failure_restores_all_targets_and_rows() {
    let f = Fixture::new();
    f.service
        .deploy(DeploymentRequest::global("demo", ["codex", "cursor"]))
        .unwrap();
    Connection::open(f.service.paths().database_path.clone()).unwrap().execute_batch(
        "CREATE TRIGGER reject_undeploy BEFORE DELETE ON skill_targets WHEN OLD.tool='cursor' BEGIN SELECT RAISE(FAIL, 'injected failure'); END;"
    ).unwrap();
    assert_eq!(
        f.service
            .undeploy(DeploymentRequest::global("demo", ["codex", "cursor"]))
            .unwrap_err()
            .code,
        ErrorCode::InternalError
    );
    assert!(f.target("codex").exists());
    assert!(f.target("cursor").exists());
    assert_eq!(f.rows(), 2);
}

#[test]
fn project_scope_deploys_inside_requested_project() {
    let f = Fixture::new();
    let project = TempDir::new().unwrap();
    let mut request = DeploymentRequest::global("demo", ["cursor"]);
    request.scope = DeploymentScope::Project(project.path().to_path_buf());
    f.service.deploy(request).unwrap();
    assert!(project
        .path()
        .join(".agents/skills/demo/SKILL.md")
        .is_file());
    assert!(!f.target("cursor").exists());
}

#[test]
fn shared_project_agents_use_one_directory_and_all_affected_rows() {
    let f = Fixture::new();
    let project = TempDir::new().unwrap();
    let mut request = DeploymentRequest::global("demo", ["codex", "cursor"]);
    request.scope = DeploymentScope::Project(project.path().to_path_buf());
    let plan = f.service.plan_deploy(request.clone()).unwrap();
    assert_eq!(plan.targets.len(), 1);
    assert_eq!(plan.targets[0].agents, ["codex", "cursor"]);
    f.service.apply_deployment_plan(plan).unwrap();
    assert_eq!(f.rows(), 2);
    f.service.undeploy(request).unwrap();
    assert_eq!(f.rows(), 0);
    assert!(!project.path().join(".agents/skills/demo").exists());
}

#[test]
fn unsupported_project_agent_has_stable_error_before_any_target_is_written() {
    let f = Fixture::new();
    let project = TempDir::new().unwrap();
    let mut request = DeploymentRequest::global("demo", ["codex", "workbuddy"]);
    request.scope = DeploymentScope::Project(project.path().to_path_buf());
    assert_eq!(
        f.service.deploy(request).unwrap_err().code,
        ErrorCode::ProjectScopeUnsupported
    );
    assert!(!project.path().join(".agents").exists());
}

#[test]
fn repeated_deployment_keeps_target_ids_and_can_be_safely_removed() {
    let f = Fixture::new();
    let request = DeploymentRequest::global("demo", ["codex", "cursor"]);
    f.service.deploy(request.clone()).unwrap();
    let skill = f.service.show_skill("demo".into()).unwrap();
    let before = f.service.store().list_skill_targets(&skill.id).unwrap();
    f.service.deploy(request.clone()).unwrap();
    let after = f.service.store().list_skill_targets(&skill.id).unwrap();
    assert_eq!(
        before.iter().map(|r| &r.id).collect::<Vec<_>>(),
        after.iter().map(|r| &r.id).collect::<Vec<_>>()
    );
    f.service.undeploy(request).unwrap();
    assert_eq!(f.rows(), 0);
}

#[test]
fn stale_plan_detects_source_and_database_changes() {
    let f = Fixture::new();
    let plan = f
        .service
        .plan_deploy(DeploymentRequest::global("demo", ["cursor"]))
        .unwrap();
    let skill = f.service.show_skill("demo".into()).unwrap();
    fs::write(PathBuf::from(skill.central_path).join("new.txt"), "changed").unwrap();
    assert_eq!(
        f.service.apply_deployment_plan(plan).unwrap_err().code,
        ErrorCode::PlanStale
    );
    assert!(!f.target("cursor").exists());
    let plan = f
        .service
        .plan_deploy(DeploymentRequest::global("demo", ["cursor"]))
        .unwrap();
    f.service
        .deploy(DeploymentRequest::global("demo", ["codex"]))
        .unwrap();
    assert_eq!(
        f.service.apply_deployment_plan(plan).unwrap_err().code,
        ErrorCode::PlanStale
    );
    assert!(!f.target("cursor").exists());
}

#[test]
fn undeploy_never_deletes_an_unmanaged_directory() {
    let f = Fixture::new();
    fs::create_dir_all(f.target("cursor")).unwrap();
    fs::write(f.target("cursor").join("user.txt"), "keep").unwrap();
    assert_eq!(
        f.service
            .undeploy(DeploymentRequest::global("demo", ["cursor"]))
            .unwrap_err()
            .code,
        ErrorCode::TargetConflict
    );
    assert_eq!(
        fs::read_to_string(f.target("cursor").join("user.txt")).unwrap(),
        "keep"
    );
}

#[cfg(unix)]
#[test]
fn preflight_rejects_unwritable_parent_without_partial_deployment() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new();
    let parent = f.home.path().join(".cursor");
    fs::set_permissions(&parent, fs::Permissions::from_mode(0o555)).unwrap();
    let error = f
        .service
        .deploy(DeploymentRequest::global("demo", ["codex", "cursor"]))
        .unwrap_err();
    fs::set_permissions(parent, fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(error.code, ErrorCode::TargetConflict);
    assert_eq!(error.details["reason"], "not_writable");
    assert!(!f.target("codex").exists());
    assert_eq!(f.rows(), 0);
}

#[test]
fn staged_activation_refuses_a_target_created_after_preparation() {
    use crate::core::sync_engine::{PreparedDeployment, SyncMode};
    let f = Fixture::new();
    let skill = f.service.show_skill("demo".into()).unwrap();
    let mut staged = PreparedDeployment::prepare(
        Some(std::path::Path::new(&skill.central_path)),
        &f.target("cursor"),
        SyncMode::Copy,
        None,
    )
    .unwrap();
    fs::create_dir_all(f.target("cursor")).unwrap();
    fs::write(f.target("cursor").join("user.txt"), "keep").unwrap();
    assert!(staged
        .activate()
        .unwrap_err()
        .to_string()
        .contains("PLAN_STALE"));
    drop(staged);
    assert_eq!(
        fs::read_to_string(f.target("cursor").join("user.txt")).unwrap(),
        "keep"
    );
}

#[test]
fn rollback_retains_backup_when_user_changes_an_activated_copy() {
    use crate::core::sync_engine::{deployment_fingerprint, PreparedDeployment, SyncMode};
    let f = Fixture::new();
    let skill = f.service.show_skill("demo".into()).unwrap();
    let target = f.target("cursor");
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("original.txt"), "original").unwrap();
    let expected = deployment_fingerprint(&target).unwrap();
    let mut staged = PreparedDeployment::prepare(
        Some(std::path::Path::new(&skill.central_path)),
        &target,
        SyncMode::Copy,
        expected,
    )
    .unwrap();
    staged.activate().unwrap();
    fs::write(target.join("user.txt"), "new user content").unwrap();
    assert!(staged
        .rollback()
        .unwrap_err()
        .to_string()
        .contains("ROLLBACK_CONFLICT"));
    drop(staged);
    assert_eq!(
        fs::read_to_string(target.join("user.txt")).unwrap(),
        "new user content"
    );
    let backup = fs::read_dir(target.parent().unwrap())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with(".skills-hub-backup-")
        })
        .unwrap();
    assert_eq!(
        fs::read_to_string(backup.join("original.txt")).unwrap(),
        "original"
    );
}

#[test]
fn deployment_checks_schema_before_the_operation_lock() {
    use crate::services::operation_lock::{OperationKind, OperationLock};
    let f = Fixture::new();
    let _lock = OperationLock::acquire(f.service.paths(), OperationKind::Install).unwrap();
    assert_eq!(
        f.service
            .deploy(DeploymentRequest::global("demo", ["cursor"]))
            .unwrap_err()
            .code,
        ErrorCode::OperationBusy
    );
    Connection::open(f.service.paths().database_path.clone())
        .unwrap()
        .execute_batch("PRAGMA user_version=99")
        .unwrap();
    assert_eq!(
        f.service
            .deploy(DeploymentRequest::global("demo", ["cursor"]))
            .unwrap_err()
            .code,
        ErrorCode::IncompatibleDatabase
    );
    assert!(!f.target("cursor").exists());
}

#[cfg(unix)]
#[test]
fn project_symlink_cannot_escape_the_project_root() {
    let f = Fixture::new();
    let project = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    std::os::unix::fs::symlink(outside.path(), project.path().join(".agents")).unwrap();
    let mut request = DeploymentRequest::global("demo", ["cursor"]);
    request.scope = DeploymentScope::Project(project.path().to_path_buf());
    assert_eq!(
        f.service.deploy(request).unwrap_err().code,
        ErrorCode::TargetConflict
    );
    assert!(!outside.path().join("skills/demo").exists());
}

#[test]
fn failure_record_does_not_turn_unmanaged_content_into_owned_content() {
    use crate::core::skill_store::SkillTargetRecord;
    let f = Fixture::new();
    let skill = f.service.show_skill("demo".into()).unwrap();
    fs::create_dir_all(f.target("cursor")).unwrap();
    fs::copy(
        PathBuf::from(&skill.central_path).join("SKILL.md"),
        f.target("cursor").join("SKILL.md"),
    )
    .unwrap();
    f.service
        .store()
        .upsert_skill_target(&SkillTargetRecord {
            id: "old-failure".into(),
            skill_id: skill.id,
            tool: "cursor".into(),
            scope: "global".into(),
            project_path: None,
            target_path: f.target("cursor").to_string_lossy().into_owned(),
            mode: "copy".into(),
            status: "error".into(),
            last_error: Some("TARGET_EXISTS".into()),
            synced_at: None,
        })
        .unwrap();
    assert_eq!(
        f.service
            .undeploy(DeploymentRequest::global("demo", ["cursor"]))
            .unwrap_err()
            .code,
        ErrorCode::TargetConflict
    );
    assert!(f.target("cursor").join("SKILL.md").is_file());
}

#[cfg(unix)]
#[test]
fn aliases_of_one_shared_directory_are_deployed_and_removed_once() {
    use crate::core::sync_engine::SyncMode;
    use crate::core::tool_adapters::{save_tool_config, CustomToolConfig, ToolConfig};
    let f = Fixture::new();
    let root = f.home.path().join("custom-tools");
    let alias = f.home.path().join("alias-tools");
    fs::create_dir_all(&root).unwrap();
    std::os::unix::fs::symlink(&root, &alias).unwrap();
    save_tool_config(
        f.service.store(),
        ToolConfig {
            disabled_builtin_tools: vec![],
            custom_tools: [("custom_first", root.clone()), ("custom_second", alias)]
                .into_iter()
                .map(|(key, path)| CustomToolConfig {
                    key: key.into(),
                    label: key.into(),
                    avatar: None,
                    skills_dir: path.to_string_lossy().into_owned(),
                    project_skills_dir: None,
                    sync_mode: SyncMode::Copy,
                    enabled: true,
                })
                .collect(),
        },
    )
    .unwrap();
    let request = DeploymentRequest::global("demo", ["custom_first", "custom_second"]);
    let plan = f.service.plan_deploy(request.clone()).unwrap();
    assert_eq!(plan.targets.len(), 1);
    f.service.apply_deployment_plan(plan).unwrap();
    assert_eq!(f.rows(), 2);
    f.service.undeploy(request).unwrap();
    assert!(!root.join("demo").exists());
    assert_eq!(f.rows(), 0);
}

#[test]
fn explicit_desktop_overwrite_remains_atomic_when_database_commit_fails() {
    let f = Fixture::new();
    for agent in ["codex", "cursor"] {
        fs::create_dir_all(f.target(agent)).unwrap();
        fs::write(f.target(agent).join("original.txt"), agent).unwrap();
    }
    Connection::open(f.service.paths().database_path.clone()).unwrap().execute_batch(
        "CREATE TRIGGER reject_deploy BEFORE INSERT ON skill_targets WHEN NEW.tool='cursor' BEGIN SELECT RAISE(FAIL, 'injected failure'); END;"
    ).unwrap();
    let mut request = DeploymentRequest::global("demo", ["codex", "cursor"]);
    request.overwrite = true;
    assert_eq!(
        f.service.deploy(request).unwrap_err().code,
        ErrorCode::InternalError
    );
    for agent in ["codex", "cursor"] {
        assert_eq!(
            fs::read_to_string(f.target(agent).join("original.txt")).unwrap(),
            agent
        );
        assert!(!f.target(agent).join("SKILL.md").exists());
    }
    assert_eq!(f.rows(), 0);
}

#[test]
fn copied_target_baseline_describes_the_materialized_copy() {
    let f = Fixture::new();
    let skill = f.service.show_skill("demo".into()).unwrap();
    let metadata = PathBuf::from(skill.central_path).join(".git");
    fs::create_dir_all(&metadata).unwrap();
    fs::write(metadata.join("config"), "local metadata").unwrap();
    f.service
        .deploy(DeploymentRequest::global("demo", ["cursor"]))
        .unwrap();
    assert!(!f.target("cursor").join(".git").exists());
    f.service
        .undeploy(DeploymentRequest::global("demo", ["cursor"]))
        .unwrap();
    assert!(!f.target("cursor").exists());
}

#[cfg(target_os = "macos")]
#[test]
fn preflight_checks_directory_acl_before_preparing_any_target() {
    let f = Fixture::new();
    let parent = f.home.path().join(".cursor");
    assert!(std::process::Command::new("/bin/chmod")
        .args(["+a", "everyone deny add_file,add_subdirectory,delete_child"])
        .arg(&parent)
        .status()
        .unwrap()
        .success());
    let result = f
        .service
        .deploy(DeploymentRequest::global("demo", ["codex", "cursor"]));
    assert!(std::process::Command::new("/bin/chmod")
        .arg("-N")
        .arg(&parent)
        .status()
        .unwrap()
        .success());
    let error = result.unwrap_err();
    assert_eq!(error.code, ErrorCode::TargetConflict);
    assert_eq!(error.details["reason"], "not_writable");
    assert!(!f.home.path().join(".codex/skills").exists());
}

#[test]
fn shared_directory_rejects_incompatible_explicit_modes_before_writing() {
    use crate::core::sync_engine::SyncMode;
    use crate::core::tool_adapters::{save_tool_config, CustomToolConfig, ToolConfig};
    let f = Fixture::new();
    let root = f.home.path().join("custom-tools");
    save_tool_config(
        f.service.store(),
        ToolConfig {
            disabled_builtin_tools: vec![],
            custom_tools: [
                ("custom_first", SyncMode::Copy),
                ("custom_second", SyncMode::Symlink),
            ]
            .into_iter()
            .map(|(key, sync_mode)| CustomToolConfig {
                key: key.into(),
                label: key.into(),
                avatar: None,
                skills_dir: root.to_string_lossy().into_owned(),
                project_skills_dir: None,
                sync_mode,
                enabled: true,
            })
            .collect(),
        },
    )
    .unwrap();
    let error = f
        .service
        .deploy(DeploymentRequest::global(
            "demo",
            ["custom_first", "custom_second"],
        ))
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::TargetConflict);
    assert_eq!(error.details["reason"], "incompatible_shared_modes");
    assert!(!root.join("demo").exists());
    assert_eq!(f.rows(), 0);
}
