use super::{
    record_auto_update_progress, record_auto_update_progress_snapshot, record_auto_update_result,
    record_auto_update_started, record_auto_update_triggered, AutoUpdateProgressSnapshot,
    AutoUpdateRunResult, AutoUpdateSkillProgress,
};
use crate::core::auto_update::{
    get_auto_update_config, is_auto_update_due, set_auto_update_config, AutoUpdateConfig,
    AutoUpdateIntervalUnit, AutoUpdateSchedule, AutoUpdateScheduleType,
    AUTO_UPDATE_LAST_CHECKED_KEY, AUTO_UPDATE_LAST_ERROR_KEY, AUTO_UPDATE_LAST_FAILED_KEY,
    AUTO_UPDATE_LAST_STATUS_KEY, AUTO_UPDATE_LAST_UPDATED_KEY, DEFAULT_AUTO_UPDATE_INTERVAL_HOURS,
};
use crate::core::skill_store::{SkillRecord, SkillStore};

fn make_store() -> (tempfile::TempDir, SkillStore) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = dir.path().join("test.db");
    let store = SkillStore::new(db);
    store.ensure_schema().expect("ensure_schema");
    (dir, store)
}

#[test]
fn updated_library_with_pending_tools_is_partial_not_ok() {
    let (_dir, store) = make_store();
    record_auto_update_result(
        &store,
        &AutoUpdateRunResult {
            checked: 1,
            unchanged: 0,
            updated: 1,
            failed: 0,
            errors: vec![],
            progress: AutoUpdateProgressSnapshot {
                total: 1,
                succeeded: vec![AutoUpdateSkillProgress {
                    skill_id: "one".into(),
                    name: "one".into(),
                    reason: Some("TOOLS_PENDING|1".into()),
                }],
                ..Default::default()
            },
        },
    )
    .unwrap();
    let config = get_auto_update_config(&store).unwrap();
    assert_eq!(config.last_status.as_deref(), Some("partial"));
    assert_eq!(config.last_failed, 0);
    assert_eq!(config.last_updated, 1);
}

fn make_skill(id: &str, source_type: &str, central_path: &str) -> SkillRecord {
    SkillRecord {
        id: id.to_string(),
        name: id.to_string(),
        description: None,
        source_type: source_type.to_string(),
        source_ref: Some("/tmp/source".to_string()),
        source_subpath: None,
        source_revision: None,
        central_path: central_path.to_string(),
        content_hash: None,
        created_at: 1,
        updated_at: 1,
        last_sync_at: None,
        last_seen_at: 1,
        enabled: true,
        status: "ok".to_string(),
    }
}

fn hourly_schedule(hours: i64) -> AutoUpdateSchedule {
    AutoUpdateSchedule {
        schedule_type: AutoUpdateScheduleType::Interval,
        interval_value: hours,
        interval_unit: AutoUpdateIntervalUnit::Hours,
        daily_time: "03:00".to_string(),
    }
}

fn minute_schedule(minutes: i64) -> AutoUpdateSchedule {
    AutoUpdateSchedule {
        schedule_type: AutoUpdateScheduleType::Interval,
        interval_value: minutes,
        interval_unit: AutoUpdateIntervalUnit::Minutes,
        daily_time: "03:00".to_string(),
    }
}

fn daily_schedule(time: &str) -> AutoUpdateSchedule {
    AutoUpdateSchedule {
        schedule_type: AutoUpdateScheduleType::Daily,
        interval_value: 24,
        interval_unit: AutoUpdateIntervalUnit::Hours,
        daily_time: time.to_string(),
    }
}

#[test]
fn default_config_is_disabled_with_24_hour_interval() {
    let (_dir, store) = make_store();

    let config = get_auto_update_config(&store).unwrap();

    assert!(!config.enabled);
    assert_eq!(config.interval_hours, DEFAULT_AUTO_UPDATE_INTERVAL_HOURS);
    assert_eq!(config.last_run_at, None);
    assert_eq!(config.last_status.as_deref(), None);
}

#[test]
fn config_roundtrips_and_rejects_invalid_interval() {
    let (_dir, store) = make_store();

    let saved = set_auto_update_config(
        &store,
        AutoUpdateConfig {
            enabled: true,
            interval_hours: 12,
            schedule: hourly_schedule(12),
            local_skill_count: 0,
            protected_local_skill_count: 0,
            last_run_at: None,
            last_started_at: None,
            last_finished_at: None,
            last_status: None,
            last_error: None,
            last_checked: 0,
            last_unchanged: 0,
            last_updated: 0,
            last_failed: 0,
            progress: AutoUpdateProgressSnapshot::default(),
        },
    )
    .unwrap();

    assert!(saved.enabled);
    assert_eq!(saved.interval_hours, 12);
    assert_eq!(saved.schedule, hourly_schedule(12));
    assert_eq!(get_auto_update_config(&store).unwrap().interval_hours, 12);

    let err = set_auto_update_config(
        &store,
        AutoUpdateConfig {
            enabled: true,
            interval_hours: 0,
            schedule: minute_schedule(10),
            local_skill_count: 0,
            protected_local_skill_count: 0,
            last_run_at: None,
            last_started_at: None,
            last_finished_at: None,
            last_status: None,
            last_error: None,
            last_checked: 0,
            last_unchanged: 0,
            last_updated: 0,
            last_failed: 0,
            progress: AutoUpdateProgressSnapshot::default(),
        },
    )
    .unwrap_err();
    assert!(err.to_string().contains("interval"));
}

#[test]
fn schedule_supports_minutes_and_daily_time() {
    let (_dir, store) = make_store();

    let saved = set_auto_update_config(
        &store,
        AutoUpdateConfig {
            enabled: true,
            interval_hours: 1,
            schedule: minute_schedule(30),
            local_skill_count: 0,
            protected_local_skill_count: 0,
            last_run_at: None,
            last_started_at: None,
            last_finished_at: None,
            last_status: None,
            last_error: None,
            last_checked: 0,
            last_unchanged: 0,
            last_updated: 0,
            last_failed: 0,
            progress: AutoUpdateProgressSnapshot::default(),
        },
    )
    .unwrap();
    assert_eq!(saved.interval_hours, 1);
    assert_eq!(saved.schedule, minute_schedule(30));

    let saved = set_auto_update_config(
        &store,
        AutoUpdateConfig {
            schedule: daily_schedule("23:45"),
            ..saved
        },
    )
    .unwrap();
    assert_eq!(saved.interval_hours, 24);
    assert_eq!(saved.schedule, daily_schedule("23:45"));
}

#[test]
fn due_check_respects_enabled_state_and_interval() {
    let disabled = AutoUpdateConfig {
        enabled: false,
        interval_hours: 24,
        schedule: hourly_schedule(24),
        local_skill_count: 0,
        protected_local_skill_count: 0,
        last_run_at: Some(1_000),
        last_started_at: Some(1_000),
        last_finished_at: Some(1_000),
        last_status: None,
        last_error: None,
        last_checked: 0,
        last_unchanged: 0,
        last_updated: 0,
        last_failed: 0,
        progress: AutoUpdateProgressSnapshot::default(),
    };
    assert!(!is_auto_update_due(&disabled, 1_000 + 48 * 60 * 60 * 1000));

    let enabled_never_run = AutoUpdateConfig {
        enabled: true,
        ..disabled.clone()
    };
    let enabled_never_run = AutoUpdateConfig {
        last_run_at: None,
        ..enabled_never_run
    };
    assert!(is_auto_update_due(&enabled_never_run, 1_000));

    let recent = AutoUpdateConfig {
        enabled: true,
        interval_hours: 24,
        schedule: hourly_schedule(24),
        last_run_at: Some(1_000),
        ..disabled
    };
    assert!(!is_auto_update_due(&recent, 1_000 + 23 * 60 * 60 * 1000));
    assert!(is_auto_update_due(&recent, 1_000 + 24 * 60 * 60 * 1000));

    let minute_interval = AutoUpdateConfig {
        schedule: minute_schedule(30),
        last_run_at: Some(1_000),
        ..recent
    };
    assert!(!is_auto_update_due(
        &minute_interval,
        1_000 + 29 * 60 * 1000
    ));
    assert!(is_auto_update_due(&minute_interval, 1_000 + 30 * 60 * 1000));
}

#[test]
fn eligible_skills_include_git_and_local_sources() {
    let (_dir, store) = make_store();
    store
        .upsert_skill(&make_skill("git-skill", "git", "/tmp/git-skill"))
        .unwrap();
    store
        .upsert_skill(&make_skill("local-skill", "local", "/tmp/local-skill"))
        .unwrap();
    store
        .upsert_skill(&make_skill("other-skill", "generated", "/tmp/other-skill"))
        .unwrap();

    let ids = crate::core::auto_update::list_auto_update_skill_ids(&store).unwrap();

    assert_eq!(
        ids,
        vec!["git-skill".to_string(), "local-skill".to_string()]
    );
}

#[test]
fn config_reports_local_skills_for_permission_hint() {
    let (_dir, store) = make_store();
    store
        .upsert_skill(&make_skill("local-skill", "local", "/tmp/local-skill"))
        .unwrap();
    store
        .upsert_skill(&make_skill("git-skill", "git", "/tmp/git-skill"))
        .unwrap();

    let config = get_auto_update_config(&store).unwrap();

    assert_eq!(config.local_skill_count, 1);
}

#[test]
fn progress_snapshot_is_persisted_while_update_is_running() {
    let (_dir, store) = make_store();
    record_auto_update_started(&store, 60).unwrap();

    record_auto_update_progress(
        &store,
        &AutoUpdateRunResult {
            checked: 60,
            unchanged: 45,
            updated: 12,
            failed: 3,
            errors: vec!["skill-a: network timeout".to_string()],
            progress: AutoUpdateProgressSnapshot::default(),
        },
    )
    .unwrap();

    let config = get_auto_update_config(&store).unwrap();

    assert_eq!(config.last_status.as_deref(), Some("running"));
    assert_eq!(config.last_checked, 60);
    assert_eq!(config.last_unchanged, 45);
    assert_eq!(config.last_updated, 12);
    assert_eq!(config.last_failed, 3);
    assert_eq!(
        config.last_error.as_deref(),
        Some("skill-a: network timeout")
    );
}

#[test]
fn completed_legacy_result_derives_unchanged_count() {
    let (_dir, store) = make_store();
    store
        .set_setting(AUTO_UPDATE_LAST_STATUS_KEY, "error")
        .unwrap();
    store
        .set_setting(AUTO_UPDATE_LAST_CHECKED_KEY, "43")
        .unwrap();
    store
        .set_setting(AUTO_UPDATE_LAST_UPDATED_KEY, "1")
        .unwrap();
    store.set_setting(AUTO_UPDATE_LAST_FAILED_KEY, "2").unwrap();

    let config = get_auto_update_config(&store).unwrap();

    assert_eq!(config.last_unchanged, 40);
}

#[test]
fn running_legacy_result_does_not_treat_pending_items_as_unchanged() {
    let (_dir, store) = make_store();
    store
        .set_setting(AUTO_UPDATE_LAST_STATUS_KEY, "running")
        .unwrap();
    store
        .set_setting(AUTO_UPDATE_LAST_CHECKED_KEY, "43")
        .unwrap();
    store
        .set_setting(AUTO_UPDATE_LAST_UPDATED_KEY, "1")
        .unwrap();
    store.set_setting(AUTO_UPDATE_LAST_FAILED_KEY, "2").unwrap();

    let config = get_auto_update_config(&store).unwrap();

    assert_eq!(config.last_unchanged, 0);
}

#[test]
fn corrupt_legacy_counts_do_not_overflow_unchanged_fallback() {
    let (_dir, store) = make_store();
    store
        .set_setting(AUTO_UPDATE_LAST_STATUS_KEY, "ok")
        .unwrap();
    store
        .set_setting(AUTO_UPDATE_LAST_CHECKED_KEY, "5")
        .unwrap();
    store
        .set_setting(AUTO_UPDATE_LAST_UPDATED_KEY, &usize::MAX.to_string())
        .unwrap();
    store.set_setting(AUTO_UPDATE_LAST_FAILED_KEY, "1").unwrap();

    let config = get_auto_update_config(&store).unwrap();

    assert_eq!(config.last_unchanged, 0);
}

#[test]
fn starting_update_clears_previous_result_and_progress() {
    let (_dir, store) = make_store();
    record_auto_update_progress(
        &store,
        &AutoUpdateRunResult {
            checked: 2,
            unchanged: 0,
            updated: 1,
            failed: 1,
            errors: vec!["old-skill: old error".to_string()],
            progress: AutoUpdateProgressSnapshot {
                total: 2,
                succeeded: vec![AutoUpdateSkillProgress {
                    skill_id: "done".to_string(),
                    name: "Done".to_string(),
                    reason: None,
                }],
                failed: vec![AutoUpdateSkillProgress {
                    skill_id: "bad".to_string(),
                    name: "Bad".to_string(),
                    reason: Some("old error".to_string()),
                }],
                running: None,
                pending: vec![],
            },
        },
    )
    .unwrap();

    record_auto_update_started(&store, 3).unwrap();

    let config = get_auto_update_config(&store).unwrap();

    assert_eq!(config.last_status.as_deref(), Some("running"));
    assert!(config.last_started_at.is_some());
    assert_eq!(config.last_finished_at, None);
    assert_eq!(config.last_checked, 3);
    assert_eq!(config.last_unchanged, 0);
    assert_eq!(config.last_updated, 0);
    assert_eq!(config.last_failed, 0);
    assert_eq!(config.last_error.as_deref(), Some(""));
    assert_eq!(config.progress.total, 3);
    assert!(config.progress.succeeded.is_empty());
    assert!(config.progress.failed.is_empty());
    assert!(config.progress.running.is_none());
    assert!(config.progress.pending.is_empty());
}

#[test]
fn started_and_finished_times_are_recorded_separately() {
    let (_dir, store) = make_store();

    record_auto_update_started(&store, 1).unwrap();
    let running = get_auto_update_config(&store).unwrap();
    assert!(running.last_started_at.is_some());
    assert_eq!(running.last_finished_at, None);

    record_auto_update_result(
        &store,
        &AutoUpdateRunResult {
            checked: 1,
            unchanged: 0,
            updated: 1,
            failed: 0,
            errors: vec![],
            progress: AutoUpdateProgressSnapshot::default(),
        },
    )
    .unwrap();

    let finished = get_auto_update_config(&store).unwrap();
    assert!(finished.last_started_at.is_some());
    assert!(finished.last_finished_at.is_some());
    assert!(finished.last_finished_at >= finished.last_started_at);
}

#[test]
fn triggered_update_clears_previous_result_using_current_eligible_count() {
    let (_dir, store) = make_store();
    store
        .upsert_skill(&make_skill("git-skill", "git", "/tmp/git-skill"))
        .unwrap();
    store
        .upsert_skill(&make_skill("local-skill", "local", "/tmp/local-skill"))
        .unwrap();
    store
        .set_setting(AUTO_UPDATE_LAST_ERROR_KEY, "old: failed")
        .unwrap();

    record_auto_update_triggered(&store).unwrap();

    let config = get_auto_update_config(&store).unwrap();

    assert_eq!(config.last_status.as_deref(), Some("running"));
    assert_eq!(config.last_checked, 2);
    assert_eq!(config.last_failed, 0);
    assert_eq!(config.last_error.as_deref(), Some(""));
    assert_eq!(config.progress.total, 2);
    assert_eq!(config.progress.pending.len(), 2);
    assert!(config.progress.failed.is_empty());
}

#[test]
fn structured_progress_snapshot_tracks_success_failure_running_and_pending() {
    let (_dir, store) = make_store();

    record_auto_update_progress_snapshot(
        &store,
        &AutoUpdateProgressSnapshot {
            total: 4,
            succeeded: vec![AutoUpdateSkillProgress {
                skill_id: "done".to_string(),
                name: "Done Skill".to_string(),
                reason: None,
            }],
            failed: vec![AutoUpdateSkillProgress {
                skill_id: "bad".to_string(),
                name: "Bad Skill".to_string(),
                reason: Some("network timeout".to_string()),
            }],
            running: Some(AutoUpdateSkillProgress {
                skill_id: "now".to_string(),
                name: "Now Skill".to_string(),
                reason: None,
            }),
            pending: vec![AutoUpdateSkillProgress {
                skill_id: "next".to_string(),
                name: "Next Skill".to_string(),
                reason: None,
            }],
        },
    )
    .unwrap();

    let config = get_auto_update_config(&store).unwrap();

    assert_eq!(config.progress.total, 4);
    assert_eq!(config.progress.succeeded[0].name, "Done Skill");
    assert_eq!(
        config.progress.failed[0].reason.as_deref(),
        Some("network timeout")
    );
    assert_eq!(
        config
            .progress
            .running
            .as_ref()
            .map(|item| item.skill_id.as_str()),
        Some("now")
    );
    assert_eq!(config.progress.pending[0].skill_id, "next");
}

#[test]
fn legacy_error_progress_uses_skill_name_when_available() {
    let (_dir, store) = make_store();
    let mut skill = make_skill(
        "64798624-ca2a-4811-8747-00147567facf",
        "local",
        "/tmp/youdaonote",
    );
    skill.name = "有道云笔记".to_string();
    store.upsert_skill(&skill).unwrap();
    store
        .set_setting(AUTO_UPDATE_LAST_CHECKED_KEY, "1")
        .unwrap();
    store
        .set_setting(
            AUTO_UPDATE_LAST_ERROR_KEY,
            "64798624-ca2a-4811-8747-00147567facf: source path not found: \"/Users/may/Downloads/youdaonote\"",
        )
        .unwrap();

    let config = get_auto_update_config(&store).unwrap();

    assert_eq!(config.progress.failed[0].name, "有道云笔记");
    assert_eq!(
        config.progress.failed[0].reason.as_deref(),
        Some("source path not found: \"/Users/may/Downloads/youdaonote\"")
    );
}

#[test]
fn unchanged_skill_is_checked_but_not_counted_as_updated() {
    let app = tauri::test::mock_app();
    let (_dir, store) = make_store();
    let source = tempfile::tempdir().unwrap();
    let central_root = tempfile::tempdir().unwrap();
    std::fs::write(source.path().join("SKILL.md"), b"---\nname: x\n---\n").unwrap();
    std::fs::write(source.path().join("a.txt"), b"same").unwrap();
    store
        .set_setting(
            "central_repo_path",
            central_root.path().to_string_lossy().as_ref(),
        )
        .unwrap();
    let paths = crate::runtime_paths_for_tauri(app.handle()).unwrap();
    crate::core::installer::install_local_skill(
        &paths,
        &store,
        source.path(),
        Some("unchanged".to_string()),
    )
    .unwrap();

    let result = super::run_auto_update_now(app.handle(), &store).unwrap();

    assert_eq!(result.checked, 1);
    assert_eq!(result.unchanged, 1);
    assert_eq!(result.updated, 0);
    assert_eq!(result.failed, 0);
    assert_eq!(
        result.checked,
        result.unchanged + result.updated + result.failed
    );
}

#[test]
fn unbound_local_skills_are_not_source_update_candidates() {
    let (_dir, store) = make_store();
    let mut unbound = make_skill("unbound", "local", "/central/unbound");
    unbound.source_ref = None;
    store.upsert_skill(&unbound).unwrap();
    store
        .upsert_skill(&make_skill("bound", "local", "/central/bound"))
        .unwrap();
    store
        .upsert_skill(&make_skill("git", "git", "/central/git"))
        .unwrap();
    let entries = super::list_auto_update_skill_entries(&store).unwrap();
    assert_eq!(
        entries
            .iter()
            .map(|entry| entry.skill_id.as_str())
            .collect::<Vec<_>>(),
        vec!["bound", "git"]
    );
    assert_eq!(super::count_local_auto_update_skills(&store).unwrap().0, 1);
}

#[test]
fn interrupted_auto_update_recovers_persisted_progress_after_restart() {
    let (dir, store) = make_store();
    record_auto_update_started(&store, 3).unwrap();
    store
        .set_setting(super::AUTO_UPDATE_LAST_STARTED_AT_KEY, "1000")
        .unwrap();
    store
        .set_setting(super::AUTO_UPDATE_LAST_RUN_AT_KEY, "1000")
        .unwrap();
    store
        .set_setting(super::AUTO_UPDATE_LAST_UNCHANGED_KEY, "1")
        .unwrap();
    let entry = |id: &str| AutoUpdateSkillProgress {
        skill_id: id.into(),
        name: id.into(),
        reason: None,
    };
    record_auto_update_progress_snapshot(
        &store,
        &AutoUpdateProgressSnapshot {
            total: 3,
            succeeded: vec![entry("done")],
            running: Some(entry("interrupted")),
            pending: vec![entry("pending")],
            ..Default::default()
        },
    )
    .unwrap();
    drop(store);
    let reopened = SkillStore::new(dir.path().join("test.db"));
    let config = get_auto_update_config(&reopened).unwrap();
    assert_eq!(config.last_status.as_deref(), Some("stopped"));
    assert!(config.last_finished_at.is_some());
    assert_eq!(config.last_unchanged, 1);
    assert_eq!(config.progress.succeeded[0].skill_id, "done");
    assert!(config.progress.running.is_none());
    assert_eq!(
        config
            .progress
            .pending
            .iter()
            .map(|item| item.skill_id.as_str())
            .collect::<Vec<_>>(),
        vec!["interrupted", "pending"]
    );
    assert_eq!(
        reopened
            .get_setting(AUTO_UPDATE_LAST_STATUS_KEY)
            .unwrap()
            .as_deref(),
        Some("stopped")
    );
    let polled = get_auto_update_config(&reopened).unwrap();
    assert_eq!(polled.last_status.as_deref(), Some("stopped"));
    assert_eq!(polled.last_finished_at, config.last_finished_at);
}

#[test]
fn interrupted_auto_update_child() {
    let Some(root) = std::env::var_os("SKILLS_HUB_TEST_INTERRUPTED_UPDATE_ROOT") else {
        return;
    };
    let root = std::path::PathBuf::from(root);
    let paths = crate::core::runtime_paths::RuntimePaths::from_roots(
        crate::core::runtime_paths::RuntimeProfile::Test,
        &root,
        &root,
    );
    let store = crate::core::runtime_paths::open_store(&paths).unwrap();
    let _lock = crate::services::operation_lock::OperationLock::acquire(
        &paths,
        crate::services::operation_lock::OperationKind::AutoUpdate,
    )
    .unwrap();
    record_auto_update_started(&store, 1).unwrap();
    store
        .set_setting(super::AUTO_UPDATE_LAST_STARTED_AT_KEY, "1000")
        .unwrap();
    store
        .set_setting(super::AUTO_UPDATE_LAST_RUN_AT_KEY, "1000")
        .unwrap();
    record_auto_update_progress_snapshot(
        &store,
        &AutoUpdateProgressSnapshot {
            total: 1,
            running: Some(AutoUpdateSkillProgress {
                skill_id: "interrupted".into(),
                name: "Interrupted".into(),
                reason: None,
            }),
            ..Default::default()
        },
    )
    .unwrap();
    std::fs::write(root.join("ready"), "ready").unwrap();
    loop {
        std::thread::park();
    }
}

#[test]
fn interrupted_auto_update_recovers_only_after_worker_is_killed() {
    use crate::core::runtime_paths::{open_store, RuntimePaths, RuntimeProfile};
    use crate::services::operation_lock::{OperationKind, OperationLock};
    let root = tempfile::tempdir().unwrap();
    struct ChildGuard(std::process::Child);
    impl Drop for ChildGuard {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let mut child = ChildGuard(
        std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "core::auto_update::tests::interrupted_auto_update_child",
                "--nocapture",
            ])
            .env("SKILLS_HUB_TEST_INTERRUPTED_UPDATE_ROOT", root.path())
            .spawn()
            .unwrap(),
    );
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while !root.path().join("ready").exists() {
        assert!(
            child.0.try_wait().unwrap().is_none(),
            "worker exited before readiness"
        );
        assert!(
            std::time::Instant::now() < deadline,
            "worker did not become ready"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let paths = RuntimePaths::from_roots(RuntimeProfile::Test, root.path(), root.path());
    let store = open_store(&paths).unwrap();
    let active = get_auto_update_config(&store).unwrap();
    assert_eq!(active.last_status.as_deref(), Some("running"));
    assert!(active.last_finished_at.is_none());
    child.0.kill().unwrap();
    child.0.wait().unwrap();
    let recovered = get_auto_update_config(&store).unwrap();
    assert_eq!(recovered.last_status.as_deref(), Some("stopped"));
    assert!(recovered.last_finished_at.is_some());
    assert!(recovered.progress.running.is_none());
    let _retry_lock = OperationLock::acquire(&paths, OperationKind::AutoUpdate).unwrap();
    record_auto_update_triggered(&store).unwrap();
    assert_eq!(
        get_auto_update_config(&store)
            .unwrap()
            .last_status
            .as_deref(),
        Some("running")
    );
}

#[test]
fn interrupted_auto_update_keeps_a_new_trigger_running_during_startup() {
    let (_dir, store) = make_store();
    record_auto_update_triggered(&store).unwrap();
    let config = get_auto_update_config(&store).unwrap();
    assert_eq!(config.last_status.as_deref(), Some("running"));
    assert!(config.last_finished_at.is_none());
}

#[test]
fn interrupted_auto_update_does_not_guess_when_lock_cannot_be_opened() {
    let (dir, store) = make_store();
    record_auto_update_started(&store, 0).unwrap();
    store
        .set_setting(super::AUTO_UPDATE_LAST_STARTED_AT_KEY, "1000")
        .unwrap();
    std::fs::create_dir(dir.path().join("operation.lock")).unwrap();
    assert!(get_auto_update_config(&store).is_err());
    assert_eq!(
        store
            .get_setting(AUTO_UPDATE_LAST_STATUS_KEY)
            .unwrap()
            .as_deref(),
        Some("running")
    );
}

#[test]
fn interrupted_auto_update_recovers_legacy_state_without_inventing_completed_items() {
    let (_dir, store) = make_store();
    store
        .set_setting(AUTO_UPDATE_LAST_STATUS_KEY, "running")
        .unwrap();
    store
        .set_setting(AUTO_UPDATE_LAST_CHECKED_KEY, "5")
        .unwrap();
    store
        .set_setting(AUTO_UPDATE_LAST_UPDATED_KEY, "1")
        .unwrap();
    let config = get_auto_update_config(&store).unwrap();
    assert_eq!(config.last_status.as_deref(), Some("stopped"));
    assert_eq!(config.last_unchanged, 0);
    assert_eq!(get_auto_update_config(&store).unwrap().last_unchanged, 0);
}

#[test]
fn interrupted_auto_update_recovery_is_atomic_on_database_failure() {
    let (_dir, store) = make_store();
    record_auto_update_started(&store, 0).unwrap();
    store
        .set_setting(super::AUTO_UPDATE_LAST_STARTED_AT_KEY, "1000")
        .unwrap();
    rusqlite::Connection::open(store.db_path())
        .unwrap()
        .execute_batch(
            "CREATE TRIGGER reject_stop BEFORE UPDATE ON settings
         WHEN NEW.key = 'skill_auto_update_last_status' AND NEW.value = 'stopped'
         BEGIN SELECT RAISE(ABORT, 'test write failure'); END;",
        )
        .unwrap();
    assert!(get_auto_update_config(&store).is_err());
    assert_eq!(
        store
            .get_setting(AUTO_UPDATE_LAST_STATUS_KEY)
            .unwrap()
            .as_deref(),
        Some("running")
    );
    assert_eq!(
        store
            .get_setting(super::AUTO_UPDATE_LAST_FINISHED_AT_KEY)
            .unwrap()
            .as_deref(),
        Some("")
    );
}

#[test]
fn interrupted_auto_update_failed_trigger_stops_immediately() {
    let (_dir, store) = make_store();
    let error =
        super::trigger_auto_update_with(&store, || anyhow::bail!("scheduler refused")).unwrap_err();
    assert_eq!(error.to_string(), "scheduler refused");
    let config = get_auto_update_config(&store).unwrap();
    assert_eq!(config.last_status.as_deref(), Some("stopped"));
    assert!(config.last_finished_at.is_some());
}

#[test]
fn interrupted_auto_update_trigger_does_not_reset_an_active_writer() {
    use crate::core::runtime_paths::{open_store, RuntimePaths, RuntimeProfile};
    use crate::services::operation_lock::{OperationKind, OperationLock};
    let root = tempfile::tempdir().unwrap();
    let paths = RuntimePaths::from_roots(RuntimeProfile::Test, root.path(), root.path());
    let store = open_store(&paths).unwrap();
    let _lock = OperationLock::acquire(&paths, OperationKind::AutoUpdate).unwrap();
    record_auto_update_started(&store, 3).unwrap();
    store
        .set_setting(AUTO_UPDATE_LAST_UPDATED_KEY, "1")
        .unwrap();
    let called = std::cell::Cell::new(false);
    let result = super::trigger_auto_update_with(&store, || {
        called.set(true);
        Ok(())
    });
    assert!(result.is_err());
    assert!(!called.get());
    let config = get_auto_update_config(&store).unwrap();
    assert_eq!(config.last_updated, 1);
    assert_eq!(config.last_status.as_deref(), Some("running"));
}

#[test]
fn interrupted_auto_update_trigger_releases_lock_before_worker_starts() {
    use crate::core::runtime_paths::{open_store, RuntimePaths, RuntimeProfile};
    use crate::services::operation_lock::{OperationKind, OperationLock};
    let root = tempfile::tempdir().unwrap();
    let paths = RuntimePaths::from_roots(RuntimeProfile::Test, root.path(), root.path());
    let store = open_store(&paths).unwrap();
    super::trigger_auto_update_with(&store, || {
        let _worker_lock = OperationLock::acquire(&paths, OperationKind::AutoUpdate)?;
        record_auto_update_started(&store, 0)
    })
    .unwrap();
    assert_eq!(
        get_auto_update_config(&store)
            .unwrap()
            .last_status
            .as_deref(),
        Some("running")
    );
}

#[test]
fn interrupted_auto_update_late_trigger_error_preserves_an_active_worker() {
    use crate::core::runtime_paths::{open_store, RuntimePaths, RuntimeProfile};
    use crate::services::operation_lock::{OperationKind, OperationLock};
    let root = tempfile::tempdir().unwrap();
    let paths = RuntimePaths::from_roots(RuntimeProfile::Test, root.path(), root.path());
    let store = open_store(&paths).unwrap();
    let mut worker_lock = None;
    let result = super::trigger_auto_update_with(&store, || {
        worker_lock = Some(OperationLock::acquire(&paths, OperationKind::AutoUpdate)?);
        record_auto_update_started(&store, 1)?;
        anyhow::bail!("late scheduler error")
    });
    assert!(result.is_err());
    let config = get_auto_update_config(&store).unwrap();
    assert_eq!(config.last_status.as_deref(), Some("running"));
    assert!(config.last_finished_at.is_none());
}

#[test]
fn interrupted_auto_update_rejects_duplicate_trigger_during_startup_grace() {
    let (_dir, store) = make_store();
    record_auto_update_triggered(&store).unwrap();
    let before = get_auto_update_config(&store).unwrap().last_started_at;
    let called = std::cell::Cell::new(false);
    let result = super::trigger_auto_update_with(&store, || {
        called.set(true);
        Ok(())
    });
    assert!(result.is_err());
    assert!(!called.get());
    assert_eq!(
        get_auto_update_config(&store).unwrap().last_started_at,
        before
    );
}
