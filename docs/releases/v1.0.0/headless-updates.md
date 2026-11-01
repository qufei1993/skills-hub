# Headless Skill updates

[简体中文](headless-updates.zh.md) · [#152](https://github.com/qufei1993/skills-hub/issues/152) · [PR #193](https://github.com/qufei1993/skills-hub/pull/193)

Scheduled updates and **Update now** keep using a separate worker process. The worker no longer creates a Skills Hub window, and macOS configures its prohibited activation policy before the event loop starts, avoiding the transient generic `exec` Dock icon.

Previously, Tauri created configured windows before the application's setup callback. That callback also changed the macOS activation policy only after launch, then synchronously performed the update. Suppressing window creation in the runtime context and setting the policy before `run` fixes the startup behavior without moving updates into the desktop process.

## Validation during implementation

- Reproduced on macOS using the development desktop executable with temporary test data. Temporary instrumentation redirected runtime paths, reported the Tauri window count, and held setup for four seconds for observation; it did not change window creation or activation policy.
- A native AppKit/CoreGraphics observer sampled the child PID every 10 ms. Before the fix: one created/on-screen window, and regular activation policy observed. After the fix: zero created/on-screen windows, only prohibited activation policy observed, and the empty-library update completed successfully.
- Regression tests cover background window suppression, unchanged normal-launch window configuration, and exact task argument recognition. The suppression test failed before the fix and passed afterward.
- `npm run check` passed: network boundary checks, lint, 377 frontend tests (one skipped), build, Rust format/Clippy, 672 Rust unit tests and 21 integration tests. Version consistency checks passed for that development build.
- `npm run tauri:dev` started on an isolated frontend port with temporary test paths and a separate app identifier; native inspection confirmed one visible normal window and regular activation policy. Temporary instrumentation was removed before submission. Independent read-only review found no blocking issues.
- Windows visual behavior still needs native Windows verification; macOS results do not constitute a Windows runtime test.

Tracks #152. This follows SanjiFlip's window-creation investigation in #155 while retaining independent background execution, as requested in review.
