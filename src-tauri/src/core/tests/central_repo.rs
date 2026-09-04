use std::path::PathBuf;

use crate::core::central_repo::{
    ensure_central_repo, plan_central_repo_migration, resolve_central_repo_path,
    validate_central_repo_path_change,
};
use crate::core::skill_store::{SkillRecord, SkillStore};

fn make_store() -> (tempfile::TempDir, SkillStore) {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = SkillStore::new(dir.path().join("test.db"));
    store.ensure_schema().expect("ensure_schema");
    (dir, store)
}

#[test]
fn resolve_uses_setting_when_present() {
    let (dir, store) = make_store();
    let app = tauri::test::mock_app();
    let expected = dir.path().join("central");
    store
        .set_setting("central_repo_path", expected.to_string_lossy().as_ref())
        .unwrap();

    let got = resolve_central_repo_path(app.handle(), &store).unwrap();
    assert_eq!(got, expected);
}

#[test]
fn ensure_central_repo_creates_dir() {
    let dir = tempfile::tempdir().expect("tempdir");
    let p: PathBuf = dir.path().join("a/b/c");
    assert!(!p.exists());
    ensure_central_repo(&p).unwrap();
    assert!(p.exists());
}

#[test]
fn storage_path_rejects_tool_directory_overlap() {
    let dir = tempfile::tempdir().expect("tempdir");
    let current = dir.path().join("central");
    let tool_root = dir.path().join(".agents/skills");

    let err = validate_central_repo_path_change(
        &current,
        &tool_root,
        std::slice::from_ref(&tool_root),
        &[],
    )
    .unwrap_err();

    assert!(err.to_string().contains("tool Skills directory"));
}

#[test]
fn storage_path_rejects_local_source_overlap() {
    let dir = tempfile::tempdir().expect("tempdir");
    let current = dir.path().join("central");
    let source = dir.path().join("workspace/my-skill");
    let destination = dir.path().join("workspace");

    let err =
        validate_central_repo_path_change(&current, &destination, &[], &[source]).unwrap_err();

    assert!(err.to_string().contains("original source directory"));
}

#[test]
fn storage_path_accepts_independent_directory() {
    let dir = tempfile::tempdir().expect("tempdir");
    let current = dir.path().join("central");
    let destination = dir.path().join("new-central");
    let tool_root = dir.path().join("tool/skills");
    let source = dir.path().join("source/my-skill");

    validate_central_repo_path_change(&current, &destination, &[tool_root], &[source]).unwrap();
}

fn skill_record(id: &str, path: PathBuf) -> SkillRecord {
    SkillRecord {
        id: id.to_string(),
        name: id.to_string(),
        description: None,
        source_type: "local".to_string(),
        source_ref: None,
        source_subpath: None,
        source_revision: None,
        central_path: path.to_string_lossy().to_string(),
        content_hash: None,
        created_at: 1,
        updated_at: 1,
        last_sync_at: None,
        last_seen_at: 1,
        enabled: true,
        status: "ok".to_string(),
    }
}

#[test]
fn migration_plan_checks_all_conflicts_before_any_skill_moves() {
    let dir = tempfile::tempdir().expect("tempdir");
    let old_root = dir.path().join("old");
    let new_root = dir.path().join("new");
    let first = old_root.join("first");
    let second = old_root.join("second");
    std::fs::create_dir_all(&first).unwrap();
    std::fs::create_dir_all(&second).unwrap();
    std::fs::create_dir_all(new_root.join("second")).unwrap();
    let skills = vec![
        skill_record("first", first.clone()),
        skill_record("second", second.clone()),
    ];

    let err = plan_central_repo_migration(&skills, &new_root).unwrap_err();

    assert!(err.to_string().contains("target path already exists"));
    assert!(first.exists());
    assert!(second.exists());
    assert!(!new_root.join("first").exists());
}
