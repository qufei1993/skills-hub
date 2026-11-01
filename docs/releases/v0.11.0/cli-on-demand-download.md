# CLI on-demand download validation

[中文](cli-on-demand-download.zh.md) · [Release overview](README.md)

## Current behavior

Production installers exclude the CLI. Enabling or updating AI management downloads the fixed desktop version and platform file, verifies embedded size and SHA-256 values, and installs through protected atomic replacement. Matching verified installations work offline. Startup and status reads do not update the CLI or official Skill. Local test packages embed a CLI; see [local test builds](local-test-builds.md).

CLI and desktop assets share the original repository release. CLI files enter a draft and undergo digest verification; desktop assets are added before the complete release is published and anonymous downloads are checked. Draft retries add only missing CLI files. Conflicting bytes are rejected, and public releases missing CLI assets are not modified.

## Completed validation (2026-09-30)

- PR #169 commit `ae4e272` passed the full local check: 290 frontend, 629 Rust unit, 2 compatibility and 19 CLI integration tests.
- The same commit's [CI](https://github.com/qufei1993/skills-hub/actions/runs/36709756936) passed web, Rust, Windows console and change detection checks. The five-platform native CLI check was skipped by configuration for this PR run.
- Workflow and version checks passed. Coverage includes failure preservation, preflight, progress/retry, immutable resources and package content verification.
- The macOS ARM size-validation package from the on-demand implementation was **10,728,379 bytes (10.23 MiB)**, compared with the prior 0.11.0 asset's **16,280,098 bytes (15.53 MiB)**: **34.1% smaller**, with no CLI in the app. Updater artifact signing was disabled for this check. It does not validate the latest release candidate, platform signing or notarization.
- Development process startup was confirmed during the on-demand work without visual confirmation of that window. The local test build's separate window validation is recorded in its own report.

## Remaining release gates

- Run five-platform native verification, platform signing/notarization and release asset integrity checks at the final candidate tag. Check real anonymous downloads after publication. PR CI does not replace these release gates.
- The existing v0.11.0 draft and tag still represent the previous bundled build and must not be published as this implementation. A release owner must choose the candidate version/tag.

The user-deferred missing central Skill directory/undeploy issue is unchanged.

Checkpoint results, build-tool changes, implementation decisions and CI troubleshooting remain in the [historical implementation record](cli-on-demand-download-history.md), rather than being appended to the current release conclusion.
