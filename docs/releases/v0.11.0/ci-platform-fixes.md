# Native CI fixes

Skill updates now resolve built-in tool detection and Kimi destinations against the runtime home, matching deployment. Previously the updater consulted the host home, so a clean Linux CI host skipped the fixture's Cursor copy. Regression coverage checks both an installed tool and a tool removed from the runtime home.

Windows MSVC builds embed the Common Controls v6 manifest in executables, including library test harnesses. The manifest matches Tauri's default, and the default resource manifest is disabled for this path to avoid duplicate embedding. This follows the [Tauri test manifest workaround](https://github.com/tauri-apps/tauri/blob/dev/examples/api/src-tauri/build.rs) for `STATUS_ENTRYPOINT_NOT_FOUND` before tests start.

Validation: the missing-runtime-tool regression failed before the fix and passes afterward. Full local checks and native CI are required before release.
