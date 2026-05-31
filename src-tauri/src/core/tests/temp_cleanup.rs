use std::fs;
use std::time::Duration;

use super::{cleanup_old_git_temp_dirs_in, mark_temp_dir};

#[test]
fn cleanup_removes_only_marked_prefixed_dirs() {
    let dir = tempfile::tempdir().unwrap();
    let cache = dir.path();

    let d1 = cache.join("skills-syncer-git-1");
    let d2 = cache.join("skills-syncer-git-2");
    let d3 = cache.join("other-3");

    fs::create_dir_all(&d1).unwrap();
    fs::create_dir_all(&d2).unwrap();
    fs::create_dir_all(&d3).unwrap();

    mark_temp_dir(&d1).unwrap();
    mark_temp_dir(&d3).unwrap();

    let removed = cleanup_old_git_temp_dirs_in(cache, Duration::from_secs(0)).unwrap();
    assert_eq!(removed, 1);
    assert!(!d1.exists());
    assert!(d2.exists(), "未标记的不应删除");
    assert!(d3.exists(), "前缀不匹配的不应删除");
}
