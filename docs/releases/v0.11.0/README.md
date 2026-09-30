# Skills Hub v0.11.0

[中文](README.zh.md)

v0.11.0 lets you manage Skills through AI conversations using the official `manage-skills-hub` Skill and `skillshub-cli`. Enable AI management in Settings with one click. AI installations sync to detected, enabled tools by default, and desktop lists refresh on navigation and window focus. The CLI shares the desktop library and is bundled with the desktop, follows its version, and works after the desktop is closed. Installation and updates happen only through the explicit AI management enable action, never during ordinary startup. See [Agent-first CLI](agent-first-cli.md) and [verified desktop bridge](verified-cli-bridge.md).

## Incremental Git installation

Reinstalling a multi-Skill repository updates existing Skills from the same repository, branch selection, and subpath, while installing new Skills. Unchanged Skills retain their records. The desktop picker shows installation/update counts, uses green, blue, and amber badges for new Skills, update checks, and conflicts, and skips source or name conflicts; individual failures do not block the remaining selections.

Updates preserve Skill IDs, tags, enablement, and existing deployment configuration. Local edits and updates that would remove managed files are held back. Older records without content hashes are checked against their original Git revision; if that revision is no longer available in the cache, reinstallation is held back for manual review. Cross-source overwrite and interactive renaming are not included. Repository content follows the configured Git cache freshness policy.

## Cline desktop support

The existing Cline adapter now detects `~/.cline` and syncs global Skills to `~/.cline/skills` and project Skills to `.cline/skills`, matching [Cline’s documented directories](https://docs.cline.bot/customization/skills). Shared-directory previews no longer group Cline with `.agents/skills` tools. Existing files in `.agents/skills` are not moved or deleted; deploy the desired Skills to Cline again to populate its native directory.

## Release status

This is release preparation, not evidence of publication. The release workflow advertises exactly macOS arm64/x64, Windows x64, and Linux GNU arm64/x64, each on a matching native runner. This work was validated locally on macOS arm64; the other native jobs and configured signing/notarization must pass in CI before release. No release, signing, or notarization was performed during preparation.

The CLI is not published to npm. No npm scope, package ownership, token, or Trusted Publishing configuration is required.

## Publication prerequisites

- Protect the `release` GitHub environment and restrict it to approved release tags. The workflow only triggers on `v*` tags and checks the tag against every product version.
- Every release tag independently runs the five-platform native `verify` matrix at that tag's SHA, including CLI/compatibility tests and Windows native bridge tests. Build and publication jobs explicitly depend on this gate; previous PR/main CI results are not used as substitutes.
- macOS uses the existing imported signing identity and conditionally submits the desktop and CLI for notarization. Windows reuses desktop signing configuration when present and otherwise emits an explicit unsigned warning; updater signatures are not Windows Authenticode signatures.

Both macOS notarization responses must report `Accepted` before stapling and validating the app ticket and regenerating the updater archive and signature. Standalone Mach-O executables cannot be stapled; verify the CLI code signature and binary hash instead.

## Validation

The compatibility test starts with a frozen v0.10.1 shared-schema fixture, performs CLI install/tag/deploy, reopens the desktop service with identical runtime paths, and reads the result with v0.10.1-compatible SQL. It verifies schema 6 stays unchanged and unknown newer schemas fail closed without writes. Full frontend/Rust checks, native bundled CLI smoke, and desktop development startup are recorded in the task report.

References: [GitHub native runners](https://docs.github.com/en/actions/reference/runners/github-hosted-runners).

[Standalone CLI installation](cli-installation.md): one-command setup and upgrades without npm.
