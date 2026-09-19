use std::fs;
use std::path::{Path, PathBuf};

use serde_json::json;
use tempfile::TempDir;

use crate::core::runtime_paths::{RuntimePaths, RuntimeProfile};
use crate::core::skill_store::{SkillRecord, SkillTargetRecord};
use crate::services::error::ErrorCode;
use crate::services::library::{AdoptRequest, RemoveRequest, TagAction, TagSelector};
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

    let error = fixture
        .service
        .apply_tag_action(TagAction::Delete {
            tag: TagSelector::Name("界面".into()),
            confirmed: false,
        })
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::ConfirmationRequired);
    assert_eq!(
        error.details,
        json!({ "affected_skill_count": 2, "tag": "界面" })
    );
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
            tag: TagSelector::Name("界面".into()),
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
