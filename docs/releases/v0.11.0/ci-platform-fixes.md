# Native CI fixes

Skill updates now resolve built-in tool detection and Kimi destinations against the runtime home, matching deployment. Previously the updater consulted the host home, so a clean Linux CI host skipped the fixture's Cursor copy. Regression coverage checks both an installed tool and a tool removed from the runtime home.

Windows MSVC builds embed the Common Controls v6 manifest in executables, including library test harnesses. The manifest matches Tauri's default, and the default resource manifest is disabled for this path to avoid duplicate embedding. This follows the [Tauri test manifest workaround](https://github.com/tauri-apps/tauri/blob/dev/examples/api/src-tauri/build.rs) for `STATUS_ENTRYPOINT_NOT_FOUND` before tests start.

Validation: the missing-runtime-tool regression failed before the fix and passes afterward. Full local checks and native CI are required before release.

Windows native diagnostics confirmed that `MoveFileExW` and the Win32 path-based rename reopen the guarded parent and fail with sharing violation 32. `NtSetInformationFile` with a NULL root and a single filename succeeds under the existing read-only sharing guard. Temporary files retain DELETE access and allow read sharing only, preventing external writes or moves before publication. Rename and failed-operation cleanup both use the existing file handle; no ancestor sharing restriction is relaxed. Regression checks cover creation, replacement, locked-target preservation, cleanup, and invalid destination names. A lightweight Windows probe uses the production rename helper, while native CI runs the full bridge test group.

## CI scope and local iteration

Run focused tests locally while developing, then `npm run check` before committing. Automatic CI keeps the web checks and runs Linux Rust checks plus lightweight Windows probes only when the changed paths require native validation. Frontend and documentation-only diffs skip native checks; dependencies, bundled Skills, build scripts, workflow files and unknown paths require them. PR comparisons cover the complete diff from the base merge point, while main pushes compare the pushed range. Missing comparison history fails the detection job instead of silently skipping validation.

The five-platform debug/release matrix is manual through the CI workflow's Run workflow action. Normal PR updates and main pushes no longer trigger that matrix. Restore the Rust cache before preparing the debug sidecar. The release workflow retains its independent five-platform verification gate before publication.
