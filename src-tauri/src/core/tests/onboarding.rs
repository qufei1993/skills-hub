use std::fs;

use super::build_onboarding_plan_in_home;

#[test]
fn groups_by_name_and_detects_conflicts_by_fingerprint() {
    let home = tempfile::tempdir().unwrap();

    // Cursor installed
    fs::create_dir_all(home.path().join(".cursor")).unwrap();
    fs::create_dir_all(home.path().join(".cursor/skills/foo")).unwrap();
    fs::write(home.path().join(".cursor/skills/foo/a.txt"), b"cursor").unwrap();

    // Codex installed
    fs::create_dir_all(home.path().join(".codex")).unwrap();
    fs::create_dir_all(home.path().join(".codex/skills/foo")).unwrap();
    fs::write(home.path().join(".codex/skills/foo/a.txt"), b"codex").unwrap();

    // Codex .system should be ignored
    fs::create_dir_all(home.path().join(".codex/skills/.system")).unwrap();
    fs::write(home.path().join(".codex/skills/.system/SKILL.md"), b"x").unwrap();

    let plan = build_onboarding_plan_in_home(home.path(), None, None).unwrap();
    assert_eq!(plan.total_tools_scanned, 2);
    assert_eq!(plan.total_skills_found, 2);
    assert_eq!(plan.groups.len(), 1);
    assert_eq!(plan.groups[0].name, "foo");
    assert!(plan.groups[0].has_conflict, "同名但内容不同应冲突");
    assert_eq!(plan.groups[0].variants.len(), 2);
}

#[test]
#[cfg(unix)]
fn excludes_central_repo_path() {
    use std::os::unix::fs::symlink;

    let home = tempfile::tempdir().unwrap();

    // Cursor installed
    std::fs::create_dir_all(home.path().join(".cursor")).unwrap();
    std::fs::create_dir_all(home.path().join(".cursor/skills")).unwrap();

    let central = home.path().join("central");
    std::fs::create_dir_all(central.join("skill-a")).unwrap();

    let link_path = home.path().join(".cursor/skills/skill-a");
    symlink(central.join("skill-a"), &link_path).unwrap();

    let plan = build_onboarding_plan_in_home(home.path(), Some(&central), None).unwrap();
    assert_eq!(plan.total_skills_found, 0);
}

#[test]
fn excludes_managed_skill_targets() {
    let home = tempfile::tempdir().unwrap();

    // Cursor installed
    fs::create_dir_all(home.path().join(".cursor")).unwrap();
    fs::create_dir_all(home.path().join(".cursor/skills/foo")).unwrap();
    fs::write(home.path().join(".cursor/skills/foo/a.txt"), b"cursor").unwrap();

    let mut exclude = std::collections::HashSet::new();
    exclude.insert(super::managed_target_key(
        "cursor",
        &home.path().join(".cursor/skills/foo"),
    ));

    let plan = build_onboarding_plan_in_home(home.path(), None, Some(&exclude)).unwrap();
    assert_eq!(plan.total_skills_found, 0);
}
