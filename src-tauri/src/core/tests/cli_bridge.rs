use super::*;
use crate::core::runtime_paths::{RuntimePaths, RuntimeProfile};
use std::fs::{self, OpenOptions};

const VERSION: &str = "0.10.1";
// SHA-256 of the fixture bytes "abc".
const HASH: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

struct Fixture {
    root: tempfile::TempDir,
    source: PathBuf,
    destination: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
        let source = root.path().join("source");
        fs::write(&source, b"abc").unwrap();
        let destination = root.path().join("bridge");
        Self {
            root,
            source,
            destination,
        }
    }
    fn publish(&self) -> Result<CliBridgeStatus> {
        publish_cli_bridge(&self.source, &self.destination, VERSION, HASH)
    }
    fn no_stamps(&self) {
        assert!(!self.destination.join(VERSION_STAMP).exists());
        assert!(!self.destination.join(HASH_STAMP).exists());
    }
}

#[test]
fn cli_bridge_hash_failure_invalidates_previous_stamps() {
    let f = Fixture::new();
    fs::create_dir_all(&f.destination).unwrap();
    fs::write(f.destination.join(VERSION_STAMP), VERSION).unwrap();
    fs::write(f.destination.join(HASH_STAMP), HASH).unwrap();
    let error =
        publish_cli_bridge(&f.source, &f.destination, VERSION, &"0".repeat(64)).unwrap_err();
    assert!(error.to_string().contains("CLI_BRIDGE_HASH_MISMATCH"));
    f.no_stamps();
    let malformed =
        publish_cli_bridge(&f.source, &f.destination, VERSION, "wrong-hash").unwrap_err();
    assert!(malformed.to_string().contains("CLI_BRIDGE_HASH_MISMATCH"));
}

#[test]
fn cli_bridge_replaces_existing_binary_and_completes_stamps() {
    let f = Fixture::new();
    fs::create_dir_all(&f.destination).unwrap();
    fs::write(f.destination.join(BINARY_NAME), b"old").unwrap();
    assert_eq!(f.publish().unwrap().status, CliBridgeHealth::Valid);
    assert_eq!(fs::read(f.destination.join(BINARY_NAME)).unwrap(), b"abc");
    assert_eq!(
        fs::read_to_string(f.destination.join(VERSION_STAMP))
            .unwrap()
            .trim(),
        VERSION
    );
    assert_eq!(
        fs::read_to_string(f.destination.join(HASH_STAMP))
            .unwrap()
            .trim(),
        HASH
    );
    assert_eq!(
        cli_bridge_status(&f.destination, VERSION, HASH).status,
        CliBridgeHealth::Valid
    );
}

#[test]
fn cli_bridge_interruption_and_missing_or_tampered_files_are_never_valid() {
    let f = Fixture::new();
    assert_eq!(
        cli_bridge_status(&f.destination, VERSION, HASH).status,
        CliBridgeHealth::Missing
    );
    fs::create_dir_all(&f.destination).unwrap();
    fs::write(
        f.destination.join(".skillshub-cli-interrupted.tmp"),
        b"partial",
    )
    .unwrap();
    assert_eq!(
        cli_bridge_status(&f.destination, VERSION, HASH).status,
        CliBridgeHealth::Damaged
    );
    f.publish().unwrap();
    fs::remove_file(f.destination.join(VERSION_STAMP)).unwrap();
    assert_eq!(
        cli_bridge_status(&f.destination, VERSION, HASH).reason,
        Some(CliBridgeReason::StampMissing)
    );
    f.publish().unwrap();
    fs::write(f.destination.join(BINARY_NAME), b"tampered").unwrap();
    assert_eq!(
        cli_bridge_status(&f.destination, VERSION, HASH).reason,
        Some(CliBridgeReason::HashMismatch)
    );
    f.publish().unwrap();
    assert_eq!(
        cli_bridge_status(&f.destination, "0.10.2", HASH).reason,
        Some(CliBridgeReason::VersionMismatch)
    );
}

#[test]
fn cli_bridge_rejects_invalid_metadata_and_missing_source_without_valid_stamps() {
    let f = Fixture::new();
    for (version, hash) in [("", HASH), ("../unsafe", HASH), (VERSION, "wrong-hash")] {
        assert!(publish_cli_bridge(&f.source, &f.destination, version, hash).is_err());
        f.no_stamps();
    }
    f.publish().unwrap();
    fs::remove_file(&f.source).unwrap();
    assert!(f
        .publish()
        .unwrap_err()
        .to_string()
        .contains("CLI_BRIDGE_SOURCE_MISSING"));
    f.no_stamps();
}

#[test]
fn cli_bridge_development_publication_never_changes_production() {
    let f = Fixture::new();
    let prod = RuntimePaths::from_roots(RuntimeProfile::Production, f.root.path(), f.root.path());
    let dev = RuntimePaths::from_roots(RuntimeProfile::Development, f.root.path(), f.root.path());
    fs::create_dir_all(&prod.cli_bridge_dir).unwrap();
    fs::write(prod.cli_bridge_dir.join(BINARY_NAME), b"production").unwrap();
    publish_cli_bridge(&f.source, &dev.cli_bridge_dir, VERSION, HASH).unwrap();
    assert_eq!(
        fs::read(prod.cli_bridge_dir.join(BINARY_NAME)).unwrap(),
        b"production"
    );
    assert_eq!(
        dev.cli_bridge_dir,
        f.root.path().join(".skills-hub-dev/bin")
    );
}

#[cfg(unix)]
#[test]
fn cli_bridge_requires_owner_execute_and_does_not_execute_binary_for_status() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new();
    f.publish().unwrap();
    let binary = f.destination.join(BINARY_NAME);
    assert_eq!(
        fs::metadata(&binary).unwrap().permissions().mode() & 0o777,
        0o700
    );
    // Fixture is not an executable format; a valid status must not spawn it.
    assert_eq!(
        cli_bridge_status(&f.destination, VERSION, HASH).status,
        CliBridgeHealth::Valid
    );
    fs::set_permissions(&binary, fs::Permissions::from_mode(0o600)).unwrap();
    assert_eq!(
        cli_bridge_status(&f.destination, VERSION, HASH).reason,
        Some(CliBridgeReason::NotExecutable)
    );
}

#[test]
fn cli_bridge_late_failure_removes_both_stamps() {
    let f = Fixture::new();
    fs::create_dir_all(f.destination.join(BINARY_NAME)).unwrap();
    assert!(f.publish().is_err());
    f.no_stamps();
}

#[test]
fn cli_bridge_startup_failure_is_recoverable_damaged_state_with_fixed_reason() {
    let f = Fixture::new();
    let missing = f.root.path().join("private-user-path/missing");
    let failed = publish_cli_bridge_on_startup(&missing, &f.destination, VERSION, HASH);
    assert_eq!(failed.0.status, CliBridgeHealth::Damaged);
    assert_eq!(failed.0.reason, Some(CliBridgeReason::SourceMissing));
    f.no_stamps();
    let recovered = publish_cli_bridge_on_startup(&f.source, &f.destination, VERSION, HASH);
    assert_eq!(recovered.0.status, CliBridgeHealth::Valid);
}

#[test]
fn cli_bridge_status_reports_in_progress_without_modifying_files() {
    let f = Fixture::new();
    f.publish().unwrap();
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .open(f.destination.join(LOCK_FILE))
        .unwrap();
    lock.lock_exclusive().unwrap();
    assert_eq!(
        cli_bridge_status(&f.destination, VERSION, HASH).reason,
        Some(CliBridgeReason::PublicationInProgress)
    );
    assert!(f.destination.join(VERSION_STAMP).exists());
    assert!(f
        .publish()
        .unwrap_err()
        .to_string()
        .contains("CLI_BRIDGE_PUBLICATION_IN_PROGRESS"));
    assert!(f.destination.join(VERSION_STAMP).exists());
}

#[cfg(unix)]
#[test]
fn cli_bridge_dev_directory_cannot_redirect_to_production() {
    let f = Fixture::new();
    let prod = f.root.path().join(".skills-hub");
    let dev = f.root.path().join(".skills-hub-dev");
    fs::create_dir_all(prod.join("bin")).unwrap();
    fs::write(prod.join("bin").join(BINARY_NAME), b"production").unwrap();
    std::os::unix::fs::symlink(&prod, &dev).unwrap();
    assert!(publish_cli_bridge(&f.source, &dev.join("bin"), VERSION, HASH).is_err());
    assert_eq!(
        fs::read(prod.join("bin").join(BINARY_NAME)).unwrap(),
        b"production"
    );
}

#[cfg(any(unix, windows))]
#[test]
fn cli_bridge_symlinked_dev_root_does_not_create_missing_production_bin() {
    let f = Fixture::new();
    let prod = f.root.path().join(".skills-hub");
    let dev = f.root.path().join(".skills-hub-dev");
    fs::create_dir(&prod).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&prod, &dev).unwrap();
    #[cfg(windows)]
    junction::create(&prod, &dev).unwrap();
    let state = publish_cli_bridge_on_startup(&f.source, &dev.join("bin"), VERSION, HASH);
    assert_eq!(state.0.status, CliBridgeHealth::Damaged);
    assert!(!prod.join("bin").exists());
    assert_eq!(fs::read_dir(&prod).unwrap().count(), 0);
}

#[cfg(unix)]
#[test]
fn cli_bridge_rejects_redirected_ancestor_above_the_bridge_root() {
    let f = Fixture::new();
    let prod = f.root.path().join("production");
    let redirected = f.root.path().join("redirected-home");
    fs::create_dir(&prod).unwrap();
    std::os::unix::fs::symlink(&prod, &redirected).unwrap();
    assert!(publish_cli_bridge(
        &f.source,
        &redirected.join(".skills-hub-dev/bin"),
        VERSION,
        HASH
    )
    .is_err());
    assert_eq!(fs::read_dir(prod).unwrap().count(), 0);
}

#[cfg(unix)]
#[test]
fn cli_bridge_pinned_directory_rejects_redirection_before_later_writes() {
    let f = Fixture::new();
    let directory = BridgeDirectory::open(&f.destination, true).unwrap();
    let original = f.root.path().join("original");
    let prod = f.root.path().join("production");
    fs::create_dir(&prod).unwrap();
    fs::rename(&f.destination, &original).unwrap();
    std::os::unix::fs::symlink(&prod, &f.destination).unwrap();
    assert!(directory.temp().is_err());
    assert!(directory
        .open_file(OsStr::new(LOCK_FILE), true, false)
        .is_err());
    assert_eq!(fs::read_dir(prod).unwrap().count(), 0);
    assert_eq!(fs::read_dir(original).unwrap().count(), 0);
}
