use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::core::runtime_paths::{open_store, RuntimePaths, RuntimeProfile};
use crate::core::skill_store::{IncompatibleDatabaseError, SkillStore};

fn file_snapshot(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    walkdir::WalkDir::new(root)
        .into_iter()
        .map(|entry| entry.unwrap())
        .map(|entry| {
            let bytes = if entry.file_type().is_file() {
                std::fs::read(entry.path()).unwrap()
            } else {
                Vec::new()
            };
            (
                entry.path().strip_prefix(root).unwrap().to_path_buf(),
                bytes,
            )
        })
        .collect()
}

fn production_legacy_fixture(data: &Path, home: &Path) -> PathBuf {
    let path = data.join("com.tauri.dev/skills_hub.db");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    rusqlite::Connection::open(&path)
        .unwrap()
        .execute_batch(include_str!("../../../tests/fixtures/v0.10.1-schema.sql"))
        .unwrap();
    let store = SkillStore::new(path.clone());
    let central = home.join("production-central");
    let target = home.join("production-agent/demo");
    for directory in [&central, &target] {
        std::fs::create_dir_all(directory).unwrap();
        std::fs::write(directory.join("SKILL.md"), "production content").unwrap();
    }
    store
        .set_setting("central_repo_path", central.to_str().unwrap())
        .unwrap();
    store
        .set_setting("github_token", "legacy-fixture-value")
        .unwrap();
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute("INSERT INTO skills (id,name,source_type,central_path,created_at,updated_at,last_seen_at,status) VALUES ('legacy','legacy','local',?1,1,1,1,'ok')", [central.to_str().unwrap()]).unwrap();
    conn.execute("INSERT INTO skill_targets (id,skill_id,tool,target_path,mode,status) VALUES ('target','legacy','codex',?1,'copy','ok')", [target.to_str().unwrap()]).unwrap();
    path
}

#[test]
fn future_schema_is_rejected_before_legacy_migration_without_touching_any_files() {
    for wal in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let data = root.path().join("data");
        let home = root.path().join("home");
        production_legacy_fixture(&data, &home);
        let paths = RuntimePaths::from_roots(RuntimeProfile::Production, &home, &data);
        std::fs::create_dir_all(&paths.app_data_dir).unwrap();
        SkillStore::new(paths.database_path.clone())
            .ensure_schema()
            .unwrap();
        let conn = rusqlite::Connection::open(&paths.database_path).unwrap();
        if wal {
            conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0;")
                .unwrap();
        }
        conn.pragma_update(None, "user_version", 99).unwrap();
        let backup = paths.database_path.with_extension("bak-existing");
        std::fs::copy(data.join("com.tauri.dev/skills_hub.db"), backup).unwrap();
        let before = file_snapshot(root.path());

        let error = open_store(&paths).unwrap_err();
        assert_eq!(
            error.downcast_ref::<IncompatibleDatabaseError>(),
            Some(&IncompatibleDatabaseError {
                found_version: 99,
                supported_version: 6
            })
        );
        let service = crate::services::skills_hub::SkillsHubService::open(paths).unwrap();
        assert_eq!(
            service.list_skills().unwrap_err().code,
            crate::services::error::ErrorCode::IncompatibleDatabase
        );
        assert_eq!(service.doctor().unwrap().database_status, "incompatible");
        assert_eq!(file_snapshot(root.path()), before, "WAL enabled: {wal}");
    }
}

#[test]
fn test_profile_never_imports_or_scrubs_production_legacy_state() {
    let root = tempfile::tempdir().unwrap();
    let data = root.path().join("data");
    let home = root.path().join("home");
    let legacy = production_legacy_fixture(&data, &home);
    let legacy_before = file_snapshot(legacy.parent().unwrap());
    let home_before = file_snapshot(&home);
    let paths = RuntimePaths::from_roots(RuntimeProfile::Test, &home, &data);
    let store = open_store(&paths).unwrap();
    assert!(store.list_skills().unwrap().is_empty());
    assert!(store.list_skill_targets("legacy").unwrap().is_empty());
    assert_eq!(store.get_setting("central_repo_path").unwrap(), None);
    assert_eq!(
        crate::core::central_repo::resolve_central_repo_path(&paths, &store).unwrap(),
        paths.default_central_repo
    );
    assert_eq!(file_snapshot(legacy.parent().unwrap()), legacy_before);
    assert_eq!(file_snapshot(&home), home_before);
    // Explicit state already stored in this profile remains available on reopen.
    store
        .set_setting(
            "central_repo_path",
            paths.default_central_repo.to_str().unwrap(),
        )
        .unwrap();
    let reopened = open_store(&paths).unwrap();
    assert_eq!(
        reopened
            .get_setting("central_repo_path")
            .unwrap()
            .as_deref(),
        paths.default_central_repo.to_str()
    );
}

#[test]
fn production_still_migrates_previous_stable_legacy_paths_and_targets() {
    let root = tempfile::tempdir().unwrap();
    let data = root.path().join("data");
    let home = root.path().join("home");
    production_legacy_fixture(&data, &home);
    let paths = RuntimePaths::from_roots(RuntimeProfile::Production, &home, &data);
    let store = open_store(&paths).unwrap();
    assert_eq!(store.list_skills().unwrap().len(), 1);
    assert_eq!(store.list_skill_targets("legacy").unwrap().len(), 1);
    assert_eq!(
        store.get_setting("central_repo_path").unwrap().as_deref(),
        home.join("production-central").to_str()
    );
}

#[test]
fn build_profiles_share_data_but_keep_distinct_bridge_namespaces() {
    for (debug, profile, identifier, central, bridge) in [
        (
            true,
            RuntimeProfile::Development,
            "com.qufei1993.skillshub",
            ".skillshub",
            ".skills-hub-dev",
        ),
        (
            false,
            RuntimeProfile::Production,
            "com.qufei1993.skillshub",
            ".skillshub",
            ".skills-hub",
        ),
    ] {
        let selected = RuntimeProfile::for_build(debug);
        assert_eq!(selected, profile);
        let paths = RuntimePaths::from_roots(selected, "/fixture/home", "/fixture/data");
        assert_eq!(
            paths.app_data_dir,
            PathBuf::from("/fixture/data").join(identifier)
        );
        assert_eq!(
            paths.database_path,
            PathBuf::from("/fixture/data")
                .join(identifier)
                .join("skills_hub.db")
        );
        assert_eq!(
            paths.default_central_repo,
            PathBuf::from("/fixture/home").join(central)
        );
        assert_eq!(
            paths.cli_bridge_dir,
            PathBuf::from("/fixture/home").join(bridge).join("bin")
        );
    }
}

#[test]
fn production_and_development_share_data_and_test_paths_remain_separate() {
    let prod = RuntimePaths::from_roots(RuntimeProfile::Production, "/home/may", "/data");
    let dev = RuntimePaths::from_roots(RuntimeProfile::Development, "/home/may", "/data");
    assert_eq!(
        prod.database_path,
        PathBuf::from("/data/com.qufei1993.skillshub/skills_hub.db")
    );
    assert_eq!(dev.database_path, prod.database_path);
    assert_eq!(dev.app_data_dir, prod.app_data_dir);
    assert_eq!(dev.default_central_repo, prod.default_central_repo);
    assert_eq!(dev.git_cache_dir, prod.git_cache_dir);
    assert_eq!(dev.recycle_bin_dir, prod.recycle_bin_dir);
    assert_ne!(dev.cli_bridge_dir, prod.cli_bridge_dir);
    let test = RuntimePaths::from_roots(RuntimeProfile::Test, "/home/may", "/data");
    assert_ne!(test.database_path, prod.database_path);
    assert_ne!(test.default_central_repo, prod.default_central_repo);
    assert_eq!(
        prod.default_central_repo,
        PathBuf::from("/home/may/.skillshub")
    );
    assert_eq!(
        prod.cli_bridge_dir,
        PathBuf::from("/home/may/.skills-hub/bin")
    );
}

#[test]
fn development_reopens_production_data_without_importing_the_old_development_database() {
    let root = tempfile::tempdir().unwrap();
    let data = root.path().join("data");
    let home = root.path().join("home");
    production_legacy_fixture(&data, &home);
    let old_dev = data.join("com.qufei1993.skillshub.dev/skills_hub.db");
    std::fs::create_dir_all(old_dev.parent().unwrap()).unwrap();
    std::fs::write(&old_dev, b"old development database must remain untouched").unwrap();
    let before = file_snapshot(old_dev.parent().unwrap());
    let dev = RuntimePaths::from_roots(RuntimeProfile::Development, &home, &data);
    let prod = RuntimePaths::from_roots(RuntimeProfile::Production, &home, &data);
    let dev_store = open_store(&dev).unwrap();
    assert_eq!(dev_store.list_skills().unwrap().len(), 1);
    assert_eq!(dev_store.list_skill_targets("legacy").unwrap().len(), 1);
    dev_store
        .set_setting("shared-test-setting", "from-development")
        .unwrap();
    let prod_store = open_store(&prod).unwrap();
    assert_eq!(
        prod_store
            .get_setting("shared-test-setting")
            .unwrap()
            .as_deref(),
        Some("from-development")
    );
    prod_store
        .set_setting("shared-test-setting", "from-production")
        .unwrap();
    assert_eq!(
        open_store(&dev)
            .unwrap()
            .get_setting("shared-test-setting")
            .unwrap()
            .as_deref(),
        Some("from-production")
    );
    assert_eq!(file_snapshot(old_dev.parent().unwrap()), before);
}

#[test]
fn local_test_feature_keeps_development_bridge_with_shared_application_data() {
    let expected = if cfg!(any(debug_assertions, feature = "local-test")) {
        RuntimeProfile::Development
    } else {
        RuntimeProfile::Production
    };
    assert_eq!(RuntimeProfile::current(), expected);
    let paths =
        RuntimePaths::from_roots(RuntimeProfile::current(), "/fixture/home", "/fixture/data");
    assert!(paths.app_data_dir.ends_with("com.qufei1993.skillshub"));
    if cfg!(feature = "local-test") {
        assert!(paths.cli_bridge_dir.ends_with(".skills-hub-dev/bin"));
    }
}
