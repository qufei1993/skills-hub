use super::*;
use crate::core::cli_bridge::{
    cli_bridge_status, publish_cli_bridge, BINARY_NAME, HASH_STAMP, VERSION_STAMP,
};
use crate::core::network_proxy::app_http_client;
const HASH: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
fn manifest() -> CliManifest {
    CliManifest {
        version: "0.11.0".into(),
        source_commit: "a".repeat(40),
        target: "aarch64-apple-darwin".into(),
        profile: "release".into(),
        asset_name: "skillshub-cli-0.11.0-darwin-arm64".into(),
        size: 3,
        sha256: HASH.into(),
    }
}
#[test]
fn cli_distribution_downloads_verified_bytes_and_reports_phases() {
    let mut server = mockito::Server::new();
    let request = server.mock("GET", "/cli").with_body("abc").create();
    let root = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    let phases = std::sync::Mutex::new(Vec::new());
    let client = app_http_client("", Some(2)).unwrap();
    let result = download_and_publish(
        &manifest(),
        &root.path().join("bridge"),
        &format!("{}/cli", server.url()),
        &client,
        &|phase, _, _| phases.lock().unwrap().push(phase),
    )
    .unwrap();
    assert_eq!(result.status, CliBridgeHealth::Valid);
    assert_eq!(std::fs::read(result.path).unwrap(), b"abc");
    assert!(phases
        .lock()
        .unwrap()
        .contains(&CliPreparationPhase::Verifying));
    request.assert();
}
#[test]
fn cli_distribution_failures_preserve_verified_previous_cli() {
    for (status, bytes, expected) in [
        (404, "private response", "CLI_DOWNLOAD_UNAVAILABLE"),
        (200, "ab", "CLI_INTEGRITY_FAILED"),
        (200, "abcd", "CLI_INTEGRITY_FAILED"),
        (200, "xyz", "CLI_INTEGRITY_FAILED"),
    ] {
        let mut server = mockito::Server::new();
        let request = server
            .mock("GET", "/cli")
            .with_status(status)
            .with_body(bytes)
            .create();
        let root = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
        let source = root.path().join("old");
        std::fs::write(&source, b"abc").unwrap();
        let bridge = root.path().join("bridge");
        publish_cli_bridge(&source, &bridge, "0.10.1", HASH).unwrap();
        let error = download_and_publish(
            &manifest(),
            &bridge,
            &format!("{}/cli", server.url()),
            &app_http_client("", Some(2)).unwrap(),
            &|_, _, _| {},
        )
        .unwrap_err();
        assert_eq!(error.to_string(), expected);
        assert_eq!(std::fs::read(bridge.join(BINARY_NAME)).unwrap(), b"abc");
        assert_eq!(
            std::fs::read_to_string(bridge.join(VERSION_STAMP))
                .unwrap()
                .trim(),
            "0.10.1"
        );
        assert_eq!(
            cli_bridge_status(&bridge, "0.10.1", HASH).status,
            CliBridgeHealth::Valid
        );
        request.assert();
    }
}
#[test]
fn cli_distribution_reuses_valid_installation_offline_but_rejects_tampered_hash_stamp() {
    let root = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    let source = root.path().join("cli");
    std::fs::write(&source, b"abc").unwrap();
    let bridge = root.path().join("bridge");
    publish_cli_bridge(&source, &bridge, "0.11.0", HASH).unwrap();
    let client = app_http_client("", Some(1)).unwrap();
    assert_eq!(
        download_and_publish(
            &manifest(),
            &bridge,
            "http://127.0.0.1:9",
            &client,
            &|_, _, _| {}
        )
        .unwrap()
        .status,
        CliBridgeHealth::Valid
    );
    std::fs::write(bridge.join(BINARY_NAME), b"xyz").unwrap();
    std::fs::write(
        bridge.join(HASH_STAMP),
        format!("{:x}", Sha256::digest(b"xyz")),
    )
    .unwrap();
    assert!(download_and_publish(
        &manifest(),
        &bridge,
        "http://127.0.0.1:9",
        &client,
        &|_, _, _| {}
    )
    .is_err());
}
#[test]
fn cli_distribution_timeout_preserves_old_installation() {
    let mut server = mockito::Server::new();
    let _request = server
        .mock("GET", "/cli")
        .with_chunked_body(|writer| {
            std::thread::sleep(std::time::Duration::from_millis(1300));
            let _ = writer.write_all(b"abc");
            Ok(())
        })
        .create();
    let root = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    let source = root.path().join("old");
    std::fs::write(&source, b"abc").unwrap();
    let bridge = root.path().join("bridge");
    publish_cli_bridge(&source, &bridge, "0.10.1", HASH).unwrap();
    let result = download_and_publish(
        &manifest(),
        &bridge,
        &format!("{}/cli", server.url()),
        &app_http_client("", Some(1)).unwrap(),
        &|_, _, _| {},
    );
    assert!(result.is_err());
    assert_eq!(
        cli_bridge_status(&bridge, "0.10.1", HASH).status,
        CliBridgeHealth::Valid
    );
}

#[test]
fn local_test_cli_installs_verified_embedded_bytes_without_downloading() {
    let root = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    let bridge = root.path().join("dev-bridge");
    let phases = std::sync::Mutex::new(Vec::new());
    let status = prepare_local_cli(&manifest(), b"abc", &bridge, &|phase, _, _| {
        phases.lock().unwrap().push(phase)
    })
    .unwrap();
    assert_eq!(status.status, CliBridgeHealth::Valid);
    assert_eq!(std::fs::read(status.path).unwrap(), b"abc");
    let phases = phases.lock().unwrap();
    assert!(phases.contains(&CliPreparationPhase::Verifying));
    assert!(phases.contains(&CliPreparationPhase::Installing));
    assert!(!phases.contains(&CliPreparationPhase::Downloading));
}

#[test]
fn local_test_cli_integrity_failures_preserve_existing_cli() {
    let root = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    let source = root.path().join("old");
    std::fs::write(&source, b"abc").unwrap();
    let bridge = root.path().join("dev-bridge");
    publish_cli_bridge(&source, &bridge, "0.10.1", HASH).unwrap();
    for bytes in [b"ab".as_slice(), b"xyz".as_slice()] {
        assert_eq!(
            prepare_local_cli(&manifest(), bytes, &bridge, &|_, _, _| {})
                .unwrap_err()
                .to_string(),
            "CLI_INTEGRITY_FAILED"
        );
        assert_eq!(
            cli_bridge_status(&bridge, "0.10.1", HASH).status,
            CliBridgeHealth::Valid
        );
    }
}

#[cfg(skills_hub_local_bundle)]
#[test]
fn release_local_test_installs_embedded_cli_without_network() {
    let root = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    let manifest = embedded_cli_manifest().unwrap();
    let phases = std::sync::Mutex::new(Vec::new());
    let status = prepare_cli(
        &manifest,
        &root.path().join("bridge"),
        Some("http://127.0.0.1:9"),
        &|phase, _, _| phases.lock().unwrap().push(phase),
    )
    .unwrap();
    assert_eq!(status.status, CliBridgeHealth::Valid);
    assert!(!phases
        .lock()
        .unwrap()
        .contains(&CliPreparationPhase::Downloading));
    let result = std::process::Command::new(&status.path)
        .arg("--version")
        .output()
        .unwrap();
    assert!(result.status.success());
    assert!(String::from_utf8_lossy(&result.stdout).contains(&manifest.version));
    let paths = crate::core::runtime_paths::RuntimePaths::from_roots(
        crate::core::runtime_paths::RuntimeProfile::current(),
        root.path(),
        root.path(),
    );
    assert_eq!(
        paths.profile,
        crate::core::runtime_paths::RuntimeProfile::Development
    );
    assert!(paths.cli_bridge_dir.ends_with(".skills-hub-dev/bin"));
    assert_eq!(
        crate::core::github_token::GITHUB_TOKEN_KEYRING_SERVICE,
        "com.skills-hub.github-token.dev"
    );
}
