use std::path::PathBuf;

use crate::core::runtime_paths::{RuntimePaths, RuntimeProfile};

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
