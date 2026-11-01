use std::fs;
use std::path::{Path, PathBuf};

use tempfile::TempDir;

use crate::core::runtime_paths::{RuntimePaths, RuntimeProfile};
use crate::core::skill_store::{SkillRecord, SkillTargetRecord};
use crate::services::error::ErrorCode;
use crate::services::library::{
    classify_direct_manifest_stat, map_batch_adopt_error, set_adopt_after_revalidation_hook,
    AdoptRequest, RemoveRequest, TagAction, TagSelector,
};
use crate::services::skills_hub::SkillsHubService;

struct Fixture {
    _home: TempDir,
    _data: TempDir,
    service: SkillsHubService,
    paths: RuntimePaths,
}

impl Fixture {
    fn new() -> Self {
        let home = TempDir::new().unwrap();
        let data = TempDir::new().unwrap();
        let paths = RuntimePaths::from_roots(RuntimeProfile::Test, home.path(), data.path());
        let service = SkillsHubService::open(paths.clone()).unwrap();
        Self {
            _home: home,
            _data: data,
            service,
            paths,
        }
    }

    fn add_skill(&self, id: &str, name: &str, body: &str) -> SkillRecord {
        let central = self.paths.default_central_repo.join(name);
        write_skill(&central, name, body);
        let record = SkillRecord {
            id: id.into(),
            name: name.into(),
            description: Some(format!("{name} description")),
            source_type: "git".into(),
            source_ref: Some(format!("https://example.test/{name}.git")),
            source_subpath: Some(format!("skills/{name}")),
            source_revision: Some("abc123".into()),
            central_path: central.to_string_lossy().into_owned(),
            content_hash: crate::core::content_hash::hash_dir(&central).ok(),
            created_at: 10,
            updated_at: 20,
            last_sync_at: Some(30),
            last_seen_at: 40,
            enabled: true,
            status: "ok".into(),
        };
        self.service.store().upsert_skill(&record).unwrap();
        record
    }

    fn central_skill_exists(&self, name: &str) -> bool {
        self.paths.default_central_repo.join(name).exists()
    }

    fn adopt_codex_skill(&self) -> (PathBuf, SkillRecord) {
        let source = self._home.path().join(".codex/skills/demo");
        write_skill(&source, "demo", "original");
        let installed = self
            .service
            .import_discovered_skill(&source, Some("demo".into()))
            .unwrap()
            .unwrap();
        let record = self
            .service
            .store()
            .get_skill_by_id(&installed.id)
            .unwrap()
            .unwrap();
        (source, record)
    }
}

fn write_skill(path: &Path, name: &str, body: &str) {
    fs::create_dir_all(path).unwrap();
    fs::write(
        path.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: Test\n---\n\n{body}\n"),
    )
    .unwrap();
}

#[test]
fn discovery_excludes_adopted_local_sources_but_keeps_unmanaged_skills() {
    use crate::core::onboarding::build_onboarding_plan_for_runtime;
    let fixture = Fixture::new();
    let source = fixture._home.path().join(".codex/skills/demo");
    write_skill(&source, "demo", "original");
    let plan = fixture
        .service
        .plan_adopt_direct_with_name(&source, Some("demo".into()))
        .unwrap();
    fixture
        .service
        .adopt(AdoptRequest::confirmed(plan.id))
        .unwrap();
    write_skill(
        &fixture._home.path().join(".codex/skills/new-skill"),
        "new-skill",
        "new content",
    );

    let plan = build_onboarding_plan_for_runtime(
        &fixture.paths,
        fixture.service.store(),
        fixture._home.path(),
    )
    .unwrap();
    assert_eq!(plan.total_skills_found, 1);
    assert_eq!(plan.groups.len(), 1);
    assert_eq!(plan.groups[0].name, "new-skill");
}

#[cfg(any(unix, windows))]
#[test]
fn discovery_excludes_managed_source_aliases_across_tools() {
    use crate::core::onboarding::build_onboarding_plan_for_runtime;
    let fixture = Fixture::new();
    let source = fixture._home.path().join(".codex/skills/demo");
    write_skill(&source, "demo", "original");
    fixture
        .service
        .import_discovered_skill(&source, Some("demo".into()))
        .unwrap();
    let alias = fixture._home.path().join(".cursor/skills/alias");
    fs::create_dir_all(alias.parent().unwrap()).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&source, &alias).unwrap();
    #[cfg(windows)]
    junction::create(&source, &alias).unwrap();

    let plan = build_onboarding_plan_for_runtime(
        &fixture.paths,
        fixture.service.store(),
        fixture._home.path(),
    )
    .unwrap();
    assert_eq!(plan.total_skills_found, 0);
    assert!(plan.groups.is_empty());
    assert!(source.join("SKILL.md").is_file());
    assert!(alias.join("SKILL.md").is_file());
}

#[test]
fn direct_adopt_of_a_managed_source_is_an_empty_plan_even_with_a_name() {
    let fixture = Fixture::new();
    let source = fixture.paths.app_data_dir.join("original/demo");
    write_skill(&source, "demo", "original");
    let plan = fixture
        .service
        .plan_adopt_direct_with_name(&source, Some("demo".into()))
        .unwrap();
    fixture
        .service
        .adopt(AdoptRequest::confirmed(plan.id))
        .unwrap();

    let plan = fixture
        .service
        .plan_adopt_direct_with_name(&source, Some("demo".into()))
        .unwrap();
    assert!(plan.candidates.is_empty());
    assert_eq!(plan.excluded.len(), 1);
    assert_eq!(plan.excluded[0].reason, "managed_source");
}

#[test]
fn repeated_discovery_import_preserves_source_library_metadata_and_deployments() {
    use crate::core::content_hash::hash_dir_strict;
    use crate::services::deployment::DeploymentRequest;
    let fixture = Fixture::new();
    let source = fixture._home.path().join(".codex/skills/demo");
    write_skill(&source, "demo", "original");
    fs::create_dir_all(fixture._home.path().join(".claude")).unwrap();
    let installed = fixture
        .service
        .import_discovered_skill(&source, Some("demo".into()))
        .unwrap()
        .unwrap();
    fixture
        .service
        .store()
        .set_skill_tag_names(&installed.id, &["saved tag".into()])
        .unwrap();
    fixture
        .service
        .deploy(DeploymentRequest::global("demo", ["claude_code"]))
        .unwrap();
    let before = fixture.service.show_skill("demo".into()).unwrap();
    let records = fixture.service.store().list_skills().unwrap();
    let source_before = hash_dir_strict(&source).unwrap();
    let central_before = hash_dir_strict(Path::new(&installed.central_path)).unwrap();
    let target_hashes = before
        .targets
        .iter()
        .map(|target| hash_dir_strict(Path::new(&target.target_path)).unwrap())
        .collect::<Vec<_>>();

    for name in ["demo", "another-name"] {
        assert!(fixture
            .service
            .import_discovered_skill(&source, Some(name.into()))
            .unwrap()
            .is_none());
    }
    assert_eq!(fixture.service.store().list_skills().unwrap(), records);
    assert_eq!(fixture.service.show_skill("demo".into()).unwrap(), before);
    assert_eq!(hash_dir_strict(&source).unwrap(), source_before);
    assert_eq!(
        hash_dir_strict(Path::new(&installed.central_path)).unwrap(),
        central_before
    );
    for (target, hash) in before.targets.iter().zip(target_hashes) {
        assert_eq!(
            hash_dir_strict(Path::new(&target.target_path)).unwrap(),
            hash
        );
        assert_eq!(
            fs::read(Path::new(&target.target_path).join("SKILL.md")).unwrap(),
            fs::read(source.join("SKILL.md")).unwrap()
        );
    }
    assert!(fixture
        .service
        .store()
        .list_recycle_bin_rows()
        .unwrap()
        .is_empty());
}

#[test]
fn discovery_import_does_not_hide_invalid_sources_or_unmanaged_name_conflicts() {
    let fixture = Fixture::new();
    fixture.add_skill("existing", "demo", "saved");
    let source = fixture.paths.app_data_dir.join("different/demo");
    write_skill(&source, "demo", "different content");
    let error = fixture
        .service
        .import_discovered_skill(&source, Some("demo".into()))
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::TargetConflict);
    fs::remove_file(source.join("SKILL.md")).unwrap();
    let error = fixture
        .service
        .import_discovered_skill(&source, Some("renamed".into()))
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidSource);
    assert_eq!(fixture.service.store().list_skills().unwrap().len(), 1);
}

#[test]
fn discovery_import_takes_over_identical_source_when_syncing_back_to_its_tool() {
    use crate::services::deployment::DeploymentRequest;
    let fixture = Fixture::new();
    let source = fixture._home.path().join(".codex/skills/demo");
    write_skill(&source, "demo", "original");
    fs::create_dir_all(fixture._home.path().join(".claude")).unwrap();
    let installed = fixture
        .service
        .import_discovered_skill(&source, Some("demo".into()))
        .unwrap()
        .unwrap();
    let request = DeploymentRequest::global("demo", ["codex"]);
    let before = fixture.service.store().list_skills().unwrap();
    let preview = fixture.service.plan_deploy(request.clone()).unwrap();
    assert_eq!(fixture.service.store().list_skills().unwrap(), before);
    assert!(!fs::symlink_metadata(&source)
        .unwrap()
        .file_type()
        .is_symlink());
    fixture.service.apply_deployment_plan(preview).unwrap();
    let record = fixture
        .service
        .store()
        .get_skill_by_id(&installed.id)
        .unwrap()
        .unwrap();
    assert_eq!(
        record.source_ref.as_deref(),
        Some(installed.central_path.as_str())
    );
    assert!(record.external_local_source().is_none());
    let outcome = fixture.service.deploy(request.clone()).unwrap();
    if outcome.targets[0].mode != crate::core::sync_engine::SyncMode::Copy {
        assert_eq!(
            fs::canonicalize(&source).unwrap(),
            fs::canonicalize(&installed.central_path).unwrap()
        );
        fs::write(
            Path::new(&installed.central_path).join("SKILL.md"),
            "updated centrally",
        )
        .unwrap();
        assert_eq!(
            fs::read_to_string(source.join("SKILL.md")).unwrap(),
            "updated centrally"
        );
    }
    fixture.service.undeploy(request.clone()).unwrap();
    assert!(!source.exists());
    assert!(Path::new(&installed.central_path).join("SKILL.md").exists());
    fixture.service.deploy(request).unwrap();
    fixture
        .service
        .deploy(DeploymentRequest::global("demo", ["claude_code"]))
        .unwrap();
    assert_eq!(
        fixture
            .service
            .show_skill("demo".into())
            .unwrap()
            .targets
            .len(),
        2
    );
}

#[test]
fn source_takeover_preserves_different_content_even_with_explicit_overwrite() {
    use crate::core::content_hash::hash_dir_strict;
    use crate::services::deployment::DeploymentRequest;
    for relative in ["SKILL.md", ".git/config", "__pycache__/demo.pyc"] {
        let fixture = Fixture::new();
        let (source, record) = fixture.adopt_codex_skill();
        let changed = source.join(relative);
        fs::create_dir_all(changed.parent().unwrap()).unwrap();
        fs::write(changed, "user edits that must survive").unwrap();
        let before = hash_dir_strict(&source).unwrap();
        let mut request = DeploymentRequest::global("demo", ["codex"]);
        request.overwrite = true;
        request.overwrite_if_same_content = true;
        let error = fixture.service.deploy(request).unwrap_err();
        assert_eq!(error.code, ErrorCode::TargetConflict);
        assert_eq!(error.details["reason"], "source_content_differs");
        assert_eq!(hash_dir_strict(&source).unwrap(), before);
        assert!(!fs::symlink_metadata(&source)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(
            fixture.service.store().get_skill_by_id(&record.id).unwrap(),
            Some(record.clone())
        );
        assert!(fixture
            .service
            .store()
            .list_skill_targets(&record.id)
            .unwrap()
            .is_empty());
    }
}

#[test]
fn source_takeover_never_bypasses_other_protected_paths() {
    use crate::services::deployment::DeploymentRequest;
    for overlap in ["parent", "child", "another_skill", "central"] {
        let fixture = Fixture::new();
        let (source, mut record) = fixture.adopt_codex_skill();
        match overlap {
            "parent" => {
                record.source_ref = Some(source.parent().unwrap().to_string_lossy().into_owned())
            }
            "child" => {
                record.source_ref = Some(source.join("nested").to_string_lossy().into_owned())
            }
            "another_skill" => {
                let mut other = fixture.add_skill("other", "other", "original");
                other.source_type = "local".into();
                other.source_ref = Some(source.to_string_lossy().into_owned());
                fixture.service.store().upsert_skill(&other).unwrap();
            }
            "central" => record.central_path = source.to_string_lossy().into_owned(),
            _ => unreachable!(),
        }
        fixture.service.store().upsert_skill(&record).unwrap();
        let mut request = DeploymentRequest::global("demo", ["codex"]);
        request.overwrite = true;
        let error = fixture.service.deploy(request).unwrap_err();
        assert_eq!(error.code, ErrorCode::TargetConflict);
        assert_eq!(
            error.details["reason"], "overlaps_skill_source",
            "{overlap}"
        );
        assert!(!fs::symlink_metadata(&source)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(
            fixture.service.store().get_skill_by_id(&record.id).unwrap(),
            Some(record)
        );
    }
}

#[test]
fn source_takeover_rolls_back_files_targets_and_source_binding_on_commit_failure() {
    use crate::core::content_hash::{hash_dir, hash_dir_strict};
    use crate::services::deployment::DeploymentRequest;
    let fixture = Fixture::new();
    let (source, record) = fixture.adopt_codex_skill();
    fs::create_dir_all(fixture._home.path().join(".claude")).unwrap();
    fixture
        .service
        .store()
        .record_source_check_success(&record, hash_dir(&source).unwrap())
        .unwrap();
    let record = fixture
        .service
        .store()
        .get_skill_by_id(&record.id)
        .unwrap()
        .unwrap();
    let baseline = fixture.service.store().source_baseline(&record.id).unwrap();
    let checks = fixture.service.store().source_checks().unwrap();
    let before = hash_dir_strict(&source).unwrap();
    rusqlite::Connection::open(&fixture.paths.database_path).unwrap().execute_batch(
        "CREATE TRIGGER reject_takeover BEFORE UPDATE OF source_ref ON skills BEGIN SELECT RAISE(FAIL, 'injected takeover failure'); END;"
    ).unwrap();
    let error = fixture
        .service
        .deploy(DeploymentRequest::global("demo", ["codex", "claude_code"]))
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::InternalError);
    assert_eq!(error.details["stage"], "commit_records");
    assert_eq!(error.details["recovery"], serde_json::json!([]));
    assert!(!fs::symlink_metadata(&source)
        .unwrap()
        .file_type()
        .is_symlink());
    assert_eq!(hash_dir_strict(&source).unwrap(), before);
    assert_eq!(
        hash_dir_strict(Path::new(&record.central_path)).unwrap(),
        before
    );
    assert!(!fixture._home.path().join(".claude/skills/demo").exists());
    assert!(fixture
        .service
        .store()
        .list_skill_targets(&record.id)
        .unwrap()
        .is_empty());
    assert_eq!(
        fixture.service.store().get_skill_by_id(&record.id).unwrap(),
        Some(record.clone())
    );
    assert_eq!(
        serde_json::to_value(fixture.service.store().source_baseline(&record.id).unwrap()).unwrap(),
        serde_json::to_value(baseline).unwrap()
    );
    assert_eq!(fixture.service.store().source_checks().unwrap(), checks);
}

#[test]
fn source_takeover_rejects_edits_after_preview_without_changing_the_binding() {
    use crate::services::deployment::DeploymentRequest;
    let fixture = Fixture::new();
    let (source, record) = fixture.adopt_codex_skill();
    let plan = fixture
        .service
        .plan_deploy(DeploymentRequest::global("demo", ["codex"]))
        .unwrap();
    fs::write(source.join("SKILL.md"), "edited after preview").unwrap();
    assert_eq!(
        fixture
            .service
            .apply_deployment_plan(plan)
            .unwrap_err()
            .code,
        ErrorCode::PlanStale
    );
    assert_eq!(
        fs::read_to_string(source.join("SKILL.md")).unwrap(),
        "edited after preview"
    );
    assert_eq!(
        fixture.service.store().get_skill_by_id(&record.id).unwrap(),
        Some(record)
    );
}

#[test]
fn source_takeover_preserves_writes_to_the_original_during_activation() {
    use crate::core::sync_engine::{set_deployment_race_hook, DeploymentRacePoint};
    use crate::services::deployment::DeploymentRequest;
    let fixture = Fixture::new();
    let (source, record) = fixture.adopt_codex_skill();
    let parent = source.parent().unwrap().to_path_buf();
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
        fs::write(backup.join("SKILL.md"), "concurrent source edit").unwrap();
    });
    assert_eq!(
        fixture
            .service
            .deploy(DeploymentRequest::global("demo", ["codex"]))
            .unwrap_err()
            .code,
        ErrorCode::PlanStale
    );
    assert!(!fs::symlink_metadata(&source)
        .unwrap()
        .file_type()
        .is_symlink());
    assert_eq!(
        fs::read_to_string(source.join("SKILL.md")).unwrap(),
        "concurrent source edit"
    );
    assert_eq!(
        fixture.service.store().get_skill_by_id(&record.id).unwrap(),
        Some(record.clone())
    );
    assert!(fixture
        .service
        .store()
        .list_skill_targets(&record.id)
        .unwrap()
        .is_empty());
}

#[test]
fn source_takeover_honors_explicit_copy_mode_without_retaining_an_external_source() {
    use crate::core::sync_engine::SyncMode;
    use crate::core::tool_adapters::{save_tool_config, CustomToolConfig, ToolConfig};
    use crate::services::deployment::DeploymentRequest;
    let fixture = Fixture::new();
    let (source, record) = fixture.adopt_codex_skill();
    save_tool_config(
        fixture.service.store(),
        ToolConfig {
            disabled_builtin_tools: vec![],
            custom_tools: vec![CustomToolConfig {
                key: "custom_copy".into(),
                label: "Copy source".into(),
                avatar: None,
                skills_dir: source.parent().unwrap().to_string_lossy().into_owned(),
                project_skills_dir: None,
                sync_mode: SyncMode::Copy,
                enabled: true,
            }],
        },
    )
    .unwrap();
    let request = DeploymentRequest::global("demo", ["custom_copy"]);
    assert_eq!(
        fixture.service.deploy(request.clone()).unwrap().targets[0].mode,
        SyncMode::Copy
    );
    let current = fixture
        .service
        .store()
        .get_skill_by_id(&record.id)
        .unwrap()
        .unwrap();
    assert_eq!(
        current.source_ref.as_deref(),
        Some(current.central_path.as_str())
    );
    fixture.service.undeploy(request.clone()).unwrap();
    assert!(!source.exists());
    fixture.service.deploy(request).unwrap();
    assert!(source.join("SKILL.md").is_file());
}

#[test]
fn remove_requires_confirmation_without_touching_the_skill() {
    let fixture = Fixture::new();
    fixture.add_skill("demo-id", "demo", "saved");

    let plan = fixture.service.plan_remove("demo".into()).unwrap();
    let error = fixture
        .service
        .remove(RemoveRequest::unconfirmed(plan.id))
        .unwrap_err();

    assert_eq!(error.code, ErrorCode::ConfirmationRequired);
    assert!(fixture.central_skill_exists("demo"));
    assert!(fixture
        .service
        .store()
        .get_skill_by_id("demo-id")
        .unwrap()
        .is_some());
}

#[test]
fn remove_rejects_a_plan_after_database_or_filesystem_state_changes() {
    let fixture = Fixture::new();
    let skill = fixture.add_skill("demo-id", "demo", "saved");
    let plan = fixture.service.plan_remove("demo-id".into()).unwrap();
    fs::write(Path::new(&skill.central_path).join("user.txt"), "changed").unwrap();

    let error = fixture
        .service
        .remove(RemoveRequest::confirmed(plan.id))
        .unwrap_err();

    assert_eq!(error.code, ErrorCode::PlanStale);
    assert!(fixture.central_skill_exists("demo"));
}

#[test]
fn remove_archives_recoverable_source_tags_targets_and_metadata() {
    let fixture = Fixture::new();
    let skill = fixture.add_skill("demo-id", "demo", "saved");
    fixture
        .service
        .store()
        .set_skill_tag_names(&skill.id, &["Writing".into(), "内容".into()])
        .unwrap();
    let target_path = fixture.paths.app_data_dir.join("agent/demo");
    write_skill(&target_path, "demo", "saved");
    fixture
        .service
        .store()
        .upsert_skill_target(&SkillTargetRecord {
            id: "target-id".into(),
            skill_id: skill.id.clone(),
            tool: "cursor".into(),
            scope: "global".into(),
            project_path: None,
            target_path: target_path.to_string_lossy().into_owned(),
            mode: "copy".into(),
            status: "ok".into(),
            last_error: None,
            synced_at: Some(50),
        })
        .unwrap();
    let plan = fixture.service.plan_remove("demo-id".into()).unwrap();

    let outcome = fixture
        .service
        .remove(RemoveRequest::confirmed(plan.id))
        .unwrap();

    assert!(!fixture.central_skill_exists("demo"));
    assert!(!target_path.exists());
    let item = outcome.item.unwrap();
    assert_eq!(item.source_type, "git");
    assert_eq!(item.source_ref, skill.source_ref);
    assert_eq!(item.targets.len(), 1);
    assert_eq!(item.tags, vec!["Writing", "内容"]);
    assert_eq!(item.description, skill.description);
    assert!(Path::new(&item.trash_path).join("SKILL.md").is_file());
}

#[test]
fn adopt_excludes_managed_sources_targets_invalid_content_and_escaping_links() {
    let fixture = Fixture::new();
    let source_root = fixture.paths.app_data_dir.join("agent-skills");
    let existing_source = source_root.join("existing-source");
    let existing_target = source_root.join("existing-target");
    let fresh = source_root.join("fresh");
    write_skill(&existing_source, "existing-source", "source");
    write_skill(&existing_target, "existing-target", "target");
    write_skill(&fresh, "fresh", "fresh");
    fs::create_dir_all(source_root.join("invalid")).unwrap();
    let mut source_record = fixture.add_skill("source-id", "source-managed", "central");
    source_record.source_type = "local".into();
    source_record.source_ref = Some(existing_source.to_string_lossy().into_owned());
    fixture
        .service
        .store()
        .upsert_skill(&source_record)
        .unwrap();
    let target_record = fixture.add_skill("target-id", "target-managed", "central");
    fixture
        .service
        .store()
        .upsert_skill_target(&SkillTargetRecord {
            id: "managed-target".into(),
            skill_id: target_record.id,
            tool: "codex".into(),
            scope: "global".into(),
            project_path: None,
            target_path: existing_target.to_string_lossy().into_owned(),
            mode: "copy".into(),
            status: "ok".into(),
            last_error: None,
            synced_at: Some(1),
        })
        .unwrap();

    #[cfg(unix)]
    {
        let outside = fixture.paths.app_data_dir.join("outside");
        write_skill(&outside, "outside", "outside");
        std::os::unix::fs::symlink(&outside, source_root.join("escape")).unwrap();
    }

    let plan = fixture.service.plan_adopt(&source_root).unwrap();

    assert_eq!(
        plan.candidates
            .iter()
            .map(|candidate| candidate.name.as_str())
            .collect::<Vec<_>>(),
        vec!["fresh"]
    );
    assert!(plan
        .excluded
        .iter()
        .any(|item| item.reason == "managed_source"));
    assert!(plan
        .excluded
        .iter()
        .any(|item| item.reason == "managed_target"));
    assert!(plan
        .excluded
        .iter()
        .any(|item| item.reason == "missing_skill_md"));
    #[cfg(unix)]
    assert!(plan
        .excluded
        .iter()
        .any(|item| item.reason == "path_escape"));
}

#[test]
fn adopt_noncanonical_source_with_exclusions_succeeds() {
    let fixture = Fixture::new();
    let source = fixture.paths.app_data_dir.join("external");
    write_skill(&source.join("fresh"), "fresh", "original");
    fs::create_dir_all(source.join("empty-dir")).unwrap();
    let spelling = source.join("..").join("external");
    let plan = fixture.service.plan_adopt(&spelling).unwrap();
    assert_eq!(plan.excluded.len(), 1);
    assert_eq!(plan.excluded[0].reason, "missing_skill_md");

    let outcome = fixture
        .service
        .adopt(AdoptRequest::confirmed(plan.id))
        .unwrap();
    assert_eq!(outcome.adopted.len(), 1);
    assert!(fixture.central_skill_exists("fresh"));
    assert!(source.join("empty-dir").is_dir());
}

#[test]
#[cfg(unix)]
fn adopt_parent_alias_preserves_excluded_links() {
    let fixture = Fixture::new();
    let source = fixture.paths.app_data_dir.join("external");
    write_skill(&source.join("fresh"), "fresh", "original");
    let outside = fixture.paths.app_data_dir.join("outside");
    write_skill(&outside, "outside", "protected");
    let missing = fixture.paths.app_data_dir.join("missing");
    std::os::unix::fs::symlink(&outside, source.join("escape")).unwrap();
    std::os::unix::fs::symlink(&missing, source.join("dangling")).unwrap();
    let alias = fixture.paths.app_data_dir.join("alias");
    std::os::unix::fs::symlink(&fixture.paths.app_data_dir, &alias).unwrap();
    let plan = fixture.service.plan_adopt(alias.join("external")).unwrap();
    assert_eq!(plan.excluded.len(), 2);
    assert!(plan
        .excluded
        .iter()
        .all(|item| item.reason == "path_escape"));

    let outcome = fixture
        .service
        .adopt(AdoptRequest::confirmed(plan.id))
        .unwrap();
    assert_eq!(outcome.adopted.len(), 1);
    assert_eq!(outcome.adopted[0].name, "fresh");
    assert_eq!(fs::read_link(source.join("escape")).unwrap(), outside);
    assert_eq!(fs::read_link(source.join("dangling")).unwrap(), missing);
    assert!(outside.join("SKILL.md").is_file());
    assert!(!fixture.central_skill_exists("outside"));
}

#[test]
fn adopt_requires_confirmation_detects_stale_content_and_records_local_source() {
    let fixture = Fixture::new();
    let source_root = fixture.paths.app_data_dir.join("adopt");
    write_skill(&source_root.join("fresh"), "fresh", "before");
    let plan = fixture.service.plan_adopt(&source_root).unwrap();
    let error = fixture
        .service
        .adopt(AdoptRequest::unconfirmed(plan.id.clone()))
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::ConfirmationRequired);
    assert!(!fixture.central_skill_exists("fresh"));

    fs::write(source_root.join("fresh/SKILL.md"), "changed").unwrap();
    let error = fixture
        .service
        .adopt(AdoptRequest::confirmed(plan.id))
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::PlanStale);

    let fresh_plan = fixture.service.plan_adopt(&source_root).unwrap();
    let outcome = fixture
        .service
        .adopt(AdoptRequest::confirmed(fresh_plan.id))
        .unwrap();
    assert_eq!(outcome.adopted.len(), 1);
    let stored = fixture
        .service
        .store()
        .get_skill_by_id(&outcome.adopted[0].id)
        .unwrap()
        .unwrap();
    assert_eq!(stored.source_type, "local");
    assert_eq!(
        PathBuf::from(stored.source_ref.unwrap()),
        fs::canonicalize(source_root.join("fresh")).unwrap()
    );
}

#[test]
fn adopt_plan_is_read_only_and_rejects_unsafe_or_occupied_names() {
    let fixture = Fixture::new();
    let source = fixture.paths.app_data_dir.join("single");
    write_skill(&source, "single", "single");
    assert!(!fixture.paths.default_central_repo.exists());

    fixture.service.plan_adopt(&source).unwrap();

    assert!(!fixture.paths.default_central_repo.exists());
    let error = fixture
        .service
        .plan_adopt_with_name(&source, Some("../escape".into()))
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidArgument);

    fs::create_dir_all(&fixture.paths.default_central_repo).unwrap();
    write_skill(
        &fixture.paths.default_central_repo.join("occupied"),
        "occupied",
        "user content",
    );
    let error = fixture
        .service
        .plan_adopt_with_name(&source, Some("occupied".into()))
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::TargetConflict);
    assert_eq!(
        fs::read_to_string(fixture.paths.default_central_repo.join("occupied/SKILL.md")).unwrap(),
        "---\nname: occupied\ndescription: Test\n---\n\nuser content\n"
    );

    let colliding_source = fixture.paths.app_data_dir.join("occupied");
    write_skill(&colliding_source, "occupied", "source content");
    let renamed = fixture
        .service
        .plan_adopt_with_name(&colliding_source, Some("renamed".into()))
        .unwrap();
    assert_eq!(renamed.candidates.len(), 1);
    assert_eq!(renamed.candidates[0].name, "renamed");
}

#[test]
fn a_remove_plan_is_bound_to_database_metadata_and_operation_kind() {
    let fixture = Fixture::new();
    fixture.add_skill("demo-id", "demo", "saved");
    let remove_plan = fixture.service.plan_remove("demo-id".into()).unwrap();
    let error = fixture
        .service
        .adopt(AdoptRequest::confirmed(remove_plan.id.clone()))
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::PlanStale);

    fixture
        .service
        .store()
        .set_skill_tag_names("demo-id", &["changed".into()])
        .unwrap();
    let error = fixture
        .service
        .remove(RemoveRequest::confirmed(remove_plan.id))
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::PlanStale);
    assert!(fixture.central_skill_exists("demo"));
}

#[test]
fn remove_refuses_a_preexisting_user_modified_copy_target() {
    let fixture = Fixture::new();
    let skill = fixture.add_skill("demo-id", "demo", "saved");
    let target_path = fixture.paths.app_data_dir.join("agent/demo");
    write_skill(&target_path, "demo", "user changed");
    fixture
        .service
        .store()
        .upsert_skill_target(&SkillTargetRecord {
            id: "target-id".into(),
            skill_id: skill.id,
            tool: "cursor".into(),
            scope: "global".into(),
            project_path: None,
            target_path: target_path.to_string_lossy().into_owned(),
            mode: "copy".into(),
            status: "ok".into(),
            last_error: None,
            synced_at: Some(50),
        })
        .unwrap();

    let error = fixture.service.plan_remove("demo-id".into()).unwrap_err();

    assert_eq!(error.code, ErrorCode::TargetConflict);
    assert_eq!(error.details["reason"], "target_modified");
    assert!(fixture.central_skill_exists("demo"));
    assert!(target_path.exists());
}

#[test]
fn tag_actions_are_unicode_case_insensitive_and_report_exact_impact() {
    let fixture = Fixture::new();
    fixture.add_skill("one", "one", "one");
    fixture.add_skill("two", "two", "two");

    fixture
        .service
        .apply_tag_action(TagAction::Add {
            skill: "one".into(),
            tags: vec!["RéACT".into()],
        })
        .unwrap();
    fixture
        .service
        .apply_tag_action(TagAction::Add {
            skill: "two".into(),
            tags: vec!["réact".into()],
        })
        .unwrap();
    assert_eq!(
        fixture
            .service
            .store()
            .list_tags_with_counts()
            .unwrap()
            .len(),
        1
    );

    let renamed = fixture
        .service
        .apply_tag_action(TagAction::Rename {
            tag: TagSelector::Name("RÉACT".into()),
            name: "界面".into(),
        })
        .unwrap();
    assert_eq!(renamed.affected_skill_count, 2);

    let plan = fixture
        .service
        .plan_tag_delete(TagSelector::Name("界面".into()))
        .unwrap();
    assert_eq!(plan.affected_skill_count, 2);
    assert_eq!(plan.affected_skill_ids, vec!["one", "two"]);
    let error = fixture
        .service
        .apply_tag_action(TagAction::Delete {
            plan_id: plan.id.clone(),
            confirmed: false,
        })
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::ConfirmationRequired);
    assert_eq!(
        fixture
            .service
            .store()
            .list_tags_with_counts()
            .unwrap()
            .len(),
        1
    );

    let deleted = fixture
        .service
        .apply_tag_action(TagAction::Delete {
            plan_id: plan.id,
            confirmed: true,
        })
        .unwrap();
    assert_eq!(deleted.affected_skill_count, 2);
    assert!(fixture
        .service
        .store()
        .list_tags_with_counts()
        .unwrap()
        .is_empty());
}

#[test]
fn tag_delete_plan_rejects_changed_links_and_cannot_be_applied_as_another_operation() {
    let fixture = Fixture::new();
    fixture.add_skill("one", "one", "one");
    fixture.add_skill("two", "two", "two");
    fixture
        .service
        .apply_tag_action(TagAction::Add {
            skill: "one".into(),
            tags: vec!["Shared".into()],
        })
        .unwrap();
    let plan = fixture
        .service
        .plan_tag_delete(TagSelector::Name("shared".into()))
        .unwrap();

    let wrong_operation = fixture
        .service
        .remove(RemoveRequest::confirmed(plan.id.clone()))
        .unwrap_err();
    assert_eq!(wrong_operation.code, ErrorCode::PlanStale);

    fixture
        .service
        .apply_tag_action(TagAction::Add {
            skill: "two".into(),
            tags: vec!["SHARED".into()],
        })
        .unwrap();
    let error = fixture
        .service
        .apply_tag_action(TagAction::Delete {
            plan_id: plan.id,
            confirmed: true,
        })
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::PlanStale);
    assert_eq!(
        fixture
            .service
            .store()
            .list_tags_with_counts()
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn adopt_source_disappearance_and_symlink_replacement_are_plan_stale() {
    let fixture = Fixture::new();
    let source = fixture.paths.app_data_dir.join("source");
    write_skill(&source, "source", "source");
    let missing_plan = fixture.service.plan_adopt(&source).unwrap();
    fs::remove_dir_all(&source).unwrap();
    let error = fixture
        .service
        .adopt(AdoptRequest::confirmed(missing_plan.id))
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::PlanStale);

    #[cfg(unix)]
    {
        write_skill(&source, "source", "source");
        let link_plan = fixture.service.plan_adopt(&source).unwrap();
        fs::remove_dir_all(&source).unwrap();
        let elsewhere = fixture.paths.app_data_dir.join("elsewhere");
        write_skill(&elsewhere, "elsewhere", "elsewhere");
        std::os::unix::fs::symlink(&elsewhere, &source).unwrap();
        let error = fixture
            .service
            .adopt(AdoptRequest::confirmed(link_plan.id))
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::PlanStale);
    }
}

#[test]
fn adopt_batch_maps_source_and_manifest_drift_after_revalidation_to_plan_stale() {
    let fixture = Fixture::new();
    let source = fixture.paths.app_data_dir.join("source");
    write_skill(&source, "source", "source");
    let manifest_plan = fixture
        .service
        .plan_adopt_direct_with_name(&source, None)
        .unwrap();
    let manifest = source.join("SKILL.md");
    set_adopt_after_revalidation_hook(move || fs::remove_file(manifest).unwrap());
    let error = fixture
        .service
        .adopt(AdoptRequest::confirmed(manifest_plan.id))
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::PlanStale);

    write_skill(&source, "source", "source");
    let source_plan = fixture
        .service
        .plan_adopt_direct_with_name(&source, None)
        .unwrap();
    let removed_source = source.clone();
    set_adopt_after_revalidation_hook(move || fs::remove_dir_all(removed_source).unwrap());
    let error = fixture
        .service
        .adopt(AdoptRequest::confirmed(source_plan.id))
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::PlanStale);
}

#[test]
fn adopt_io_failures_are_not_misclassified_as_stale() {
    let manifest_error = classify_direct_manifest_stat(Err(std::io::Error::new(
        std::io::ErrorKind::PermissionDenied,
        "injected manifest stat failure",
    )))
    .unwrap_err();
    assert_eq!(manifest_error.code, ErrorCode::InternalError);

    let target_error =
        crate::core::installer::validate_adopt_target_stat(Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "injected target stat failure",
        )))
        .unwrap_err();
    let mapped = map_batch_adopt_error(target_error);
    assert_eq!(mapped.code, ErrorCode::InternalError);

    crate::core::installer::validate_adopt_target_stat(Err(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "injected missing target",
    )))
    .unwrap();
    let existing_target = tempfile::tempdir().unwrap();
    let existing_error = crate::core::installer::validate_adopt_target_stat(fs::symlink_metadata(
        existing_target.path(),
    ))
    .unwrap_err();
    assert_eq!(
        map_batch_adopt_error(existing_error).code,
        ErrorCode::PlanStale
    );

    let stale = map_batch_adopt_error(crate::core::installer::AdoptPlanStaleError.into());
    assert_eq!(stale.code, ErrorCode::PlanStale);
}

#[test]
fn remove_deleted_skill_and_changed_target_are_plan_stale() {
    let fixture = Fixture::new();
    fixture.add_skill("gone-id", "gone", "gone");
    let gone_plan = fixture.service.plan_remove("gone-id".into()).unwrap();
    fixture.service.store().delete_skill("gone-id").unwrap();
    let error = fixture
        .service
        .remove(RemoveRequest::confirmed(gone_plan.id))
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::PlanStale);

    let skill = fixture.add_skill("target-id", "target", "target");
    let target_path = fixture.paths.app_data_dir.join("agent/target");
    write_skill(&target_path, "target", "target");
    fixture
        .service
        .store()
        .upsert_skill_target(&SkillTargetRecord {
            id: "target-record".into(),
            skill_id: skill.id,
            tool: "cursor".into(),
            scope: "global".into(),
            project_path: None,
            target_path: target_path.to_string_lossy().into_owned(),
            mode: "copy".into(),
            status: "ok".into(),
            last_error: None,
            synced_at: Some(50),
        })
        .unwrap();
    let target_plan = fixture.service.plan_remove("target-id".into()).unwrap();
    fs::write(target_path.join("changed.txt"), "user change").unwrap();
    let error = fixture
        .service
        .remove(RemoveRequest::confirmed(target_plan.id))
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::PlanStale);
}

#[test]
fn adopt_binds_the_central_root_and_never_writes_to_a_new_setting() {
    let fixture = Fixture::new();
    let source = fixture.paths.app_data_dir.join("source");
    write_skill(&source, "source", "source");
    let central_a = fixture.paths.app_data_dir.join("central-a");
    let central_b = fixture.paths.default_central_repo.clone();
    fixture
        .service
        .store()
        .set_setting("central_repo_path", central_a.to_string_lossy().as_ref())
        .unwrap();
    let plan = fixture.service.plan_adopt(&source).unwrap();
    fixture
        .service
        .store()
        .delete_setting("central_repo_path")
        .unwrap();

    let error = fixture
        .service
        .adopt(AdoptRequest::confirmed(plan.id))
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::PlanStale);
    assert!(!central_a.join("source").exists());
    assert!(!central_b.join("source").exists());
}

#[test]
fn direct_adopt_does_not_silently_choose_a_container_child() {
    let fixture = Fixture::new();
    let container = fixture.paths.app_data_dir.join("container");
    write_skill(&container.join("only-child"), "only-child", "child");
    assert_eq!(
        fixture
            .service
            .plan_adopt(&container)
            .unwrap()
            .candidates
            .len(),
        1
    );

    let error = fixture
        .service
        .plan_adopt_direct_with_name(&container, None)
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidSource);
    assert_eq!(error.message, "SKILL_INVALID|missing_skill_md");
}

#[test]
fn multi_candidate_adopt_rolls_back_files_and_database_when_the_second_insert_fails() {
    let fixture = Fixture::new();
    let source = fixture.paths.app_data_dir.join("container");
    write_skill(&source.join("one"), "one", "one");
    write_skill(&source.join("two"), "two", "two");
    let plan = fixture.service.plan_adopt(&source).unwrap();
    let connection = rusqlite::Connection::open(&fixture.paths.database_path).unwrap();
    connection
        .execute_batch(
            "CREATE TRIGGER fail_second_adopt BEFORE INSERT ON skills
             WHEN NEW.name = 'two' BEGIN SELECT RAISE(FAIL, 'second insert failed'); END;",
        )
        .unwrap();

    let error = fixture
        .service
        .adopt(AdoptRequest::confirmed(plan.id))
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::InternalError);
    assert!(fixture.service.store().list_skills().unwrap().is_empty());
    assert!(!fixture.central_skill_exists("one"));
    assert!(!fixture.central_skill_exists("two"));
    assert!(source.join("one/SKILL.md").is_file());
    assert!(source.join("two/SKILL.md").is_file());
}

#[test]
fn adopt_excludes_bidirectional_overlaps_with_managed_sources_and_targets() {
    let fixture = Fixture::new();
    let root = fixture.paths.app_data_dir.join("overlaps");
    let source_parent = root.join("source-parent");
    let source_child = source_parent.join("candidate");
    write_skill(&source_child, "candidate", "candidate");
    let mut parent_record = fixture.add_skill("source-parent-id", "managed-a", "managed");
    parent_record.source_type = "local".into();
    parent_record.source_ref = Some(source_parent.to_string_lossy().into_owned());
    fixture
        .service
        .store()
        .upsert_skill(&parent_record)
        .unwrap();
    let plan = fixture.service.plan_adopt(&source_parent).unwrap();
    assert!(plan
        .excluded
        .iter()
        .any(|item| item.reason == "managed_source"));

    let candidate_parent = root.join("candidate-parent");
    write_skill(&candidate_parent, "candidate-parent", "parent");
    let nested_source = candidate_parent.join("nested");
    fs::create_dir_all(&nested_source).unwrap();
    let mut child_record = fixture.add_skill("source-child-id", "managed-b", "managed");
    child_record.source_type = "local".into();
    child_record.source_ref = Some(nested_source.to_string_lossy().into_owned());
    fixture.service.store().upsert_skill(&child_record).unwrap();
    let plan = fixture.service.plan_adopt(&candidate_parent).unwrap();
    assert!(plan.candidates.is_empty());
    assert!(plan
        .excluded
        .iter()
        .any(|item| item.reason == "managed_source"));

    let target_parent = root.join("target-parent");
    let target_child = target_parent.join("candidate");
    write_skill(&target_child, "candidate", "candidate");
    let target_owner = fixture.add_skill("target-parent-id", "managed-c", "managed");
    fixture
        .service
        .store()
        .upsert_skill_target(&SkillTargetRecord {
            id: "target-parent-record".into(),
            skill_id: target_owner.id,
            tool: "cursor".into(),
            scope: "global".into(),
            project_path: None,
            target_path: target_parent.to_string_lossy().into_owned(),
            mode: "copy".into(),
            status: "ok".into(),
            last_error: None,
            synced_at: None,
        })
        .unwrap();
    let plan = fixture.service.plan_adopt(&target_parent).unwrap();
    assert!(plan
        .excluded
        .iter()
        .any(|item| item.reason == "managed_target"));

    let candidate_target_parent = root.join("candidate-target-parent");
    write_skill(
        &candidate_target_parent,
        "candidate-target-parent",
        "candidate",
    );
    let nested_target = candidate_target_parent.join("nested");
    fs::create_dir_all(&nested_target).unwrap();
    let target_owner = fixture.add_skill("target-child-id", "managed-d", "managed");
    fixture
        .service
        .store()
        .upsert_skill_target(&SkillTargetRecord {
            id: "target-child-record".into(),
            skill_id: target_owner.id,
            tool: "codex".into(),
            scope: "global".into(),
            project_path: None,
            target_path: nested_target.to_string_lossy().into_owned(),
            mode: "copy".into(),
            status: "ok".into(),
            last_error: None,
            synced_at: None,
        })
        .unwrap();
    let plan = fixture
        .service
        .plan_adopt(&candidate_target_parent)
        .unwrap();
    assert!(plan.candidates.is_empty());
    assert!(plan
        .excluded
        .iter()
        .any(|item| item.reason == "managed_target"));
}

#[test]
fn adopt_custom_names_use_cross_platform_safe_filename_rules() {
    let fixture = Fixture::new();
    let source = fixture.paths.app_data_dir.join("source");
    write_skill(&source, "source", "source");
    for name in [
        "CON", "con.txt", "name.", "name ", "bad:name", "bad*name", "bad?name", "bad<name",
        "bad>name", "bad|name",
    ] {
        let error = fixture
            .service
            .plan_adopt_with_name(&source, Some(name.into()))
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidArgument, "{name}");
    }
}

#[test]
fn missing_central_is_reported_as_an_error_for_all_source_kinds() {
    let fixture = Fixture::new();
    for (index, kind, bound, enabled) in [
        (0, "git", true, true),
        (1, "local", false, true),
        (2, "local", true, false),
    ] {
        let id = format!("missing-{index}");
        let mut skill = fixture.add_skill(&id, &id, "saved");
        skill.source_type = kind.into();
        skill.enabled = enabled;
        let source = fixture._home.path().join(format!("source-{index}"));
        write_skill(&source, &id, "saved");
        skill.source_ref = bound.then(|| source.to_string_lossy().into_owned());
        fixture.service.store().upsert_skill(&skill).unwrap();
        fs::remove_dir_all(&skill.central_path).unwrap();
        let shown = fixture.service.show_skill(id.clone().into()).unwrap();

        assert_eq!(shown.content_status, "error");
        assert_eq!(
            shown.source_error.as_deref(),
            Some("SKILL_ISSUE|centralMissing")
        );
        assert_eq!(
            fixture
                .service
                .skill_status(id.into())
                .unwrap()
                .content_status,
            "error"
        );
    }
    assert!(fixture
        .service
        .list_skills()
        .unwrap()
        .iter()
        .all(|skill| skill.content_status == "error"));
}

#[test]
fn missing_central_record_can_be_removed_without_a_recycle_entry() {
    let fixture = Fixture::new();
    let skill = fixture.add_skill("missing-id", "missing", "saved");
    fs::remove_dir_all(&skill.central_path).unwrap();
    let plan = fixture.service.plan_remove(skill.id.clone().into());

    let plan = plan.expect("missing content must not prevent explicit record deletion");
    assert!(!plan.creates_recycle_bin_entry);
    fixture
        .service
        .remove(RemoveRequest::confirmed(plan.id))
        .unwrap();
    assert!(fixture
        .service
        .store()
        .get_skill_by_id(&skill.id)
        .unwrap()
        .is_none());
    assert!(fixture
        .service
        .store()
        .list_device_sync_trash()
        .unwrap()
        .is_empty());
}

#[test]
fn missing_central_deletion_preserves_sources_and_tool_content_and_revalidates() {
    let fixture = Fixture::new();
    let mut skill = fixture.add_skill("missing-id", "missing", "saved");
    let source = fixture._home.path().join("original");
    write_skill(&source, "missing", "source content");
    skill.source_type = "local".into();
    skill.source_ref = Some(source.to_string_lossy().into_owned());
    fixture.service.store().upsert_skill(&skill).unwrap();
    let copy = fixture._home.path().join("tools/missing");
    write_skill(&copy, "missing", "independent tool edits");
    fixture
        .service
        .store()
        .upsert_skill_target(&SkillTargetRecord {
            id: "copy-id".into(),
            skill_id: skill.id.clone(),
            tool: "codex".into(),
            scope: "global".into(),
            project_path: None,
            target_path: copy.to_string_lossy().into_owned(),
            mode: "copy".into(),
            status: "ok".into(),
            last_error: None,
            synced_at: None,
        })
        .unwrap();
    fs::remove_dir_all(&skill.central_path).unwrap();
    let plan = fixture
        .service
        .plan_remove(skill.id.clone().into())
        .unwrap();
    assert!(!plan.creates_recycle_bin_entry);
    assert!(plan.targets.is_empty());
    // A manual recovery after confirmation was prepared must invalidate that plan.
    write_skill(
        Path::new(&skill.central_path),
        "missing",
        "manually restored",
    );
    assert_eq!(
        fixture
            .service
            .remove(RemoveRequest::confirmed(plan.id))
            .unwrap_err()
            .code,
        ErrorCode::PlanStale
    );
    assert_eq!(
        fixture
            .service
            .show_skill(skill.id.clone().into())
            .unwrap()
            .content_status,
        "ok"
    );
    fs::remove_dir_all(&skill.central_path).unwrap();
    let plan = fixture
        .service
        .plan_remove(skill.id.clone().into())
        .unwrap();
    let removed = fixture
        .service
        .remove(RemoveRequest::confirmed(plan.id))
        .unwrap();
    assert!(removed.item.is_none());
    assert!(fixture
        .service
        .store()
        .get_skill_by_id(&skill.id)
        .unwrap()
        .is_none());
    assert!(fixture
        .service
        .store()
        .list_skill_targets(&skill.id)
        .unwrap()
        .is_empty());
    assert!(fs::read_to_string(copy.join("SKILL.md"))
        .unwrap()
        .contains("independent tool edits"));
    assert!(fs::read_to_string(source.join("SKILL.md"))
        .unwrap()
        .contains("source content"));
}
