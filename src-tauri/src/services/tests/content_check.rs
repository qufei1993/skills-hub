use std::fs;

use crate::core::cancel_token::CancelToken;
use crate::core::runtime_paths::{RuntimePaths, RuntimeProfile};
use crate::services::content_check::{ContentCheckRun, ContentCheckSummary};
use crate::services::error::ErrorCode;
use crate::services::install::InstallRequest;
use crate::services::operation_lock::OperationKind;
use crate::services::skills_hub::SkillsHubService;

struct Fixture {
    _home: tempfile::TempDir,
    data: tempfile::TempDir,
    service: SkillsHubService,
    ids: Vec<String>,
}

impl Fixture {
    fn new(count: usize) -> Self {
        let home = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        let service = SkillsHubService::open(RuntimePaths::from_roots(
            RuntimeProfile::Test,
            home.path(),
            data.path(),
        ))
        .unwrap();
        let mut ids = vec![];
        for n in 0..count {
            let source = data.path().join(format!("source-{n}"));
            fs::create_dir(&source).unwrap();
            fs::write(
                source.join("SKILL.md"),
                format!("---\nname: demo-{n}\ndescription: Test\n---\noriginal\n"),
            )
            .unwrap();
            service.install(InstallRequest::local(source)).unwrap();
            ids.push(
                service
                    .list_skills()
                    .unwrap()
                    .into_iter()
                    .find(|s| s.name == format!("demo-{n}"))
                    .unwrap()
                    .id,
            );
        }
        Self {
            _home: home,
            data,
            service,
            ids,
        }
    }

    fn check(&self, ids: &[String]) -> ContentCheckRun {
        self.service
            .check_content_sources(
                &uuid::Uuid::new_v4().to_string(),
                ids,
                &CancelToken::new(),
                |_| {},
            )
            .unwrap()
    }
}

#[test]
fn content_check_persists_lightweight_observations_and_reads_them_without_source_access() {
    let f = Fixture::new(2);
    fs::write(
        f.data.path().join("source-1/SKILL.md"),
        "---\nname: demo-1\ndescription: Test\n---\nchanged\n",
    )
    .unwrap();
    let before = f.service.store().list_skills().unwrap();
    let run = f.check(&f.ids);
    assert_eq!((run.total, run.completed), (2, 2));
    assert!(!run.cancelled);
    assert_eq!(run.summaries[0].status, "unchanged");
    assert_eq!(run.summaries[1].status, "changed");
    assert!(run
        .summaries
        .iter()
        .all(|s| s.signature.len() == 64 && s.checked_at > 1_700_000_000_000));
    assert_eq!(before, f.service.store().list_skills().unwrap());
    assert!(f
        .service
        .store()
        .workspace_list::<serde_json::Value>("content-preview")
        .unwrap()
        .is_empty());
    let json = serde_json::to_string(&run).unwrap();
    assert!(!json.contains("fingerprint"));
    assert!(!json.contains("changes"));
    // Reading last observations must work without touching unavailable sources.
    fs::remove_dir_all(f.data.path().join("source-0")).unwrap();
    fs::remove_dir_all(f.data.path().join("source-1")).unwrap();
    let reopened = SkillsHubService::open(f.service.paths().clone()).unwrap();
    let cached = reopened.cached_content_checks().unwrap().unwrap();
    assert_eq!(cached.check_id, run.check_id);
    assert_eq!(cached.checked_at, run.checked_at);
    assert_eq!(cached.completed, 2);
    assert!(cached.summaries.iter().any(|s| s.status == "changed"));
}

#[test]
fn content_check_cache_filters_rebound_disabled_and_removed_skills_without_writing() {
    let f = Fixture::new(3);
    f.check(&f.ids);
    let mut rebound = f
        .service
        .store()
        .get_skill_by_id(&f.ids[0])
        .unwrap()
        .unwrap();
    rebound.source_subpath = Some("different".into());
    f.service.store().upsert_skill(&rebound).unwrap();
    f.service
        .store()
        .set_skill_enabled(&f.ids[1], false)
        .unwrap();
    f.service.store().delete_skill(&f.ids[2]).unwrap();
    let before = f
        .service
        .store()
        .workspace_list::<ContentCheckSummary>("content-check-summary")
        .unwrap();
    let cached = f.service.cached_content_checks().unwrap().unwrap();
    assert!(cached.summaries.is_empty());
    assert_eq!((cached.total, cached.completed), (1, 0));
    assert_eq!(
        before,
        f.service
            .store()
            .workspace_list::<ContentCheckSummary>("content-check-summary")
            .unwrap()
    );
}

#[test]
fn content_check_cancellation_keeps_completed_items_and_retains_previous_other_observations() {
    let f = Fixture::new(2);
    let first = f.check(&f.ids);
    let token = CancelToken::new();
    let mut events = vec![];
    let id = uuid::Uuid::new_v4().to_string();
    let stopped = f
        .service
        .check_content_sources(&id, &f.ids, &token, |event| {
            if event.completed == 1 {
                token.cancel();
            }
            events.push(event);
        })
        .unwrap();
    assert!(stopped.cancelled);
    assert_eq!((stopped.total, stopped.completed), (2, 1));
    assert_eq!(stopped.summaries[0].skill_id, f.ids[0]);
    assert_eq!(events.first().unwrap().phase, "preparing");
    assert_eq!(events.last().unwrap().phase, "completed");
    assert!(events.iter().all(|event| event.check_id == id));
    assert!(events
        .windows(2)
        .all(|pair| pair[0].completed <= pair[1].completed));
    let reopened = SkillsHubService::open(f.service.paths().clone()).unwrap();
    let cached = reopened.cached_content_checks().unwrap().unwrap();
    assert!(cached.cancelled);
    assert_eq!(cached.completed, 2);
    assert_eq!(
        cached
            .summaries
            .iter()
            .find(|s| s.skill_id == f.ids[1])
            .unwrap(),
        &first.summaries[1]
    );
}

#[test]
fn content_check_cancelled_before_start_does_not_replace_persisted_run() {
    let f = Fixture::new(1);
    let first = f.check(&f.ids);
    let token = CancelToken::new();
    token.cancel();
    let stopped = f
        .service
        .check_content_sources(&uuid::Uuid::new_v4().to_string(), &f.ids, &token, |_| {})
        .unwrap();
    assert!(stopped.cancelled);
    assert_eq!(stopped.completed, 0);
    assert_eq!(
        f.service.cached_content_checks().unwrap().unwrap().check_id,
        first.check_id
    );
}

#[test]
fn content_check_operation_conflict_does_not_replace_cache_or_preview() {
    let f = Fixture::new(1);
    let first = f.check(&f.ids);
    let _held = f.service.begin_write(OperationKind::AutoUpdate).unwrap();
    let error = f
        .service
        .check_content_sources(
            &uuid::Uuid::new_v4().to_string(),
            &f.ids,
            &CancelToken::new(),
            |_| {},
        )
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationBusy);
    assert_eq!(
        f.service.cached_content_checks().unwrap().unwrap().check_id,
        first.check_id
    );
}

#[test]
fn content_check_records_individual_failure_and_bounds_persistence_to_latest_per_skill() {
    let f = Fixture::new(2);
    fs::remove_dir_all(f.data.path().join("source-0")).unwrap();
    let ids = vec![f.ids[0].clone(), f.ids[0].clone(), f.ids[1].clone()];
    let run = f.check(&ids);
    assert_eq!((run.total, run.completed), (2, 2));
    assert_eq!(run.summaries[0].status, "failed");
    assert_eq!(
        run.summaries[0].error,
        Some(format!(
            "INVALID_SOURCE|{}",
            f.service
                .store()
                .get_skill_by_id(&f.ids[0])
                .unwrap()
                .unwrap()
                .source_ref
                .unwrap()
        ))
    );
    assert_eq!(run.summaries[1].status, "unchanged");
    f.check(&f.ids);
    assert_eq!(
        f.service
            .store()
            .workspace_list::<ContentCheckSummary>("content-check-summary")
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        f.service
            .store()
            .workspace_list::<ContentCheckRun>("content-check-run")
            .unwrap()
            .len(),
        1
    );
    f.service.store().delete_skill(&f.ids[0]).unwrap();
    f.check(&f.ids[1..]);
    assert_eq!(
        f.service
            .store()
            .workspace_list::<ContentCheckSummary>("content-check-summary")
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn content_check_update_file_lock_conflict_is_busy_without_changing_cache() {
    let f = Fixture::new(1);
    let first = f.check(&f.ids);
    let _held =
        crate::core::installer::acquire_skill_update_lock(f.service.paths(), f.service.store())
            .unwrap();
    let error = f
        .service
        .check_content_sources(
            &uuid::Uuid::new_v4().to_string(),
            &f.ids,
            &CancelToken::new(),
            |_| {},
        )
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationBusy);
    assert_eq!(
        f.service.cached_content_checks().unwrap().unwrap().check_id,
        first.check_id
    );
}

#[test]
fn content_check_failure_does_not_persist_secret_source_references() {
    let f = Fixture::new(1);
    let mut record = f
        .service
        .store()
        .get_skill_by_id(&f.ids[0])
        .unwrap()
        .unwrap();
    record.source_type = "unsupported".into();
    record.source_ref =
        Some("https://user:secret-password@example.invalid/repo?token=secret-token".into());
    f.service.store().upsert_skill(&record).unwrap();
    let run = f.check(&f.ids);
    assert_eq!(run.summaries[0].status, "failed");
    assert_eq!(run.summaries[0].error.as_deref(), Some("INVALID_SOURCE"));
    let cached = serde_json::to_string(&f.service.cached_content_checks().unwrap()).unwrap();
    assert!(!cached.contains("secret-password"));
    assert!(!cached.contains("secret-token"));
    assert!(!cached.contains("example.invalid"));
}

#[test]
fn content_check_rejects_observation_when_identity_changes_during_the_batch() {
    let f = Fixture::new(2);
    let run = f
        .service
        .check_content_sources(
            &uuid::Uuid::new_v4().to_string(),
            &f.ids,
            &CancelToken::new(),
            |event| {
                if event.phase == "checking" && event.completed == 1 {
                    // Deterministic stand-in for a core-only writer between the
                    // service snapshot and this item's observation.
                    let mut record = f
                        .service
                        .store()
                        .get_skill_by_id(&f.ids[1])
                        .unwrap()
                        .unwrap();
                    record.source_subpath = Some("new-identity".into());
                    f.service.store().upsert_skill(&record).unwrap();
                }
            },
        )
        .unwrap();
    assert_eq!(run.summaries[0].status, "unchanged");
    assert_eq!(run.summaries[1].status, "failed");
    assert_eq!(run.summaries[1].error.as_deref(), Some("PLAN_STALE"));
    let cached = f.service.cached_content_checks().unwrap().unwrap();
    assert_eq!(cached.completed, 1);
    assert_eq!(cached.summaries[0].skill_id, f.ids[0]);
}

#[test]
fn content_check_storage_failure_is_not_reported_as_completed_or_a_fresh_run() {
    let f = Fixture::new(2);
    let previous = f.check(&f.ids);
    let mut phases = vec![];
    let failed = f.service.check_content_sources(
        &uuid::Uuid::new_v4().to_string(), &f.ids, &CancelToken::new(), |event| {
            phases.push(event.phase.clone());
            if event.phase == "checking" && event.completed == 1 {
                // Fail subsequent summary writes through the real SQLite path.
                let conn = rusqlite::Connection::open(f.service.store().db_path()).unwrap();
                conn.execute_batch("CREATE TRIGGER fail_check_summary BEFORE INSERT ON workspace_documents WHEN NEW.kind='content-check-summary' BEGIN SELECT RAISE(FAIL, 'injected storage failure'); END;").unwrap();
            }
        },
    ).unwrap_err();
    assert_eq!(failed.code, ErrorCode::InternalError);
    assert!(!phases.iter().any(|phase| phase == "completed"));
    let cached = f.service.cached_content_checks().unwrap().unwrap();
    assert_eq!(cached.check_id, previous.check_id);
    assert_eq!(
        cached
            .summaries
            .iter()
            .find(|s| s.skill_id == f.ids[1])
            .unwrap(),
        &previous.summaries[1]
    );
}
