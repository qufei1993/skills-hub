use std::path::PathBuf;

use crate::core::skill_store::{SkillRecord, SkillStore, SkillTargetRecord};
use rusqlite::Connection;

fn make_store() -> (tempfile::TempDir, SkillStore) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = dir.path().join("test.db");
    let store = SkillStore::new(db);
    store.ensure_schema().expect("ensure_schema");
    (dir, store)
}

fn make_skill(id: &str, name: &str, central_path: &str, updated_at: i64) -> SkillRecord {
    SkillRecord {
        id: id.to_string(),
        name: name.to_string(),
        description: None,
        source_type: "local".to_string(),
        source_ref: Some("/tmp/source".to_string()),
        source_subpath: None,
        source_revision: None,
        central_path: central_path.to_string(),
        content_hash: None,
        created_at: 1,
        updated_at,
        last_sync_at: None,
        last_seen_at: 1,
        enabled: true,
        status: "ok".to_string(),
    }
}

#[test]
fn schema_is_idempotent() {
    let (_dir, store) = make_store();
    store.ensure_schema().expect("ensure_schema again");
}

#[test]
fn migrates_v3_targets_to_global_scope() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = dir.path().join("test.db");
    let conn = Connection::open(&db).unwrap();
    conn.execute_batch(
        "CREATE TABLE skills (
          id TEXT PRIMARY KEY,
          name TEXT NOT NULL,
          description TEXT NULL,
          source_type TEXT NOT NULL,
          source_ref TEXT NULL,
          source_subpath TEXT NULL,
          source_revision TEXT NULL,
          central_path TEXT NOT NULL UNIQUE,
          content_hash TEXT NULL,
          created_at INTEGER NOT NULL,
          updated_at INTEGER NOT NULL,
          last_sync_at INTEGER NULL,
          last_seen_at INTEGER NOT NULL,
          status TEXT NOT NULL
        );
        CREATE TABLE skill_targets (
          id TEXT PRIMARY KEY,
          skill_id TEXT NOT NULL,
          tool TEXT NOT NULL,
          target_path TEXT NOT NULL,
          mode TEXT NOT NULL,
          status TEXT NOT NULL,
          last_error TEXT NULL,
          synced_at INTEGER NULL,
          UNIQUE(skill_id, tool),
          FOREIGN KEY(skill_id) REFERENCES skills(id) ON DELETE CASCADE
        );
        CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
        CREATE TABLE discovered_skills (
          id TEXT PRIMARY KEY,
          tool TEXT NOT NULL,
          found_path TEXT NOT NULL,
          name_guess TEXT NULL,
          fingerprint TEXT NULL,
          found_at INTEGER NOT NULL,
          imported_skill_id TEXT NULL,
          FOREIGN KEY(imported_skill_id) REFERENCES skills(id) ON DELETE SET NULL
        );
        INSERT INTO skills (
          id, name, description, source_type, source_ref, source_subpath, source_revision,
          central_path, content_hash, created_at, updated_at, last_sync_at, last_seen_at, status
        ) VALUES (
          's1', 'S1', NULL, 'local', NULL, NULL, NULL,
          '/central/s1', NULL, 1, 2, NULL, 1, 'ok'
        );
        INSERT INTO skill_targets (
          id, skill_id, tool, target_path, mode, status, last_error, synced_at
        ) VALUES (
          't1', 's1', 'cursor', '/target/s1', 'copy', 'ok', NULL, 3
        );
        PRAGMA user_version = 3;",
    )
    .unwrap();
    drop(conn);

    let store = SkillStore::new(db);
    store.ensure_schema().unwrap();

    let target = store
        .get_skill_target("s1", "cursor", "global", None)
        .unwrap()
        .unwrap();
    assert_eq!(target.target_path, "/target/s1");
    assert_eq!(target.scope, "global");
    assert!(target.project_path.is_none());
}

#[test]
fn settings_roundtrip_and_update() {
    let (_dir, store) = make_store();

    assert_eq!(store.get_setting("missing").unwrap(), None);
    store.set_setting("k", "v1").unwrap();
    assert_eq!(store.get_setting("k").unwrap().as_deref(), Some("v1"));
    store.set_setting("k", "v2").unwrap();
    assert_eq!(store.get_setting("k").unwrap().as_deref(), Some("v2"));

    store.set_onboarding_completed(true).unwrap();
    assert_eq!(
        store
            .get_setting("onboarding_completed")
            .unwrap()
            .as_deref(),
        Some("true")
    );
    store.set_onboarding_completed(false).unwrap();
    assert_eq!(
        store
            .get_setting("onboarding_completed")
            .unwrap()
            .as_deref(),
        Some("false")
    );
}

#[test]
fn central_repo_migration_metadata_is_atomic() {
    let (_dir, store) = make_store();
    let original = make_skill("one", "One", "/old/one", 1);
    store.upsert_skill(&original).unwrap();

    let err = store
        .commit_central_repo_migration(
            &[
                ("one".to_string(), "/new/one".to_string(), 2),
                ("missing".to_string(), "/new/missing".to_string(), 2),
            ],
            "/new",
        )
        .unwrap_err();

    assert!(err.to_string().contains("skill not found"));
    assert_eq!(
        store.get_skill_by_id("one").unwrap().unwrap().central_path,
        "/old/one"
    );
    assert_eq!(store.get_setting("central_repo_path").unwrap(), None);
}

#[test]
fn skills_upsert_list_get_delete() {
    let (_dir, store) = make_store();

    let a = make_skill("a", "A", "/central/a", 10);
    let b = make_skill("b", "B", "/central/b", 20);
    store.upsert_skill(&a).unwrap();
    store.upsert_skill(&b).unwrap();

    let listed = store.list_skills().unwrap();
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].id, "b");
    assert_eq!(listed[1].id, "a");

    let got = store.get_skill_by_id("a").unwrap().unwrap();
    assert_eq!(got.name, "A");

    let mut a2 = a.clone();
    a2.name = "A2".to_string();
    a2.updated_at = 30;
    store.upsert_skill(&a2).unwrap();
    assert_eq!(store.get_skill_by_id("a").unwrap().unwrap().name, "A2");
    assert_eq!(store.list_skills().unwrap()[0].id, "a");

    store.delete_skill("a").unwrap();
    assert!(store.get_skill_by_id("a").unwrap().is_none());
}

#[test]
fn skill_targets_upsert_unique_constraint_and_list_order() {
    let (_dir, store) = make_store();
    let skill = make_skill("s1", "S1", "/central/s1", 1);
    store.upsert_skill(&skill).unwrap();

    let t1 = SkillTargetRecord {
        id: "t1".to_string(),
        skill_id: "s1".to_string(),
        tool: "cursor".to_string(),
        scope: "global".to_string(),
        project_path: None,
        target_path: "/target/1".to_string(),
        mode: "copy".to_string(),
        status: "ok".to_string(),
        last_error: None,
        synced_at: None,
    };
    store.upsert_skill_target(&t1).unwrap();
    assert_eq!(
        store
            .get_skill_target("s1", "cursor", "global", None)
            .unwrap()
            .unwrap()
            .target_path,
        "/target/1"
    );

    let mut t1b = t1.clone();
    t1b.id = "t2".to_string();
    t1b.target_path = "/target/2".to_string();
    store.upsert_skill_target(&t1b).unwrap();
    assert_eq!(
        store
            .get_skill_target("s1", "cursor", "global", None)
            .unwrap()
            .unwrap()
            .id,
        "t1",
        "unique(skill_id, tool) 冲突时应更新现有行而不是替换 id"
    );
    assert_eq!(
        store
            .get_skill_target("s1", "cursor", "global", None)
            .unwrap()
            .unwrap()
            .target_path,
        "/target/2"
    );

    let t2 = SkillTargetRecord {
        id: "t3".to_string(),
        skill_id: "s1".to_string(),
        tool: "claude_code".to_string(),
        scope: "global".to_string(),
        project_path: None,
        target_path: "/target/cc".to_string(),
        mode: "copy".to_string(),
        status: "ok".to_string(),
        last_error: None,
        synced_at: None,
    };
    store.upsert_skill_target(&t2).unwrap();

    let targets = store.list_skill_targets("s1").unwrap();
    assert_eq!(targets.len(), 2);
    assert_eq!(targets[0].tool, "claude_code");
    assert_eq!(targets[1].tool, "cursor");

    store
        .delete_skill_target("s1", "cursor", "global", None)
        .unwrap();
    assert!(store
        .get_skill_target("s1", "cursor", "global", None)
        .unwrap()
        .is_none());
}

#[test]
fn project_targets_coexist_by_project_path_and_delete_precisely() {
    let (_dir, store) = make_store();
    let skill = make_skill("s1", "S1", "/central/s1", 1);
    store.upsert_skill(&skill).unwrap();

    let global = SkillTargetRecord {
        id: "global".to_string(),
        skill_id: "s1".to_string(),
        tool: "cursor".to_string(),
        scope: "global".to_string(),
        project_path: None,
        target_path: "/global/cursor/s1".to_string(),
        mode: "copy".to_string(),
        status: "ok".to_string(),
        last_error: None,
        synced_at: Some(1),
    };
    let project_a = SkillTargetRecord {
        id: "project-a".to_string(),
        skill_id: "s1".to_string(),
        tool: "cursor".to_string(),
        scope: "project".to_string(),
        project_path: Some("/projects/a".to_string()),
        target_path: "/projects/a/.agents/skills/s1".to_string(),
        mode: "symlink".to_string(),
        status: "ok".to_string(),
        last_error: None,
        synced_at: Some(2),
    };
    let project_b = SkillTargetRecord {
        id: "project-b".to_string(),
        skill_id: "s1".to_string(),
        tool: "cursor".to_string(),
        scope: "project".to_string(),
        project_path: Some("/projects/b".to_string()),
        target_path: "/projects/b/.agents/skills/s1".to_string(),
        mode: "symlink".to_string(),
        status: "ok".to_string(),
        last_error: None,
        synced_at: Some(3),
    };

    store.upsert_skill_target(&global).unwrap();
    store.upsert_skill_target(&project_a).unwrap();
    store.upsert_skill_target(&project_b).unwrap();

    assert_eq!(store.list_skill_targets("s1").unwrap().len(), 3);
    assert_eq!(
        store
            .get_skill_target("s1", "cursor", "global", None)
            .unwrap()
            .unwrap()
            .target_path,
        "/global/cursor/s1"
    );
    assert_eq!(
        store
            .get_skill_target("s1", "cursor", "project", Some("/projects/a"))
            .unwrap()
            .unwrap()
            .target_path,
        "/projects/a/.agents/skills/s1"
    );
    assert_eq!(
        store
            .get_skill_target("s1", "cursor", "project", Some("/projects/b"))
            .unwrap()
            .unwrap()
            .target_path,
        "/projects/b/.agents/skills/s1"
    );

    let mut updated_project_a = project_a.clone();
    updated_project_a.id = "project-a-new-id".to_string();
    updated_project_a.target_path = "/projects/a/.agents/skills/s1-updated".to_string();
    store.upsert_skill_target(&updated_project_a).unwrap();

    let got_project_a = store
        .get_skill_target("s1", "cursor", "project", Some("/projects/a"))
        .unwrap()
        .unwrap();
    assert_eq!(got_project_a.id, "project-a");
    assert_eq!(
        got_project_a.target_path,
        "/projects/a/.agents/skills/s1-updated"
    );
    assert_eq!(store.list_skill_targets("s1").unwrap().len(), 3);

    store
        .delete_skill_target("s1", "cursor", "project", Some("/projects/a"))
        .unwrap();

    assert!(store
        .get_skill_target("s1", "cursor", "project", Some("/projects/a"))
        .unwrap()
        .is_none());
    assert!(store
        .get_skill_target("s1", "cursor", "project", Some("/projects/b"))
        .unwrap()
        .is_some());
    assert!(store
        .get_skill_target("s1", "cursor", "global", None)
        .unwrap()
        .is_some());
}

#[test]
fn deleting_skill_cascades_targets() {
    let (_dir, store) = make_store();
    let skill = make_skill("s1", "S1", "/central/s1", 1);
    store.upsert_skill(&skill).unwrap();

    let t = SkillTargetRecord {
        id: "t1".to_string(),
        skill_id: "s1".to_string(),
        tool: "cursor".to_string(),
        scope: "global".to_string(),
        project_path: None,
        target_path: "/target/1".to_string(),
        mode: "copy".to_string(),
        status: "ok".to_string(),
        last_error: None,
        synced_at: None,
    };
    store.upsert_skill_target(&t).unwrap();
    assert_eq!(store.list_skill_targets("s1").unwrap().len(), 1);

    store.delete_skill("s1").unwrap();
    assert_eq!(store.list_skill_targets("s1").unwrap().len(), 0);
}

#[test]
fn tags_can_be_created_renamed_linked_and_deleted() {
    let (_dir, store) = make_store();
    let skill = make_skill("s1", "S1", "/central/s1", 1);
    store.upsert_skill(&skill).unwrap();

    let frontend = store.create_tag(" Frontend ").unwrap();
    assert_eq!(frontend.name, "Frontend");
    assert!(store.create_tag("frontend").is_err());

    let docs = store.create_tag("Docs").unwrap();
    store.set_skill_tags("s1", &[frontend.id, docs.id]).unwrap();
    store
        .set_skill_tags("s1", &[frontend.id, frontend.id, docs.id])
        .unwrap();

    let linked = store.get_skill_tags("s1").unwrap();
    assert_eq!(linked.len(), 2);
    assert_eq!(linked[0].name, "Docs");
    assert_eq!(linked[1].name, "Frontend");

    let renamed = store.rename_tag(frontend.id, "UI").unwrap();
    assert_eq!(renamed.name, "UI");
    assert!(store.rename_tag(renamed.id, "docs").is_err());

    let tags = store.list_tags_with_counts().unwrap();
    assert_eq!(tags.len(), 2);
    assert_eq!(tags[0].name, "Docs");
    assert_eq!(tags[0].skill_count, 1);
    assert_eq!(tags[1].name, "UI");
    assert_eq!(tags[1].skill_count, 1);

    store.delete_tag(docs.id).unwrap();
    let linked = store.get_skill_tags("s1").unwrap();
    assert_eq!(linked.len(), 1);
    assert_eq!(linked[0].name, "UI");
}

#[test]
fn tag_links_are_removed_when_skill_is_deleted_and_untagged_is_counted() {
    let (_dir, store) = make_store();
    let tagged = make_skill("tagged", "Tagged", "/central/tagged", 2);
    let untagged = make_skill("untagged", "Untagged", "/central/untagged", 1);
    store.upsert_skill(&tagged).unwrap();
    store.upsert_skill(&untagged).unwrap();

    let tag = store.create_tag("Frontend").unwrap();
    store.set_skill_tags("tagged", &[tag.id]).unwrap();

    assert_eq!(store.list_untagged_skill_ids().unwrap(), vec!["untagged"]);

    store.delete_skill("tagged").unwrap();
    assert!(store.get_skill_tags("tagged").unwrap().is_empty());
    assert_eq!(store.list_tags_with_counts().unwrap()[0].skill_count, 0);
}

#[test]
fn description_stored_and_retrieved() {
    let (_dir, store) = make_store();
    let mut skill = make_skill("d1", "D1", "/central/d1", 1);
    skill.description = Some("A test skill description".to_string());
    store.upsert_skill(&skill).unwrap();

    let got = store.get_skill_by_id("d1").unwrap().unwrap();
    assert_eq!(got.description.as_deref(), Some("A test skill description"));
}

#[test]
fn description_null_by_default() {
    let (_dir, store) = make_store();
    let skill = make_skill("d2", "D2", "/central/d2", 1);
    store.upsert_skill(&skill).unwrap();

    let got = store.get_skill_by_id("d2").unwrap().unwrap();
    assert!(got.description.is_none());
}

#[test]
fn update_skill_description_backfills() {
    let (_dir, store) = make_store();
    let skill = make_skill("d3", "D3", "/central/d3", 1);
    store.upsert_skill(&skill).unwrap();

    assert!(store
        .get_skill_by_id("d3")
        .unwrap()
        .unwrap()
        .description
        .is_none());

    store
        .update_skill_description("d3", Some("backfilled"))
        .unwrap();
    assert_eq!(
        store
            .get_skill_by_id("d3")
            .unwrap()
            .unwrap()
            .description
            .as_deref(),
        Some("backfilled")
    );
}

#[test]
fn error_context_includes_db_path() {
    let store = SkillStore::new(PathBuf::from("/this/path/should/not/exist/test.db"));
    let err = store.ensure_schema().unwrap_err();
    let msg = format!("{:#}", err);
    assert!(msg.contains("failed to open db at"), "{msg}");
}
