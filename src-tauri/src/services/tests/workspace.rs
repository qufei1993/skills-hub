use crate::core::runtime_paths::{RuntimePaths, RuntimeProfile};
use crate::services::{
    deployment::DeploymentScope,
    install::InstallRequest,
    skills_hub::SkillsHubService,
    workspace::{ApplicationRequest, Preset, PresetMember},
};
use std::fs;
use tempfile::TempDir;

struct Fixture {
    home: TempDir,
    data: TempDir,
    service: SkillsHubService,
    id: String,
}
impl Fixture {
    fn new() -> Self {
        let home = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        let service = SkillsHubService::open(RuntimePaths::from_roots(
            RuntimeProfile::Test,
            home.path(),
            data.path(),
        ))
        .unwrap();
        let source = data.path().join("source");
        fs::create_dir(&source).unwrap();
        fs::write(
            source.join("SKILL.md"),
            "---\nname: demo\ndescription: Demo\n---\nbody",
        )
        .unwrap();
        service.install(InstallRequest::local(source)).unwrap();
        fs::create_dir(home.path().join(".codex")).unwrap();
        fs::create_dir(home.path().join(".cursor")).unwrap();
        let id = service.list_skills().unwrap()[0].id.clone();
        Self {
            home,
            data,
            service,
            id,
        }
    }
    fn members(&self) -> Vec<PresetMember> {
        vec![PresetMember {
            skill_id: self.id.clone(),
            tools: vec!["codex".into()],
        }]
    }
    fn request(&self, scopes: Vec<DeploymentScope>) -> ApplicationRequest {
        ApplicationRequest {
            preset_id: None,
            members: self.members(),
            scopes,
            remove: false,
        }
    }
    fn install_member(&self, name: &str) -> (String, String) {
        let source = self.data.path().join(format!("source-{name}"));
        fs::create_dir(&source).unwrap();
        let content = format!(
            "---\nname: {name}\ndescription: {name} fixture\n---\nUnique content for {name}\n"
        );
        fs::write(source.join("SKILL.md"), &content).unwrap();
        self.service.install(InstallRequest::local(source)).unwrap();
        let id = self
            .service
            .list_skills()
            .unwrap()
            .into_iter()
            .find(|skill| skill.name == name)
            .unwrap()
            .id;
        (id, content)
    }
}

fn seed_application_history(
    f: &Fixture,
    template: &serde_json::Value,
    id: &str,
    updated_at: i64,
    confirmed: bool,
    status: &str,
) {
    let mut journal = template.clone();
    journal["record"]["id"] = id.into();
    journal["record"]["confirmed"] = confirmed.into();
    journal["record"]["items"][0]["status"] = status.into();
    // Retention follows the existing history order, not creation time.
    journal["record"]["createdAt"] = (1_000_000 - updated_at).into();
    let conn = rusqlite::Connection::open(f.service.store().db_path()).unwrap();
    conn.execute(
        "INSERT INTO workspace_documents(kind,id,payload,updated_at) VALUES ('application',?1,?2,?3)",
        rusqlite::params![id, serde_json::to_string(&journal).unwrap(), updated_at],
    )
    .unwrap();
}

#[test]
fn application_history_limits_existing_terminal_records_and_protects_recovery() {
    let f = Fixture::new();
    let preview = f
        .service
        .preview_application(f.request(vec![DeploymentScope::Global]))
        .unwrap();
    let template: serde_json::Value = f
        .service
        .store()
        .workspace_get("application", &preview.id)
        .unwrap()
        .unwrap();
    for index in 0..105 {
        seed_application_history(
            &f,
            &template,
            &format!("finished-{index:03}"),
            index,
            true,
            ["done", "undone", "failed", "blocked"][index as usize % 4],
        );
    }
    for status in ["pending", "running", "interrupted", "future-state"] {
        seed_application_history(&f, &template, status, -1, true, status);
    }
    for id in ["finished-000", "finished-104"] {
        f.service
            .store()
            .workspace_save("undo-preview", id, &Vec::<String>::new())
            .unwrap();
    }
    f.service
        .store()
        .workspace_save("target-use", "last-user", &"finished-000")
        .unwrap();
    let history = f.service.application_history().unwrap();
    assert_eq!(history.len(), 104);
    assert_eq!(history[0].id, "finished-104");
    assert_eq!(history[99].id, "finished-005");
    for status in ["pending", "running", "interrupted", "future-state"] {
        assert!(history.iter().any(|record| record.id == status));
    }
    assert!(!history.iter().any(|record| record.id == preview.id));
    assert!(f
        .service
        .store()
        .workspace_get::<serde_json::Value>("application", &preview.id)
        .unwrap()
        .is_some());
    for kind in ["application", "undo-preview"] {
        assert!(f
            .service
            .store()
            .workspace_get::<serde_json::Value>(kind, "finished-000")
            .unwrap()
            .is_none());
        assert!(f
            .service
            .store()
            .workspace_get::<serde_json::Value>(kind, "finished-104")
            .unwrap()
            .is_some());
    }
    assert_eq!(
        f.service
            .store()
            .workspace_get::<String>("target-use", "last-user")
            .unwrap()
            .as_deref(),
        Some("finished-000")
    );
    // No schema/version migration is needed; older journals remain readable.
    let conn = rusqlite::Connection::open(f.service.store().db_path()).unwrap();
    assert_eq!(
        conn.query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
            .unwrap(),
        6
    );
}

#[test]
fn application_history_remains_readable_during_auto_update_and_defers_cleanup() {
    use crate::services::operation_lock::{OperationKind, OperationLock};

    let f = Fixture::new();
    let preview = f
        .service
        .preview_application(f.request(vec![DeploymentScope::Global]))
        .unwrap();
    f.service.execute_application(&preview.id).unwrap();
    let template: serde_json::Value = f
        .service
        .store()
        .workspace_get("application", &preview.id)
        .unwrap()
        .unwrap();
    let lock = OperationLock::acquire(f.service.paths(), OperationKind::AutoUpdate).unwrap();
    // A real application writer lock must not block the common read-only path.
    assert_eq!(f.service.application_history().unwrap().len(), 1);
    for index in 0..105 {
        seed_application_history(
            &f,
            &template,
            &format!("finished-{index:03}"),
            index,
            true,
            "done",
        );
    }
    assert_eq!(f.service.application_history().unwrap().len(), 106);
    assert_eq!(
        f.service
            .store()
            .workspace_list::<serde_json::Value>("application")
            .unwrap()
            .len(),
        106,
        "a busy reader must not perform cleanup without the application write lock"
    );
    drop(lock);
    let history = f.service.application_history().unwrap();
    assert_eq!(history.len(), 100);
    assert!(history.iter().any(|record| record.id == preview.id));
    assert!(!history.iter().any(|record| record.id == "finished-000"));

    // Other lock failures are real errors, not a reason to silently skip cleanup.
    seed_application_history(&f, &template, "overflow", -1, true, "done");
    let lock_path = f.service.paths().app_data_dir.join("operation.lock");
    fs::remove_file(&lock_path).unwrap();
    fs::create_dir(&lock_path).unwrap();
    let error = f.service.application_history().unwrap_err();
    assert_eq!(error.code, crate::services::error::ErrorCode::InternalError);
}

#[test]
fn saving_application_keeps_current_record_within_limit_and_preserves_targets() {
    let f = Fixture::new();
    let initial = f
        .service
        .preview_application(f.request(vec![DeploymentScope::Global]))
        .unwrap();
    f.service.execute_application(&initial.id).unwrap();
    let reused = f
        .service
        .preview_application(f.request(vec![DeploymentScope::Global]))
        .unwrap();
    f.service.execute_application(&reused.id).unwrap();
    // Refresh the older application so the later reuse is the one that expires.
    f.service.recheck_application(&initial.id).unwrap();
    let template: serde_json::Value = f
        .service
        .store()
        .workspace_get("application", &reused.id)
        .unwrap()
        .unwrap();
    let conn = rusqlite::Connection::open(f.service.store().db_path()).unwrap();
    conn.execute(
        "UPDATE workspace_documents SET updated_at=-1 WHERE kind='application' AND id=?1",
        [&reused.id],
    )
    .unwrap();
    for index in 0..99 {
        seed_application_history(
            &f,
            &template,
            &format!("other-{index:03}"),
            index,
            true,
            "done",
        );
    }
    let rows = f.service.store().list_skill_targets(&f.id).unwrap();
    let content = fs::read(f.home.path().join(".codex/skills/demo/SKILL.md")).unwrap();
    // A normal journal save immediately enforces the cap, not just the history UI.
    f.service.recheck_application(&initial.id).unwrap();
    let all: Vec<serde_json::Value> = f.service.store().workspace_list("application").unwrap();
    assert_eq!(all.len(), 100);
    assert!(all.iter().any(|j| j["record"]["id"] == initial.id));
    assert!(!all.iter().any(|j| j["record"]["id"] == reused.id));
    assert_eq!(rows, f.service.store().list_skill_targets(&f.id).unwrap());
    assert_eq!(
        fs::read(f.home.path().join(".codex/skills/demo/SKILL.md")).unwrap(),
        content
    );
    let undo = f.service.preview_application_undo(&initial.id).unwrap();
    assert_eq!(undo.len(), 1);
    assert!(!undo[0].can_remove);
    assert_eq!(undo[0].reason, "laterUse");
    f.service.undo_application(&initial.id).unwrap();
    assert_eq!(rows, f.service.store().list_skill_targets(&f.id).unwrap());
}

#[test]
fn discard_removes_only_unexecuted_previews_and_is_idempotent() {
    let f = Fixture::new();
    let pending = f
        .service
        .preview_application(f.request(vec![DeploymentScope::Global]))
        .unwrap();
    f.service.preview_application_undo(&pending.id).unwrap();
    f.service.discard_application(&pending.id).unwrap();
    f.service.discard_application(&pending.id).unwrap();
    for kind in ["application", "undo-preview"] {
        assert!(f
            .service
            .store()
            .workspace_get::<serde_json::Value>(kind, &pending.id)
            .unwrap()
            .is_none());
    }
    let target = f.home.path().join(".codex/skills/demo");
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("personal.md"), "keep").unwrap();
    let blocked = f
        .service
        .preview_application(f.request(vec![DeploymentScope::Global]))
        .unwrap();
    assert_eq!(blocked.items[0].status, "blocked");
    f.service.discard_application(&blocked.id).unwrap();
    assert!(f
        .service
        .store()
        .workspace_get::<serde_json::Value>("application", &blocked.id)
        .unwrap()
        .is_none());
    assert_eq!(
        fs::read_to_string(target.join("personal.md")).unwrap(),
        "keep"
    );

    let confirmed = f
        .service
        .preview_application(f.request(vec![DeploymentScope::Global]))
        .unwrap();
    f.service.execute_application(&confirmed.id).unwrap();
    let before: serde_json::Value = f
        .service
        .store()
        .workspace_get("application", &confirmed.id)
        .unwrap()
        .unwrap();
    f.service.discard_application(&confirmed.id).unwrap();
    assert_eq!(
        f.service
            .store()
            .workspace_get::<serde_json::Value>("application", &confirmed.id)
            .unwrap(),
        Some(before.clone())
    );
    for status in ["running", "interrupted", "done"] {
        seed_application_history(&f, &before, status, -1, false, status);
        f.service.discard_application(status).unwrap();
        assert!(f
            .service
            .store()
            .workspace_get::<serde_json::Value>("application", status)
            .unwrap()
            .is_some());
    }
    assert_eq!(
        fs::read_to_string(target.join("personal.md")).unwrap(),
        "keep"
    );
}

#[test]
fn retention_failure_rolls_back_journal_save_and_history_cleanup() {
    let f = Fixture::new();
    let pending = f
        .service
        .preview_application(f.request(vec![DeploymentScope::Global]))
        .unwrap();
    let before: serde_json::Value = f
        .service
        .store()
        .workspace_get("application", &pending.id)
        .unwrap()
        .unwrap();
    for index in 0..101 {
        seed_application_history(
            &f,
            &before,
            &format!("finished-{index:03}"),
            index,
            true,
            "done",
        );
    }
    let conn = rusqlite::Connection::open(f.service.store().db_path()).unwrap();
    conn.execute_batch("CREATE TRIGGER reject_history_delete BEFORE DELETE ON workspace_documents WHEN OLD.kind='application' BEGIN SELECT RAISE(ABORT,'injected deletion failure'); END;").unwrap();
    assert!(f.service.execute_application(&pending.id).is_err());
    assert_eq!(
        f.service
            .store()
            .workspace_get::<serde_json::Value>("application", &pending.id)
            .unwrap(),
        Some(before)
    );
    assert!(!f.home.path().join(".codex/skills/demo").exists());
    assert!(f.service.application_history().is_err());
    let count: i64 = conn
        .query_row(
            "SELECT count(*) FROM workspace_documents WHERE kind='application'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 102);
    conn.execute_batch("DROP TRIGGER reject_history_delete")
        .unwrap();
    assert_eq!(f.service.application_history().unwrap().len(), 100);
    assert_eq!(
        f.service.execute_application(&pending.id).unwrap().items[0].status,
        "done"
    );
}

#[test]
fn preset_roundtrip_crud_does_not_deploy_and_revision_protects_edits() {
    let f = Fixture::new();
    let preset = f
        .service
        .save_preset(Preset {
            id: String::new(),
            name: "Work".into(),
            description: "Purpose".into(),
            members: f.members(),
            revision: 0,
        })
        .unwrap();
    assert_eq!(f.service.list_presets().unwrap(), vec![preset.clone()]);
    assert!(f.service.list_skills().unwrap()[0].targets.is_empty());
    let mut edit = preset.clone();
    edit.name = "New".into();
    f.service.save_preset(edit).unwrap();
    assert!(f.service.save_preset(preset.clone()).is_err());
    assert!(f
        .service
        .delete_preset(&preset.id, preset.revision)
        .is_err());
    f.service
        .delete_preset(&preset.id, preset.revision + 1)
        .unwrap();
    assert!(f.service.list_presets().unwrap().is_empty());
}
#[test]
fn global_and_multiple_projects_coexist_and_retry_is_idempotent_after_restart() {
    let f = Fixture::new();
    let global = f
        .service
        .preview_application(f.request(vec![DeploymentScope::Global]))
        .unwrap();
    f.service.execute_application(&global.id).unwrap();
    let p1 = f.data.path().join("one");
    let p2 = f.data.path().join("two");
    fs::create_dir(&p1).unwrap();
    fs::create_dir(&p2).unwrap();
    let plan = f
        .service
        .preview_application(f.request(vec![
            DeploymentScope::Project(p1),
            DeploymentScope::Project(p2),
        ]))
        .unwrap();
    let result = f.service.execute_application(&plan.id).unwrap();
    assert!(result.items.iter().all(|i| i.status == "done"));
    let rows = f.service.store().list_skill_targets(&f.id).unwrap();
    assert_eq!(rows.len(), 3);
    let reopened = SkillsHubService::open(RuntimePaths::from_roots(
        RuntimeProfile::Test,
        f.home.path(),
        f.data.path(),
    ))
    .unwrap();
    reopened.execute_application(&plan.id).unwrap();
    assert_eq!(rows, reopened.store().list_skill_targets(&f.id).unwrap());
    assert_eq!(reopened.application_history().unwrap().len(), 2);
}

#[test]
fn large_preset_preserves_per_member_tools_across_projects_and_later_reuse() {
    let f = Fixture::new();
    let mut samples = vec![(
        "demo".to_string(),
        f.id.clone(),
        "---\nname: demo\ndescription: Demo\n---\nbody".to_string(),
    )];
    for index in 1..20 {
        let name = format!("member-{index:02}");
        let (id, content) = f.install_member(&name);
        samples.push((name, id, content));
    }
    let members: Vec<_> = samples
        .iter()
        .enumerate()
        .map(|(index, (_, id, _))| PresetMember {
            skill_id: id.clone(),
            tools: match index % 3 {
                0 => vec!["codex".into()],
                1 => vec!["cursor".into()],
                _ => vec!["codex".into(), "cursor".into()],
            },
        })
        .collect();
    let preset = f
        .service
        .save_preset(Preset {
            id: String::new(),
            name: "Mixed tool project setup".into(),
            description: "Twenty members, with individual tool assignments".into(),
            members: members.clone(),
            revision: 0,
        })
        .unwrap();
    let projects: Vec<_> = (0..3)
        .map(|index| {
            let path = f.data.path().join(format!("large-project-{index}"));
            fs::create_dir(&path).unwrap();
            fs::canonicalize(path).unwrap()
        })
        .collect();
    let request = || ApplicationRequest {
        preset_id: Some(preset.id.clone()),
        members: members.clone(),
        scopes: projects
            .iter()
            .cloned()
            .map(DeploymentScope::Project)
            .collect(),
        remove: false,
    };
    let first = f.service.preview_application(request()).unwrap();
    assert_eq!(first.items.len(), 60);
    assert!(first.items.iter().all(|item| item.action == "add"));
    let first = f.service.execute_application(&first.id).unwrap();
    assert!(first.items.iter().all(|item| item.status == "done"));

    let check_targets = || {
        let mut all_rows = Vec::new();
        for ((name, id, content), member) in samples.iter().zip(&members) {
            let rows = f.service.store().list_skill_targets(id).unwrap();
            assert_eq!(rows.len(), 3 * member.tools.len());
            for project in &projects {
                let target = project.join(".agents/skills").join(name);
                assert_eq!(
                    fs::read_to_string(target.join("SKILL.md")).unwrap(),
                    *content
                );
                let item = first
                    .items
                    .iter()
                    .find(|item| {
                        item.skill_id == *id
                            && item.scope == DeploymentScope::Project(project.clone())
                    })
                    .unwrap();
                assert_eq!(item.tools, member.tools);
                // Both tools share one project directory but retain separate assignments.
                assert_eq!(item.targets.len(), 1);
                assert_eq!(item.targets[0].path, target);
                assert_eq!(item.targets[0].agents, member.tools);
                for tool in &member.tools {
                    assert_eq!(
                        rows.iter()
                            .filter(|row| {
                                row.tool == *tool
                                    && row.scope == "project"
                                    && row.project_path.as_deref() == project.to_str()
                                    && row.target_path == target.to_string_lossy()
                                    && row.status == "ok"
                            })
                            .count(),
                        1
                    );
                }
            }
            all_rows.extend(rows);
        }
        // 14 single-tool members and 6 dual-tool members across three projects.
        assert_eq!(all_rows.len(), 78);
        all_rows.sort_by(|a, b| a.id.cmp(&b.id));
        all_rows
    };
    let original_rows = check_targets();
    let reused = f.service.preview_application(request()).unwrap();
    assert_eq!(reused.items.len(), 60);
    assert!(reused.items.iter().all(|item| item.action == "existing"));
    let reused = f.service.execute_application(&reused.id).unwrap();
    assert!(reused.items.iter().all(|item| item.status == "done"));
    assert_eq!(check_targets(), original_rows);

    let undo = f.service.preview_application_undo(&first.id).unwrap();
    assert_eq!(undo.len(), 78);
    assert!(undo
        .iter()
        .all(|item| !item.can_remove && item.reason == "laterUse"));
    f.service.undo_application(&first.id).unwrap();
    assert_eq!(check_targets(), original_rows);
    assert!(!f.home.path().join(".codex/skills").exists());
    assert!(!f.home.path().join(".cursor/skills").exists());
}

#[test]
fn deleted_preset_member_is_blocked_without_preventing_other_members_or_retrying_them() {
    use crate::services::library::RemoveRequest;

    let f = Fixture::new();
    let (survivor, content) = f.install_member("survivor");
    let mut members = f.members();
    members.push(PresetMember {
        skill_id: survivor.clone(),
        tools: vec!["cursor".into()],
    });
    let preset = f
        .service
        .save_preset(Preset {
            id: String::new(),
            name: "One missing member".into(),
            description: String::new(),
            members,
            revision: 0,
        })
        .unwrap();
    let removal = f.service.plan_remove("demo".into()).unwrap();
    f.service
        .remove(RemoveRequest::confirmed(removal.id))
        .unwrap();
    assert_eq!(f.service.list_presets().unwrap(), vec![preset.clone()]);
    let plan = f
        .service
        .preview_application(ApplicationRequest {
            preset_id: Some(preset.id.clone()),
            members: preset.members.clone(),
            scopes: vec![DeploymentScope::Global],
            remove: false,
        })
        .unwrap();
    assert_eq!(plan.items[0].status, "blocked");
    assert_eq!(plan.items[0].reason.as_deref(), Some("SKILL_NOT_FOUND"));
    assert!(plan.items[0].targets.is_empty());
    let result = f.service.execute_application(&plan.id).unwrap();
    assert_eq!(result.items[0].status, "blocked");
    assert_eq!(result.items[1].status, "done");
    assert_eq!(result.preset, Some(preset));
    let target = f.home.path().join(".cursor/skills/survivor/SKILL.md");
    assert_eq!(fs::read_to_string(&target).unwrap(), content);
    assert!(!f.home.path().join(".codex/skills/demo").exists());
    let before = f.service.store().list_skill_targets(&survivor).unwrap();
    let rechecked = f.service.recheck_application(&plan.id).unwrap();
    assert_eq!(rechecked.items[0].status, "blocked");
    assert_eq!(rechecked.items[1].status, "done");
    f.service.execute_application(&plan.id).unwrap();
    assert_eq!(
        f.service.store().list_skill_targets(&survivor).unwrap(),
        before
    );
    assert_eq!(fs::read_to_string(target).unwrap(), content);
}

#[test]
fn stale_preview_does_not_write_and_conflict_keeps_other_projects_successful() {
    let f = Fixture::new();
    let plan = f
        .service
        .preview_application(f.request(vec![DeploymentScope::Global]))
        .unwrap();
    let central = f.service.list_skills().unwrap()[0].central_path.clone();
    fs::write(std::path::Path::new(&central).join("SKILL.md"), "changed").unwrap();
    assert!(f.service.execute_application(&plan.id).is_err());
    assert!(f.service.list_skills().unwrap()[0].targets.is_empty());
    let good = f.data.path().join("good");
    let bad = f.data.path().join("bad");
    fs::create_dir(&good).unwrap();
    fs::create_dir_all(bad.join(".agents/skills/demo")).unwrap();
    fs::write(bad.join(".agents/skills/demo/keep"), "personal").unwrap();
    let plan = f
        .service
        .preview_application(f.request(vec![
            DeploymentScope::Project(good),
            DeploymentScope::Project(bad.clone()),
        ]))
        .unwrap();
    let conflict = plan.items[1].conflict.as_ref().unwrap();
    assert_eq!(conflict.agent, "codex");
    assert_eq!(conflict.reason, "unmanaged_target");
    assert_eq!(
        std::path::Path::new(&conflict.path),
        fs::canonicalize(bad.join(".agents/skills/demo")).unwrap()
    );
    assert!(plan.items[1].targets.is_empty());
    let result = f.service.execute_application(&plan.id).unwrap();
    assert_eq!(result.items[0].status, "done");
    assert_eq!(result.items[1].status, "blocked");
    assert_eq!(result.items[1].conflict, plan.items[1].conflict);
    assert_eq!(
        fs::read_to_string(bad.join(".agents/skills/demo/keep")).unwrap(),
        "personal"
    );
}

#[test]
fn application_conflicts_preserve_modified_copy_details_and_clear_them_after_recheck() {
    use crate::core::sync_engine::SyncMode;
    use crate::core::tool_adapters::{save_tool_config, CustomToolConfig, ToolConfig};

    let f = Fixture::new();
    let project = f.data.path().join("review-project");
    fs::create_dir(&project).unwrap();
    let project = fs::canonicalize(project).unwrap();
    save_tool_config(
        f.service.store(),
        ToolConfig {
            disabled_builtin_tools: vec![],
            custom_tools: ["custom_editor", "custom_reviewer"]
                .into_iter()
                .map(|key| CustomToolConfig {
                    key: key.into(),
                    label: key.into(),
                    avatar: None,
                    skills_dir: f.home.path().join(key).to_string_lossy().into_owned(),
                    project_skills_dir: Some(format!(".test/{key}/skills")),
                    sync_mode: SyncMode::Copy,
                    enabled: true,
                })
                .collect(),
        },
    )
    .unwrap();
    let mut request = f.request(vec![DeploymentScope::Project(project.clone())]);
    request.members[0].tools = vec!["custom_reviewer".into()];
    let initial = f.service.preview_application(request.clone()).unwrap();
    f.service.execute_application(&initial.id).unwrap();
    let reviewer = project.join(".test/custom_reviewer/skills/demo");
    let editor = project.join(".test/custom_editor/skills/demo");
    assert!(!fs::symlink_metadata(&reviewer)
        .unwrap()
        .file_type()
        .is_symlink());
    let original = fs::read(reviewer.join("SKILL.md")).unwrap();
    request.members[0].tools = vec!["custom_editor".into(), "custom_reviewer".into()];
    let pending = f.service.preview_application(request.clone()).unwrap();
    assert_eq!(pending.items[0].targets.len(), 2);

    fs::write(reviewer.join("SKILL.md"), "personal edited content").unwrap();
    fs::write(reviewer.join("personal.md"), "keep my notes").unwrap();
    let check_protected_files = || {
        assert_eq!(
            fs::read_to_string(reviewer.join("SKILL.md")).unwrap(),
            "personal edited content"
        );
        assert_eq!(
            fs::read_to_string(reviewer.join("personal.md")).unwrap(),
            "keep my notes"
        );
        assert!(!editor.exists());
        let skill = f.service.show_skill("demo".into()).unwrap();
        assert_eq!(
            fs::read(std::path::Path::new(&skill.central_path).join("SKILL.md")).unwrap(),
            original
        );
    };
    let rechecked = f.service.recheck_application(&pending.id).unwrap();
    assert!(
        rechecked.items[0].targets.is_empty(),
        "do not show target paths from a stale successful preview"
    );
    let blocked = f.service.preview_application(request).unwrap();
    assert_eq!(blocked.items[0].status, "blocked");
    assert_eq!(blocked.items[0].reason.as_deref(), Some("TARGET_CONFLICT"));
    let conflict = blocked.items[0].conflict.as_ref().unwrap();
    assert_eq!(conflict.agent, "custom_reviewer");
    assert_eq!(conflict.path, reviewer.to_string_lossy());
    assert_eq!(conflict.reason, "target_modified");
    assert_eq!(rechecked.items[0].conflict, blocked.items[0].conflict);
    check_protected_files();
    let result = f.service.execute_application(&blocked.id).unwrap();
    assert_eq!(result.items[0].status, "blocked");
    assert_eq!(result.items[0].conflict, blocked.items[0].conflict);
    check_protected_files();
    let history = f.service.application_history().unwrap();
    assert_eq!(
        history.iter().find(|r| r.id == blocked.id).unwrap().items[0].conflict,
        blocked.items[0].conflict
    );

    // The test user resolves only their disposable copy, then retries the same record.
    fs::write(reviewer.join("SKILL.md"), &original).unwrap();
    fs::remove_file(reviewer.join("personal.md")).unwrap();
    let resolved = f.service.recheck_application(&blocked.id).unwrap();
    assert_eq!(resolved.items[0].status, "pending");
    assert!(resolved.items[0].reason.is_none());
    assert!(resolved.items[0].conflict.is_none());
    assert_eq!(resolved.items[0].targets.len(), 2);
    assert!(serde_json::to_value(&resolved).unwrap()["items"][0]
        .get("conflict")
        .is_none());
    let applied = f.service.execute_application(&blocked.id).unwrap();
    assert_eq!(applied.items[0].status, "done");
    assert!(applied.items[0].conflict.is_none());
    assert_eq!(fs::read(editor.join("SKILL.md")).unwrap(), original);
    assert_eq!(fs::read(reviewer.join("SKILL.md")).unwrap(), original);
}

#[test]
fn historical_conflicts_without_details_remain_readable_and_can_be_rechecked() {
    let f = Fixture::new();
    let target = f.home.path().join(".codex/skills/demo");
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("personal.md"), "keep").unwrap();
    let plan = f
        .service
        .preview_application(f.request(vec![DeploymentScope::Global]))
        .unwrap();
    f.service.execute_application(&plan.id).unwrap();
    let mut legacy: serde_json::Value = f
        .service
        .store()
        .workspace_get("application", &plan.id)
        .unwrap()
        .unwrap();
    legacy["record"]["items"][0]
        .as_object_mut()
        .unwrap()
        .remove("conflict");
    f.service
        .store()
        .workspace_save("application", &plan.id, &legacy)
        .unwrap();
    let old_record = f
        .service
        .application_history()
        .unwrap()
        .into_iter()
        .find(|r| r.id == plan.id)
        .unwrap();
    assert_eq!(
        old_record.items[0].reason.as_deref(),
        Some("TARGET_CONFLICT")
    );
    assert!(old_record.items[0].conflict.is_none());
    let reviewed = f.service.recheck_application(&plan.id).unwrap();
    assert_eq!(
        reviewed.items[0].conflict.as_ref().unwrap().reason,
        "unmanaged_target"
    );
    assert_eq!(
        fs::read_to_string(target.join("personal.md")).unwrap(),
        "keep"
    );
}

#[test]
fn undo_retains_later_explicit_reuse_and_user_modifications() {
    let f = Fixture::new();
    let a = f
        .service
        .preview_application(f.request(vec![DeploymentScope::Global]))
        .unwrap();
    assert_eq!(
        f.service
            .execute_application(&a.id)
            .unwrap()
            .undo_target_count,
        Some(1)
    );
    let b = f
        .service
        .preview_application(f.request(vec![DeploymentScope::Global]))
        .unwrap();
    assert_eq!(b.items[0].action, "existing");
    assert_eq!(
        f.service
            .execute_application(&b.id)
            .unwrap()
            .undo_target_count,
        Some(0)
    );
    assert_eq!(
        f.service
            .application_history()
            .unwrap()
            .iter()
            .find(|record| record.id == a.id)
            .unwrap()
            .undo_target_count,
        Some(1),
        "later reuse must retain the entry point that explains its protection"
    );
    let undo = f.service.preview_application_undo(&a.id).unwrap();
    assert_eq!(undo[0].reason, "laterUse");
    f.service.undo_application(&a.id).unwrap();
    assert!(f.home.path().join(".codex/skills/demo/SKILL.md").exists());

    let project = f.data.path().join("personal");
    fs::create_dir(&project).unwrap();
    let mut request = f.request(vec![DeploymentScope::Project(project.clone())]);
    request.members[0].tools = vec!["cursor".into()];
    let application = f.service.preview_application(request).unwrap();
    f.service.execute_application(&application.id).unwrap();
    let target = project.join(".agents/skills/demo/SKILL.md");
    fs::write(&target, "personal edit after application").unwrap();
    let undo = f.service.preview_application_undo(&application.id).unwrap();
    assert_eq!(undo.len(), 1);
    assert!(!undo[0].can_remove);
    assert_eq!(undo[0].reason, "contentChanged");
    f.service.undo_application(&application.id).unwrap();
    assert_eq!(
        fs::read_to_string(target).unwrap(),
        "personal edit after application"
    );
}

#[test]
fn repairing_an_existing_target_does_not_make_it_undoable() {
    let f = Fixture::new();
    f.service
        .deploy(crate::services::deployment::DeploymentRequest::global(
            "demo",
            ["codex"],
        ))
        .unwrap();
    let before = f.service.store().list_skill_targets(&f.id).unwrap();
    let target = f.home.path().join(".codex/skills/demo");
    crate::core::sync_engine::remove_path_permanently(&target).unwrap();
    let plan = f
        .service
        .preview_application(f.request(vec![DeploymentScope::Global]))
        .unwrap();
    assert_eq!(plan.items[0].action, "repair");
    assert_eq!(
        f.service
            .execute_application(&plan.id)
            .unwrap()
            .undo_target_count,
        Some(0)
    );
    assert_eq!(
        f.service.store().list_skill_targets(&f.id).unwrap()[0].id,
        before[0].id
    );
    assert!(f
        .service
        .preview_application_undo(&plan.id)
        .unwrap()
        .is_empty());
    f.service.undo_application(&plan.id).unwrap();
    assert!(target.join("SKILL.md").exists());
}

#[test]
fn repair_with_a_new_tool_target_exposes_only_the_new_receipt_for_undo() {
    let f = Fixture::new();
    f.service
        .deploy(crate::services::deployment::DeploymentRequest::global(
            "demo",
            ["codex"],
        ))
        .unwrap();
    let original = f.service.store().list_skill_targets(&f.id).unwrap();
    let mut request = f.request(vec![DeploymentScope::Global]);
    request.members[0].tools.push("cursor".into());
    let preview = f.service.preview_application(request).unwrap();
    assert_eq!(preview.items[0].action, "repair");
    assert_eq!(preview.undo_target_count, Some(0));
    let applied = f.service.execute_application(&preview.id).unwrap();
    assert_eq!(applied.undo_target_count, Some(1));
    assert_eq!(
        f.service
            .recheck_application(&applied.id)
            .unwrap()
            .undo_target_count,
        Some(1)
    );
    let undo = f.service.preview_application_undo(&applied.id).unwrap();
    assert_eq!(undo.len(), 1);
    assert_eq!(undo[0].tool, "cursor");
    assert!(undo[0].can_remove);
    f.service.undo_application(&applied.id).unwrap();
    let retained = f.service.store().list_skill_targets(&f.id).unwrap();
    assert_eq!(retained.len(), 1);
    assert_eq!(retained[0].id, original[0].id);
    assert_eq!(retained[0].tool, "codex");
    assert_eq!(retained[0].target_path, original[0].target_path);
    assert!(f.home.path().join(".codex/skills/demo/SKILL.md").exists());
    assert!(!f.home.path().join(".cursor/skills/demo").exists());
    assert_eq!(
        f.service.application_history().unwrap()[0].undo_target_count,
        Some(0)
    );
}

#[test]
fn undo_summary_reads_only_journal_receipts_and_does_not_persist_preview_or_summary() {
    let f = Fixture::new();
    let preview = f
        .service
        .preview_application(f.request(vec![DeploymentScope::Global]))
        .unwrap();
    assert_eq!(
        serde_json::to_value(&preview).unwrap()["undoTargetCount"],
        0
    );
    let applied = f.service.execute_application(&preview.id).unwrap();
    assert_eq!(
        serde_json::to_value(&applied).unwrap()["undoTargetCount"],
        1
    );
    let stored: serde_json::Value = f
        .service
        .store()
        .workspace_get("application", &applied.id)
        .unwrap()
        .unwrap();
    assert!(stored["record"].get("undoTargetCount").is_none());
    let conn = rusqlite::Connection::open(f.service.store().db_path()).unwrap();
    let snapshot = || {
        let mut statement = conn
            .prepare("SELECT kind,id,payload,updated_at FROM workspace_documents ORDER BY kind,id")
            .unwrap();
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            })
            .unwrap();
        rows.collect::<rusqlite::Result<Vec<_>>>().unwrap()
    };
    let before = snapshot();
    let central = std::path::PathBuf::from(&f.service.list_skills().unwrap()[0].central_path);
    let saved = f.data.path().join("temporarily-unavailable-content");
    fs::rename(&central, &saved).unwrap();
    fs::write(&central, "not a skill directory").unwrap();
    // A summary must work with unavailable content and while another connection
    // holds the writer reservation; opening a full undo preview would write.
    conn.execute_batch("BEGIN IMMEDIATE").unwrap();
    let history = f.service.application_history().unwrap();
    assert_eq!(history[0].undo_target_count, Some(1));
    assert_eq!(snapshot(), before);
    conn.execute_batch("ROLLBACK").unwrap();
    assert!(f
        .service
        .store()
        .workspace_get::<serde_json::Value>("undo-preview", &applied.id)
        .unwrap()
        .is_none());
    let undo = f.service.preview_application_undo(&applied.id).unwrap();
    assert!(!undo[0].can_remove);
    assert_eq!(undo[0].reason, "contentChanged");
    fs::remove_file(&central).unwrap();
    fs::rename(saved, central).unwrap();
    assert!(f.service.preview_application_undo(&applied.id).unwrap()[0].can_remove);
    f.service.undo_application(&applied.id).unwrap();
    assert_eq!(
        f.service.application_history().unwrap()[0].undo_target_count,
        Some(0)
    );
}
#[test]
fn undo_requires_preview_and_preserves_central_content() {
    let f = Fixture::new();
    let a = f
        .service
        .preview_application(f.request(vec![DeploymentScope::Global]))
        .unwrap();
    f.service.execute_application(&a.id).unwrap();
    assert!(f.service.undo_application(&a.id).is_err());
    assert!(f.service.preview_application_undo(&a.id).unwrap()[0].can_remove);
    f.service.undo_application(&a.id).unwrap();
    assert!(!f.home.path().join(".codex/skills/demo").exists());
    assert!(
        std::path::Path::new(&f.service.list_skills().unwrap()[0].central_path)
            .join("SKILL.md")
            .exists()
    );
}

#[test]
fn undo_receipts_without_progress_flag_remain_compatible() {
    let f = Fixture::new();
    let plan = f
        .service
        .preview_application(f.request(vec![DeploymentScope::Global]))
        .unwrap();
    f.service.execute_application(&plan.id).unwrap();
    for already_undone in [false, true] {
        let mut journal: serde_json::Value = f
            .service
            .store()
            .workspace_get("application", &plan.id)
            .unwrap()
            .unwrap();
        journal["receipts"][0][0]
            .as_object_mut()
            .unwrap()
            .remove("undone");
        f.service
            .store()
            .workspace_save("application", &plan.id, &journal)
            .unwrap();
        assert_eq!(
            f.service.application_history().unwrap()[0].undo_target_count,
            Some(usize::from(!already_undone))
        );
        let undo = f.service.preview_application_undo(&plan.id).unwrap();
        assert_eq!(undo.len(), 1);
        assert_eq!(undo[0].can_remove, !already_undone);
        if already_undone {
            assert_eq!(undo[0].reason, "alreadyUndone");
        } else {
            f.service.undo_application(&plan.id).unwrap();
        }
    }
    assert!(f
        .service
        .store()
        .list_skill_targets(&f.id)
        .unwrap()
        .is_empty());
}

#[test]
fn migration_keeps_shared_version_and_existing_records_compatible() {
    let f = Fixture::new();
    let conn = rusqlite::Connection::open(f.service.store().db_path()).unwrap();
    conn.execute_batch(
        "DROP TABLE workspace_documents; DELETE FROM settings WHERE key='schema.workspace';",
    )
    .unwrap();
    f.service.store().ensure_schema().unwrap();
    f.service.store().ensure_schema().unwrap();
    assert_eq!(
        conn.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        6
    );
    assert_eq!(
        conn.query_row("SELECT count(*) FROM skills", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
    // The previous stable shared-table write contract still works after upgrade.
    conn.execute(
        "UPDATE skills SET description='old client' WHERE id=?1",
        [&f.id],
    )
    .unwrap();
    assert_eq!(
        f.service.list_skills().unwrap()[0].description.as_deref(),
        Some("old client")
    );
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
#[test]
fn partial_undo_persists_completed_projects_before_a_later_failure() {
    use std::os::unix::fs::PermissionsExt;

    let f = Fixture::new();
    let first = f.data.path().join("first");
    let second = f.data.path().join("second");
    fs::create_dir(&first).unwrap();
    fs::create_dir(&second).unwrap();
    let plan = f
        .service
        .preview_application(f.request(vec![
            DeploymentScope::Project(first.clone()),
            DeploymentScope::Project(second.clone()),
        ]))
        .unwrap();
    f.service.execute_application(&plan.id).unwrap();
    let undo = f.service.preview_application_undo(&plan.id).unwrap();
    assert_eq!(undo.len(), 2);
    assert!(undo.iter().all(|item| item.can_remove));

    let blocked_parent = second.join(".agents/skills");
    let permissions = fs::metadata(&blocked_parent).unwrap().permissions();
    fs::set_permissions(&blocked_parent, fs::Permissions::from_mode(0o555)).unwrap();
    let result = f.service.undo_application(&plan.id);
    fs::set_permissions(&blocked_parent, permissions).unwrap();
    assert!(result.is_err());
    assert!(!first.join(".agents/skills/demo").exists());
    assert!(second.join(".agents/skills/demo/SKILL.md").exists());

    let reopened = SkillsHubService::open(RuntimePaths::from_roots(
        RuntimeProfile::Test,
        f.home.path(),
        f.data.path(),
    ))
    .unwrap();
    let record = reopened
        .application_history()
        .unwrap()
        .into_iter()
        .find(|record| record.id == plan.id)
        .unwrap();
    assert_eq!(record.items[0].status, "undone");
    assert_eq!(record.items[1].status, "done");
    assert_eq!(record.undo_target_count, Some(1));

    let retry = reopened.preview_application_undo(&plan.id).unwrap();
    assert_eq!(retry.iter().filter(|item| item.can_remove).count(), 1);
    assert_eq!(
        retry
            .iter()
            .filter(|item| item.reason == "alreadyUndone")
            .count(),
        1
    );
    reopened.undo_application(&plan.id).unwrap();
    let record = reopened
        .application_history()
        .unwrap()
        .into_iter()
        .find(|record| record.id == plan.id)
        .unwrap();
    assert!(record.items.iter().all(|item| item.status == "undone"));
    assert_eq!(record.undo_target_count, Some(0));
    assert!(reopened
        .store()
        .list_skill_targets(&f.id)
        .unwrap()
        .is_empty());
    assert!(!second.join(".agents/skills/demo").exists());
    assert!(
        std::path::Path::new(&reopened.list_skills().unwrap()[0].central_path)
            .join("SKILL.md")
            .exists()
    );
}

#[test]
fn content_restore_preserves_later_deployments_and_rejects_personal_edits() {
    let f = Fixture::new();
    let source = f.data.path().join("source/SKILL.md");
    fs::write(&source, "---\nname: demo\n---\nnew version").unwrap();
    let plan = f.service.preview_content(&f.id, false).unwrap();
    assert!(!plan.changes.is_empty());
    f.service.execute_content(&plan.id).unwrap();
    assert_eq!(f.service.recoverable_skills().unwrap(), vec![f.id.clone()]);
    let deployment = f
        .service
        .preview_application(f.request(vec![DeploymentScope::Global]))
        .unwrap();
    f.service.execute_application(&deployment.id).unwrap();
    let plan = f.service.preview_content(&f.id, true).unwrap();
    f.service.execute_content(&plan.id).unwrap();
    assert!(f.home.path().join(".codex/skills/demo/SKILL.md").exists());
    assert!(
        fs::read_to_string(f.home.path().join(".codex/skills/demo/SKILL.md"))
            .unwrap()
            .contains("body")
    );
    let paths = RuntimePaths::from_roots(RuntimeProfile::Test, f.home.path(), f.data.path());
    let result = crate::core::installer::update_managed_skill_from_source_with_lock_held(
        &paths,
        f.service.store(),
        &f.id,
        true,
    )
    .unwrap();
    assert!(!result.changed);
    // An explicit update is permitted after restoration; a later edit blocks recovery.
    let plan = f.service.preview_content(&f.id, false).unwrap();
    f.service.execute_content(&plan.id).unwrap();
    fs::write(
        f.home.path().join(".codex/skills/demo/SKILL.md"),
        "personal edit",
    )
    .unwrap();
    assert!(f.service.preview_content(&f.id, true).is_err());
}
#[test]
fn two_skills_keep_independent_recovery_and_stale_source_is_rejected() {
    let f = Fixture::new();
    let other = f.data.path().join("other");
    fs::create_dir(&other).unwrap();
    fs::write(other.join("SKILL.md"), "---\nname: other\n---\nold").unwrap();
    f.service
        .install(InstallRequest::local(other.clone()))
        .unwrap();
    let second = f.service.show_skill("other".into()).unwrap().id;
    fs::write(
        f.data.path().join("source/SKILL.md"),
        "---\nname: demo\n---\nnew",
    )
    .unwrap();
    let stale = f.service.preview_content(&f.id, false).unwrap();
    fs::write(
        f.data.path().join("source/SKILL.md"),
        "---\nname: demo\n---\nnewer",
    )
    .unwrap();
    assert!(f.service.execute_content(&stale.id).is_err());
    for (id, path) in [
        (&f.id, f.data.path().join("source/SKILL.md")),
        (&second, other.join("SKILL.md")),
    ] {
        fs::write(path, "---\nname: updated\n---\nnewest").unwrap();
        let plan = f.service.preview_content(id, false).unwrap();
        f.service.execute_content(&plan.id).unwrap();
    }
    assert_eq!(f.service.recoverable_skills().unwrap().len(), 2);
    let plan = f.service.preview_content(&f.id, true).unwrap();
    f.service.execute_content(&plan.id).unwrap();
    assert_eq!(f.service.recoverable_skills().unwrap(), vec![second]);
}
#[test]
fn shared_project_removal_preserves_the_other_logical_tool() {
    let f = Fixture::new();
    let project = f.data.path().join("project");
    fs::create_dir(&project).unwrap();
    let scope = DeploymentScope::Project(project.clone());
    let mut request = f.request(vec![scope.clone()]);
    request.members[0].tools = vec!["codex".into(), "cursor".into()];
    let plan = f.service.preview_application(request).unwrap();
    f.service.execute_application(&plan.id).unwrap();
    let mut remove = f.request(vec![scope]);
    remove.remove = true;
    let plan = f.service.preview_application(remove).unwrap();
    f.service.execute_application(&plan.id).unwrap();
    let rows = f.service.store().list_skill_targets(&f.id).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].tool, "cursor");
    assert!(project.join(".agents/skills/demo/SKILL.md").exists());
}

#[test]
fn undo_shared_targets_retains_only_the_later_explicitly_reused_tool() {
    let f = Fixture::new();
    let project = f.data.path().join("project");
    fs::create_dir(&project).unwrap();
    let scope = DeploymentScope::Project(project.clone());
    let mut original = f.request(vec![scope.clone()]);
    original.members[0].tools = vec!["codex".into(), "cursor".into()];
    let original = f.service.preview_application(original).unwrap();
    f.service.execute_application(&original.id).unwrap();
    assert_eq!(
        f.service.store().list_skill_targets(&f.id).unwrap().len(),
        2
    );

    let reuse = f
        .service
        .preview_application(f.request(vec![scope]))
        .unwrap();
    assert_eq!(reuse.items[0].action, "existing");
    f.service.execute_application(&reuse.id).unwrap();

    let undo = f.service.preview_application_undo(&original.id).unwrap();
    let codex = undo.iter().find(|item| item.tool == "codex").unwrap();
    assert!(!codex.can_remove);
    assert_eq!(codex.reason, "laterUse");
    let cursor = undo.iter().find(|item| item.tool == "cursor").unwrap();
    assert!(
        cursor.can_remove,
        "Cursor was not selected by the later operation"
    );
    f.service.undo_application(&original.id).unwrap();

    let targets = f.service.store().list_skill_targets(&f.id).unwrap();
    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].tool, "codex");
    assert!(project.join(".agents/skills/demo/SKILL.md").exists());
    let central = f.service.list_skills().unwrap()[0].central_path.clone();
    assert!(
        fs::read_to_string(std::path::Path::new(&central).join("SKILL.md"))
            .unwrap()
            .contains("body")
    );
    let reopened = SkillsHubService::open(RuntimePaths::from_roots(
        RuntimeProfile::Test,
        f.home.path(),
        f.data.path(),
    ))
    .unwrap();
    let after = reopened.preview_application_undo(&original.id).unwrap();
    assert!(after.iter().all(|item| !item.can_remove));
    assert_eq!(
        reopened
            .application_history()
            .unwrap()
            .iter()
            .find(|record| record.id == original.id)
            .unwrap()
            .undo_target_count,
        Some(1),
        "one protected receipt remains even though the other receipt was undone"
    );
    assert_eq!(
        after
            .iter()
            .find(|item| item.tool == "codex")
            .unwrap()
            .reason,
        "laterUse"
    );
    assert_eq!(
        after
            .iter()
            .find(|item| item.tool == "cursor")
            .unwrap()
            .reason,
        "alreadyUndone"
    );
}

#[test]
fn source_rebinding_preserves_identity_content_targets_and_rechecks_preview() {
    let f = Fixture::new();
    let apply = f
        .service
        .preview_application(f.request(vec![DeploymentScope::Global]))
        .unwrap();
    f.service.execute_application(&apply.id).unwrap();
    let before = f.service.store().get_skill_by_id(&f.id).unwrap().unwrap();
    let rows = f.service.store().list_skill_targets(&f.id).unwrap();
    let replacement = f.data.path().join("replacement");
    fs::create_dir(&replacement).unwrap();
    fs::write(
        replacement.join("SKILL.md"),
        "---\nname: different\n---\nnew source",
    )
    .unwrap();
    let plan = f
        .service
        .preview_source_repair(&f.id, "local", replacement.to_str().unwrap(), None)
        .unwrap();
    assert_eq!(
        f.service.store().get_skill_by_id(&f.id).unwrap().unwrap(),
        before
    );
    f.service.apply_source_repair(&plan.id).unwrap();
    let after = f.service.store().get_skill_by_id(&f.id).unwrap().unwrap();
    assert_eq!(before.id, after.id);
    assert_eq!(before.content_hash, after.content_hash);
    assert_eq!(before.name, after.name);
    assert_eq!(rows, f.service.store().list_skill_targets(&f.id).unwrap());
    assert!(
        fs::read_to_string(std::path::Path::new(&after.central_path).join("SKILL.md"))
            .unwrap()
            .contains("body")
    );
    let plan = f
        .service
        .preview_source_repair(&f.id, "local", replacement.to_str().unwrap(), None)
        .unwrap();
    fs::write(replacement.join("SKILL.md"), "changed after preview").unwrap();
    assert!(f.service.apply_source_repair(&plan.id).is_err());
}
#[test]
fn retry_only_rechecks_unfinished_targets_and_history_keeps_original_preset() {
    let f = Fixture::new();
    let p = f
        .service
        .save_preset(Preset {
            id: String::new(),
            revision: 0,
            name: "Original".into(),
            description: String::new(),
            members: f.members(),
        })
        .unwrap();
    let missing = f.data.path().join("later");
    let good = f.data.path().join("good");
    fs::create_dir(&good).unwrap();
    let mut req = f.request(vec![
        DeploymentScope::Project(good),
        DeploymentScope::Project(missing.clone()),
    ]);
    req.preset_id = Some(p.id.clone());
    let plan = f.service.preview_application(req).unwrap();
    let result = f.service.execute_application(&plan.id).unwrap();
    assert_eq!(result.items[1].status, "blocked");
    let original_row = f.service.store().list_skill_targets(&f.id).unwrap()[0].clone();
    fs::create_dir(&missing).unwrap();
    let recheck = f.service.recheck_application(&plan.id).unwrap();
    assert_eq!(recheck.items[0].status, "done");
    assert_eq!(recheck.items[1].status, "pending");
    f.service.execute_application(&plan.id).unwrap();
    assert!(f
        .service
        .store()
        .list_skill_targets(&f.id)
        .unwrap()
        .contains(&original_row));
    f.service.delete_preset(&p.id, p.revision).unwrap();
    assert_eq!(
        f.service.application_history().unwrap()[0]
            .preset
            .as_ref()
            .unwrap()
            .name,
        "Original"
    );
}
#[test]
fn full_history_keeps_immutable_snapshots_and_undo_after_preset_deletion() {
    let f = Fixture::new();
    let second_source = f.data.path().join("second-source");
    fs::create_dir(&second_source).unwrap();
    fs::write(
        second_source.join("SKILL.md"),
        "---\nname: second\ndescription: Second\n---\nsecond body",
    )
    .unwrap();
    f.service
        .install(InstallRequest::local(second_source))
        .unwrap();
    let second_id = f
        .service
        .list_skills()
        .unwrap()
        .into_iter()
        .find(|skill| skill.name == "second")
        .unwrap()
        .id;
    let mut members = f.members();
    members.push(PresetMember {
        skill_id: second_id.clone(),
        tools: vec!["cursor".into()],
    });
    let preset = f
        .service
        .save_preset(Preset {
            id: String::new(),
            name: "Original".into(),
            description: "Original description".into(),
            members: members.clone(),
            revision: 0,
        })
        .unwrap();
    let mut oldest_id = String::new();
    for index in 0..25 {
        let project = f.data.path().join(format!("history-{index}"));
        fs::create_dir(&project).unwrap();
        let record = f
            .service
            .preview_application(ApplicationRequest {
                preset_id: Some(preset.id.clone()),
                members: members.clone(),
                scopes: vec![DeploymentScope::Project(project)],
                remove: false,
            })
            .unwrap();
        if index == 0 {
            oldest_id = record.id.clone();
        }
        let result = f.service.execute_application(&record.id).unwrap();
        assert!(result.items.iter().all(|item| item.status == "done"));
    }
    let mut before = f.service.application_history().unwrap();
    assert_eq!(before.len(), 25);
    assert_eq!(
        before
            .iter()
            .map(|record| &record.id)
            .collect::<std::collections::HashSet<_>>()
            .len(),
        25
    );
    before.sort_by(|a, b| a.id.cmp(&b.id));
    let mut edited = preset.clone();
    edited.name = "Renamed".into();
    edited.description = "Changed description".into();
    edited.members = vec![PresetMember {
        skill_id: second_id.clone(),
        tools: vec!["codex".into(), "cursor".into()],
    }];
    let edited = f.service.save_preset(edited).unwrap();
    f.service
        .delete_preset(&edited.id, edited.revision)
        .unwrap();

    let reopened = SkillsHubService::open(RuntimePaths::from_roots(
        RuntimeProfile::Test,
        f.home.path(),
        f.data.path(),
    ))
    .unwrap();
    assert!(reopened.list_presets().unwrap().is_empty());
    let mut after = reopened.application_history().unwrap();
    after.sort_by(|a, b| a.id.cmp(&b.id));
    assert_eq!(
        serde_json::to_value(&after).unwrap(),
        serde_json::to_value(&before).unwrap()
    );
    let undo = reopened.preview_application_undo(&oldest_id).unwrap();
    assert_eq!(undo.len(), 2);
    assert!(undo.iter().all(|item| item.can_remove));
    reopened.undo_application(&oldest_id).unwrap();
    assert_eq!(
        reopened.store().list_skill_targets(&f.id).unwrap().len(),
        24
    );
    assert_eq!(
        reopened
            .store()
            .list_skill_targets(&second_id)
            .unwrap()
            .len(),
        24
    );
    assert!(!f.data.path().join("history-0/.agents/skills/demo").exists());
    assert!(!f
        .data
        .path()
        .join("history-0/.agents/skills/second")
        .exists());
    assert!(f
        .data
        .path()
        .join("history-24/.agents/skills/demo/SKILL.md")
        .exists());
    assert!(f
        .data
        .path()
        .join("history-24/.agents/skills/second/SKILL.md")
        .exists());
}

#[test]
fn later_legacy_deploy_and_recreated_target_are_not_undone() {
    let f = Fixture::new();
    let plan = f
        .service
        .preview_application(f.request(vec![DeploymentScope::Global]))
        .unwrap();
    f.service.execute_application(&plan.id).unwrap();
    f.service
        .deploy(crate::services::deployment::DeploymentRequest::global(
            "demo",
            ["codex"],
        ))
        .unwrap();
    assert!(!f.service.preview_application_undo(&plan.id).unwrap()[0].can_remove);
    let project = f.data.path().join("p");
    fs::create_dir(&project).unwrap();
    let mut req = f.request(vec![DeploymentScope::Project(project.clone())]);
    req.members[0].tools = vec!["cursor".into()];
    let plan = f.service.preview_application(req).unwrap();
    f.service.execute_application(&plan.id).unwrap();
    let target = project.join(".agents/skills/demo");
    let moved = project.join("old-demo");
    fs::rename(&target, &moved).unwrap();
    crate::core::sync_engine::copy_dir_recursive(&moved, &target).unwrap();
    assert!(!f.service.preview_application_undo(&plan.id).unwrap()[0].can_remove);
    assert!(target.join("SKILL.md").exists());
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
#[test]
fn retry_from_project_alias_keeps_new_targets_undoable() {
    let f = Fixture::new();
    let project = f.data.path().join("project");
    let alias = f.data.path().join("project-alias");
    let target = project.join(".agents/skills/demo");
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("personal.md"), "unmanaged content").unwrap();
    std::os::unix::fs::symlink(&project, &alias).unwrap();
    let plan = f
        .service
        .preview_application(f.request(vec![DeploymentScope::Project(alias)]))
        .unwrap();
    assert_eq!(plan.items[0].status, "blocked");
    f.service.execute_application(&plan.id).unwrap();

    fs::remove_dir_all(&target).unwrap();
    let recheck = f.service.recheck_application(&plan.id).unwrap();
    assert_eq!(recheck.items[0].status, "pending");
    let result = f.service.execute_application(&plan.id).unwrap();
    assert_eq!(result.items[0].status, "done");
    assert!(target.join("SKILL.md").exists());

    let reopened = SkillsHubService::open(RuntimePaths::from_roots(
        RuntimeProfile::Test,
        f.home.path(),
        f.data.path(),
    ))
    .unwrap();
    let undo = reopened.preview_application_undo(&plan.id).unwrap();
    assert_eq!(
        undo.len(),
        1,
        "the retried addition must have an undo receipt"
    );
    assert!(undo[0].can_remove);
    reopened.undo_application(&plan.id).unwrap();
    assert!(!target.exists());
    assert!(reopened
        .store()
        .list_skill_targets(&f.id)
        .unwrap()
        .is_empty());
    assert!(
        std::path::Path::new(&reopened.list_skills().unwrap()[0].central_path)
            .join("SKILL.md")
            .exists()
    );
}

#[test]
fn interrupted_execution_requires_recheck_and_preserves_uncertain_ownership() {
    let f = Fixture::new();
    let plan = f
        .service
        .preview_application(f.request(vec![DeploymentScope::Global]))
        .unwrap();
    let mut journal: serde_json::Value = f
        .service
        .store()
        .workspace_get("application", &plan.id)
        .unwrap()
        .unwrap();
    journal["record"]["confirmed"] = true.into();
    journal["record"]["items"][0]["status"] = "running".into();
    f.service
        .store()
        .workspace_save("application", &plan.id, &journal)
        .unwrap();
    let result = f.service.execute_application(&plan.id).unwrap();
    assert_eq!(result.items[0].status, "interrupted");
    assert!(f.service.list_skills().unwrap()[0].targets.is_empty());
    f.service.recheck_application(&plan.id).unwrap();
    let result = f.service.execute_application(&plan.id).unwrap();
    assert_eq!(result.items[0].status, "done");
}

#[test]
fn automatic_update_preserves_personal_edits_even_when_source_changes() {
    let f = Fixture::new();
    let current = f.service.store().get_skill_by_id(&f.id).unwrap().unwrap();
    fs::write(
        std::path::Path::new(&current.central_path).join("SKILL.md"),
        "personal edit",
    )
    .unwrap();
    fs::write(
        f.data.path().join("source/SKILL.md"),
        "---\nname: demo\n---\nupstream change",
    )
    .unwrap();
    let paths = RuntimePaths::from_roots(RuntimeProfile::Test, f.home.path(), f.data.path());
    let error = crate::core::installer::update_managed_skill_from_source_with_lock_held(
        &paths,
        f.service.store(),
        &f.id,
        true,
    )
    .err()
    .expect("automatic update must reject personal edits");
    assert!(error.to_string().contains("CENTRAL_MODIFIED"));
    assert_eq!(
        fs::read_to_string(std::path::Path::new(&current.central_path).join("SKILL.md")).unwrap(),
        "personal edit"
    );
    assert!(f.service.recoverable_skills().unwrap().is_empty());
}
#[test]
fn source_repair_rejects_embedded_credentials_before_persistence() {
    let f = Fixture::new();
    assert!(f
        .service
        .preview_source_repair(
            &f.id,
            "git",
            "https://user:secret@example.com/repo.git",
            None
        )
        .is_err());
    assert!(f
        .service
        .preview_source_repair(
            &f.id,
            "git",
            "https://example.com/repo.git?token=secret",
            None
        )
        .is_err());
    assert!(f
        .service
        .store()
        .workspace_list::<serde_json::Value>("source-repair")
        .unwrap()
        .is_empty());
}
#[test]
fn stale_undo_confirmation_does_not_replace_the_reviewed_preview() {
    let f = Fixture::new();
    let a = f
        .service
        .preview_application(f.request(vec![DeploymentScope::Global]))
        .unwrap();
    f.service.execute_application(&a.id).unwrap();
    f.service.preview_application_undo(&a.id).unwrap();
    let b = f
        .service
        .preview_application(f.request(vec![DeploymentScope::Global]))
        .unwrap();
    f.service.execute_application(&b.id).unwrap();
    assert!(f.service.undo_application(&a.id).is_err());
    assert!(f.service.undo_application(&a.id).is_err());
    assert!(f.home.path().join(".codex/skills/demo").exists());
}
