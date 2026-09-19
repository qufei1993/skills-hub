use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use fs2::FileExt;
use serde::Serialize;
use sha2::{Digest, Sha256};

pub const BINARY_NAME: &str = if cfg!(windows) {
    "skillshub-cli.exe"
} else {
    "skillshub-cli"
};
pub const VERSION_STAMP: &str = "skillshub-cli.version";
pub const HASH_STAMP: &str = "skillshub-cli.sha256";
const LOCK_FILE: &str = ".skillshub-cli.lock";

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CliBridgeHealth {
    Missing,
    Valid,
    Damaged,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CliBridgeReason {
    DirectoryMissing,
    BinaryMissing,
    StampMissing,
    VersionMismatch,
    HashMismatch,
    NotExecutable,
    InvalidMetadata,
    SourceMissing,
    IoError,
    PublicationInProgress,
}

impl CliBridgeReason {
    pub const fn code(self) -> &'static str {
        match self {
            Self::DirectoryMissing => "CLI_BRIDGE_DIRECTORY_MISSING",
            Self::BinaryMissing => "CLI_BRIDGE_BINARY_MISSING",
            Self::StampMissing => "CLI_BRIDGE_STAMP_MISSING",
            Self::VersionMismatch => "CLI_BRIDGE_VERSION_MISMATCH",
            Self::HashMismatch => "CLI_BRIDGE_HASH_MISMATCH",
            Self::NotExecutable => "CLI_BRIDGE_NOT_EXECUTABLE",
            Self::InvalidMetadata => "CLI_BRIDGE_INVALID_METADATA",
            Self::SourceMissing => "CLI_BRIDGE_SOURCE_MISSING",
            Self::IoError => "CLI_BRIDGE_IO_ERROR",
            Self::PublicationInProgress => "CLI_BRIDGE_PUBLICATION_IN_PROGRESS",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CliBridgeStatus {
    pub status: CliBridgeHealth,
    pub reason: Option<CliBridgeReason>,
    pub path: PathBuf,
    pub version: Option<String>,
}

/// Retained even when startup publication fails before a bridge directory exists.
#[derive(Clone, Debug)]
pub struct CliBridgeStartupState(pub CliBridgeStatus);

pub fn publish_cli_bridge_on_startup(
    source: &Path,
    destination: &Path,
    version: &str,
    expected_hash: &str,
) -> CliBridgeStartupState {
    match publish_cli_bridge(source, destination, version, expected_hash) {
        Ok(status) => CliBridgeStartupState(status),
        Err(error) => {
            let reason = [
                CliBridgeReason::HashMismatch,
                CliBridgeReason::InvalidMetadata,
                CliBridgeReason::SourceMissing,
                CliBridgeReason::PublicationInProgress,
            ]
            .into_iter()
            .find(|reason| error.to_string() == reason.code())
            .unwrap_or(CliBridgeReason::IoError);
            log::warn!("CLI bridge publication failed: {}", reason.code());
            CliBridgeStartupState(CliBridgeStatus::damaged(destination, reason))
        }
    }
}

pub fn bundled_cli_bridge_status(destination: &Path) -> CliBridgeStatus {
    cli_bridge_status(
        destination,
        env!("CARGO_PKG_VERSION"),
        option_env!("SKILLS_HUB_BUNDLED_CLI_SHA256").unwrap_or(""),
    )
}

pub fn publish_bundled_cli_bridge(source: &Path, destination: &Path) -> CliBridgeStartupState {
    publish_cli_bridge_on_startup(
        source,
        destination,
        env!("CARGO_PKG_VERSION"),
        option_env!("SKILLS_HUB_BUNDLED_CLI_SHA256").unwrap_or(""),
    )
}

impl CliBridgeStatus {
    pub fn damaged(destination: &Path, reason: CliBridgeReason) -> Self {
        Self {
            status: CliBridgeHealth::Damaged,
            reason: Some(reason),
            path: destination.join(BINARY_NAME),
            version: None,
        }
    }
}

fn io_error(_: impl std::fmt::Display) -> anyhow::Error {
    anyhow::anyhow!(CliBridgeReason::IoError.code())
}

fn regular_file(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|metadata| metadata.is_file())
        .unwrap_or(false)
}

fn metadata_valid(version: &str, hash: &str) -> bool {
    !version.is_empty()
        && version.len() <= 128
        && version
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b".-+".contains(&byte))
        && version.as_bytes()[0].is_ascii_digit()
        && hash_valid(hash)
}

fn hash_valid(hash: &str) -> bool {
    hash.len() == 64
        && hash
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn hash_file(path: &Path) -> Result<String> {
    let mut file = File::open(path).map_err(io_error)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0; 64 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(io_error)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn invalidate_stamps(destination: &Path) -> Result<()> {
    let mut failed = false;
    for name in [VERSION_STAMP, HASH_STAMP] {
        if let Err(error) = fs::remove_file(destination.join(name)) {
            failed |= error.kind() != std::io::ErrorKind::NotFound;
        }
    }
    if failed {
        bail!(CliBridgeReason::IoError.code());
    }
    Ok(())
}

fn atomic_stamp(destination: &Path, name: &str, value: &str) -> Result<()> {
    let mut temp = tempfile::Builder::new()
        .prefix(".skillshub-cli-")
        .suffix(".tmp")
        .tempfile_in(destination)
        .map_err(io_error)?;
    writeln!(temp, "{value}").map_err(io_error)?;
    temp.as_file().sync_all().map_err(io_error)?;
    temp.persist(destination.join(name)).map_err(io_error)?;
    Ok(())
}

fn real_bridge_directory(destination: &Path) -> bool {
    [Some(destination), destination.parent()]
        .into_iter()
        .flatten()
        .all(|path| {
            fs::symlink_metadata(path)
                .map(|metadata| metadata.is_dir())
                .unwrap_or(false)
        })
}

pub fn publish_cli_bridge(
    source: &Path,
    destination: &Path,
    version: &str,
    expected_hash: &str,
) -> Result<CliBridgeStatus> {
    fs::create_dir_all(destination).map_err(io_error)?;
    if !real_bridge_directory(destination) {
        bail!(CliBridgeReason::IoError.code());
    }
    let lock_path = destination.join(LOCK_FILE);
    if lock_path.exists() && !regular_file(&lock_path) {
        bail!(CliBridgeReason::IoError.code());
    }
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(lock_path)
        .map_err(io_error)?;
    lock.try_lock_exclusive()
        .map_err(|_| anyhow::anyhow!(CliBridgeReason::PublicationInProgress.code()))?;
    invalidate_stamps(destination)?;
    let result = (|| {
        if !hash_valid(expected_hash) {
            bail!(CliBridgeReason::HashMismatch.code());
        }
        if !metadata_valid(version, expected_hash) {
            bail!(CliBridgeReason::InvalidMetadata.code());
        }
        if !regular_file(source) {
            bail!(CliBridgeReason::SourceMissing.code());
        }
        if hash_file(source)? != expected_hash {
            bail!(CliBridgeReason::HashMismatch.code());
        }
        let mut temp = tempfile::Builder::new()
            .prefix(".skillshub-cli-")
            .suffix(".tmp")
            .tempfile_in(destination)
            .map_err(io_error)?;
        let mut input = File::open(source).map_err(io_error)?;
        std::io::copy(&mut input, &mut temp).map_err(io_error)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            temp.as_file()
                .set_permissions(fs::Permissions::from_mode(0o700))
                .map_err(io_error)?;
        }
        temp.as_file().sync_all().map_err(io_error)?;
        if hash_file(temp.path())? != expected_hash {
            bail!(CliBridgeReason::HashMismatch.code());
        }
        temp.persist(destination.join(BINARY_NAME))
            .map_err(io_error)?;
        atomic_stamp(destination, HASH_STAMP, expected_hash)?;
        atomic_stamp(destination, VERSION_STAMP, version)?;
        Ok(CliBridgeStatus {
            status: CliBridgeHealth::Valid,
            reason: None,
            path: destination.join(BINARY_NAME),
            version: Some(version.to_string()),
        })
    })();
    if result.is_err() {
        let _ = invalidate_stamps(destination);
    }
    result
}

/// Read-only verification; never spawns the CLI or opens application credentials.
pub fn cli_bridge_status(
    destination: &Path,
    version: &str,
    expected_hash: &str,
) -> CliBridgeStatus {
    let damaged = |reason| CliBridgeStatus::damaged(destination, reason);
    match fs::symlink_metadata(destination) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return CliBridgeStatus {
                status: CliBridgeHealth::Missing,
                reason: Some(CliBridgeReason::DirectoryMissing),
                path: destination.join(BINARY_NAME),
                version: None,
            }
        }
        Err(_) => return damaged(CliBridgeReason::IoError),
        Ok(_) => {}
    }
    if !real_bridge_directory(destination) {
        return damaged(CliBridgeReason::IoError);
    }
    let lock = match File::open(destination.join(LOCK_FILE)) {
        Ok(file) => {
            if FileExt::try_lock_shared(&file).is_err() {
                return damaged(CliBridgeReason::PublicationInProgress);
            }
            Some(file)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(_) => return damaged(CliBridgeReason::IoError),
    };
    let _lock = lock;
    if !metadata_valid(version, expected_hash) {
        return damaged(CliBridgeReason::InvalidMetadata);
    }
    let binary = destination.join(BINARY_NAME);
    if !regular_file(&binary) {
        return damaged(CliBridgeReason::BinaryMissing);
    }
    if ![VERSION_STAMP, HASH_STAMP]
        .iter()
        .all(|name| regular_file(&destination.join(name)))
    {
        return damaged(CliBridgeReason::StampMissing);
    }
    let stamp_version = match fs::read_to_string(destination.join(VERSION_STAMP)) {
        Ok(value) => value,
        Err(_) => return damaged(CliBridgeReason::IoError),
    };
    if stamp_version.trim() != version {
        return damaged(CliBridgeReason::VersionMismatch);
    }
    let stamp_hash = match fs::read_to_string(destination.join(HASH_STAMP)) {
        Ok(value) => value,
        Err(_) => return damaged(CliBridgeReason::IoError),
    };
    if stamp_hash.trim() != expected_hash {
        return damaged(CliBridgeReason::HashMismatch);
    }
    match hash_file(&binary) {
        Ok(hash) if hash == expected_hash => {}
        Ok(_) => return damaged(CliBridgeReason::HashMismatch),
        Err(_) => return damaged(CliBridgeReason::IoError),
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if fs::metadata(&binary)
            .map(|m| m.permissions().mode() & 0o100 == 0)
            .unwrap_or(true)
        {
            return damaged(CliBridgeReason::NotExecutable);
        }
    }
    CliBridgeStatus {
        status: CliBridgeHealth::Valid,
        reason: None,
        path: binary,
        version: Some(version.to_string()),
    }
}

#[cfg(test)]
#[path = "tests/cli_bridge.rs"]
mod tests;
