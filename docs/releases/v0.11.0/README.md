# Skills Hub v0.11.0

[中文](README.zh.md)

Installer links are generated on the [GitHub release page](https://github.com/qufei1993/skills-hub/releases/tag/v0.11.0).

v0.11.0 lets you manage Skills through AI conversations using the official `manage-skills-hub` Skill and `skillshub-cli`. Enable AI management in Settings with one click. AI installations sync to detected, enabled tools by default, and desktop lists refresh on navigation and window focus. The CLI shares the desktop library and follows its version, but is downloaded only when enabling or updating AI management. The desktop installer contains no CLI binary. First setup requires network access; a verified matching installation works offline and after the desktop is closed. Startup does not update the CLI or official Skill. See [Agent-first CLI](agent-first-cli.md) and [verified desktop bridge](verified-cli-bridge.md).

## Incremental Git installation

Reinstalling a multi-Skill repository updates existing Skills from the same repository, branch selection, and subpath, while installing new Skills. Unchanged Skills retain their records. The desktop picker shows installation/update counts, uses green, blue, and amber badges for new Skills, update checks, and conflicts, and skips source or name conflicts; individual failures do not block the remaining selections. Conflicting items have visibly disabled checkboxes, and descriptions longer than three lines can be expanded or collapsed.

Updates preserve Skill IDs, tags, enablement, and existing deployment configuration. Local edits and updates that would remove managed files are held back. Older records without content hashes are checked against their original Git revision; if that revision is no longer available in the cache, reinstallation is held back for manual review. Cross-source overwrite and interactive renaming are not included. Repository content follows the configured Git cache freshness policy.

## Cline desktop support

The existing Cline adapter now detects `~/.cline` and syncs global Skills to `~/.cline/skills` and project Skills to `.cline/skills`, matching [Cline’s documented directories](https://docs.cline.bot/customization/skills). Shared-directory previews no longer group Cline with `.agents/skills` tools. Existing files in `.agents/skills` are not moved or deleted; deploy the desired Skills to Cline again to populate its native directory.

## DeepSeek Harness custom home

Global Skill deployment, discovery, and installation detection use `$DSH_HOME/skills` and `$DSH_HOME` when a nonblank `DSH_HOME` is inherited by Skills Hub. Unset or blank values fall back to `~/.dsh`; `~`, `~/`, and `~\` expand to the current user's home. Project deployment continues to use `<project>/.dsh/skills`.

After changing the environment variable, restart Skills Hub (and its launcher if needed) so it inherits the new value. For Skills previously deployed to another directory, cancel their DeepSeek Harness sync and then sync them again. Cancellation uses the saved deployment path, even when the new home is absent, and retains modified copies instead of deleting them. Back up and resolve any reported conflicts before retrying. Startup does not move files or discard records; direct redeployment with a mismatched recorded path remains blocked. Explicit Harness `dshHome` configuration and `customSkillDirs` still require a custom tool in Skills Hub.

## Developer builds

[Local installation test builds](local-test-builds.md) embed a local CLI for testing before publication. They use a separate app identity, development credentials and CLI directory, and disable production updates. Skill data, configuration, caches, recycle bin and write lock remain shared with production; operations affect real tool directories. Production packaging still requires a matching final-byte release CLI manifest and excludes the executable.

## Release preparation and gates

This record does not imply publication. The previous bundled-build draft was replaced with the on-demand CLI candidate at tag v0.11.0. Keep the replacement release as a draft until the release owner explicitly approves publication.

- Protect the `release` GitHub environment and restrict it to approved tags. The workflow triggers on `v*` tags and verifies all product versions.
- Each tag independently runs the five-platform native CLI verification matrix at its SHA: macOS arm64/x64, Windows x64 and Linux GNU arm64/x64. Compatibility and Windows bridge tests are included; prior PR/main CI does not replace this gate. Desktop packaging covers macOS arm64/x64, Windows x64 and Linux GNU arm64/x64.
- Sign the CLI and complete configured notarization before generating its final-byte manifest. Desktop builds consume the matching manifests and exclude CLI binaries. macOS uses the configured signing identity; Windows signs when configured and otherwise reports unsigned artifacts. Updater signatures are separate from Windows Authenticode signatures.
- CLI and desktop notarization are independent and must each report `Accepted` when configured. Staple and validate the desktop ticket before regenerating the updater archive and signature. Standalone Mach-O CLI files cannot be stapled; verify their code signature and hash.
- Stage CLI binaries, checksums, manifests and desktop assets in the same original-repository release draft. Verify remote CLI sizes and digests before publication; check anonymous downloads afterward. Conflicting version assets are not overwritten. Use the original repository GITHUB_TOKEN with Contents write permission; no separate resource repository or npm publishing credentials are required.

The release workflow prepares a draft only; it does not publish automatically. After CLI and desktop asset upload and verification, wait for release-owner approval. Explicit approval is required before publication and anonymous download verification. Public downloads cannot be verified while the release is a draft.

Draft lookup falls back to the authenticated, paginated release list when GitHub's release-by-tag endpoint returns 404. This prevents creation of a duplicate draft; multiple releases with the same tag are rejected for manual reconciliation.

## Validation records

The compatibility test uses a frozen v0.10.1 shared-schema fixture, performs CLI install/tag/deploy, reopens the desktop service with the same runtime paths, and reads results with v0.10.1-compatible SQL. Shared schema 6 remains unchanged; unknown newer schemas reject writes.

- [Current on-demand CLI validation and remaining gates](cli-on-demand-download.md)
- [Local installation test build validation](local-test-builds.md)
- [Verified CLI bridge and build modes](verified-cli-bridge.md)
- [Standalone CLI installation](cli-installation.md)

- [AI management after device sync](ai-management-sync-hash.md)

- [Release build reuse](build-performance.md)

- [Linux desktop packaging](linux-desktop-packaging.md)
