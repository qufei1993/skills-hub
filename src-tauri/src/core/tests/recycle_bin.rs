use super::{DeletionSource, RecycleBinService};
use crate::core::skill_store::{SkillRecord, SkillStore, SkillTargetRecord};
use std::fs;

fn skill(root: &std::path::Path) -> SkillRecord {
    SkillRecord {
        id: "skill-1".into(),
        name: "wechat-article".into(),
        description: Some("Saved description".into()),
        source_type: "local".into(),
        source_ref: Some("/another/device/project/skill".into()),
        source_subpath: None,
        source_revision: None,
        central_path: root.join("central/wechat-article").to_string_lossy().into(),
        content_hash: Some("hash".into()),
        created_at: 10,
        updated_at: 20,
        last_sync_at: Some(30),
        last_seen_at: 40,
        enabled: false,
        status: "ok".into(),
    }
}

#[test]
fn archive_and_restore_preserves_skill_tags_source_and_targets() {
    let root = tempfile::tempdir().unwrap();
    let store = SkillStore::new(root.path().join("store.db"));
    store.ensure_schema().unwrap();
    let record = skill(root.path());
    fs::create_dir_all(&record.central_path).unwrap();
    fs::write(
        std::path::Path::new(&record.central_path).join("SKILL.md"),
        "saved",
    )
    .unwrap();
    store.upsert_skill(&record).unwrap();
    store
        .set_skill_tag_names(&record.id, &["写作".into(), "内容".into()])
        .unwrap();
    store
        .upsert_skill_target(&SkillTargetRecord {
            id: "target-1".into(),
            skill_id: record.id.clone(),
            tool: "claude-code".into(),
            scope: "global".into(),
            project_path: None,
            target_path: root
                .path()
                .join("tools/claude/wechat-article")
                .to_string_lossy()
                .into(),
            mode: "copy".into(),
            status: "ok".into(),
            last_error: None,
            synced_at: Some(50),
        })
        .unwrap();
    let service = RecycleBinService::new(&store, root.path().join("trash"));

    let item = service
        .archive(&record.id, DeletionSource::Manual, 1_000)
        .unwrap();
    assert!(store.get_skill_by_id(&record.id).unwrap().is_none());
    service.restore(&item.id).unwrap();

    let restored = store.get_skill_by_id(&record.id).unwrap().unwrap();
    assert_eq!(restored.description, record.description);
    assert_eq!(restored.source_ref, record.source_ref);
    assert_eq!(restored.enabled, record.enabled);
    assert_eq!(
        fs::read_to_string(std::path::Path::new(&restored.central_path).join("SKILL.md")).unwrap(),
        "saved"
    );
    let tags = store
        .get_skill_tags(&record.id)
        .unwrap()
        .into_iter()
        .map(|tag| tag.name)
        .collect::<Vec<_>>();
    assert_eq!(tags, vec!["内容", "写作"]);
    assert_eq!(
        store.list_skill_targets(&record.id).unwrap()[0].tool,
        "claude-code"
    );
}

#[test]
fn cleanup_removes_only_expired_items_and_their_files() {
    let root = tempfile::tempdir().unwrap();
    let store = SkillStore::new(root.path().join("store.db"));
    store.ensure_schema().unwrap();
    let service = RecycleBinService::new(&store, root.path().join("trash"));
    let first = skill(root.path());
    fs::create_dir_all(&first.central_path).unwrap();
    fs::write(
        std::path::Path::new(&first.central_path).join("SKILL.md"),
        "expired",
    )
    .unwrap();
    store.upsert_skill(&first).unwrap();
    let expired = service
        .archive(&first.id, DeletionSource::Sync, 1_000)
        .unwrap();
    let mut second = skill(root.path());
    second.id = "skill-2".into();
    second.name = "kept".into();
    second.central_path = root.path().join("central/kept").to_string_lossy().into();
    fs::create_dir_all(&second.central_path).unwrap();
    fs::write(
        std::path::Path::new(&second.central_path).join("SKILL.md"),
        "kept",
    )
    .unwrap();
    store.upsert_skill(&second).unwrap();
    let kept = service
        .archive(&second.id, DeletionSource::Manual, 2_000)
        .unwrap();

    service
        .cleanup_expired(1_000 + 30 * 24 * 60 * 60 * 1_000)
        .unwrap();
    assert!(!std::path::Path::new(&expired.trash_path).exists());
    assert!(std::path::Path::new(&kept.trash_path).exists());
    assert_eq!(service.list().unwrap().len(), 1);
}

#[test]
fn clear_removes_all_items_and_their_files() {
    let root = tempfile::tempdir().unwrap();
    let store = SkillStore::new(root.path().join("store.db"));
    store.ensure_schema().unwrap();
    let service = RecycleBinService::new(&store, root.path().join("trash"));
    let first = skill(root.path());
    fs::create_dir_all(&first.central_path).unwrap();
    fs::write(
        std::path::Path::new(&first.central_path).join("SKILL.md"),
        "first",
    )
    .unwrap();
    store.upsert_skill(&first).unwrap();
    let first_item = service
        .archive(&first.id, DeletionSource::Manual, 1_000)
        .unwrap();

    let mut second = skill(root.path());
    second.id = "skill-2".into();
    second.name = "second".into();
    second.central_path = root.path().join("central/second").to_string_lossy().into();
    fs::create_dir_all(&second.central_path).unwrap();
    fs::write(
        std::path::Path::new(&second.central_path).join("SKILL.md"),
        "second",
    )
    .unwrap();
    store.upsert_skill(&second).unwrap();
    let second_item = service
        .archive(&second.id, DeletionSource::Sync, 2_000)
        .unwrap();

    assert_eq!(
        service
            .clear(&[first_item.id.clone(), second_item.id.clone()])
            .unwrap(),
        2
    );
    assert!(service.list().unwrap().is_empty());
    assert!(!std::path::Path::new(&first_item.trash_path).exists());
    assert!(!std::path::Path::new(&second_item.trash_path).exists());
}

#[test]
fn clear_rejects_a_changed_recycle_bin_snapshot() {
    let root = tempfile::tempdir().unwrap();
    let store = SkillStore::new(root.path().join("store.db"));
    store.ensure_schema().unwrap();
    let service = RecycleBinService::new(&store, root.path().join("trash"));
    let first = skill(root.path());
    fs::create_dir_all(&first.central_path).unwrap();
    fs::write(
        std::path::Path::new(&first.central_path).join("SKILL.md"),
        "first",
    )
    .unwrap();
    store.upsert_skill(&first).unwrap();
    let confirmed = service
        .archive(&first.id, DeletionSource::Manual, 1_000)
        .unwrap();

    let mut second = skill(root.path());
    second.id = "skill-2".into();
    second.name = "second".into();
    second.central_path = root.path().join("central/second").to_string_lossy().into();
    fs::create_dir_all(&second.central_path).unwrap();
    fs::write(
        std::path::Path::new(&second.central_path).join("SKILL.md"),
        "second",
    )
    .unwrap();
    store.upsert_skill(&second).unwrap();
    let added = service
        .archive(&second.id, DeletionSource::Sync, 2_000)
        .unwrap();

    let error = service.clear(&[confirmed.id]).unwrap_err();
    assert!(error.to_string().contains("RECYCLE_BIN_CHANGED"));
    assert_eq!(service.list().unwrap().len(), 2);
    assert!(std::path::Path::new(&added.trash_path).exists());
}

#[test]
fn failed_restore_removes_its_new_copy_and_keeps_the_recycle_bin_item() {
    let root = tempfile::tempdir().unwrap();
    let store = SkillStore::new(root.path().join("store.db"));
    store.ensure_schema().unwrap();
    let original = skill(root.path());
    fs::create_dir_all(&original.central_path).unwrap();
    fs::write(
        std::path::Path::new(&original.central_path).join("SKILL.md"),
        "saved",
    )
    .unwrap();
    store.upsert_skill(&original).unwrap();
    let service = RecycleBinService::new(&store, root.path().join("trash"));
    let item = service
        .archive(&original.id, DeletionSource::Manual, 1_000)
        .unwrap();

    let mut blocker = skill(root.path());
    blocker.id = "blocking-skill".into();
    blocker.name = "blocking-skill".into();
    store.upsert_skill(&blocker).unwrap();

    assert!(service.restore(&item.id).is_err());
    assert!(!std::path::Path::new(&original.central_path).exists());
    assert_eq!(service.list().unwrap().len(), 1);
    assert!(std::path::Path::new(&item.trash_path).exists());
}

#[test]
fn restore_only_recreates_displayed_targets_and_keeps_history_disabled() {
    let root = tempfile::tempdir().unwrap();
    let store = SkillStore::new(root.path().join("store.db"));
    store.ensure_schema().unwrap();
    let record = skill(root.path());
    fs::create_dir_all(&record.central_path).unwrap();
    fs::write(
        std::path::Path::new(&record.central_path).join("SKILL.md"),
        "saved",
    )
    .unwrap();
    store.upsert_skill(&record).unwrap();
    for id in ["visible", "historical"] {
        store
            .upsert_skill_target(&SkillTargetRecord {
                id: id.into(),
                skill_id: record.id.clone(),
                tool: id.into(),
                scope: "global".into(),
                project_path: None,
                target_path: root.path().join(id).to_string_lossy().into(),
                mode: "copy".into(),
                status: "ok".into(),
                last_error: None,
                synced_at: Some(1),
            })
            .unwrap();
    }
    let service = RecycleBinService::new(&store, root.path().join("trash"));
    let item = service
        .archive(&record.id, DeletionSource::Manual, 1_000)
        .unwrap();
    service
        .restore_with_targets(&item.id, Some(&["visible".into()]))
        .unwrap();
    assert!(root.path().join("visible/SKILL.md").exists());
    assert!(!root.path().join("historical").exists());
    let targets = store.list_skill_targets(&record.id).unwrap();
    assert_eq!(
        targets
            .iter()
            .find(|t| t.id == "historical")
            .unwrap()
            .status,
        "disabled"
    );
    assert_eq!(
        targets.iter().find(|t| t.id == "visible").unwrap().status,
        "ok"
    );
}

#[cfg(unix)]
#[test]
fn archive_unlinks_tool_target_without_removing_original_local_source() {
    let root = tempfile::tempdir().unwrap();
    let store = SkillStore::new(root.path().join("store.db"));
    store.ensure_schema().unwrap();
    let mut record = skill(root.path());
    let original = root.path().join("original");
    fs::create_dir_all(&original).unwrap();
    fs::write(original.join("SKILL.md"), "original").unwrap();
    record.source_ref = Some(original.to_string_lossy().into());
    fs::create_dir_all(&record.central_path).unwrap();
    fs::write(
        std::path::Path::new(&record.central_path).join("SKILL.md"),
        "saved",
    )
    .unwrap();
    store.upsert_skill(&record).unwrap();
    let target = root.path().join("tool-link");
    std::os::unix::fs::symlink(&original, &target).unwrap();
    store
        .upsert_skill_target(&SkillTargetRecord {
            id: "link".into(),
            skill_id: record.id.clone(),
            tool: "codex".into(),
            scope: "global".into(),
            project_path: None,
            target_path: target.to_string_lossy().into(),
            mode: "symlink".into(),
            status: "ok".into(),
            last_error: None,
            synced_at: None,
        })
        .unwrap();
    let service = RecycleBinService::new(&store, root.path().join("trash"));
    let item = service
        .archive(&record.id, DeletionSource::Manual, 1_000)
        .unwrap();
    assert!(fs::symlink_metadata(&target).is_err());
    assert_eq!(
        fs::read_to_string(original.join("SKILL.md")).unwrap(),
        "original"
    );
    assert_eq!(
        fs::read_to_string(std::path::Path::new(&item.trash_path).join("SKILL.md")).unwrap(),
        "saved"
    );
}

#[test]
fn archive_database_failure_keeps_original_and_tool_files() {
    let root = tempfile::tempdir().unwrap();
    let db = root.path().join("store.db");
    let store = SkillStore::new(db.clone());
    store.ensure_schema().unwrap();
    let record = skill(root.path());
    fs::create_dir_all(&record.central_path).unwrap();
    fs::write(
        std::path::Path::new(&record.central_path).join("SKILL.md"),
        "saved",
    )
    .unwrap();
    store.upsert_skill(&record).unwrap();
    let target = root.path().join("tool-copy");
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("SKILL.md"), "saved").unwrap();
    store
        .upsert_skill_target(&SkillTargetRecord {
            id: "copy".into(),
            skill_id: record.id.clone(),
            tool: "cursor".into(),
            scope: "global".into(),
            project_path: None,
            target_path: target.to_string_lossy().into(),
            mode: "copy".into(),
            status: "ok".into(),
            last_error: None,
            synced_at: None,
        })
        .unwrap();
    let conn = rusqlite::Connection::open(db).unwrap();
    conn.execute_batch("CREATE TRIGGER reject_archive BEFORE INSERT ON device_sync_tombstones BEGIN SELECT RAISE(FAIL, 'blocked'); END;").unwrap();
    let service = RecycleBinService::new(&store, root.path().join("trash"));
    assert!(service
        .archive(&record.id, DeletionSource::Manual, 1_000)
        .is_err());
    assert!(store.get_skill_by_id(&record.id).unwrap().is_some());
    assert_eq!(
        fs::read_to_string(target.join("SKILL.md")).unwrap(),
        "saved"
    );
    assert_eq!(
        fs::read_to_string(std::path::Path::new(&record.central_path).join("SKILL.md")).unwrap(),
        "saved"
    );
    assert!(service.list().unwrap().is_empty());
    conn.execute_batch("DROP TRIGGER reject_archive;").unwrap();
    let item = service
        .archive(&record.id, DeletionSource::Manual, 1_000)
        .unwrap();
    assert!(!target.exists());
    assert!(!std::path::Path::new(&record.central_path).exists());
    assert_eq!(
        fs::read_to_string(std::path::Path::new(&item.trash_path).join("SKILL.md")).unwrap(),
        "saved"
    );
}

#[test]
fn deleting_a_skill_whose_content_is_already_gone_removes_the_record() {
    let root = tempfile::tempdir().unwrap();
    let store = SkillStore::new(root.path().join("store.db"));
    store.ensure_schema().unwrap();
    let record = skill(root.path());
    // The central folder is deliberately never created: this is a record the library kept
    // after its content disappeared, and deleting it used to fail every time with
    // "Skill content is missing".
    store.upsert_skill(&record).unwrap();
    store
        .upsert_skill_target(&SkillTargetRecord {
            id: "target-1".into(),
            skill_id: record.id.clone(),
            tool: "codex".into(),
            scope: "global".into(),
            project_path: None,
            target_path: root
                .path()
                .join("tools/codex/wechat-article")
                .to_string_lossy()
                .into(),
            mode: "junction".into(),
            status: "ok".into(),
            last_error: None,
            synced_at: Some(50),
        })
        .unwrap();
    let service = RecycleBinService::new(&store, root.path().join("trash"));

    let item = service
        .archive(&record.id, DeletionSource::Manual, 1_000)
        .unwrap();

    assert_eq!(item.skill_name, record.name);
    assert_eq!(item.trash_path, "", "no content was kept");
    assert!(store.get_skill_by_id(&record.id).unwrap().is_none());
    assert!(
        store.list_skill_targets(&record.id).unwrap().is_empty(),
        "tool rows are removed with the record"
    );
    assert!(
        service.list().unwrap().is_empty(),
        "a Skill with no content leaves nothing to restore, so nothing enters the recycle bin"
    );
}
