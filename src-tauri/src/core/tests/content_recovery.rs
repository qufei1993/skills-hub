use std::fs;
use std::path::{Path, PathBuf};

use serde_json::json;
use tempfile::TempDir;

use crate::core::content_hash::hash_dir_strict;
use crate::core::runtime_paths::{RuntimePaths, RuntimeProfile};
use crate::core::skill_store::SkillTargetRecord;
use crate::services::error::ErrorCode;
use crate::services::install::InstallRequest;
use crate::services::skills_hub::SkillsHubService;

struct Fixture {
    root: TempDir,
    service: SkillsHubService,
    source: PathBuf,
    central: PathBuf,
    id: String,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let service = SkillsHubService::open(RuntimePaths::from_roots(
            RuntimeProfile::Test,
            root.path().join("home"),
            root.path().join("data"),
        ))
        .unwrap();
        let source = root.path().join("source");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("SKILL.md"), "---\nname: demo\n---\nold").unwrap();
        fs::write(source.join("run.sh"), "#!/bin/sh\nexit 0\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(source.join("run.sh"), fs::Permissions::from_mode(0o644)).unwrap();
        }
        fs::create_dir(source.join("old-empty")).unwrap();
        let installed = service.install(InstallRequest::local(&source)).unwrap();
        Self {
            root,
            service,
            source,
            central: installed.central_path.into(),
            id: installed.id,
        }
    }

    fn update(&self) {
        fs::write(self.source.join("SKILL.md"), "---\nname: demo\n---\nnew").unwrap();
        let preview = self.service.preview_content(&self.id, false).unwrap();
        assert_eq!(
            self.service.execute_content(&preview.id).unwrap()["changed"],
            true
        );
    }

    fn target(&self, path: &Path, mode: &str) {
        self.service
            .store()
            .upsert_skill_target(&SkillTargetRecord {
                id: "review-target".into(),
                skill_id: self.id.clone(),
                tool: "codex".into(),
                scope: "global".into(),
                project_path: None,
                target_path: path.to_string_lossy().into_owned(),
                mode: mode.into(),
                status: "ok".into(),
                last_error: None,
                synced_at: Some(1),
            })
            .unwrap();
    }

    fn assert_target_conflict_is_preserved(&self, path: &Path) {
        let central_hash = hash_dir_strict(&self.central).unwrap();
        let target_hash = crate::core::sync_engine::deployment_fingerprint(path).unwrap();
        let record = self.service.store().get_skill_by_id(&self.id).unwrap();
        let targets = self.service.store().list_skill_targets(&self.id).unwrap();
        let recoverable = self.service.recoverable_skills().unwrap();
        for restore in [false, true] {
            let error = self.service.preview_content(&self.id, restore).unwrap_err();
            assert_eq!(error.code, ErrorCode::TargetConflict);
            assert_eq!(error.details["skill_id"], self.id);
            assert_eq!(error.details["agent"], "codex");
            assert_eq!(error.details["path"], path.to_string_lossy().as_ref());
            assert_eq!(error.details["reason"], "modified_target");
        }
        assert_eq!(hash_dir_strict(&self.central).unwrap(), central_hash);
        assert_eq!(
            crate::core::sync_engine::deployment_fingerprint(path).unwrap(),
            target_hash
        );
        assert_eq!(
            self.service.store().get_skill_by_id(&self.id).unwrap(),
            record
        );
        assert_eq!(
            self.service.store().list_skill_targets(&self.id).unwrap(),
            targets
        );
        assert_eq!(self.service.recoverable_skills().unwrap(), recoverable);
    }
}

#[test]
fn repeated_content_checks_keep_one_plan_and_reject_old_confirmations() {
    let f = Fixture::new();
    let central_hash = hash_dir_strict(&f.central).unwrap();
    let first = f.service.preview_content(&f.id, false).unwrap();
    assert!(first.changes.is_empty());
    let latest = f.service.preview_content(&f.id, false).unwrap();
    assert!(latest.changes.is_empty());
    assert_ne!(first.id, latest.id);

    let plans = f
        .service
        .store()
        .workspace_list::<crate::core::content_recovery::ContentPreview>("content-preview")
        .unwrap();
    assert_eq!(plans.len(), 1);
    assert_eq!(plans[0].id, latest.id);
    assert_eq!(
        f.service.execute_content(&first.id).unwrap_err().code,
        ErrorCode::PlanStale
    );
    // Preserve the existing single-Skill confirmation contract for no-op plans.
    assert_eq!(
        f.service.execute_content(&latest.id).unwrap()["changed"],
        false
    );
    assert_eq!(hash_dir_strict(&f.central).unwrap(), central_hash);
}

#[test]
fn replacing_a_content_check_preserves_other_skills_and_restore_plans() {
    let f = Fixture::new();
    f.update();
    let restore = f.service.preview_content(&f.id, true).unwrap();
    let first = f.service.preview_content(&f.id, false).unwrap();
    let other_source = f.root.path().join("other-source");
    fs::create_dir(&other_source).unwrap();
    fs::write(other_source.join("SKILL.md"), "---\nname: other\n---\nbody").unwrap();
    let other = f
        .service
        .install(InstallRequest::local(&other_source))
        .unwrap();
    let other_plan = f.service.preview_content(&other.id, false).unwrap();
    let latest = f.service.preview_content(&f.id, false).unwrap();

    let plans = f
        .service
        .store()
        .workspace_list::<crate::core::content_recovery::ContentPreview>("content-preview")
        .unwrap();
    let ids = plans.iter().map(|p| p.id.as_str()).collect::<Vec<_>>();
    assert_eq!(ids.len(), 3);
    for id in [&restore.id, &other_plan.id, &latest.id] {
        assert!(ids.contains(&id.as_str()));
    }
    assert!(!ids.contains(&first.id.as_str()));
    assert_eq!(
        f.service.execute_content(&first.id).unwrap_err().code,
        ErrorCode::PlanStale
    );
    assert_eq!(
        f.service.execute_content(&restore.id).unwrap()["changed"],
        true
    );
}

#[cfg(unix)]
#[test]
fn permission_only_changes_are_previewed_applied_and_restored() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new();
    let original = hash_dir_strict(&f.central).unwrap();
    fs::set_permissions(f.source.join("run.sh"), fs::Permissions::from_mode(0o755)).unwrap();

    let update = f.service.preview_content(&f.id, false).unwrap();
    let change = update
        .changes
        .iter()
        .find(|change| change.path == "run.sh")
        .expect("permission changes must be included in the reviewed plan");
    assert_eq!(change.kind, "modified");
    assert_eq!(change.before, change.after);
    let payload = serde_json::to_value(change).unwrap();
    assert_eq!(
        payload["beforeMetadata"],
        json!({"kind":"file","mode":0o644})
    );
    assert_eq!(
        payload["afterMetadata"],
        json!({"kind":"file","mode":0o755})
    );
    assert_eq!(hash_dir_strict(&f.central).unwrap(), original);
    assert_eq!(
        f.service.execute_content(&update.id).unwrap()["changed"],
        true
    );
    assert_eq!(
        fs::metadata(f.central.join("run.sh"))
            .unwrap()
            .permissions()
            .mode()
            & 0o7777,
        0o755
    );

    let restore = f.service.preview_content(&f.id, true).unwrap();
    let change = restore
        .changes
        .iter()
        .find(|change| change.path == "run.sh")
        .unwrap();
    let payload = serde_json::to_value(change).unwrap();
    assert_eq!(payload["beforeMetadata"]["mode"], 0o755);
    assert_eq!(payload["afterMetadata"]["mode"], 0o644);
    f.service.execute_content(&restore.id).unwrap();
    assert_eq!(hash_dir_strict(&f.central).unwrap(), original);
}

#[test]
fn content_check_summary_preserves_plans_and_detects_local_content_and_empty_directories() {
    let f = Fixture::new();
    let plan = f.service.preview_content(&f.id, false).unwrap();
    let check = || {
        let mut items = Vec::new();
        super::check_batch(
            f.service.paths(),
            f.service.store(),
            std::slice::from_ref(&f.id),
            None,
            |_, result| items.push(result.unwrap()),
        )
        .unwrap();
        items.pop().unwrap()
    };
    assert!(!check().changed);
    fs::create_dir(f.source.join("new-empty")).unwrap();
    assert!(check().changed);
    fs::remove_dir(f.source.join("new-empty")).unwrap();
    assert!(!check().changed);
    fs::write(f.central.join("SKILL.md"), "local edit").unwrap();
    assert!(check().changed);
    assert_eq!(
        fs::read_to_string(f.central.join("SKILL.md")).unwrap(),
        "local edit"
    );
    let plans = f
        .service
        .store()
        .workspace_list::<super::ContentPreview>("content-preview")
        .unwrap();
    assert_eq!(plans.len(), 1);
    assert_eq!(plans[0].id, plan.id);
}

#[test]
fn content_check_discovers_changes_without_bypassing_preview_target_conflict_checks() {
    let f = Fixture::new();
    let target = f.root.path().join("modified-target");
    crate::core::sync_engine::copy_dir_recursive(&f.central, &target).unwrap();
    f.target(&target, "copy");
    fs::write(target.join("run.sh"), "personal change").unwrap();
    fs::write(
        f.source.join("SKILL.md"),
        "---\nname: demo\n---\nnew source",
    )
    .unwrap();
    let central_before = hash_dir_strict(&f.central).unwrap();
    let target_before = hash_dir_strict(&target).unwrap();
    let mut items = Vec::new();
    super::check_batch(
        f.service.paths(),
        f.service.store(),
        std::slice::from_ref(&f.id),
        None,
        |_, result| items.push(result.unwrap()),
    )
    .unwrap();
    assert!(items[0].changed);
    assert_eq!(items[0].target_count, 1);
    assert_eq!(
        f.service.preview_content(&f.id, false).unwrap_err().code,
        ErrorCode::TargetConflict
    );
    assert_eq!(hash_dir_strict(&f.central).unwrap(), central_before);
    assert_eq!(hash_dir_strict(&target).unwrap(), target_before);
    assert!(f
        .service
        .store()
        .workspace_list::<super::ContentPreview>("content-preview")
        .unwrap()
        .is_empty());
}

#[cfg(unix)]
#[test]
fn content_check_summary_detects_permission_only_changes() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new();
    fs::set_permissions(f.source.join("run.sh"), fs::Permissions::from_mode(0o755)).unwrap();
    let mut items = Vec::new();
    super::check_batch(
        f.service.paths(),
        f.service.store(),
        std::slice::from_ref(&f.id),
        None,
        |_, result| items.push(result.unwrap()),
    )
    .unwrap();
    assert!(items[0].changed);
    assert_ne!(items[0].central_hash, items[0].source_hash);
    assert_eq!(
        fs::metadata(f.central.join("run.sh"))
            .unwrap()
            .permissions()
            .mode()
            & 0o7777,
        0o644
    );
}

#[test]
fn empty_directory_changes_are_previewed_applied_and_restored() {
    let f = Fixture::new();
    let original = hash_dir_strict(&f.central).unwrap();
    fs::remove_dir(f.source.join("old-empty")).unwrap();
    fs::create_dir(f.source.join("new-empty")).unwrap();

    let update = f.service.preview_content(&f.id, false).unwrap();
    assert_eq!(
        update.changes.len(),
        2,
        "directory-only changes cannot be reported as unchanged"
    );
    let added = update
        .changes
        .iter()
        .find(|change| change.path == "new-empty")
        .unwrap();
    let removed = update
        .changes
        .iter()
        .find(|change| change.path == "old-empty")
        .unwrap();
    assert_eq!(added.kind, "added");
    assert_eq!(removed.kind, "removed");
    assert_eq!(
        serde_json::to_value(added).unwrap()["afterMetadata"],
        json!({"kind":"directory"})
    );
    assert_eq!(
        serde_json::to_value(removed).unwrap()["beforeMetadata"],
        json!({"kind":"directory"})
    );
    assert_eq!(hash_dir_strict(&f.central).unwrap(), original);
    assert_eq!(
        f.service.execute_content(&update.id).unwrap()["changed"],
        true
    );
    assert!(f.central.join("new-empty").is_dir());
    assert!(!f.central.join("old-empty").exists());

    let restore = f.service.preview_content(&f.id, true).unwrap();
    assert_eq!(restore.changes.len(), 2);
    assert!(restore
        .changes
        .iter()
        .any(|change| change.path == "old-empty" && change.kind == "added"));
    assert!(restore
        .changes
        .iter()
        .any(|change| change.path == "new-empty" && change.kind == "removed"));
    f.service.execute_content(&restore.id).unwrap();
    assert_eq!(hash_dir_strict(&f.central).unwrap(), original);
}

#[test]
fn modified_copy_previews_return_safe_target_conflict_without_mutating_content() {
    let f = Fixture::new();
    f.update();
    let target = f.root.path().join("copy-target");
    crate::core::sync_engine::copy_dir_recursive(&f.central, &target).unwrap();
    f.target(&target, "copy");
    fs::write(target.join("run.sh"), "personal edit").unwrap();
    f.assert_target_conflict_is_preserved(&target);
}

#[cfg(unix)]
#[test]
fn redirected_link_previews_return_safe_target_conflict_without_mutating_content() {
    let f = Fixture::new();
    f.update();
    let other = f.root.path().join("personal-directory");
    fs::create_dir(&other).unwrap();
    fs::write(other.join("personal.txt"), "keep").unwrap();
    let target = f.root.path().join("link-target");
    std::os::unix::fs::symlink(&other, &target).unwrap();
    f.target(&target, "symlink");
    f.assert_target_conflict_is_preserved(&target);
    assert_eq!(
        fs::read_to_string(other.join("personal.txt")).unwrap(),
        "keep"
    );
}

#[test]
fn restoring_a_disabled_skill_keeps_it_and_its_targets_disabled() {
    let f = Fixture::new();
    let original = hash_dir_strict(&f.central).unwrap();
    f.update();
    let target = f.root.path().join("disabled-copy");
    crate::core::sync_engine::copy_dir_recursive(&f.central, &target).unwrap();
    f.target(&target, "copy");
    // The disabled state retains the logical target after removing its files.
    fs::remove_dir_all(&target).unwrap();
    f.service
        .store()
        .update_skill_target_status(&f.id, "codex", "global", None, "disabled")
        .unwrap();
    f.service.store().set_skill_enabled(&f.id, false).unwrap();
    let targets = f.service.store().list_skill_targets(&f.id).unwrap();

    let restore = f.service.preview_content(&f.id, true).unwrap();
    let result = f.service.execute_content(&restore.id).unwrap();
    assert_eq!(result["changed"], true);
    assert_eq!(result["pending_targets"], json!([]));
    assert_eq!(hash_dir_strict(&f.central).unwrap(), original);
    assert!(
        !f.service
            .store()
            .get_skill_by_id(&f.id)
            .unwrap()
            .unwrap()
            .enabled
    );
    assert_eq!(
        f.service.store().list_skill_targets(&f.id).unwrap(),
        targets
    );
    assert!(
        !target.exists(),
        "restoration must not redeploy disabled content"
    );
    assert!(f.service.recoverable_skills().unwrap().is_empty());
}

#[test]
fn saved_previews_without_entry_metadata_remain_readable() {
    let plan = json!({
        "id":"saved", "skillId":"demo", "name":"demo", "targets":[],
        "restore":false, "fingerprint":"saved-fingerprint",
        "changes":[{"path":"SKILL.md","kind":"modified","before":"old","after":"new"}]
    });
    let decoded: super::ContentPreview = serde_json::from_value(plan).unwrap();
    assert_eq!(decoded.changes[0].before.as_deref(), Some("old"));
    assert_eq!(decoded.changes[0].after.as_deref(), Some("new"));
    assert!(decoded.changes[0].before_metadata.is_none());
    assert!(decoded.changes[0].after_metadata.is_none());
}
