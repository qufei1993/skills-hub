use std::fs;

use crate::core::content_hash::hash_dir;

#[test]
fn sync_conflict_hash_ignores_only_python_cache_files() {
    use super::{hash_dir_for_sync_conflict, hash_dir_strict};
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("SKILL.md"), b"original").unwrap();
    let expected = hash_dir_for_sync_conflict(dir.path()).unwrap();
    let strict = hash_dir_strict(dir.path()).unwrap();
    fs::create_dir(dir.path().join("__pycache__")).unwrap();
    fs::write(
        dir.path().join("__pycache__/module.cpython-313.pyc"),
        b"cache",
    )
    .unwrap();
    assert_eq!(hash_dir_for_sync_conflict(dir.path()).unwrap(), expected);
    assert_ne!(hash_dir_strict(dir.path()).unwrap(), strict);
    fs::write(dir.path().join("__pycache__/notes.md"), b"user notes").unwrap();
    assert_ne!(hash_dir_for_sync_conflict(dir.path()).unwrap(), expected);
}

#[cfg(unix)]
#[test]
fn sync_conflict_hash_protects_document_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let document = dir.path().join("SKILL.md");
    fs::write(&document, b"original").unwrap();
    fs::set_permissions(&document, fs::Permissions::from_mode(0o644)).unwrap();
    let expected = super::hash_dir_for_sync_conflict(dir.path()).unwrap();
    fs::set_permissions(&document, fs::Permissions::from_mode(0o600)).unwrap();
    assert_ne!(
        super::hash_dir_for_sync_conflict(dir.path()).unwrap(),
        expected
    );
}

#[cfg(unix)]
#[test]
fn sync_conflict_hash_protects_symlinks_disguised_as_python_cache() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("SKILL.md"), b"original").unwrap();
    let expected = super::hash_dir_for_sync_conflict(dir.path()).unwrap();
    fs::create_dir(dir.path().join("__pycache__")).unwrap();
    std::os::unix::fs::symlink("../SKILL.md", dir.path().join("__pycache__/module.pyc")).unwrap();
    assert_ne!(
        super::hash_dir_for_sync_conflict(dir.path()).unwrap(),
        expected
    );
}

#[test]
fn hash_changes_with_content_and_ignores_git_dir() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();

    fs::create_dir_all(root.join("sub")).unwrap();
    fs::write(root.join("a.txt"), b"hello").unwrap();
    fs::write(root.join("sub/b.txt"), b"world").unwrap();

    let h1 = hash_dir(root).unwrap();

    fs::create_dir_all(root.join(".git")).unwrap();
    fs::write(root.join(".git/ignored"), b"ignored").unwrap();
    let h2 = hash_dir(root).unwrap();
    assert_eq!(h1, h2, "应忽略 .git 内容");

    fs::write(root.join("a.txt"), b"hello2").unwrap();
    let h3 = hash_dir(root).unwrap();
    assert_ne!(h2, h3);
}

#[test]
fn hash_uses_unambiguous_path_and_content_boundaries() {
    let first = tempfile::tempdir().expect("tempdir");
    fs::write(first.path().join("a"), b"bc").unwrap();

    let second = tempfile::tempdir().expect("tempdir");
    fs::write(second.path().join("ab"), b"c").unwrap();

    assert_ne!(
        hash_dir(first.path()).unwrap(),
        hash_dir(second.path()).unwrap()
    );
}

#[test]
fn hash_distinguishes_an_empty_file_from_an_empty_directory() {
    let file_tree = tempfile::tempdir().expect("tempdir");
    fs::write(file_tree.path().join("entry"), b"").unwrap();

    let directory_tree = tempfile::tempdir().expect("tempdir");
    fs::create_dir(directory_tree.path().join("entry")).unwrap();

    assert_ne!(
        hash_dir(file_tree.path()).unwrap(),
        hash_dir(directory_tree.path()).unwrap()
    );
}

#[test]
fn hash_includes_files_that_are_copied_to_managed_targets() {
    let dir = tempfile::tempdir().expect("tempdir");
    fs::write(dir.path().join(".gitignore"), b"first\n").unwrap();
    fs::write(dir.path().join(".DS_Store"), b"metadata-1").unwrap();
    let first = hash_dir(dir.path()).unwrap();

    fs::write(dir.path().join(".gitignore"), b"second\n").unwrap();
    let second = hash_dir(dir.path()).unwrap();
    assert_ne!(first, second, ".gitignore 会被复制，因此必须参与指纹");

    fs::write(dir.path().join(".DS_Store"), b"metadata-2").unwrap();
    let third = hash_dir(dir.path()).unwrap();
    assert_ne!(second, third, ".DS_Store 会被复制，因此必须参与指纹");
}

#[test]
fn hash_is_independent_of_creation_order() {
    let first = tempfile::tempdir().expect("tempdir");
    fs::create_dir(first.path().join("sub")).unwrap();
    fs::write(first.path().join("z.txt"), b"z").unwrap();
    fs::write(first.path().join("sub/a.txt"), b"a").unwrap();

    let second = tempfile::tempdir().expect("tempdir");
    fs::write(second.path().join("z.txt"), b"z").unwrap();
    fs::create_dir(second.path().join("sub")).unwrap();
    fs::write(second.path().join("sub/a.txt"), b"a").unwrap();

    assert_eq!(
        hash_dir(first.path()).unwrap(),
        hash_dir(second.path()).unwrap()
    );
}

#[cfg(unix)]
#[test]
fn hash_changes_when_copied_file_permissions_change() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().expect("tempdir");
    let script = dir.path().join("run.sh");
    fs::write(&script, b"#!/bin/sh\n").unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o644)).unwrap();
    let before = hash_dir(dir.path()).unwrap();

    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    let after = hash_dir(dir.path()).unwrap();

    assert_ne!(before, after);
}
