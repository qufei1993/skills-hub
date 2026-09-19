use tempfile::TempDir;

use crate::core::runtime_paths::{RuntimePaths, RuntimeProfile};
use crate::services::operation_lock::{OperationKind, OperationLock, OperationLockError};

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
