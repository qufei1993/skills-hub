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
    assert_eq!(outcome.item.source_type, "git");
    assert_eq!(outcome.item.source_ref, skill.source_ref);
    assert_eq!(outcome.item.targets.len(), 1);
    assert_eq!(outcome.item.tags, vec!["Writing", "内容"]);
    assert_eq!(outcome.item.description, skill.description);
    assert!(Path::new(&outcome.item.trash_path)
        .join("SKILL.md")
        .is_file());
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
