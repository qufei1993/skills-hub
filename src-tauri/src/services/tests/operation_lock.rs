use tempfile::TempDir;

use crate::core::runtime_paths::{RuntimePaths, RuntimeProfile};
use crate::services::operation_lock::{OperationKind, OperationLock, OperationLockError};

#[test]
fn development_and_production_contend_for_the_same_write_lock() {
    let home = TempDir::new().unwrap();
    let data = TempDir::new().unwrap();
    let dev = RuntimePaths::from_roots(RuntimeProfile::Development, home.path(), data.path());
    let prod = RuntimePaths::from_roots(RuntimeProfile::Production, home.path(), data.path());
    let lock = OperationLock::acquire(&dev, OperationKind::Install).unwrap();
    assert!(matches!(
        OperationLock::acquire(&prod, OperationKind::Deploy),
        Err(OperationLockError::Busy(OperationKind::Deploy))
    ));
    drop(lock);
    OperationLock::acquire(&prod, OperationKind::Deploy).unwrap();
}

#[test]
fn rejects_a_second_writer_for_the_same_app_data_directory() {
    let home = TempDir::new().unwrap();
    let data = TempDir::new().unwrap();
    let paths = RuntimePaths::from_roots(RuntimeProfile::Test, home.path(), data.path());

    let first = OperationLock::acquire(&paths, OperationKind::Install).unwrap();
    let error = OperationLock::acquire(&paths, OperationKind::Deploy).unwrap_err();
    assert!(matches!(
        error,
        OperationLockError::Busy(OperationKind::Deploy)
    ));

    drop(first);
    OperationLock::acquire(&paths, OperationKind::Deploy).unwrap();
}
