//! Opt-in acceptance probe for an explicitly prepared, disposable mixed library.
//! Never runs in the normal gate and never installs or executes downloaded code.
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use serde_json::{json, Value};

use crate::core::cancel_token::CancelToken;
use crate::core::content_hash::hash_dir_strict;
use crate::core::runtime_paths::{RuntimePaths, RuntimeProfile};
use crate::services::skills_hub::SkillsHubService;

fn git_counts(trace: &Path) -> (usize, usize, usize) {
    let mut counts = (0, 0, 0);
    for line in fs::read_to_string(trace).unwrap_or_default().lines() {
        let event: Value = serde_json::from_str(line).unwrap();
        if event["event"] == "start" {
            if let Some(args) = event["argv"].as_array() {
                if args.iter().any(|arg| arg == "fetch") {
                    if args.iter().any(|arg| arg == "--stdin") {
                        counts.2 += 1; // Partial-clone blob materialization.
                    } else {
                        counts.0 += 1;
                    }
                }
                counts.1 += usize::from(args.iter().any(|arg| arg == "clone"));
            }
        }
    }
    counts
}

#[test]
#[ignore = "manual live GitHub acceptance; requires a disposable CLI-installed fixture"]
fn mixed_github_check_records_real_network_work_and_preserves_content() {
    const CHILD: &str = "SKILLS_HUB_LIVE_GITHUB_CHECK_CHILD";
    const TEST: &str = "services::tests::live_github_content_check::mixed_github_check_records_real_network_work_and_preserves_content";
    let root = PathBuf::from(
        std::env::var_os("SKILLS_HUB_LIVE_GITHUB_ROOT")
            .expect("set SKILLS_HUB_LIVE_GITHUB_ROOT to a disposable CLI-installed fixture"),
    )
    .canonicalize()
    .unwrap();
    let temporary_roots = [std::env::temp_dir(), PathBuf::from("/tmp")];
    assert!(temporary_roots
        .iter()
        .filter_map(|path| path.canonicalize().ok())
        .any(|path| root.starts_with(path)));
    let fixture: Value = serde_json::from_slice(
        &fs::read(root.parent().unwrap().join("fixture-manifest.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        PathBuf::from(fixture["runtimeRoot"].as_str().unwrap())
            .canonicalize()
            .unwrap(),
        root
    );
    if std::env::var_os(CHILD).is_none() {
        let trace = root.parent().unwrap().join("live-github-trace.jsonl");
        fs::write(&trace, "").unwrap();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--ignored", "--exact", TEST, "--nocapture"])
            .env(CHILD, "1")
            .env("GIT_TRACE2_EVENT", trace)
            .env("SKILLS_HUB_GIT_FETCH_TIMEOUT_SECS", "15")
            .env("SKILLS_HUB_GIT_TIMEOUT_SECS", "15")
            .output()
            .unwrap();
        print!("{}", String::from_utf8_lossy(&output.stdout));
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    let trace = PathBuf::from(std::env::var_os("GIT_TRACE2_EVENT").unwrap());
    let overall_started = Instant::now();
    let cancel = std::sync::Arc::new(CancelToken::new());
    let deadline_cancel = cancel.clone();
    let (deadline_tx, deadline_rx) = std::sync::mpsc::channel();
    let deadline = std::thread::spawn(move || {
        if deadline_rx
            .recv_timeout(std::time::Duration::from_secs(180))
            .is_err()
        {
            deadline_cancel.cancel();
        }
    });
    let paths =
        RuntimePaths::from_roots(RuntimeProfile::Test, root.join("home"), root.join("data"));
    assert!(paths.git_cache_dir.starts_with(&root));
    let service = SkillsHubService::open(paths.clone()).unwrap();
    assert_eq!(
        service
            .store()
            .get_setting("skill_auto_update_enabled")
            .unwrap()
            .as_deref(),
        Some("false")
    );
    let skills = service.list_skills().unwrap();
    let samples = fixture["skills"].as_array().unwrap();
    assert_eq!(skills.len(), samples.len());
    let expected = samples
        .iter()
        .map(|sample| {
            (
                sample["id"].as_str().unwrap().to_string(),
                sample["expectedCheck"].as_str().unwrap().to_string(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    assert!(samples.iter().filter(|s| s["kind"] == "github").count() >= 9);
    assert_eq!(fixture["publicRepositories"].as_array().unwrap().len(), 3);
    let mut before = BTreeMap::new();
    for skill in &skills {
        assert!(skill.enabled);
        assert!(Path::new(&skill.central_path)
            .canonicalize()
            .unwrap()
            .starts_with(&root));
        before.insert(
            PathBuf::from(&skill.central_path),
            hash_dir_strict(Path::new(&skill.central_path)).unwrap(),
        );
        for target in &skill.targets {
            assert!(Path::new(&target.target_path)
                .canonicalize()
                .unwrap()
                .starts_with(&root));
            before.insert(
                PathBuf::from(&target.target_path),
                hash_dir_strict(Path::new(&target.target_path)).unwrap(),
            );
        }
    }
    let ids = skills
        .iter()
        .map(|skill| skill.id.clone())
        .collect::<Vec<_>>();
    let initial_plans = service
        .store()
        .workspace_list::<Value>("content-preview")
        .unwrap()
        .len();
    let mut evidence = vec![];
    for trial in 0..2 {
        if paths.git_cache_dir.exists() {
            fs::remove_dir_all(&paths.git_cache_dir).unwrap();
        }
        for round in 0..3 {
            let counts_before = git_counts(&trace);
            let started = Instant::now();
            let mut events = vec![];
            let run = service
                .check_content_sources(&uuid::Uuid::new_v4().to_string(), &ids, &cancel, |event| {
                    events.push(event)
                })
                .unwrap();
            let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
            let counts_after = git_counts(&trace);
            let fetches = counts_after.0 - counts_before.0;
            let clones = counts_after.1 - counts_before.1;
            let blob_fetches = counts_after.2 - counts_before.2;
            let observed = json!({"trial":trial+1,"round":round,"elapsedMs":elapsed_ms,
                "fetches":fetches,"clones":clones,"blobFetches":blob_fetches,"run":run});
            fs::write(
                root.parent()
                    .unwrap()
                    .join("live-probe-latest-observation.json"),
                serde_json::to_vec_pretty(&observed).unwrap(),
            )
            .unwrap();
            assert_eq!(
                (run.total, run.completed, run.cancelled),
                (skills.len(), skills.len(), false)
            );
            for summary in &run.summaries {
                assert_eq!(
                    Some(&summary.status),
                    expected.get(&summary.skill_id),
                    "{}: {:?}",
                    summary.name,
                    summary.error
                );
            }
            // Public transports may retry. Deterministic local Git tests assert
            // exact grouping; this live probe reports actual network attempts.
            assert!(if round == 0 {
                clones >= 3 && fetches == 0
            } else {
                fetches >= 3 && clones == 0
            });
            assert_eq!(
                service
                    .store()
                    .workspace_list::<Value>("content-preview")
                    .unwrap()
                    .len(),
                initial_plans
            );
            for (path, hash) in &before {
                assert_eq!(hash_dir_strict(path).unwrap(), *hash);
            }
            let before_cache = git_counts(&trace);
            let cache_started = Instant::now();
            let cached = service.cached_content_checks().unwrap().unwrap();
            let cache_ms = cache_started.elapsed().as_secs_f64() * 1000.0;
            assert_eq!(cached.check_id, run.check_id);
            assert_eq!(
                git_counts(&trace),
                before_cache,
                "cache reads must not fetch"
            );
            let item = json!({
                "trial":trial+1,"round":round,"cache":if round==0 {"cold"} else {"warm"},
                "elapsedMs":elapsed_ms,"fetches":fetches,"clones":clones,"blobFetches":blob_fetches,
                "cachedReadMs":cache_ms,"progressEvents":events.len(),"run":run,
            });
            println!("LIVE_GITHUB_CHECK {}", item);
            evidence.push(item);
        }
    }
    // Preview the exact same sources through the confirmation entry point and
    // compare its answer with the independently seeded fixture expectations.
    let mut preview_evidence = vec![];
    for sample in samples {
        assert!(
            overall_started.elapsed() < std::time::Duration::from_secs(180),
            "live probe deadline reached"
        );
        let id = sample["id"].as_str().unwrap();
        let started = Instant::now();
        let result = service.preview_content(id, false);
        let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
        if sample["expectedCheck"] == "failed" {
            assert!(result.is_err());
            preview_evidence
                .push(json!({"id":id,"name":sample["name"],"failed":true,"elapsedMs":elapsed_ms}));
        } else {
            let preview = result.unwrap();
            assert_eq!(
                preview.changes.is_empty(),
                sample["expectedCheck"] == "unchanged"
            );
            if sample["expectedCheck"] == "changed" {
                assert_eq!(preview.changes.len(), 2);
            }
            preview_evidence.push(json!({"id":id,"name":sample["name"],"changes":preview.changes.len(),"elapsedMs":elapsed_ms}));
        }
    }
    for (path, hash) in &before {
        assert_eq!(hash_dir_strict(path).unwrap(), *hash);
    }
    let report = json!({"root":root,"rounds":evidence,"previews":preview_evidence,"centralAndTargetsUnchanged":true});
    fs::write(
        root.parent().unwrap().join("live-probe.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    let _ = deadline_tx.send(());
    deadline.join().unwrap();
}
