use std::fmt;
use std::fs::{File, OpenOptions};
use std::io;

use fs2::FileExt;

use crate::core::runtime_paths::RuntimePaths;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OperationKind {
    Install,
    Update,
    Deploy,
    Undeploy,
    Delete,
    Restore,
    StorageMigration,
    AutoUpdate,
    DeviceSync,
}

impl fmt::Display for OperationKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Install => "install",
            Self::Update => "update",
            Self::Deploy => "deploy",
            Self::Undeploy => "undeploy",
            Self::Delete => "delete",
            Self::Restore => "restore",
            Self::StorageMigration => "storage-migration",
            Self::AutoUpdate => "auto-update",
            Self::DeviceSync => "device-sync",
        };
        formatter.write_str(value)
    }
}

#[derive(Debug)]
pub enum OperationLockError {
    Busy(OperationKind),
    Io(io::Error),
}

impl fmt::Display for OperationLockError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Busy(kind) => write!(formatter, "operation is already in progress: {kind}"),
            Self::Io(_) => formatter.write_str("failed to acquire operation lock"),
        }
    }
}

impl std::error::Error for OperationLockError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Busy(_) => None,
            Self::Io(error) => Some(error),
        }
    }
}

impl From<io::Error> for OperationLockError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

#[derive(Debug)]
pub struct OperationLock {
    file: File,
}

impl OperationLock {
    pub fn acquire(paths: &RuntimePaths, kind: OperationKind) -> Result<Self, OperationLockError> {
        std::fs::create_dir_all(&paths.app_data_dir)?;
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(paths.app_data_dir.join("operation.lock"))?;

        match file.try_lock_exclusive() {
            Ok(()) => Ok(Self { file }),
            Err(error)
                if error.kind() == io::ErrorKind::WouldBlock
                    || error.raw_os_error() == fs2::lock_contended_error().raw_os_error() =>
            {
                Err(OperationLockError::Busy(kind))
            }
            Err(error) => Err(OperationLockError::Io(error)),
        }
    }
}

impl Drop for OperationLock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.file);
    }
}
