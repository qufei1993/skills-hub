use std::fs;
use std::path::PathBuf;

use tempfile::TempDir;

use crate::core::content_hash::hash_dir_strict;
use crate::core::runtime_paths::{RuntimePaths, RuntimeProfile};
use crate::services::deployment::{DeploymentRequest, DeploymentScope};
use crate::services::error::ErrorCode;
use crate::services::skills_hub::SkillsHubService;
use crate::services::source_conflict::SourceConflictChoice::{Central, Tool};

struct Fixture {
    _home: TempDir,
    _data: TempDir,
    paths: RuntimePaths,
    service: SkillsHubService,
    source: PathBuf,
    central: PathBuf,
    id: String,
}
impl Fixture {
    fn new() -> Self {
        let home = TempDir::new().unwrap();
        let data = TempDir::new().unwrap();
        let paths = RuntimePaths::from_roots(RuntimeProfile::Test, home.path(), data.path());
        let service = SkillsHubService::open(paths.clone()).unwrap();
        let source = home.path().join(".codex/skills/demo");
        fs::create_dir_all(&source).unwrap();
        fs::write(
            source.join("SKILL.md"),
            "---\nname: demo\ndescription: Central description\n---\ncentral version\n",
        )
        .unwrap();
        fs::write(source.join("central-only.txt"), "central content").unwrap();
        let installed = service
            .import_discovered_skill(&source, Some("demo".into()))
            .unwrap()
            .unwrap();
        for agent in [".claude", ".cursor"] {
            fs::create_dir_all(home.path().join(agent)).unwrap();
        }
        service
            .deploy(DeploymentRequest::global("demo", ["claude_code", "cursor"]))
            .unwrap();
        fs::write(
            source.join("SKILL.md"),
            "---\nname: demo\ndescription: Tool description\n---\ntool version\n",
        )
        .unwrap();
        fs::remove_file(source.join("central-only.txt")).unwrap();
        fs::write(source.join("tool-only.txt"), "tool extra").unwrap();
        fs::write(source.join("binary.bin"), [0, 255, 1, 0]).unwrap();
        fs::create_dir_all(source.join(".git")).unwrap();
        fs::write(source.join(".git/config"), "local metadata").unwrap();
        Self {
            _home: home,
            _data: data,
            central: PathBuf::from(&installed.central_path),
            source,
            id: installed.id,
            paths,
            service,
        }
    }
    fn target(&self, agent: &str) -> PathBuf {
        self._home.path().join(format!(".{agent}/skills/demo"))
    }
    fn preview(&self) -> crate::services::source_conflict::SourceConflictPreview {
        self.service
            .preview_source_conflict(self.source.to_str().unwrap())
            .unwrap()
    }
}

#[test]
fn source_conflict_preview_is_read_only_and_labels_complete_differences() {
    let f = Fixture::new();
    let before = f.service.store().get_skill_by_id(&f.id).unwrap();
    let central = hash_dir_strict(&f.central).unwrap();
    let source = hash_dir_strict(&f.source).unwrap();
    let preview = f.preview();
    assert!(preview.central.blocked.is_none());
    assert!(preview.tool.blocked.is_none());
    assert_eq!(preview.central.targets.len(), 1);
    assert_eq!(preview.tool.targets.len(), 3);
    let manifest = preview
        .changes
        .iter()
        .find(|change| change.path == "SKILL.md")
        .unwrap();
    assert!(manifest
        .before
        .as_ref()
        .unwrap()
        .contains("central version"));
    assert!(manifest.after.as_ref().unwrap().contains("tool version"));
    assert!(preview
        .changes
        .iter()
        .any(|change| change.path == ".git/config"));
    assert!(preview
        .changes
        .iter()
        .any(|change| change.path == "central-only.txt" && change.kind == "removed"));
    assert!(preview
        .changes
        .iter()
        .any(|change| change.path == "binary.bin" && change.after.is_none()));
    assert_eq!(hash_dir_strict(&f.central).unwrap(), central);
    assert_eq!(hash_dir_strict(&f.source).unwrap(), source);
    assert_eq!(f.service.store().get_skill_by_id(&f.id).unwrap(), before);
    assert!(!f
        .paths
        .app_data_dir
        .join("source-conflict-backups")
        .exists());
}

#[test]
fn source_conflict_keep_central_takes_over_source_without_persistent_backups() {
    let f = Fixture::new();
    let old_central = hash_dir_strict(&f.central).unwrap();
    let preview = f.preview();
    let result = f
        .service
        .resolve_source_conflict(&preview.id, Central)
        .unwrap();
    assert_eq!(hash_dir_strict(&f.central).unwrap(), old_central);
    assert!(f.source.join("central-only.txt").exists());
    assert!(!f.source.join("tool-only.txt").exists());
    let record = f.service.store().get_skill_by_id(&f.id).unwrap().unwrap();
    assert_eq!(
        record.source_ref.as_deref(),
        Some(record.central_path.as_str())
    );
    assert_eq!(
        f.service.show_skill("demo".into()).unwrap().targets.len(),
        3
    );
    let reopened = SkillsHubService::open(f.paths.clone()).unwrap();
    assert_eq!(
        reopened
            .resolve_source_conflict(&preview.id, Central)
            .unwrap()
            .id,
        result.id
    );
    assert_eq!(
        reopened
            .resolve_source_conflict(&preview.id, Tool)
            .unwrap_err()
            .code,
        ErrorCode::PlanStale
    );
}

#[test]
fn source_conflict_keep_tool_updates_central_links_and_copies_in_one_commit() {
    let f = Fixture::new();
    let original = hash_dir_strict(&f.source).unwrap();
    let preview = f.preview();
    f.service
        .resolve_source_conflict(&preview.id, Tool)
        .unwrap();
    assert_eq!(hash_dir_strict(&f.central).unwrap(), original);
    for path in [&f.source, &f.target("claude"), &f.target("cursor")] {
        assert!(fs::read_to_string(path.join("SKILL.md"))
            .unwrap()
            .contains("tool version"));
        assert_eq!(
            fs::read_to_string(path.join("tool-only.txt")).unwrap(),
            "tool extra"
        );
        assert!(!path.join("central-only.txt").exists());
    }
    assert!(f.central.join(".git/config").exists());
    let record = f.service.store().get_skill_by_id(&f.id).unwrap().unwrap();
    assert_eq!(record.description.as_deref(), Some("Tool description"));
    assert!(record.external_local_source().is_none());
    assert!(!f
        .paths
        .app_data_dir
        .join("source-conflict-backups")
        .exists());
    f.service
        .undeploy(DeploymentRequest::global("demo", ["codex"]))
        .unwrap();
    f.service
        .deploy(DeploymentRequest::global("demo", ["codex"]))
        .unwrap();
}

#[test]
fn source_conflict_modified_other_copy_blocks_tool_choice_but_not_central_choice() {
    let f = Fixture::new();
    fs::write(
        f.target("cursor").join("SKILL.md"),
        "independent user changes",
    )
    .unwrap();
    let preview = f.preview();
    assert!(preview.central.blocked.is_none());
    assert!(preview.tool.blocked.is_some());
    assert_eq!(
        f.service
            .resolve_source_conflict(&preview.id, Tool)
            .unwrap_err()
            .code,
        ErrorCode::TargetConflict
    );
    f.service
        .resolve_source_conflict(&preview.id, Central)
        .unwrap();
    assert_eq!(
        fs::read_to_string(f.target("cursor").join("SKILL.md")).unwrap(),
        "independent user changes"
    );
}

#[test]
fn source_conflict_rejects_stale_content_and_deployments_before_mutation() {
    for changed in ["central", "tool", "target"] {
        let f = Fixture::new();
        let preview = f.preview();
        let path = match changed {
            "central" => f.central.clone(),
            "tool" => f.source.clone(),
            _ => f.target("cursor"),
        };
        fs::write(path.join("new-user-file"), "concurrent edit").unwrap();
        assert_eq!(
            f.service
                .resolve_source_conflict(&preview.id, Central)
                .unwrap_err()
                .code,
            ErrorCode::PlanStale
        );
        assert!(!fs::symlink_metadata(&f.source)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(
            fs::read_to_string(path.join("new-user-file")).unwrap(),
            "concurrent edit"
        );
    }
}

#[test]
fn source_conflict_database_failure_restores_central_source_all_targets_and_metadata() {
    let f = Fixture::new();
    let record = f.service.store().get_skill_by_id(&f.id).unwrap();
    let rows = f.service.store().list_skill_targets(&f.id).unwrap();
    let before = [&f.central, &f.source, &f.target("cursor")]
        .iter()
        .map(|path| hash_dir_strict(path).unwrap())
        .collect::<Vec<_>>();
    let preview = f.preview();
    rusqlite::Connection::open(&f.paths.database_path).unwrap().execute_batch("CREATE TRIGGER fail_resolution BEFORE INSERT ON workspace_documents WHEN NEW.kind='source-conflict-result' BEGIN SELECT RAISE(FAIL, 'injected failure'); END;").unwrap();
    let error = f
        .service
        .resolve_source_conflict(&preview.id, Tool)
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::InternalError);
    assert_eq!(error.details["recovery"], serde_json::json!([]));
    let after = [&f.central, &f.source, &f.target("cursor")]
        .iter()
        .map(|path| hash_dir_strict(path).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(before, after);
    assert_eq!(f.service.store().get_skill_by_id(&f.id).unwrap(), record);
    assert_eq!(f.service.store().list_skill_targets(&f.id).unwrap(), rows);
    assert!(f
        .service
        .store()
        .workspace_list::<serde_json::Value>("source-conflict-backup")
        .unwrap()
        .is_empty());
    assert!(error.details.get("backupPath").is_none());
    assert!(!f
        .paths
        .app_data_dir
        .join("source-conflict-backups")
        .exists());
}

#[cfg(unix)]
#[test]
fn source_conflict_snapshots_links_without_following_them_and_detects_concurrent_writes() {
    use crate::core::sync_engine::{set_deployment_race_hook, DeploymentRacePoint};
    let f = Fixture::new();
    let outside = TempDir::new().unwrap();
    fs::write(
        outside.path().join("external"),
        "must not be copied or modified",
    )
    .unwrap();
    std::os::unix::fs::symlink(
        outside.path().join("external"),
        f.source.join("external-link"),
    )
    .unwrap();
    let preview = f.preview();
    assert!(preview
        .changes
        .iter()
        .any(|change| change.path == "external-link"
            && change.after_metadata.as_ref().unwrap().kind == "symlink"));
    let parent = f.source.parent().unwrap().to_path_buf();
    set_deployment_race_hook(DeploymentRacePoint::BeforeBaselineRead, move || {
        let backup = fs::read_dir(parent)
            .unwrap()
            .map(|item| item.unwrap().path())
            .find(|path| {
                path.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with(".skills-hub-backup-")
            })
            .unwrap();
        fs::write(backup.join("new-user-file"), "write during activation").unwrap();
    });
    assert_eq!(
        f.service
            .resolve_source_conflict(&preview.id, Tool)
            .unwrap_err()
            .code,
        ErrorCode::PlanStale
    );
    assert_eq!(
        fs::read_to_string(f.source.join("new-user-file")).unwrap(),
        "write during activation"
    );
    assert_eq!(
        fs::read_to_string(outside.path().join("external")).unwrap(),
        "must not be copied or modified"
    );
}

#[test]
fn source_conflict_never_takes_over_a_source_shared_with_another_skill() {
    let f = Fixture::new();
    let mut other = f.service.store().get_skill_by_id(&f.id).unwrap().unwrap();
    other.id = "other-skill".into();
    other.name = "other".into();
    other.central_path = f
        .paths
        .default_central_repo
        .join("other")
        .to_string_lossy()
        .into_owned();
    f.service.store().upsert_skill(&other).unwrap();
    assert_eq!(
        f.service
            .preview_source_conflict(f.source.to_str().unwrap())
            .unwrap_err()
            .code,
        ErrorCode::TargetConflict
    );
}

#[test]
fn source_conflict_does_not_replace_central_content_used_as_another_skills_source() {
    let f = Fixture::new();
    let mut other = f.service.store().get_skill_by_id(&f.id).unwrap().unwrap();
    other.id = "nested-source-owner".into();
    other.name = "nested-owner".into();
    other.central_path = f
        .paths
        .default_central_repo
        .join("nested-owner")
        .to_string_lossy()
        .into_owned();
    other.source_ref = Some(f.central.join("nested").to_string_lossy().into_owned());
    f.service.store().upsert_skill(&other).unwrap();
    let preview = f.preview();
    assert!(preview.central.blocked.is_none());
    assert!(preview.tool.blocked.is_some());
    assert_eq!(
        f.service
            .resolve_source_conflict(&preview.id, Tool)
            .unwrap_err()
            .code,
        ErrorCode::TargetConflict
    );
}

#[cfg(unix)]
#[test]
fn source_conflict_aborts_when_selected_content_cannot_be_staged() {
    let f = Fixture::new();
    let _socket = std::os::unix::net::UnixListener::bind(f.source.join("local.sock")).unwrap();
    let preview = f.preview();
    let source_hash = hash_dir_strict(&f.source).unwrap();
    let central_hash = hash_dir_strict(&f.central).unwrap();
    let error = f
        .service
        .resolve_source_conflict(&preview.id, Tool)
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::InternalError);
    assert_eq!(hash_dir_strict(&f.source).unwrap(), source_hash);
    assert_eq!(hash_dir_strict(&f.central).unwrap(), central_hash);
    assert!(!fs::symlink_metadata(&f.source)
        .unwrap()
        .file_type()
        .is_symlink());
}

#[cfg(unix)]
#[test]
fn source_conflict_does_not_write_through_a_redirected_central_parent() {
    use crate::core::sync_engine::{set_deployment_race_hook, DeploymentRacePoint};
    let f = Fixture::new();
    let outside = TempDir::new().unwrap();
    fs::create_dir(outside.path().join("demo")).unwrap();
    fs::write(
        outside.path().join("demo/SKILL.md"),
        "unrelated protected files",
    )
    .unwrap();
    let preview = f.preview();
    let parent = f.central.parent().unwrap().to_path_buf();
    let displaced = parent.with_extension("displaced");
    let other = outside.path().to_path_buf();
    set_deployment_race_hook(DeploymentRacePoint::StagingWrite, move || {
        fs::rename(&parent, displaced).unwrap();
        std::os::unix::fs::symlink(other, parent).unwrap();
    });
    assert_eq!(
        f.service
            .resolve_source_conflict(&preview.id, Tool)
            .unwrap_err()
            .code,
        ErrorCode::PlanStale
    );
    assert_eq!(
        fs::read_to_string(outside.path().join("demo/SKILL.md")).unwrap(),
        "unrelated protected files"
    );
    assert_eq!(fs::read_dir(outside.path()).unwrap().count(), 1);
    assert!(!fs::symlink_metadata(&f.source)
        .unwrap()
        .file_type()
        .is_symlink());
}

#[cfg(unix)]
#[test]
fn source_conflict_updates_existing_project_copy_alongside_global_deployments() {
    let f = Fixture::new();
    let project = TempDir::new().unwrap();
    let mut request = DeploymentRequest::global("demo", ["cursor"]);
    request.scope = DeploymentScope::Project(project.path().into());
    f.service.deploy(request).unwrap();
    let preview = f.preview();
    assert_eq!(preview.tool.targets.len(), 4);
    f.service
        .resolve_source_conflict(&preview.id, Tool)
        .unwrap();
    assert!(
        fs::read_to_string(project.path().join(".agents/skills/demo/SKILL.md"))
            .unwrap()
            .contains("tool version")
    );
}

#[test]
fn source_conflict_does_not_create_persistent_backups_or_change_existing_ones() {
    for choice in [Central, Tool] {
        let f = Fixture::new();
        let root = f.paths.app_data_dir.join("source-conflict-backups");
        let existing = root.join("existing-backup");
        fs::create_dir_all(&existing).unwrap();
        fs::write(existing.join("SKILL.md"), "previously saved content").unwrap();
        let saved = serde_json::json!({"id":"existing-backup", "backupPath":existing});
        f.service
            .store()
            .workspace_save("source-conflict-backup", "existing-backup", &saved)
            .unwrap();
        let preview = f.preview();
        f.service
            .resolve_source_conflict(&preview.id, choice)
            .unwrap();
        assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
        assert_eq!(
            fs::read_to_string(existing.join("SKILL.md")).unwrap(),
            "previously saved content"
        );
        assert_eq!(
            f.service
                .store()
                .workspace_list::<serde_json::Value>("source-conflict-backup")
                .unwrap(),
            vec![saved]
        );
    }
}
