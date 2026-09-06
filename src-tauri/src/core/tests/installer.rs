use std::fs;
use std::path::{Path, PathBuf};

use crate::core::skill_store::{SkillRecord, SkillStore, SkillTargetRecord};

fn make_store() -> (tempfile::TempDir, SkillStore) {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = SkillStore::new(dir.path().join("test.db"));
    store.ensure_schema().expect("ensure_schema");
    (dir, store)
}

fn set_central_path(store: &SkillStore, central: &Path) {
    store
        .set_setting("central_repo_path", central.to_string_lossy().as_ref())
        .unwrap();
}

#[test]
fn update_lock_rejects_a_second_updater_for_the_same_repository() {
    let dir = tempfile::tempdir().unwrap();
    let _first = super::UpdateFileLock::acquire(dir.path()).unwrap();
    let error = super::UpdateFileLock::acquire(dir.path()).unwrap_err();
    assert!(format!("{error:#}").contains("UPDATE_IN_PROGRESS"));
}

#[test]
fn record_target_sync_failure_preserves_target_and_sets_error() {
    let (_dir, store) = make_store();
    let skill = SkillRecord {
        id: "s1".to_string(),
        name: "S1".to_string(),
        description: None,
        source_type: "local".to_string(),
        source_ref: Some("/tmp/src".to_string()),
        source_subpath: None,
        source_revision: None,
        central_path: "/tmp/central".to_string(),
        content_hash: None,
        created_at: 1,
        updated_at: 1,
        last_sync_at: None,
        last_seen_at: 1,
        enabled: true,
        status: "ok".to_string(),
    };
    store.upsert_skill(&skill).unwrap();
    let target = SkillTargetRecord {
        id: "t1".to_string(),
        skill_id: skill.id,
        tool: "cursor".to_string(),
        scope: "global".to_string(),
        project_path: None,
        target_path: "/tmp/target".to_string(),
        mode: "copy".to_string(),
        status: "ok".to_string(),
        last_error: None,
        synced_at: Some(123),
    };
    store.upsert_skill_target(&target).unwrap();

    super::record_target_sync_failure(&store, &target, "copy failed").unwrap();

    let failed = store
        .get_skill_target("s1", "cursor", "global", None)
        .unwrap()
        .unwrap();
    assert_eq!(failed.status, "error");
    assert_eq!(failed.last_error.as_deref(), Some("copy failed"));
    assert_eq!(failed.target_path, "/tmp/target");
    assert_eq!(failed.synced_at, Some(123));
}

fn init_git_repo(dir: &Path) -> git2::Repository {
    let repo = git2::Repository::init(dir).unwrap();
    let sig = git2::Signature::now("t", "t@example.com").unwrap();

    let mut index = repo.index().unwrap();
    index
        .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
        .unwrap();
    let tree_id = index.write_tree().unwrap();
    {
        let tree = repo.find_tree(tree_id).unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "init", &tree, &[])
            .unwrap();
    }
    repo
}

fn commit_all(repo: &git2::Repository, msg: &str) -> git2::Oid {
    let sig = git2::Signature::now("t", "t@example.com").unwrap();
    let mut index = repo.index().unwrap();
    index
        .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
        .unwrap();
    let tree_id = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();

    let parent = repo
        .head()
        .ok()
        .and_then(|h| h.target())
        .and_then(|oid| repo.find_commit(oid).ok());
    match parent {
        Some(p) => repo
            .commit(Some("HEAD"), &sig, &sig, msg, &tree, &[&p])
            .unwrap(),
        None => repo
            .commit(Some("HEAD"), &sig, &sig, msg, &tree, &[])
            .unwrap(),
    }
}

#[test]
fn parses_github_urls() {
    let p = super::parse_github_url("https://github.com/owner/repo");
    assert_eq!(p.clone_url, "https://github.com/owner/repo.git");
    assert!(p.branch.is_none());
    assert!(p.subpath.is_none());

    let p = super::parse_github_url("anthropics/skills");
    assert_eq!(p.clone_url, "https://github.com/anthropics/skills.git");
    assert!(p.branch.is_none());
    assert!(p.subpath.is_none());

    let p = super::parse_github_url("github.com/owner/repo");
    assert_eq!(p.clone_url, "https://github.com/owner/repo.git");
    assert!(p.branch.is_none());
    assert!(p.subpath.is_none());

    let p = super::parse_github_url("https://github.com/owner/repo/tree/main/skills/x");
    assert_eq!(p.clone_url, "https://github.com/owner/repo.git");
    assert_eq!(p.branch.as_deref(), Some("main"));
    assert_eq!(p.subpath.as_deref(), Some("skills/x"));

    let p = super::parse_github_url("owner/repo/tree/main/skills/x");
    assert_eq!(p.clone_url, "https://github.com/owner/repo.git");
    assert_eq!(p.branch.as_deref(), Some("main"));
    assert_eq!(p.subpath.as_deref(), Some("skills/x"));

    let p = super::parse_github_url("https://github.com/owner/repo/blob/main/skills/x/SKILL.md");
    assert_eq!(p.clone_url, "https://github.com/owner/repo.git");
    assert_eq!(p.branch.as_deref(), Some("main"));
    assert_eq!(p.subpath.as_deref(), Some("skills/x"));

    let p = super::parse_github_url("https://github.com/owner/repo/blob/main/SKILL.md");
    assert_eq!(p.clone_url, "https://github.com/owner/repo.git");
    assert_eq!(p.branch.as_deref(), Some("main"));
    assert_eq!(p.subpath.as_deref(), Some("."));

    let p = super::parse_github_url("/local/path/to/repo");
    assert_eq!(p.clone_url, "/local/path/to/repo");
}

#[test]
fn parses_skill_md_frontmatter() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("SKILL.md");
    fs::write(
        &p,
        r#"---
name: "My Skill"
description: "Desc"
---

body
"#,
    )
    .unwrap();

    let (name, desc) = super::parse_skill_md(&p).unwrap();
    assert_eq!(name, "My Skill");
    assert_eq!(desc.as_deref(), Some("Desc"));
}

#[test]
fn parses_skill_md_frontmatter_literal_description() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("SKILL.md");
    fs::write(
        &p,
        r#"---
name: technical-writer
description: |
  Creates clear documentation, API references, guides, and
  technical content for developers and users.
author: awesome-llm-apps
---

body
"#,
    )
    .unwrap();

    let (name, desc) = super::parse_skill_md(&p).unwrap();
    assert_eq!(name, "technical-writer");
    assert_eq!(
        desc.as_deref(),
        Some("Creates clear documentation, API references, guides, and\ntechnical content for developers and users.")
    );
}

#[test]
fn parses_skill_md_frontmatter_folded_chomp_description() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("SKILL.md");
    fs::write(
        &p,
        r#"---
name: fireworks-tech-graph
description: >-
  Use when the user wants to create any technical diagram - architecture, data
  flow, flowchart, sequence, agent/memory, or concept map - and export as
  SVG+PNG.
---

body
"#,
    )
    .unwrap();

    let (name, desc) = super::parse_skill_md(&p).unwrap();
    assert_eq!(name, "fireworks-tech-graph");
    assert_eq!(
        desc.as_deref(),
        Some(
            "Use when the user wants to create any technical diagram - architecture, data flow, flowchart, sequence, agent/memory, or concept map - and export as SVG+PNG."
        )
    );
}

#[test]
fn backfill_skill_descriptions_replaces_stale_frontmatter_marker() {
    let (_dir, store) = make_store();
    let central = tempfile::tempdir().unwrap();
    fs::write(
        central.path().join("SKILL.md"),
        r#"---
name: fireworks-tech-graph
description: >-
  Correct folded description.
---
"#,
    )
    .unwrap();

    store
        .upsert_skill(&SkillRecord {
            id: "fireworks".to_string(),
            name: "fireworks-tech-graph".to_string(),
            description: Some(">-".to_string()),
            source_type: "local".to_string(),
            source_ref: None,
            source_subpath: None,
            source_revision: None,
            central_path: central.path().to_string_lossy().to_string(),
            content_hash: None,
            created_at: 1,
            updated_at: 1,
            last_sync_at: None,
            last_seen_at: 1,
            enabled: true,
            status: "ok".to_string(),
        })
        .unwrap();

    super::backfill_skill_descriptions(&store);

    let skill = store.get_skill_by_id("fireworks").unwrap().unwrap();
    assert_eq!(
        skill.description.as_deref(),
        Some("Correct folded description.")
    );
}

#[test]
fn installs_local_skill_and_updates_from_source() {
    let app = tauri::test::mock_app();
    let (_dir, store) = make_store();

    let central_root = tempfile::tempdir().unwrap();
    set_central_path(&store, central_root.path());

    let source = tempfile::tempdir().unwrap();
    fs::write(source.path().join("SKILL.md"), b"---\nname: x\n---\n").unwrap();
    fs::write(source.path().join("a.txt"), b"v1").unwrap();

    let res = super::install_local_skill(
        app.handle(),
        &store,
        source.path(),
        Some("local1".to_string()),
    )
    .unwrap();
    assert!(res.central_path.exists());

    let skill = store.get_skill_by_id(&res.skill_id).unwrap().unwrap();
    assert_eq!(skill.name, "local1");

    // add a copy target so update will resync it
    let target_root = tempfile::tempdir().unwrap();
    let target = target_root.path().join("target");
    let t = SkillTargetRecord {
        id: "t1".to_string(),
        skill_id: res.skill_id.clone(),
        tool: "unknown_tool".to_string(),
        scope: "global".to_string(),
        project_path: None,
        target_path: target.to_string_lossy().to_string(),
        mode: "copy".to_string(),
        status: "ok".to_string(),
        last_error: None,
        synced_at: None,
    };
    store.upsert_skill_target(&t).unwrap();

    crate::core::sync_engine::copy_dir_recursive(&res.central_path, &target).unwrap();
    fs::create_dir(target.join("__pycache__")).unwrap();
    fs::write(target.join("__pycache__/module.cpython-313.pyc"), b"cache").unwrap();
    fs::write(source.path().join("a.txt"), b"v2").unwrap();
    let up = super::update_managed_skill_from_source(app.handle(), &store, &res.skill_id).unwrap();
    assert_eq!(up.skill_id, res.skill_id);
    assert!(up.updated_targets.contains(&"unknown_tool".to_string()));
    assert!(PathBuf::from(
        store
            .get_skill_by_id(&res.skill_id)
            .unwrap()
            .unwrap()
            .central_path
    )
    .exists());
    assert!(
        target.join("a.txt").exists(),
        "目标路径应存在并包含同步后的文件"
    );
    assert_eq!(fs::read(target.join("a.txt")).unwrap(), b"v2");

    let err = match super::install_local_skill(
        app.handle(),
        &store,
        source.path(),
        Some("local1".to_string()),
    ) {
        Ok(_) => panic!("expected error"),
        Err(e) => e,
    };
    assert!(format!("{:#}", err).contains("skill already exists"));
}

#[test]
fn unchanged_local_skill_skips_central_and_copy_target_replacement() {
    let app = tauri::test::mock_app();
    let (_dir, store) = make_store();
    let central_root = tempfile::tempdir().unwrap();
    set_central_path(&store, central_root.path());

    let source = tempfile::tempdir().unwrap();
    fs::write(source.path().join("SKILL.md"), b"---\nname: x\n---\n").unwrap();
    fs::write(source.path().join("a.txt"), b"same").unwrap();
    let installed = super::install_local_skill(
        app.handle(),
        &store,
        source.path(),
        Some("unchanged".to_string()),
    )
    .unwrap();

    let target_root = tempfile::tempdir().unwrap();
    let target = target_root.path().join("target");
    store
        .upsert_skill_target(&SkillTargetRecord {
            id: "unchanged-target".to_string(),
            skill_id: installed.skill_id.clone(),
            tool: "cursor".to_string(),
            scope: "global".to_string(),
            project_path: None,
            target_path: target.to_string_lossy().to_string(),
            mode: "copy".to_string(),
            status: "ok".to_string(),
            last_error: None,
            synced_at: None,
        })
        .unwrap();

    let result =
        super::update_managed_skill_from_source(app.handle(), &store, &installed.skill_id).unwrap();

    assert!(result.updated_targets.is_empty());
    assert!(!target.exists());
}

#[cfg(unix)]
#[test]
fn changed_skill_does_not_overwrite_a_copy_target_with_unexpected_content() {
    use std::os::unix::fs::symlink;

    let app = tauri::test::mock_app();
    let (_dir, store) = make_store();
    let central_root = tempfile::tempdir().unwrap();
    set_central_path(&store, central_root.path());

    let source = tempfile::tempdir().unwrap();
    fs::write(source.path().join("SKILL.md"), b"---\nname: x\n---\n").unwrap();
    fs::write(source.path().join("a.txt"), b"v1").unwrap();
    let installed = super::install_local_skill(
        app.handle(),
        &store,
        source.path(),
        Some("modified-target".to_string()),
    )
    .unwrap();

    let target_root = tempfile::tempdir().unwrap();
    let manual_content = target_root.path().join("manual-content");
    fs::create_dir_all(&manual_content).unwrap();
    fs::write(manual_content.join("a.txt"), b"manual").unwrap();
    let target = target_root.path().join("target");
    symlink(&manual_content, &target).unwrap();
    store
        .upsert_skill_target(&SkillTargetRecord {
            id: "modified-target".to_string(),
            skill_id: installed.skill_id.clone(),
            tool: "test-copy-tool".to_string(),
            scope: "global".to_string(),
            project_path: None,
            target_path: target.to_string_lossy().to_string(),
            mode: "copy".to_string(),
            status: "ok".to_string(),
            last_error: None,
            synced_at: None,
        })
        .unwrap();
    fs::write(source.path().join("a.txt"), b"v2").unwrap();

    let result =
        super::update_managed_skill_from_source(app.handle(), &store, &installed.skill_id).unwrap();
    assert!(result.changed);

    assert_eq!(fs::read(manual_content.join("a.txt")).unwrap(), b"manual");
    assert!(fs::symlink_metadata(&target)
        .unwrap()
        .file_type()
        .is_symlink());
    let saved_target = store
        .get_skill_target(&installed.skill_id, "test-copy-tool", "global", None)
        .unwrap()
        .unwrap();
    assert_eq!(saved_target.status, "error");
}

#[test]
fn changed_skill_updates_central_and_healthy_targets_despite_modified_copy() {
    let app = tauri::test::mock_app();
    let (_dir, store) = make_store();
    let central_root = tempfile::tempdir().unwrap();
    set_central_path(&store, central_root.path());

    let source = tempfile::tempdir().unwrap();
    fs::write(source.path().join("SKILL.md"), b"---\nname: x\n---\n").unwrap();
    fs::write(source.path().join("a.txt"), b"v1").unwrap();
    let installed = super::install_local_skill(
        app.handle(),
        &store,
        source.path(),
        Some("transactional-update".to_string()),
    )
    .unwrap();
    let saved_before = store.get_skill_by_id(&installed.skill_id).unwrap().unwrap();
    let central_path = PathBuf::from(&saved_before.central_path);

    let targets_root = tempfile::tempdir().unwrap();
    let first_target = targets_root.path().join("first");
    let second_target = targets_root.path().join("second");
    for (id, tool, target) in [
        ("first-target", "aaa", &first_target),
        ("second-target", "zzz", &second_target),
    ] {
        fs::create_dir_all(target).unwrap();
        fs::write(target.join("SKILL.md"), b"---\nname: x\n---\n").unwrap();
        fs::write(target.join("a.txt"), b"v1").unwrap();
        store
            .upsert_skill_target(&SkillTargetRecord {
                id: id.to_string(),
                skill_id: installed.skill_id.clone(),
                tool: tool.to_string(),
                scope: "global".to_string(),
                project_path: None,
                target_path: target.to_string_lossy().to_string(),
                mode: "copy".to_string(),
                status: "ok".to_string(),
                last_error: None,
                synced_at: Some(1),
            })
            .unwrap();
    }
    fs::write(second_target.join("a.txt"), b"manual").unwrap();
    fs::write(source.path().join("a.txt"), b"v2").unwrap();

    let result =
        super::update_managed_skill_from_source(app.handle(), &store, &installed.skill_id).unwrap();
    assert!(result.changed);
    assert_eq!(fs::read(central_path.join("a.txt")).unwrap(), b"v2");
    assert_eq!(fs::read(first_target.join("a.txt")).unwrap(), b"v2");
    assert_eq!(fs::read(second_target.join("a.txt")).unwrap(), b"manual");
    assert_eq!(result.updated_targets, vec!["aaa"]);
    assert_eq!(result.pending_targets, vec!["zzz"]);
    let unchanged =
        super::update_managed_skill_from_source(app.handle(), &store, &installed.skill_id).unwrap();
    assert!(!unchanged.changed);
    assert!(unchanged.updated_targets.is_empty());
    assert!(unchanged.pending_targets.is_empty());
    assert_eq!(fs::read(second_target.join("a.txt")).unwrap(), b"manual");
    fs::write(source.path().join("a.txt"), b"v3").unwrap();
    let next =
        super::update_managed_skill_from_source(app.handle(), &store, &installed.skill_id).unwrap();
    assert_eq!(next.pending_targets, vec!["zzz"]);
    assert_eq!(fs::read(first_target.join("a.txt")).unwrap(), b"v3");
    assert_eq!(fs::read(central_path.join("a.txt")).unwrap(), b"v3");
    assert_eq!(fs::read(second_target.join("a.txt")).unwrap(), b"manual");
    let saved_after = store.get_skill_by_id(&installed.skill_id).unwrap().unwrap();
    assert_ne!(saved_after.content_hash, saved_before.content_hash);
    assert_eq!(saved_after.status, "ok");
    assert_eq!(
        store
            .get_skill_target(&installed.skill_id, "zzz", "global", None)
            .unwrap()
            .unwrap()
            .status,
        "error"
    );
    assert!(fs::read_dir(central_root.path()).unwrap().all(|entry| {
        let name = entry.unwrap().file_name().to_string_lossy().into_owned();
        name == ".skills-hub-update.lock" || !name.starts_with(".skills-hub-")
    }));
    assert!(fs::read_dir(targets_root.path())
        .unwrap()
        .all(|entry| !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".skills-hub-")));
}

#[cfg(unix)]
#[test]
fn unexpected_target_symlink_does_not_block_healthy_copy_updates() {
    use std::os::unix::fs::symlink;

    let app = tauri::test::mock_app();
    let (_dir, store) = make_store();
    let central_root = tempfile::tempdir().unwrap();
    set_central_path(&store, central_root.path());
    let source = tempfile::tempdir().unwrap();
    fs::write(source.path().join("SKILL.md"), b"---\nname: x\n---\n").unwrap();
    fs::write(source.path().join("a.txt"), b"v1").unwrap();
    let installed = super::install_local_skill(
        app.handle(),
        &store,
        source.path(),
        Some("rollback-conflict".to_string()),
    )
    .unwrap();
    let target_root = tempfile::tempdir().unwrap();
    let first = target_root.path().join("first");
    fs::create_dir_all(&first).unwrap();
    fs::write(first.join("SKILL.md"), b"---\nname: x\n---\n").unwrap();
    fs::write(first.join("a.txt"), b"v1").unwrap();
    let manual = target_root.path().join("manual");
    fs::create_dir_all(&manual).unwrap();
    fs::write(manual.join("a.txt"), b"manual").unwrap();
    let second = target_root.path().join("second");
    symlink(&manual, &second).unwrap();
    for (id, tool, target) in [
        ("rollback-first", "aaa", &first),
        ("rollback-second", "zzz", &second),
    ] {
        store
            .upsert_skill_target(&SkillTargetRecord {
                id: id.to_string(),
                skill_id: installed.skill_id.clone(),
                tool: tool.to_string(),
                scope: "global".to_string(),
                project_path: None,
                target_path: target.to_string_lossy().to_string(),
                mode: "copy".to_string(),
                status: "ok".to_string(),
                last_error: None,
                synced_at: None,
            })
            .unwrap();
    }
    fs::write(source.path().join("a.txt"), b"v2").unwrap();
    let result =
        super::update_managed_skill_from_source(app.handle(), &store, &installed.skill_id).unwrap();
    assert!(result.changed);
    assert_eq!(fs::read(first.join("a.txt")).unwrap(), b"v2");
    assert_eq!(fs::read(manual.join("a.txt")).unwrap(), b"manual");
    assert!(fs::symlink_metadata(&second)
        .unwrap()
        .file_type()
        .is_symlink());
    for (tool, status) in [("aaa", "ok"), ("zzz", "error")] {
        assert_eq!(
            store
                .get_skill_target(&installed.skill_id, tool, "global", None)
                .unwrap()
                .unwrap()
                .status,
            status
        );
    }
}

#[test]
fn changed_skill_updates_a_shared_copy_directory_only_once() {
    let app = tauri::test::mock_app();
    let (_dir, store) = make_store();
    let central_root = tempfile::tempdir().unwrap();
    set_central_path(&store, central_root.path());
    let source = tempfile::tempdir().unwrap();
    fs::write(source.path().join("SKILL.md"), b"---\nname: x\n---\n").unwrap();
    fs::write(source.path().join("a.txt"), b"v1").unwrap();
    let installed = super::install_local_skill(
        app.handle(),
        &store,
        source.path(),
        Some("shared-copy".to_string()),
    )
    .unwrap();
    let target_root = tempfile::tempdir().unwrap();
    let target = target_root.path().join("shared");
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("SKILL.md"), b"---\nname: x\n---\n").unwrap();
    fs::write(target.join("a.txt"), b"v1").unwrap();
    for (id, tool) in [("shared-a", "aaa"), ("shared-b", "bbb")] {
        store
            .upsert_skill_target(&SkillTargetRecord {
                id: id.to_string(),
                skill_id: installed.skill_id.clone(),
                tool: tool.to_string(),
                scope: "global".to_string(),
                project_path: None,
                target_path: target.to_string_lossy().to_string(),
                mode: "copy".to_string(),
                status: "ok".to_string(),
                last_error: None,
                synced_at: None,
            })
            .unwrap();
    }
    fs::write(source.path().join("a.txt"), b"v2").unwrap();

    let result =
        super::update_managed_skill_from_source(app.handle(), &store, &installed.skill_id).unwrap();

    assert_eq!(fs::read(target.join("a.txt")).unwrap(), b"v2");
    assert_eq!(result.updated_targets, vec!["aaa", "bbb"]);
    for tool in ["aaa", "bbb"] {
        let saved = store
            .get_skill_target(&installed.skill_id, tool, "global", None)
            .unwrap()
            .unwrap();
        assert_eq!(saved.status, "ok");
        assert!(saved.synced_at.is_some());
    }
}

#[test]
fn failed_update_marks_skill_error_and_success_clears_it() {
    let app = tauri::test::mock_app();
    let (dir, store) = make_store();
    set_central_path(&store, dir.path());
    let central = dir.path().join("central");
    fs::create_dir_all(&central).unwrap();
    fs::write(central.join("SKILL.md"), b"---\nname: x\n---\n").unwrap();
    let source = dir.path().join("missing-source");
    let skill = SkillRecord {
        id: "source-status".to_string(),
        name: "Source Status".to_string(),
        description: None,
        source_type: "local".to_string(),
        source_ref: Some(source.to_string_lossy().to_string()),
        source_subpath: None,
        source_revision: None,
        central_path: central.to_string_lossy().to_string(),
        content_hash: None,
        created_at: 1,
        updated_at: 1,
        last_sync_at: None,
        last_seen_at: 1,
        enabled: true,
        status: "ok".to_string(),
    };
    store.upsert_skill(&skill).unwrap();

    let error = match super::update_managed_skill_from_source(app.handle(), &store, &skill.id) {
        Ok(_) => panic!("expected source update failure"),
        Err(err) => err.to_string(),
    };
    assert!(error.contains("source path not found"), "{error}");
    assert_eq!(
        store.source_checks().unwrap()[&skill.id].0.as_deref(),
        Some("sourceMissing")
    );
    assert_eq!(
        store.get_skill_by_id(&skill.id).unwrap().unwrap().status,
        "error"
    );

    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("SKILL.md"), b"---\nname: x\n---\n").unwrap();
    super::update_managed_skill_from_source(app.handle(), &store, &skill.id).unwrap();
    assert_eq!(store.source_checks().unwrap()[&skill.id].0, None);
    assert_eq!(
        store.get_skill_by_id(&skill.id).unwrap().unwrap().status,
        "ok"
    );

    let blocked_parent = dir.path().join("blocked-parent");
    fs::write(&blocked_parent, b"not a directory").unwrap();
    let target = SkillTargetRecord {
        id: "blocked-target".to_string(),
        skill_id: skill.id.clone(),
        tool: "unknown_tool".to_string(),
        scope: "global".to_string(),
        project_path: None,
        target_path: blocked_parent.join("target").to_string_lossy().to_string(),
        mode: "copy".to_string(),
        status: "ok".to_string(),
        last_error: None,
        synced_at: None,
    };
    store.upsert_skill_target(&target).unwrap();
    fs::write(source.join("a.txt"), b"changed").unwrap();
    let result = super::update_managed_skill_from_source(app.handle(), &store, &skill.id).unwrap();
    assert_eq!(result.pending_targets, vec!["unknown_tool"]);
    assert_eq!(
        store.get_skill_by_id(&skill.id).unwrap().unwrap().status,
        "ok"
    );
    assert_eq!(
        store
            .get_skill_target(&skill.id, "unknown_tool", "global", None)
            .unwrap()
            .unwrap()
            .status,
        "error"
    );
}

#[test]
fn imports_identical_existing_local_skill_but_rejects_different_content() {
    let app = tauri::test::mock_app();
    let (_dir, store) = make_store();
    let central_root = tempfile::tempdir().unwrap();
    set_central_path(&store, central_root.path());

    let original = tempfile::tempdir().unwrap();
    fs::write(original.path().join("SKILL.md"), b"---\nname: x\n---\n").unwrap();
    fs::write(original.path().join("a.txt"), b"same").unwrap();
    let installed = super::install_local_skill(
        app.handle(),
        &store,
        original.path(),
        Some("local1".to_string()),
    )
    .unwrap();

    let discovered = tempfile::tempdir().unwrap();
    fs::write(discovered.path().join("SKILL.md"), b"---\nname: x\n---\n").unwrap();
    fs::write(discovered.path().join("a.txt"), b"same").unwrap();
    let imported = super::import_existing_local_skill(
        app.handle(),
        &store,
        discovered.path(),
        Some("local1".to_string()),
    )
    .unwrap();
    assert_eq!(imported.skill_id, installed.skill_id);
    assert_eq!(imported.central_path, installed.central_path);

    fs::write(discovered.path().join("a.txt"), b"different").unwrap();
    let err = match super::import_existing_local_skill(
        app.handle(),
        &store,
        discovered.path(),
        Some("local1".to_string()),
    ) {
        Ok(_) => panic!("expected error"),
        Err(err) => err,
    };
    assert!(format!("{err:#}").contains("skill already exists"));
}

#[test]
fn auto_update_migrates_legacy_kimi_target_without_removing_old_path() {
    let app = tauri::test::mock_app();
    let (_dir, store) = make_store();
    let central_root = tempfile::tempdir().unwrap();
    set_central_path(&store, central_root.path());

    let source = tempfile::tempdir().unwrap();
    fs::write(source.path().join("SKILL.md"), b"---\nname: x\n---\n").unwrap();
    fs::write(source.path().join("content.txt"), b"v1").unwrap();
    let installed = super::install_local_skill(
        app.handle(),
        &store,
        source.path(),
        Some("local1".to_string()),
    )
    .unwrap();

    let project = tempfile::tempdir().unwrap();
    let legacy_target = project.path().join(".agents/skills/local1");
    fs::create_dir_all(&legacy_target).unwrap();
    fs::write(legacy_target.join("content.txt"), b"v1").unwrap();
    store
        .upsert_skill_target(&SkillTargetRecord {
            id: "legacy-kimi-target".to_string(),
            skill_id: installed.skill_id.clone(),
            tool: "kimi_cli".to_string(),
            scope: "project".to_string(),
            project_path: Some(project.path().to_string_lossy().to_string()),
            target_path: legacy_target.to_string_lossy().to_string(),
            mode: "copy".to_string(),
            status: "ok".to_string(),
            last_error: None,
            synced_at: Some(1),
        })
        .unwrap();

    fs::write(source.path().join("content.txt"), b"v2").unwrap();
    let update =
        super::update_managed_skill_from_source(app.handle(), &store, &installed.skill_id).unwrap();

    let expected_target = project.path().join(".kimi-code/skills/local1");
    assert_eq!(fs::read(legacy_target.join("content.txt")).unwrap(), b"v1");
    assert_eq!(
        fs::read(expected_target.join("content.txt")).unwrap(),
        b"v2"
    );
    assert!(update.updated_targets.contains(&"kimi_cli".to_string()));
    let target = store
        .get_skill_target(
            &installed.skill_id,
            "kimi_cli",
            "project",
            Some(project.path().to_string_lossy().as_ref()),
        )
        .unwrap()
        .unwrap();
    assert_eq!(target.target_path, expected_target.to_string_lossy());
    assert_eq!(target.status, "ok");
    assert!(target.last_error.is_none());
}

#[test]
fn auto_update_preserves_conflicting_new_kimi_target_and_marks_legacy_record_failed() {
    let app = tauri::test::mock_app();
    let (_dir, store) = make_store();
    let central_root = tempfile::tempdir().unwrap();
    set_central_path(&store, central_root.path());

    let source = tempfile::tempdir().unwrap();
    fs::write(source.path().join("SKILL.md"), b"---\nname: x\n---\n").unwrap();
    fs::write(source.path().join("content.txt"), b"v1").unwrap();
    let installed = super::install_local_skill(
        app.handle(),
        &store,
        source.path(),
        Some("local1".to_string()),
    )
    .unwrap();

    let project = tempfile::tempdir().unwrap();
    let legacy_target = project.path().join(".agents/skills/local1");
    let expected_target = project.path().join(".kimi-code/skills/local1");
    fs::create_dir_all(&legacy_target).unwrap();
    fs::write(legacy_target.join("content.txt"), b"legacy").unwrap();
    fs::create_dir_all(&expected_target).unwrap();
    fs::write(expected_target.join("content.txt"), b"user-content").unwrap();
    store
        .upsert_skill_target(&SkillTargetRecord {
            id: "legacy-kimi-conflict".to_string(),
            skill_id: installed.skill_id.clone(),
            tool: "kimi_cli".to_string(),
            scope: "project".to_string(),
            project_path: Some(project.path().to_string_lossy().to_string()),
            target_path: legacy_target.to_string_lossy().to_string(),
            mode: "copy".to_string(),
            status: "ok".to_string(),
            last_error: None,
            synced_at: Some(1),
        })
        .unwrap();

    fs::write(source.path().join("content.txt"), b"v2").unwrap();
    let result =
        super::update_managed_skill_from_source(app.handle(), &store, &installed.skill_id).unwrap();
    assert!(result.changed);

    assert_eq!(
        fs::read(legacy_target.join("content.txt")).unwrap(),
        b"legacy"
    );
    assert_eq!(
        fs::read(expected_target.join("content.txt")).unwrap(),
        b"user-content"
    );
    let target = store
        .get_skill_target(
            &installed.skill_id,
            "kimi_cli",
            "project",
            Some(project.path().to_string_lossy().as_ref()),
        )
        .unwrap()
        .unwrap();
    assert_eq!(target.target_path, legacy_target.to_string_lossy());
    assert_eq!(target.status, "error");
}

#[test]
fn lists_and_installs_git_skills_without_network() {
    let app = tauri::test::mock_app();
    let (_dir, store) = make_store();
    let central_root = tempfile::tempdir().unwrap();
    set_central_path(&store, central_root.path());

    let repo_dir = tempfile::tempdir().unwrap();
    fs::write(repo_dir.path().join("SKILL.md"), "---\nname: Root\n---\n").unwrap();
    fs::create_dir_all(repo_dir.path().join("skills/a")).unwrap();
    fs::write(
        repo_dir.path().join("skills/a/SKILL.md"),
        "---\nname: A\n---\n",
    )
    .unwrap();
    let repo = init_git_repo(repo_dir.path());
    commit_all(&repo, "add skills");

    let candidates = super::list_git_skills(
        app.handle(),
        &store,
        repo_dir.path().to_string_lossy().as_ref(),
    )
    .unwrap();
    let subpaths: Vec<String> = candidates.into_iter().map(|c| c.subpath).collect();
    assert!(subpaths.contains(&".".to_string()));
    assert!(subpaths.iter().any(|s| s.ends_with("skills/a")));

    let res = super::install_git_skill_from_selection(
        app.handle(),
        &store,
        repo_dir.path().to_string_lossy().as_ref(),
        "skills/a",
        None,
    )
    .unwrap();
    assert!(res.central_path.exists());
}

#[test]
fn install_git_skill_errors_on_multi_skills_repo_root() {
    let app = tauri::test::mock_app();
    let (_dir, store) = make_store();
    let central_root = tempfile::tempdir().unwrap();
    set_central_path(&store, central_root.path());

    let repo_dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(repo_dir.path().join("skills/a")).unwrap();
    fs::create_dir_all(repo_dir.path().join("skills/b")).unwrap();
    fs::write(
        repo_dir.path().join("skills/a/SKILL.md"),
        "---\nname: A\n---\n",
    )
    .unwrap();
    fs::write(
        repo_dir.path().join("skills/b/SKILL.md"),
        "---\nname: B\n---\n",
    )
    .unwrap();
    let repo = init_git_repo(repo_dir.path());
    commit_all(&repo, "multi skills");

    let err = match super::install_git_skill(
        app.handle(),
        &store,
        repo_dir.path().to_string_lossy().as_ref(),
        None,
        None,
    ) {
        Ok(_) => panic!("expected error"),
        Err(e) => e,
    };
    assert!(format!("{:#}", err).contains("MULTI_SKILLS|"));
}

#[test]
fn lists_local_skills_with_invalid_entries() {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path();
    fs::create_dir_all(base.join("skills/a")).unwrap();
    fs::create_dir_all(base.join("skills/b")).unwrap();
    fs::create_dir_all(base.join("skills/c")).unwrap();
    fs::create_dir_all(base.join("skills/d")).unwrap();

    fs::write(base.join("skills/a/SKILL.md"), "---\nname: A\n---\n").unwrap();
    fs::write(base.join("skills/c/SKILL.md"), "name: C\n").unwrap();
    fs::write(base.join("skills/d/SKILL.md"), "---\ndescription: D\n---\n").unwrap();

    let list = super::list_local_skills(base).unwrap();

    let find = |subpath: &str| list.iter().find(|c| c.subpath == subpath).cloned();

    let a = find("skills/a").expect("skills/a");
    assert!(a.valid);
    assert_eq!(a.name, "A");

    let b = find("skills/b").expect("skills/b");
    assert!(!b.valid);
    assert_eq!(b.reason.as_deref(), Some("missing_skill_md"));

    let c = find("skills/c").expect("skills/c");
    assert!(!c.valid);
    assert_eq!(c.reason.as_deref(), Some("invalid_frontmatter"));

    let d = find("skills/d").expect("skills/d");
    assert!(!d.valid);
    assert_eq!(d.reason.as_deref(), Some("missing_name"));
}

#[test]
fn install_local_selection_validates_skill_md() {
    let app = tauri::test::mock_app();
    let (_dir, store) = make_store();

    let central_root = tempfile::tempdir().unwrap();
    set_central_path(&store, central_root.path());

    let base = tempfile::tempdir().unwrap();
    fs::create_dir_all(base.path().join("skills/a")).unwrap();
    fs::create_dir_all(base.path().join("skills/b")).unwrap();
    fs::write(
        base.path().join("skills/a/SKILL.md"),
        "---\nname: Local A\n---\n",
    )
    .unwrap();

    let res = super::install_local_skill_from_selection(
        app.handle(),
        &store,
        base.path(),
        "skills/a",
        None,
    )
    .unwrap();
    assert!(res.central_path.exists());
    let skill = store.get_skill_by_id(&res.skill_id).unwrap().unwrap();
    assert_eq!(skill.name, "Local A");

    let err = match super::install_local_skill_from_selection(
        app.handle(),
        &store,
        base.path(),
        "skills/b",
        None,
    ) {
        Ok(_) => panic!("expected error"),
        Err(e) => e,
    };
    assert!(format!("{:#}", err).contains("SKILL_INVALID|missing_skill_md"));
}

/// Issue #28: when a git subpath is "skills", the derived name should be replaced by the
/// SKILL.md name to avoid path duplication (e.g. `~/.claude/skills/skills/`).
#[test]
fn install_git_skill_uses_skill_md_name_over_subpath_skills() {
    let app = tauri::test::mock_app();
    let (_dir, store) = make_store();
    let central_root = tempfile::tempdir().unwrap();
    set_central_path(&store, central_root.path());

    // Build a repo with skills/<folder> where the folder is named "skills" (simulating
    // a URL like https://github.com/owner/repo/tree/main/skills).
    let repo_dir = tempfile::tempdir().unwrap();
    let skills_dir = repo_dir.path().join("skills");
    fs::create_dir_all(&skills_dir).unwrap();
    fs::write(
        skills_dir.join("SKILL.md"),
        "---\nname: my-real-skill\ndescription: A real skill\n---\n",
    )
    .unwrap();
    fs::write(skills_dir.join("helper.txt"), b"data").unwrap();
    let repo = init_git_repo(repo_dir.path());
    commit_all(&repo, "add skill in skills dir");

    // install_git_skill_from_selection with subpath "skills" (no user-provided name)
    let res = super::install_git_skill_from_selection(
        app.handle(),
        &store,
        repo_dir.path().to_string_lossy().as_ref(),
        "skills",
        None,
    )
    .unwrap();

    // The name should be "my-real-skill" from SKILL.md, NOT "skills" from the subpath.
    assert_eq!(res.name, "my-real-skill");
    assert!(res.central_path.ends_with("my-real-skill"));
    assert!(res.central_path.join("SKILL.md").exists());

    let skill = store.get_skill_by_id(&res.skill_id).unwrap().unwrap();
    assert_eq!(skill.name, "my-real-skill");
    assert_eq!(skill.description.as_deref(), Some("A real skill"));
}

#[test]
fn install_git_skill_rejects_container_subpath_without_skill_md() {
    let app = tauri::test::mock_app();
    let (_dir, store) = make_store();
    let central_root = tempfile::tempdir().unwrap();
    set_central_path(&store, central_root.path());

    let repo_dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(
        repo_dir
            .path()
            .join("awesome_agent_skills/technical-writer"),
    )
    .unwrap();
    fs::write(
        repo_dir
            .path()
            .join("awesome_agent_skills/technical-writer/SKILL.md"),
        "---\nname: technical-writer\n---\n",
    )
    .unwrap();
    let repo = init_git_repo(repo_dir.path());
    commit_all(&repo, "add container skill");

    let err = match super::install_git_skill_from_selection(
        app.handle(),
        &store,
        repo_dir.path().to_string_lossy().as_ref(),
        "awesome_agent_skills",
        None,
    ) {
        Ok(_) => panic!("expected invalid skill path"),
        Err(e) => e,
    };
    assert!(format!("{:#}", err).contains("SKILL_INVALID|missing_skill_md"));
}

#[test]
fn install_git_skill_selection_accepts_specific_child_under_container() {
    let app = tauri::test::mock_app();
    let (_dir, store) = make_store();
    let central_root = tempfile::tempdir().unwrap();
    set_central_path(&store, central_root.path());

    let repo_dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(
        repo_dir
            .path()
            .join("awesome_agent_skills/technical-writer"),
    )
    .unwrap();
    fs::write(
        repo_dir
            .path()
            .join("awesome_agent_skills/technical-writer/SKILL.md"),
        "---\nname: technical-writer\ndescription: docs\n---\n",
    )
    .unwrap();
    let repo = init_git_repo(repo_dir.path());
    commit_all(&repo, "add container skill");

    let res = super::install_git_skill_from_selection(
        app.handle(),
        &store,
        repo_dir.path().to_string_lossy().as_ref(),
        "awesome_agent_skills/technical-writer",
        None,
    )
    .unwrap();

    assert_eq!(res.name, "technical-writer");
    assert!(res.central_path.join("SKILL.md").exists());
}

/// Issue #28: when user explicitly provides a name, SKILL.md should NOT override it.
#[test]
fn install_git_skill_respects_user_provided_name() {
    let app = tauri::test::mock_app();
    let (_dir, store) = make_store();
    let central_root = tempfile::tempdir().unwrap();
    set_central_path(&store, central_root.path());

    let repo_dir = tempfile::tempdir().unwrap();
    let skills_dir = repo_dir.path().join("skills");
    fs::create_dir_all(&skills_dir).unwrap();
    fs::write(skills_dir.join("SKILL.md"), "---\nname: md-name\n---\n").unwrap();
    let repo = init_git_repo(repo_dir.path());
    commit_all(&repo, "add skill");

    let res = super::install_git_skill_from_selection(
        app.handle(),
        &store,
        repo_dir.path().to_string_lossy().as_ref(),
        "skills",
        Some("user-custom-name".to_string()),
    )
    .unwrap();

    // User-provided name takes priority.
    assert_eq!(res.name, "user-custom-name");
}

/// Issue #28: install_git_skill (non-selection variant) also uses SKILL.md name.
#[test]
fn install_git_skill_derives_name_from_skill_md() {
    let app = tauri::test::mock_app();
    let (_dir, store) = make_store();
    let central_root = tempfile::tempdir().unwrap();
    set_central_path(&store, central_root.path());

    let repo_dir = tempfile::tempdir().unwrap();
    fs::write(
        repo_dir.path().join("SKILL.md"),
        "---\nname: proper-name\ndescription: desc\n---\n",
    )
    .unwrap();
    let repo = init_git_repo(repo_dir.path());
    commit_all(&repo, "init");

    // The repo name (derived from path) will be something like a temp dir name.
    // After install, the name should be "proper-name" from SKILL.md.
    let res = super::install_git_skill(
        app.handle(),
        &store,
        repo_dir.path().to_string_lossy().as_ref(),
        None,
        None,
    )
    .unwrap();

    assert_eq!(res.name, "proper-name");
    assert!(res.central_path.ends_with("proper-name"));
}

/// Issue #18: repos with skills in root-level subdirectories (no `skills/` parent)
/// should be detected as multi-skill repos.
#[test]
fn install_git_skill_detects_root_level_multi_skills() {
    let app = tauri::test::mock_app();
    let (_dir, store) = make_store();
    let central_root = tempfile::tempdir().unwrap();
    set_central_path(&store, central_root.path());

    // Build a repo with skills directly in root subdirectories (no skills/ parent)
    let repo_dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(repo_dir.path().join("skill-a")).unwrap();
    fs::create_dir_all(repo_dir.path().join("skill-b")).unwrap();
    fs::write(
        repo_dir.path().join("skill-a/SKILL.md"),
        "---\nname: Skill A\n---\n",
    )
    .unwrap();
    fs::write(
        repo_dir.path().join("skill-b/SKILL.md"),
        "---\nname: Skill B\n---\n",
    )
    .unwrap();
    let repo = init_git_repo(repo_dir.path());
    commit_all(&repo, "add root-level skills");

    // install_git_skill should detect multiple skills and bail with MULTI_SKILLS
    let err = match super::install_git_skill(
        app.handle(),
        &store,
        repo_dir.path().to_string_lossy().as_ref(),
        None,
        None,
    ) {
        Ok(_) => panic!("expected MULTI_SKILLS error"),
        Err(e) => e,
    };
    assert!(format!("{:#}", err).contains("MULTI_SKILLS|"));
}

/// Issue #18: list_git_skills should discover skills in root-level subdirectories.
#[test]
fn list_git_skills_finds_root_level_skills() {
    let app = tauri::test::mock_app();
    let (_dir, store) = make_store();
    let central_root = tempfile::tempdir().unwrap();
    set_central_path(&store, central_root.path());

    let repo_dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(repo_dir.path().join("my-skill-1")).unwrap();
    fs::create_dir_all(repo_dir.path().join("my-skill-2")).unwrap();
    fs::create_dir_all(repo_dir.path().join("not-a-skill")).unwrap();
    fs::write(
        repo_dir.path().join("my-skill-1/SKILL.md"),
        "---\nname: First\n---\n",
    )
    .unwrap();
    fs::write(
        repo_dir.path().join("my-skill-2/SKILL.md"),
        "---\nname: Second\n---\n",
    )
    .unwrap();
    // not-a-skill has no SKILL.md — should NOT be discovered
    let repo = init_git_repo(repo_dir.path());
    commit_all(&repo, "add root-level skills");

    let candidates = super::list_git_skills(
        app.handle(),
        &store,
        repo_dir.path().to_string_lossy().as_ref(),
    )
    .unwrap();

    let names: Vec<String> = candidates.iter().map(|c| c.name.clone()).collect();
    assert!(names.contains(&"First".to_string()), "should find First");
    assert!(names.contains(&"Second".to_string()), "should find Second");
    // "not-a-skill" should NOT appear
    assert!(
        !candidates.iter().any(|c| c.subpath.contains("not-a-skill")),
        "should not find not-a-skill"
    );
}

#[test]
fn list_git_skills_finds_root_skill_container_layout() {
    let app = tauri::test::mock_app();
    let (_dir, store) = make_store();
    let central_root = tempfile::tempdir().unwrap();
    set_central_path(&store, central_root.path());

    let repo_dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(repo_dir.path().join("custom-agent-skills/technical-writer")).unwrap();
    fs::write(
        repo_dir
            .path()
            .join("custom-agent-skills/technical-writer/SKILL.md"),
        "---\nname: technical-writer\ndescription: docs\n---\n",
    )
    .unwrap();
    let repo = init_git_repo(repo_dir.path());
    commit_all(&repo, "add container skill");

    let candidates = super::list_git_skills(
        app.handle(),
        &store,
        repo_dir.path().to_string_lossy().as_ref(),
    )
    .unwrap();

    let candidate = candidates
        .iter()
        .find(|c| c.name == "technical-writer")
        .expect("technical-writer should be discovered");
    assert_eq!(candidate.subpath, "custom-agent-skills/technical-writer");
    assert_eq!(candidate.description.as_deref(), Some("docs"));
}

#[test]
fn collect_skill_dirs_finds_skills_under_explicit_container() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("technical-writer")).unwrap();
    fs::create_dir_all(dir.path().join("not-a-skill")).unwrap();
    fs::write(
        dir.path().join("technical-writer/SKILL.md"),
        "---\nname: technical-writer\n---\n",
    )
    .unwrap();

    let dirs = super::collect_skill_dirs(dir.path());
    let rels: Vec<String> = dirs
        .iter()
        .map(|p| {
            p.strip_prefix(dir.path())
                .unwrap_or(p)
                .to_string_lossy()
                .to_string()
        })
        .collect();
    assert_eq!(rels, vec!["technical-writer".to_string()]);
}

#[test]
fn collect_skill_dirs_finds_multiple_skills_under_explicit_container() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("technical-writer")).unwrap();
    fs::create_dir_all(dir.path().join("python-expert")).unwrap();
    fs::create_dir_all(dir.path().join("not-a-skill")).unwrap();
    fs::write(
        dir.path().join("technical-writer/SKILL.md"),
        "---\nname: technical-writer\n---\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("python-expert/SKILL.md"),
        "---\nname: python-expert\n---\n",
    )
    .unwrap();

    let dirs = super::collect_skill_dirs(dir.path());
    let rels: Vec<String> = dirs
        .iter()
        .map(|p| {
            p.strip_prefix(dir.path())
                .unwrap_or(p)
                .to_string_lossy()
                .to_string()
        })
        .collect();
    assert_eq!(
        rels,
        vec!["python-expert".to_string(), "technical-writer".to_string()]
    );
}

#[test]
fn collect_skill_dirs_scans_named_skill_containers_but_not_generic_dirs() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("agent-pack/hidden-skill")).unwrap();
    fs::create_dir_all(dir.path().join("agent-skills/visible-skill")).unwrap();
    fs::write(
        dir.path().join("agent-pack/hidden-skill/SKILL.md"),
        "---\nname: hidden\n---\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("agent-skills/visible-skill/SKILL.md"),
        "---\nname: visible\n---\n",
    )
    .unwrap();

    let dirs = super::collect_skill_dirs(dir.path());
    let rels: Vec<String> = dirs
        .iter()
        .map(|p| {
            p.strip_prefix(dir.path())
                .unwrap_or(p)
                .to_string_lossy()
                .to_string()
        })
        .collect();
    assert_eq!(rels, vec!["agent-skills/visible-skill".to_string()]);
}

#[test]
fn collect_skill_dirs_deduplicates_known_root_containers() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("skills/technical-writer")).unwrap();
    fs::write(
        dir.path().join("skills/technical-writer/SKILL.md"),
        "---\nname: technical-writer\n---\n",
    )
    .unwrap();

    let dirs = super::collect_skill_dirs(dir.path());
    assert_eq!(dirs.len(), 1);
    assert!(dirs[0].ends_with("skills/technical-writer"));
}

#[test]
fn nested_skill_discovery_respects_container_boundaries() {
    let dir = tempfile::tempdir().unwrap();
    let expected = [
        "skills/.curated/category/curated",
        "skills/engineering/code-review",
        "skills/one/two/three/deep",
    ];
    for path in expected.iter().chain(
        [
            "skills/engineering/code-review/examples/sample",
            "skills/one/two/three/four/too-deep",
            "skills/.hidden/ignored",
            "skills/node_modules/ignored",
            "skills/target/ignored",
            "skills/dist/ignored",
            "agent-pack/category/ignored",
            "agent-skills/category/ignored",
        ]
        .iter(),
    ) {
        let skill = dir.path().join(path);
        fs::create_dir_all(&skill).unwrap();
        fs::write(skill.join("SKILL.md"), "---\nname: example\n---\n").unwrap();
    }
    let actual = super::collect_skill_dirs(dir.path());
    assert_eq!(actual, expected.map(|path| dir.path().join(path)));
    let mut selected = Vec::new();
    super::collect_nested_standard_skills(
        &mut selected,
        &dir.path().join("skills"),
        super::MAX_SKILL_SCAN_DEPTH,
    );
    selected.sort();
    assert_eq!(selected, actual);
}

#[cfg(unix)]
#[test]
fn nested_skill_discovery_does_not_follow_symlinks() {
    let dir = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::create_dir_all(outside.path().join("skill")).unwrap();
    fs::write(
        outside.path().join("skill/SKILL.md"),
        "---\nname: outside\n---\n",
    )
    .unwrap();
    fs::create_dir_all(dir.path().join("skills/category")).unwrap();
    std::os::unix::fs::symlink(outside.path(), dir.path().join("skills/category/link")).unwrap();
    std::os::unix::fs::symlink(outside.path(), dir.path().join("skills/.curated")).unwrap();
    std::os::unix::fs::symlink(
        dir.path().join("skills"),
        dir.path().join("skills/category/loop"),
    )
    .unwrap();
    assert!(super::collect_skill_dirs(dir.path()).is_empty());
}

#[test]
fn lists_and_installs_nested_git_skill() {
    let app = tauri::test::mock_app();
    let (_dir, store) = make_store();
    let central = tempfile::tempdir().unwrap();
    set_central_path(&store, central.path());
    let source = tempfile::tempdir().unwrap();
    let subpath = "skills/engineering/code-review";
    fs::create_dir_all(source.path().join(subpath)).unwrap();
    fs::write(
        source.path().join(subpath).join("SKILL.md"),
        "---\nname: code-review\ndescription: Review code\n---\n",
    )
    .unwrap();
    let repo = init_git_repo(source.path());
    commit_all(&repo, "add nested skill");
    let url = source.path().to_string_lossy();
    let candidates = super::list_git_skills(app.handle(), &store, &url).unwrap();
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].subpath, subpath);
    assert_eq!(candidates[0].description.as_deref(), Some("Review code"));
    let installed = super::install_git_skill_from_selection(
        app.handle(),
        &store,
        &url,
        &candidates[0].subpath,
        None,
    )
    .unwrap();
    assert_eq!(installed.name, "code-review");
    assert!(installed.central_path.join("SKILL.md").is_file());
}

#[test]
fn issue_129_discovers_and_installs_skills_across_categories() {
    let app = tauri::test::mock_app();
    let (_dir, store) = make_store();
    let central = tempfile::tempdir().unwrap();
    set_central_path(&store, central.path());
    let source = tempfile::tempdir().unwrap();
    let skills = [
        ("code-review", "skills/engineering/code-review"),
        ("handoff", "skills/productivity/handoff"),
        ("tdd", "skills/engineering/tdd"),
    ];
    for (name, subpath) in skills {
        let path = source.path().join(subpath);
        fs::create_dir_all(&path).unwrap();
        fs::write(
            path.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: Description of {name}\n---\n"),
        )
        .unwrap();
        fs::write(path.join("content.txt"), name).unwrap();
    }
    fs::create_dir_all(source.path().join("skills/deprecated")).unwrap();
    let repo = init_git_repo(source.path());
    commit_all(&repo, "add categorized skills for issue 129");
    let url = source.path().to_string_lossy();

    let candidates = super::list_git_skills(app.handle(), &store, &url).unwrap();
    let actual: Vec<_> = candidates
        .iter()
        .map(|candidate| (candidate.name.as_str(), candidate.subpath.as_str()))
        .collect();
    assert_eq!(actual, skills);

    let error = super::install_git_skill(app.handle(), &store, &url, None, None)
        .err()
        .expect("a categorized multi-skill repo must require selection");
    assert!(format!("{error:#}").contains("MULTI_SKILLS|"));

    for candidate in candidates {
        let installed = super::install_git_skill_from_selection(
            app.handle(),
            &store,
            &url,
            &candidate.subpath,
            None,
        )
        .unwrap();
        assert_eq!(installed.name, candidate.name);
        assert_eq!(
            fs::read_to_string(installed.central_path.join("content.txt")).unwrap(),
            candidate.name
        );
        assert!(!installed.central_path.join("skills").exists());
        let record = store.get_skill_by_id(&installed.skill_id).unwrap().unwrap();
        assert_eq!(
            record.source_subpath.as_deref(),
            Some(candidate.subpath.as_str())
        );
        assert_eq!(record.description, candidate.description);
    }
}
