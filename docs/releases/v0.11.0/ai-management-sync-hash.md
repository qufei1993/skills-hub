# AI management after device sync

[中文](ai-management-sync-hash.zh.md) · [Release overview](README.md)

Device sync previously stored the portable manifest checksum in the local Skill record. The two checksums use different inputs, so an unchanged official Skill could be reported as modified, blocking AI management preflight before CLI installation. The displayed Skill version did not establish file integrity.

Device sync now stores the staged local directory checksum. Explicit AI management enablement and official Skill updates recognize a legacy record only when its portable checksum matches the current document and metadata. Recovery requires the official bundled source, a regular directory containing only a regular `SKILL.md`, and ordinary document permissions. Edited content, extra entries, links, unknown checksums and unusual file permissions remain blocked. Existing IDs, creation time, tags and deployment scope are preserved.

Status reads and preflight do not rewrite the record. Successful enablement or update commits the corrected local checksum under the existing write lock and replacement protections. No database schema change is required. Startup does not install or upgrade AI management.

CLI size and checksum verification remain enforced. A previously installed development or older build may differ even with the same version number; Enable installs the exact CLI expected by this app. Status messages now direct users to this action. Public CLI download URLs for a draft release are unavailable; draft testing requires the supported local test build. Do not publish the release merely to test this repair.

## Validation

Regression tests reproduce the original sync checksum mismatch before the fix. Coverage includes sync followed by enablement, safe legacy recovery without preflight writes, upgrading an older official bundle, preserving tags and schema compatibility, rejection of real edits and unsafe file shapes, deployment failure rollback, and CLI download failure without Skill mutation. The final `npm run check` passed: 290 frontend tests, 636 Rust unit tests, 2 compatibility tests and 19 CLI integration tests, plus network boundary, lint, build, formatting and Clippy checks. `npm run tauri:dev` built and started the repair checkout successfully. Native window inspection selected the existing production app, so it does not establish visual validation of the development window; no test enablement was performed against the real library.

The previous v0.11.0 draft installers do not include this fix. Rebuild from the approved repair commit before completing release validation. Keep the release as a draft until the release owner approves publication.
