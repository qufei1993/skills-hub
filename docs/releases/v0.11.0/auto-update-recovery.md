# Interrupted Skill update recovery

Fixes [#183](https://github.com/qufei1993/skills-hub/issues/183).

A terminated update worker could leave its persisted status as `running`, keeping Update now disabled after restarting the desktop. Windows task registration did not indicate worker liveness, and runtime polling bypassed the old macOS-only display correction.

Both configuration reads and runtime polling now reconcile interrupted updates through the shared cross-process operation lock. After a 60-second startup grace period, an unowned run becomes `stopped` in one database transaction. Completed results are preserved and the interrupted item returns to pending. A held lock prevents recovery even for long-running updates; lock errors do not imply that a worker has stopped. No scheduler process is launched by recurring runtime polling.

Manual triggers respect the same lock, release it before launching the worker, and settle failed starts without overwriting another active run. Existing settings and database schema remain compatible; no version bump or new credentials are involved.

Regression coverage terminates a real child process holding the update operation lock, verifies recovery after reopening the database and during subsequent reads, and verifies that the update button becomes available. Windows CI runs the automatic-update regression suite natively.
