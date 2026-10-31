//! Injected only into disposable checkouts; no production module is changed.
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::{types::ValueRef, Connection, OpenFlags};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tauri::Manager;

use crate::core::onboarding::build_onboarding_plan_for_runtime;
use crate::core::recycle_bin::{DeletionSource, RecycleBinService};
use crate::core::runtime_paths::{RuntimePaths, RuntimeProfile};
use crate::core::skill_store::{SkillRecord, SkillTargetRecord};
use crate::core::sync_engine::{sync_dir_with_mode_with_overwrite, SyncMode};
use crate::services::skills_hub::SkillsHubService;

const TOOLS: [(&str, &str); 7] = [
    ("claude_code", ".claude/skills"),
    ("deepseek_harness", ".dsh/skills"),
    ("github_copilot", ".copilot/skills"),
    ("junie", ".junie/skills"),
    ("trae_cn", ".trae-cn/skills"),
    ("workbuddy", ".workbuddy/skills"),
    ("zcode", ".zcode/skills"),
];

fn payload(path: &Path, name: &str, index: usize) {
    fs::create_dir_all(path.join("nested/中文目录/empty")).unwrap();
    fs::write(path.join("SKILL.md"), format!("---\nname: {name}\ndescription: Synthetic issue 208 fixture {index}\n---\n\nNever use real user data.\n")).unwrap();
    fs::write(
        path.join("nested/中文目录/body.txt"),
        format!("synthetic-{index}-中文\n"),
    )
    .unwrap();
    fs::write(path.join("bytes.bin"), (0..=255).collect::<Vec<u8>>()).unwrap();
}

fn paths(root: &Path) -> RuntimePaths {
    assert!(root.is_absolute());
    fs::create_dir_all(root.join("home")).unwrap();
    // Keep SQLite's database path short while varying source/central path length.
    let temporary_root = PathBuf::from(std::env::var_os("ISSUE208_REPRO_ROOT").unwrap());
    let database_key = hex::encode(Sha256::digest(root.to_string_lossy().as_bytes()));
    let mut paths = RuntimePaths::from_roots(
        RuntimeProfile::Test,
        root.join("home"),
        temporary_root.join("databases").join(&database_key[..16]),
    );
    paths.default_central_repo = root.join("home/.skillshub");
    paths
}

fn db_snapshot(service: &SkillsHubService) -> Value {
    let conn = Connection::open_with_flags(
        service.paths().database_path.clone(),
        OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap();
    let schema = conn.prepare("SELECT name, sql FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name").unwrap().query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))).unwrap().collect::<Result<Vec<_>, _>>().unwrap();
    let mut tables = BTreeMap::new();
    for (name, sql) in schema {
        let mut stmt = conn
            .prepare(&format!("SELECT * FROM \"{}\"", name.replace('"', "\"\"")))
            .unwrap();
        let columns = stmt
            .column_names()
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>();
        let mut cursor = stmt.query([]).unwrap();
        let mut rows = Vec::new();
        while let Some(row) = cursor.next().unwrap() {
            let values = (0..columns.len())
                .map(|i| match row.get_ref(i).unwrap() {
                    ValueRef::Null => Value::Null,
                    ValueRef::Integer(v) => json!(v),
                    ValueRef::Real(v) => json!(v),
                    ValueRef::Text(v) => json!(String::from_utf8_lossy(v)),
                    ValueRef::Blob(v) => json!({"blob_hex": hex::encode(v)}),
                })
                .collect::<Vec<_>>();
            rows.push(json!(values));
        }
        rows.sort_by_key(Value::to_string);
        tables.insert(name, json!({"schema":sql,"columns":columns,"rows":rows}));
    }
    json!(tables)
}

#[cfg(windows)]
fn file_identity(path: &Path) -> std::io::Result<Value> {
    use std::os::windows::{fs::OpenOptionsExt, io::AsRawHandle};
    #[repr(C)]
    #[derive(Default)]
    struct Information {
        attributes: u32,
        creation: [u32; 2],
        access: [u32; 2],
        write: [u32; 2],
        volume: u32,
        size_high: u32,
        size_low: u32,
        links: u32,
        index_high: u32,
        index_low: u32,
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn GetFileInformationByHandle(
            handle: *mut std::ffi::c_void,
            information: *mut Information,
        ) -> i32;
    }
    let file = fs::OpenOptions::new()
        .access_mode(0)
        .share_mode(7)
        .custom_flags(0x02200000)
        .open(path)?;
    let mut information = Information::default();
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut information) } == 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(
        json!({"volume":information.volume,"file_id":((information.index_high as u64)<<32)|information.index_low as u64,"attributes":information.attributes,"created":information.creation,"written":information.write}),
    )
}

#[cfg(unix)]
fn file_identity(path: &Path) -> std::io::Result<Value> {
    use std::os::unix::fs::MetadataExt;
    let metadata = fs::symlink_metadata(path)?;
    Ok(
        json!({"device":metadata.dev(),"inode":metadata.ino(),"mode":metadata.mode(),"mtime":metadata.mtime(),"mtime_nsec":metadata.mtime_nsec()}),
    )
}

fn inventory(root: &Path, boundary: &Path) -> Value {
    if fs::symlink_metadata(root).is_err() {
        return json!({"exists":false});
    }
    let mut entries = BTreeMap::new();
    for entry in walkdir::WalkDir::new(root)
        .follow_links(true)
        .sort_by_file_name()
    {
        match entry {
            Err(error) => {
                entries.insert(
                    format!("walk_error:{}", entries.len()),
                    json!({"error":error.to_string()}),
                );
            }
            Ok(entry) => {
                let p = entry.path();
                let relative = p
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                let item = (|| -> std::io::Result<Value> {
                    let physical = fs::canonicalize(p)?;
                    assert!(
                        physical.starts_with(fs::canonicalize(boundary).unwrap()),
                        "fixture escaped its temporary root"
                    );
                    let metadata = fs::symlink_metadata(p)?;
                    let link = fs::read_link(p).ok();
                    let mut value = json!({"kind":if link.is_some(){"link"}else if metadata.is_dir(){"directory"}else{"file"}});
                    value["identity"] = file_identity(p)?;
                    if let Some(link) = link {
                        value["link_target"] = json!(link.to_string_lossy());
                        #[cfg(windows)]
                        {
                            value["junction"] = json!(junction::exists(p).unwrap());
                        }
                    }
                    if entry.file_type().is_file() {
                        let bytes = fs::read(p)?;
                        value["length"] = json!(bytes.len());
                        value["sha256"] = json!(hex::encode(Sha256::digest(&bytes)));
                    }
                    Ok(value)
                })();
                entries.insert(
                    relative,
                    item.unwrap_or_else(
                        |e| json!({"error":e.to_string(),"os_error":e.raw_os_error()}),
                    ),
                );
            }
        }
    }
    json!({"exists":true,"entries":entries})
}

fn snapshot(service: &SkillsHubService, root: &Path) -> Value {
    let boundary = PathBuf::from(std::env::var_os("ISSUE208_REPRO_ROOT").unwrap());
    let home = root.join("home");
    let mut trees = BTreeMap::new();
    trees.insert(
        "codex".to_string(),
        inventory(&home.join(".codex/skills"), &boundary),
    );
    trees.insert(
        "central".to_string(),
        inventory(&service.paths().default_central_repo, &boundary),
    );
    trees.insert(
        "recycle_bin".to_string(),
        inventory(&service.paths().recycle_bin_dir, &boundary),
    );
    for (key, dir) in TOOLS {
        trees.insert(key.to_string(), inventory(&home.join(dir), &boundary));
    }
    json!({"trees":trees,"database":db_snapshot(service)})
}

fn checkpoint(
    service: &SkillsHubService,
    root: &Path,
    output: &Path,
    phase: &str,
    baseline: Option<&Value>,
) -> Value {
    let current = snapshot(service, root);
    fs::write(
        output.join(format!("{phase}.json")),
        serde_json::to_vec_pretty(&current).unwrap(),
    )
    .unwrap();
    fn healthy(value: &Value) -> bool {
        match value {
            Value::Object(map) => !map.contains_key("error") && map.values().all(healthy),
            Value::Array(values) => values.iter().all(healthy),
            _ => true,
        }
    }
    assert!(
        healthy(&current),
        "unreadable fixture content at {phase}; snapshot retained"
    );
    if let Some(baseline) = baseline {
        assert!(
            &current == baseline,
            "filesystem or database changed at {phase}; snapshots retained"
        );
    }
    current
}

fn import(
    app: &tauri::App<tauri::test::MockRuntime>,
    source: &Path,
    name: &str,
) -> Result<Value, String> {
    tauri::async_runtime::block_on(crate::commands::import_existing_skill(
        app.state(),
        source.to_string_lossy().into_owned(),
        Some(name.into()),
    ))
    .map(|v| serde_json::to_value(v).unwrap())
}

fn sync_source(
    app: &tauri::App<tauri::test::MockRuntime>,
    installed: &Value,
) -> Result<Value, String> {
    tauri::async_runtime::block_on(crate::commands::sync_skill_to_tool(
        app.state(),
        installed["central_path"].as_str().unwrap().into(),
        installed["skill_id"].as_str().unwrap().into(),
        "codex".into(),
        installed["name"].as_str().unwrap().into(),
        Some(true),
        Some(true),
        None,
        None,
    ))
    .map(|v| serde_json::to_value(v).unwrap())
}

fn deploy_junction(
    app: &tauri::App<tauri::test::MockRuntime>,
    service: &SkillsHubService,
    root: &Path,
    installed: &Value,
    key: &str,
    relative: &str,
) {
    let outcome = tauri::async_runtime::block_on(crate::commands::sync_skill_to_tool(
        app.state(),
        installed["central_path"].as_str().unwrap().into(),
        installed["skill_id"].as_str().unwrap().into(),
        key.into(),
        installed["name"].as_str().unwrap().into(),
        Some(false),
        Some(true),
        None,
        None,
    ))
    .unwrap();
    assert!(matches!(outcome.mode_used.as_str(), "symlink" | "junction"));
    let expected = root
        .join("home")
        .join(relative)
        .join(installed["name"].as_str().unwrap());
    assert_eq!(
        fs::canonicalize(&outcome.target_path).unwrap(),
        fs::canonicalize(&expected).unwrap()
    );
    #[cfg(windows)]
    {
        // A privileged runner may select symlink. Normalize only the fixture's
        // link type, using product APIs, to reproduce the reported junctions.
        if !junction::exists(&expected).unwrap() {
            assert!(fs::read_link(&expected).is_ok());
            crate::core::sync_engine::remove_path_any(&expected).unwrap();
            sync_dir_with_mode_with_overwrite(
                SyncMode::Junction,
                Path::new(installed["central_path"].as_str().unwrap()),
                &expected,
                false,
            )
            .unwrap();
            let mut record = service
                .store()
                .list_skill_targets(installed["skill_id"].as_str().unwrap())
                .unwrap()
                .into_iter()
                .find(|record| record.tool == key)
                .unwrap();
            record.mode = "junction".into();
            service.store().upsert_skill_target(&record).unwrap();
        }
        assert!(junction::exists(&expected).unwrap());
    }
    #[cfg(not(windows))]
    let _ = service;
    assert_eq!(
        fs::read(expected.join("bytes.bin")).unwrap(),
        fs::read(Path::new(installed["central_path"].as_str().unwrap()).join("bytes.bin")).unwrap()
    );
}

fn seed_junction(
    service: &SkillsHubService,
    root: &Path,
    skill_id: &str,
    key: &str,
    relative: &str,
) {
    let skill = service.store().get_skill_by_id(skill_id).unwrap().unwrap();
    let target = root.join("home").join(relative).join(&skill.name);
    let mode = if cfg!(windows) {
        SyncMode::Junction
    } else {
        SyncMode::Symlink
    };
    let outcome =
        sync_dir_with_mode_with_overwrite(mode, Path::new(&skill.central_path), &target, false)
            .unwrap();
    assert_eq!(outcome.mode_used, mode);
    #[cfg(windows)]
    assert!(junction::exists(&target).unwrap());
    assert_eq!(
        fs::read(target.join("bytes.bin")).unwrap(),
        fs::read(Path::new(&skill.central_path).join("bytes.bin")).unwrap()
    );
    service
        .store()
        .upsert_skill_target(&SkillTargetRecord {
            id: uuid::Uuid::new_v4().to_string(),
            skill_id: skill_id.into(),
            tool: key.into(),
            scope: "global".into(),
            project_path: None,
            target_path: target.to_string_lossy().into_owned(),
            mode: if cfg!(windows) {
                "junction".into()
            } else {
                "symlink".into()
            },
            status: "ok".into(),
            last_error: None,
            synced_at: Some(1000),
        })
        .unwrap();
}

fn official(service: &SkillsHubService) -> String {
    let central = service
        .paths()
        .default_central_repo
        .join("manage-skills-hub");
    payload(&central, "manage-skills-hub", 999);
    let id = uuid::Uuid::new_v4().to_string();
    service
        .store()
        .upsert_skill(&SkillRecord {
            id: id.clone(),
            name: "manage-skills-hub".into(),
            description: Some("Synthetic official Skill".into()),
            source_type: "git".into(),
            source_ref: Some("https://example.invalid/synthetic-official.git".into()),
            source_subpath: None,
            source_revision: None,
            central_path: central.to_string_lossy().into_owned(),
            content_hash: crate::core::content_hash::hash_dir_strict(&central).ok(),
            created_at: 1,
            updated_at: 1,
            last_sync_at: None,
            last_seen_at: 1,
            enabled: true,
            status: "ok".into(),
        })
        .unwrap();
    id
}

fn scenario(base: &Path, case: &str, fixed: bool) -> Value {
    let output = base.join("reports").join(case);
    fs::create_dir_all(&output).unwrap();
    let root = if case == "unicode-long" {
        base.join("fixtures")
            .join(case)
            .join(format!("中文 空间-{}", "a".repeat(100)))
            .join(format!("长路径-{}", "b".repeat(100)))
    } else {
        base.join("fixtures").join(case)
    };
    let paths = paths(&root);
    fs::write(output.join("fixture.json"), serde_json::to_vec_pretty(&json!({"root":root,"central":paths.default_central_repo,"source":root.join("home/.codex/skills"),"targets":TOOLS.iter().map(|(_,dir)|root.join("home").join(dir)).collect::<Vec<_>>()})).unwrap()).unwrap();
    let service = SkillsHubService::open(paths.clone()).unwrap();
    let app = tauri::test::mock_app();
    app.manage(service.clone());
    let official_id = official(&service);
    seed_junction(&service, &root, &official_id, "codex", ".codex/skills");
    for (_, dir) in TOOLS {
        fs::create_dir_all(root.join("home").join(dir)).unwrap();
    }
    for index in 0..50 {
        let name = match index {
            0 => "humanizer-zh".to_string(),
            1 => "manage-skills".to_string(),
            _ => format!("skill-{index:02}"),
        };
        payload(&root.join("home/.codex/skills").join(&name), &name, index);
    }
    let original_plan =
        build_onboarding_plan_for_runtime(&paths, service.store(), &root.join("home")).unwrap();
    assert_eq!(original_plan.groups.len(), 50);
    let mut installed = Vec::new();
    for group in &original_plan.groups {
        let selected = group.variants.iter().find(|v| v.tool == "codex").unwrap();
        let result = import(&app, &selected.path, &group.name).unwrap();
        assert!(!result.is_null());
        let error = sync_source(&app, &result).unwrap_err();
        assert!(
            error.starts_with("SKILL_TARGET_OVERLAPS_SOURCE|"),
            "{error}"
        );
        assert_eq!(
            crate::core::content_hash::hash_dir_strict(&selected.path).unwrap(),
            crate::core::content_hash::hash_dir_strict(Path::new(
                result["central_path"].as_str().unwrap()
            ))
            .unwrap()
        );
        let id = result["skill_id"].as_str().unwrap();
        service
            .store()
            .set_skill_tag_names(id, &["synthetic-issue208".into()])
            .unwrap();
        for (key, dir) in TOOLS {
            deploy_junction(&app, &service, &root, &result, key, dir);
        }
        installed.push(result);
    }
    assert_eq!(service.store().list_skills().unwrap().len(), 51);
    assert_eq!(
        service.store().list_all_skill_target_paths().unwrap().len(),
        351
    );
    assert!(service.store().list_recycle_bin_rows().unwrap().is_empty());
    let before = checkpoint(&service, &root, &output, "baseline", None);
    let mut rounds = Vec::new();
    for round in 1..=3 {
        let scanned =
            build_onboarding_plan_for_runtime(&paths, service.store(), &root.join("home")).unwrap();
        assert_eq!(scanned.groups.len(), if fixed { 0 } else { 50 });
        let mut errors = Vec::new();
        let mut skipped = 0;
        for group in &original_plan.groups {
            let source = &group
                .variants
                .iter()
                .find(|v| v.tool == "codex")
                .unwrap()
                .path;
            match import(&app, source, &group.name) {
                Ok(value) => {
                    assert!(fixed && value.is_null());
                    skipped += 1;
                }
                Err(error) => {
                    assert!(
                        !fixed
                            && error.contains("a custom name requires exactly one adopt candidate"),
                        "{error}"
                    );
                    errors.push(error);
                }
            }
        }
        assert_eq!(skipped, if fixed { 50 } else { 0 });
        assert_eq!(errors.len(), if fixed { 0 } else { 50 });
        checkpoint(
            &service,
            &root,
            &output,
            &format!("round-{round}"),
            Some(&before),
        );
        rounds.push(json!({"round":round,"fresh_scan_groups":scanned.groups.len(),"stale_selection_attempts":50,"skipped":skipped,"errors":errors,"snapshot_unchanged":true}));
    }
    for result in &installed {
        assert!(sync_source(&app, result)
            .unwrap_err()
            .starts_with("SKILL_TARGET_OVERLAPS_SOURCE|"));
    }
    checkpoint(&service, &root, &output, "after-source-sync", Some(&before));
    drop(app);
    drop(service);
    let reopened = SkillsHubService::open(paths).unwrap();
    checkpoint(&reopened, &root, &output, "after-reopen", Some(&before));
    let summary = json!({"case":case,"os":std::env::consts::OS,"fixture_root":root,"central_root":reopened.paths().default_central_repo,"database_path":reopened.paths().database_path,"recycle_bin_root":reopened.paths().recycle_bin_dir,"skills":51,"local_sources":50,"tools":TOOLS.iter().map(|v|v.0).collect::<Vec<_>>(),"target_records":351,"junctions":if cfg!(windows){351}else{0},"first_imports":50,"source_overlap_rejections":100,"rounds":rounds,"filesystem_and_all_database_tables_unchanged":true,"recycle_entries":0});
    fs::write(
        output.join("summary.json"),
        serde_json::to_vec_pretty(&summary).unwrap(),
    )
    .unwrap();
    summary
}

fn deletion_control(base: &Path, source: DeletionSource, label: &str) -> Value {
    let root = base.join("fixtures").join(label);
    let paths = paths(&root);
    let service = SkillsHubService::open(paths).unwrap();
    let app = tauri::test::mock_app();
    app.manage(service.clone());
    let original = root.join("home/.codex/skills/manage-skills");
    payload(&original, "manage-skills", 1);
    let result = import(&app, &original, "manage-skills").unwrap();
    let id = result["skill_id"].as_str().unwrap();
    seed_junction(&service, &root, id, "claude_code", ".claude/skills");
    let before = snapshot(&service, &root);
    let item = RecycleBinService::new(service.store(), service.paths().recycle_bin_dir.clone())
        .archive(id, source, 1000)
        .unwrap();
    let after = snapshot(&service, &root);
    assert_ne!(before, after);
    assert!(original.join("SKILL.md").is_file());
    assert!(!Path::new(result["central_path"].as_str().unwrap()).exists());
    assert!(!root.join("home/.claude/skills/manage-skills").exists());
    assert!(service.store().list_skills().unwrap().is_empty());
    assert_eq!(service.store().list_recycle_bin_rows().unwrap().len(), 1);
    let output = base.join("reports").join(label);
    fs::create_dir_all(&output).unwrap();
    fs::write(
        output.join("before.json"),
        serde_json::to_vec_pretty(&before).unwrap(),
    )
    .unwrap();
    fs::write(
        output.join("after.json"),
        serde_json::to_vec_pretty(&after).unwrap(),
    )
    .unwrap();
    let summary = json!({"case":label,"deletion_source":item.deletion_source,"central_removed":true,"deployment_removed":true,"database_skill_removed":true,"original_source_preserved":true,"recycle_entries":1,"snapshot_detected_changes":true});
    fs::write(
        output.join("summary.json"),
        serde_json::to_vec_pretty(&summary).unwrap(),
    )
    .unwrap();
    summary
}

#[test]
fn windows_50_skill_junction_comparison() {
    let base = PathBuf::from(
        std::env::var_os("ISSUE208_REPRO_ROOT").expect("explicit temporary root is required"),
    );
    assert!(base.is_absolute() && base.is_dir());
    let fixed = std::env::var("ISSUE208_EXPECT_FIXED").unwrap() == "true";
    let summaries = vec![
        scenario(&base, "normal", fixed),
        scenario(&base, "unicode-long", fixed),
        deletion_control(&base, DeletionSource::Manual, "manual-control"),
        deletion_control(&base, DeletionSource::Sync, "sync-control"),
    ];
    fs::write(
        base.join("summary.json"),
        serde_json::to_vec_pretty(&summaries).unwrap(),
    )
    .unwrap();
    #[cfg(windows)]
    {
        let control = base.join("fixtures/system-recycle-control");
        payload(&control, "system-recycle-control", 1);
        crate::core::sync_engine::remove_path_any(&control).unwrap();
        assert!(!control.exists());
    }
    println!("ISSUE208_REPRO_SUMMARY={}", json!(summaries));
}
