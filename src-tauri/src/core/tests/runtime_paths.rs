use std::path::PathBuf;

use crate::core::runtime_paths::{RuntimePaths, RuntimeProfile};

#[test]
fn build_profile_branches_select_isolated_database_and_bridge_namespaces() {
    for (debug, profile, identifier, central, bridge) in [
        (
            true,
            RuntimeProfile::Development,
            "com.qufei1993.skillshub.dev",
            ".skillshub-dev",
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
fn production_and_development_paths_do_not_overlap() {
    let prod = RuntimePaths::from_roots(RuntimeProfile::Production, "/home/may", "/data");
    let dev = RuntimePaths::from_roots(RuntimeProfile::Development, "/home/may", "/data");
    assert_eq!(
        prod.database_path,
        PathBuf::from("/data/com.qufei1993.skillshub/skills_hub.db")
    );
    assert_eq!(
        dev.database_path,
        PathBuf::from("/data/com.qufei1993.skillshub.dev/skills_hub.db")
    );
    assert_eq!(
        prod.default_central_repo,
        PathBuf::from("/home/may/.skillshub")
    );
    assert_eq!(
        prod.cli_bridge_dir,
        PathBuf::from("/home/may/.skills-hub/bin")
    );
}
