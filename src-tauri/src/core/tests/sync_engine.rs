use std::fs;
use std::path::PathBuf;

use crate::core::sync_engine::{
    copy_dir_recursive, path_is_protected_real_content, remove_path_safely_with,
    sync_dir_for_tool_with_overwrite, sync_dir_hybrid, sync_dir_hybrid_with_overwrite,
    sync_dir_with_mode_with_overwrite, sync_managed_copy_with_expected_hash,
    PreparedDirReplacement, SyncMode,
};

#[test]
fn copy_dir_recursive_skips_git_dir() {
    let src_dir = tempfile::tempdir().unwrap();
    let dst_dir = tempfile::tempdir().unwrap();

    fs::create_dir_all(src_dir.path().join(".git")).unwrap();
    fs::create_dir_all(src_dir.path().join("sub")).unwrap();
    fs::write(src_dir.path().join("sub/a.txt"), b"ok").unwrap();
    fs::write(src_dir.path().join(".git/secret"), b"no").unwrap();

    copy_dir_recursive(src_dir.path(), dst_dir.path()).unwrap();
    assert!(dst_dir.path().join("sub/a.txt").exists());
    assert!(!dst_dir.path().join(".git").exists());
}

#[test]
fn hybrid_sync_creates_link_and_is_idempotent_when_same_link() {
    let src_dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(src_dir.path().join("s")).unwrap();
    fs::write(src_dir.path().join("s/a.txt"), b"ok").unwrap();

    let dst_dir = tempfile::tempdir().unwrap();
    let target = dst_dir.path().join("t");

    let out = sync_dir_hybrid(src_dir.path(), &target).unwrap();
    assert!(matches!(
        out.mode_used,
        SyncMode::Symlink | SyncMode::Junction | SyncMode::Copy
    ));

    if let Ok(link) = fs::read_link(&target) {
        assert_eq!(link, src_dir.path());
        let out2 = sync_dir_hybrid(src_dir.path(), &target).unwrap();
        assert!(matches!(out2.mode_used, SyncMode::Symlink));
    }
}

#[test]
fn hybrid_sync_with_overwrite_replaces_existing() {
    let src_dir = tempfile::tempdir().unwrap();
    fs::write(src_dir.path().join("a.txt"), b"src").unwrap();

    let dst_dir = tempfile::tempdir().unwrap();
    let target = dst_dir.path().join("t");
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("old.txt"), b"old").unwrap();

    let err = sync_dir_hybrid_with_overwrite(src_dir.path(), &target, false).unwrap_err();
    assert!(format!("{:#}", err).contains("target already exists"));

    let out = sync_dir_hybrid_with_overwrite(src_dir.path(), &target, true).unwrap();
    assert!(out.replaced);
}

#[test]
fn overwrite_rejects_same_source_and_target_without_deleting_source() {
    let source = tempfile::tempdir().unwrap();
    fs::write(source.path().join("keep.txt"), b"keep").unwrap();

    let err = sync_dir_hybrid_with_overwrite(source.path(), source.path(), true).unwrap_err();

    assert!(err.to_string().contains("overlap"));
    assert_eq!(fs::read(source.path().join("keep.txt")).unwrap(), b"keep");
}

#[test]
fn copy_sync_rejects_target_inside_source_without_creating_it() {
    let source = tempfile::tempdir().unwrap();
    fs::write(source.path().join("keep.txt"), b"keep").unwrap();
    let target = source.path().join("nested/skill");

    let err = crate::core::sync_engine::sync_dir_copy_with_overwrite(source.path(), &target, false)
        .unwrap_err();

    assert!(err.to_string().contains("overlap"));
    assert!(!target.exists());
    assert_eq!(fs::read(source.path().join("keep.txt")).unwrap(), b"keep");
}

#[test]
fn overwrite_rejects_target_that_contains_source_without_deleting_either() {
    let target = tempfile::tempdir().unwrap();
    let source = target.path().join("source");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("keep.txt"), b"keep").unwrap();

    let err = sync_dir_hybrid_with_overwrite(&source, target.path(), true).unwrap_err();

    assert!(err.to_string().contains("overlap"));
    assert_eq!(fs::read(source.join("keep.txt")).unwrap(), b"keep");
    assert!(target.path().exists());
}

#[test]
fn safe_remove_sends_real_directory_to_recycler() {
    let root = tempfile::tempdir().unwrap();
    let target = root.path().join("target");
    let recycled = root.path().join("recycled");
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("keep.txt"), b"keep").unwrap();

    remove_path_safely_with(&target, |path| {
        fs::rename(path, &recycled).map_err(anyhow::Error::from)
    })
    .unwrap();

    assert!(!target.exists());
    assert_eq!(fs::read(recycled.join("keep.txt")).unwrap(), b"keep");
}

#[cfg(unix)]
#[test]
fn safe_remove_unlinks_symlink_without_recycling_target() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    let link = root.path().join("link");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("keep.txt"), b"keep").unwrap();
    std::os::unix::fs::symlink(&source, &link).unwrap();

    remove_path_safely_with(&link, |_| {
        anyhow::bail!("symlink must not be sent to recycle bin")
    })
    .unwrap();

    assert!(fs::symlink_metadata(&link).is_err());
    assert_eq!(fs::read(source.join("keep.txt")).unwrap(), b"keep");
}

#[test]
fn real_target_overlapping_protected_source_is_preserved() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    fs::create_dir_all(&source).unwrap();

    assert!(path_is_protected_real_content(&source, std::slice::from_ref(&source)).unwrap());
}

#[cfg(unix)]
#[test]
fn symlink_to_protected_source_can_be_unlinked_safely() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    let link = root.path().join("link");
    fs::create_dir_all(&source).unwrap();
    std::os::unix::fs::symlink(&source, &link).unwrap();

    assert!(!path_is_protected_real_content(&link, &[source]).unwrap());
}

#[test]
fn cursor_sync_forces_copy() {
    let src_dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(src_dir.path().join("s")).unwrap();
    fs::write(src_dir.path().join("s/a.txt"), b"ok").unwrap();

    let dst_dir = tempfile::tempdir().unwrap();
    let target = dst_dir.path().join("t");

    let out = sync_dir_for_tool_with_overwrite("cursor", src_dir.path(), &target, false).unwrap();
    assert!(matches!(out.mode_used, SyncMode::Copy));
    assert!(target.join("s/a.txt").exists());
    assert_eq!(fs::read(target.join("s/a.txt")).unwrap(), b"ok");
}

#[test]
fn explicit_copy_mode_copies_directory() {
    let src_dir = tempfile::tempdir().unwrap();
    fs::write(src_dir.path().join("a.txt"), b"ok").unwrap();
    let dst_dir = tempfile::tempdir().unwrap();
    let target = dst_dir.path().join("t");

    let out =
        sync_dir_with_mode_with_overwrite(SyncMode::Copy, src_dir.path(), &target, false).unwrap();

    assert_eq!(out.mode_used, SyncMode::Copy);
    assert_eq!(fs::read(target.join("a.txt")).unwrap(), b"ok");
}

#[test]
fn managed_copy_replaces_only_content_matching_the_expected_hash() {
    let source = tempfile::tempdir().unwrap();
    fs::write(source.path().join("a.txt"), b"new").unwrap();
    let target_root = tempfile::tempdir().unwrap();
    let target = target_root.path().join("target");
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("a.txt"), b"old").unwrap();
    let expected_hash = crate::core::content_hash::hash_dir(&target).unwrap();

    let outcome =
        sync_managed_copy_with_expected_hash(source.path(), &target, &expected_hash).unwrap();

    assert!(outcome.replaced);
    assert_eq!(fs::read(target.join("a.txt")).unwrap(), b"new");
    assert!(fs::read_dir(target_root.path()).unwrap().all(|entry| !entry
        .unwrap()
        .file_name()
        .to_string_lossy()
        .starts_with(".skills-hub-")));
}

#[test]
fn managed_copy_preserves_a_real_directory_with_unexpected_content() {
    let source = tempfile::tempdir().unwrap();
    fs::write(source.path().join("a.txt"), b"new").unwrap();
    let expected = tempfile::tempdir().unwrap();
    fs::write(expected.path().join("a.txt"), b"old").unwrap();
    let expected_hash = crate::core::content_hash::hash_dir(expected.path()).unwrap();
    let target_root = tempfile::tempdir().unwrap();
    let target = target_root.path().join("target");
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("a.txt"), b"manual").unwrap();

    let error =
        sync_managed_copy_with_expected_hash(source.path(), &target, &expected_hash).unwrap_err();

    assert!(format!("{error:#}").contains("TARGET_MODIFIED"));
    assert_eq!(fs::read(target.join("a.txt")).unwrap(), b"manual");
}

#[test]
fn managed_copy_preserves_a_target_with_only_added_git_metadata() {
    let source = tempfile::tempdir().unwrap();
    fs::write(source.path().join("a.txt"), b"new").unwrap();
    let target_root = tempfile::tempdir().unwrap();
    let target = target_root.path().join("target");
    fs::create_dir_all(target.join(".git")).unwrap();
    fs::write(target.join("a.txt"), b"old").unwrap();
    let clean = tempfile::tempdir().unwrap();
    fs::write(clean.path().join("a.txt"), b"old").unwrap();
    let expected_hash = crate::core::content_hash::hash_dir_strict(clean.path()).unwrap();
    fs::write(target.join(".git/history"), b"keep").unwrap();

    let error =
        sync_managed_copy_with_expected_hash(source.path(), &target, &expected_hash).unwrap_err();

    assert!(format!("{error:#}").contains("TARGET_MODIFIED"));
    assert_eq!(fs::read(target.join(".git/history")).unwrap(), b"keep");
}

#[cfg(unix)]
#[test]
fn managed_copy_preserves_a_target_with_only_an_added_internal_symlink() {
    use std::os::unix::fs::symlink;

    let source = tempfile::tempdir().unwrap();
    fs::write(source.path().join("a.txt"), b"new").unwrap();
    let target_root = tempfile::tempdir().unwrap();
    let target = target_root.path().join("target");
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("a.txt"), b"old").unwrap();
    let expected_hash = crate::core::content_hash::hash_dir_strict(&target).unwrap();
    symlink("a.txt", target.join("shortcut")).unwrap();

    let error =
        sync_managed_copy_with_expected_hash(source.path(), &target, &expected_hash).unwrap_err();

    assert!(format!("{error:#}").contains("TARGET_MODIFIED"));
    assert_eq!(
        fs::read_link(target.join("shortcut")).unwrap(),
        PathBuf::from("a.txt")
    );
}

#[test]
fn managed_copy_rechecks_the_actual_directory_after_staging() {
    let source = tempfile::tempdir().unwrap();
    fs::write(source.path().join("a.txt"), b"new").unwrap();
    let target_root = tempfile::tempdir().unwrap();
    let target = target_root.path().join("target");
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("a.txt"), b"old").unwrap();
    let expected_hash = crate::core::content_hash::hash_dir(&target).unwrap();
    let mut replacement =
        PreparedDirReplacement::prepare_copy(source.path(), &target, Some(expected_hash), true)
            .unwrap();

    fs::write(target.join("a.txt"), b"manual-after-staging").unwrap();
    let error = replacement.activate().unwrap_err();

    assert!(format!("{error:#}").contains("TARGET_MODIFIED"));
    assert_eq!(
        fs::read(target.join("a.txt")).unwrap(),
        b"manual-after-staging"
    );
    drop(replacement);
    assert!(fs::read_dir(target_root.path()).unwrap().all(|entry| !entry
        .unwrap()
        .file_name()
        .to_string_lossy()
        .starts_with(".skills-hub-")));
}

#[test]
fn rollback_preserves_changes_written_after_activation() {
    let source = tempfile::tempdir().unwrap();
    fs::write(source.path().join("a.txt"), b"new").unwrap();
    let target_root = tempfile::tempdir().unwrap();
    let target = target_root.path().join("target");
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("a.txt"), b"old").unwrap();
    let expected_hash = crate::core::content_hash::hash_dir(&target).unwrap();
    let mut replacement =
        PreparedDirReplacement::prepare_copy(source.path(), &target, Some(expected_hash), true)
            .unwrap();
    replacement.activate().unwrap();
    fs::write(target.join("user-created.txt"), b"keep me").unwrap();

    let error = replacement.rollback().unwrap_err();

    assert!(format!("{error:#}").contains("ROLLBACK_CONFLICT"));
    assert_eq!(fs::read(target.join("a.txt")).unwrap(), b"old");
    let recovery = fs::read_dir(target_root.path())
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with(".skills-hub-recovery-")
        })
        .expect("concurrent changes should be preserved in a recovery directory");
    assert_eq!(
        fs::read(recovery.join("user-created.txt")).unwrap(),
        b"keep me"
    );
}

#[test]
fn rollback_preserves_git_metadata_written_after_activation() {
    let source = tempfile::tempdir().unwrap();
    fs::write(source.path().join("a.txt"), b"new").unwrap();
    let target_root = tempfile::tempdir().unwrap();
    let target = target_root.path().join("target");
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("a.txt"), b"old").unwrap();
    let expected_hash = crate::core::content_hash::hash_dir_strict(&target).unwrap();
    let mut replacement =
        PreparedDirReplacement::prepare_copy(source.path(), &target, Some(expected_hash), true)
            .unwrap();
    replacement.activate().unwrap();
    fs::create_dir_all(target.join(".git")).unwrap();
    fs::write(target.join(".git/history"), b"keep").unwrap();

    let error = replacement.rollback().unwrap_err();

    assert!(format!("{error:#}").contains("ROLLBACK_CONFLICT"));
    let recovery = fs::read_dir(target_root.path())
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with(".skills-hub-recovery-")
        })
        .unwrap();
    assert_eq!(fs::read(recovery.join(".git/history")).unwrap(), b"keep");
}

#[cfg(unix)]
#[test]
fn explicit_symlink_mode_creates_symlink() {
    let src_dir = tempfile::tempdir().unwrap();
    let dst_dir = tempfile::tempdir().unwrap();
    let target = dst_dir.path().join("t");

    let out = sync_dir_with_mode_with_overwrite(SyncMode::Symlink, src_dir.path(), &target, false)
        .unwrap();

    assert_eq!(out.mode_used, SyncMode::Symlink);
    assert_eq!(fs::read_link(&target).unwrap(), src_dir.path());
}

#[cfg(not(windows))]
#[test]
fn explicit_junction_mode_reports_unsupported_platform() {
    let src_dir = tempfile::tempdir().unwrap();
    let dst_dir = tempfile::tempdir().unwrap();
    let target = dst_dir.path().join("t");

    let err = sync_dir_with_mode_with_overwrite(SyncMode::Junction, src_dir.path(), &target, false)
        .unwrap_err();

    assert!(err.to_string().contains("junction not supported"));
}

#[cfg(windows)]
#[test]
fn overwrite_removes_junction_without_removing_its_target() {
    let source = tempfile::tempdir().unwrap();
    fs::write(source.path().join("source.txt"), b"source").unwrap();

    let target_root = tempfile::tempdir().unwrap();
    let junction_target = target_root.path().join("junction-target");
    fs::create_dir_all(&junction_target).unwrap();
    fs::write(junction_target.join("keep.txt"), b"keep").unwrap();

    let target = target_root.path().join("target");
    junction::create(&junction_target, &target).unwrap();

    let out = sync_dir_hybrid_with_overwrite(source.path(), &target, true).unwrap();
    assert!(out.replaced);
    assert!(junction_target.join("keep.txt").exists());
}

#[cfg(unix)]
#[test]
fn copy_overwrite_replaces_broken_symlink_target() {
    use std::os::unix::fs::symlink;

    let src_dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(src_dir.path().join("s")).unwrap();
    fs::write(src_dir.path().join("s/a.txt"), b"ok").unwrap();

    let dst_dir = tempfile::tempdir().unwrap();
    let target = dst_dir.path().join("t");

    // Create a broken symlink at the target path.
    symlink(dst_dir.path().join("missing"), &target).unwrap();

    let out = crate::core::sync_engine::sync_dir_copy_with_overwrite(src_dir.path(), &target, true)
        .unwrap();

    assert!(matches!(out.mode_used, SyncMode::Copy));
    assert!(target.join("s/a.txt").exists());
    assert_eq!(fs::read(target.join("s/a.txt")).unwrap(), b"ok");
}
