# Skills Hub v0.11.0

[中文](README.zh.md)

v0.11.0 lets you manage Skills through AI conversations using the official `manage-skills-hub` Skill and `skillshub-cli`. Enable AI management in Settings with one click. AI installations sync to detected, enabled tools by default, and desktop lists refresh on navigation and window focus. The CLI shares the desktop library and follows its version, but is downloaded only when enabling or updating AI management. The desktop installer contains no CLI binary. First setup requires network access; a verified matching installation works offline and after the desktop is closed. Startup does not update the CLI or official Skill. See [Agent-first CLI](agent-first-cli.md) and [verified desktop bridge](verified-cli-bridge.md).

## Incremental Git installation

Reinstalling a multi-Skill repository updates existing Skills from the same repository, branch selection, and subpath, while installing new Skills. Unchanged Skills retain their records. The desktop picker shows installation/update counts, uses green, blue, and amber badges for new Skills, update checks, and conflicts, and skips source or name conflicts; individual failures do not block the remaining selections. Conflicting items have visibly disabled checkboxes, and descriptions longer than three lines can be expanded or collapsed.

Updates preserve Skill IDs, tags, enablement, and existing deployment configuration. Local edits and updates that would remove managed files are held back. Older records without content hashes are checked against their original Git revision; if that revision is no longer available in the cache, reinstallation is held back for manual review. Cross-source overwrite and interactive renaming are not included. Repository content follows the configured Git cache freshness policy.

## Cline desktop support

The existing Cline adapter now detects `~/.cline` and syncs global Skills to `~/.cline/skills` and project Skills to `.cline/skills`, matching [Cline’s documented directories](https://docs.cline.bot/customization/skills). Shared-directory previews no longer group Cline with `.agents/skills` tools. Existing files in `.agents/skills` are not moved or deleted; deploy the desired Skills to Cline again to populate its native directory.

## DeepSeek Harness custom home

Global Skill deployment, discovery, and installation detection use `$DSH_HOME/skills` and `$DSH_HOME` when a nonblank `DSH_HOME` is inherited by Skills Hub. Unset or blank values fall back to `~/.dsh`; `~`, `~/`, and `~\` expand to the current user's home. Project deployment continues to use `<project>/.dsh/skills`.

After changing the environment variable, restart Skills Hub (and its launcher if needed) so it inherits the new value. For Skills previously deployed to another directory, cancel their DeepSeek Harness sync and then sync them again. Cancellation uses the saved deployment path, even when the new home is absent, and retains modified copies instead of deleting them. Back up and resolve any reported conflicts before retrying. Startup does not move files or discard records; direct redeployment with a mismatched recorded path remains blocked. Explicit Harness `dshHome` configuration and `customSkillDirs` still require a custom tool in Skills Hub.

## Release status

This is release preparation, not evidence of publication. The release workflow builds five CLI targets (macOS arm64/x64, Windows x64, Linux GNU arm64/x64) and three desktop targets (macOS arm64/x64, Windows x64), each on a matching native runner. This work was validated locally on macOS arm64; the other native jobs and configured signing/notarization must pass in CI before release. No release, signing, or notarization was performed during preparation.

The CLI is not published to npm. No npm scope, package ownership, token, or Trusted Publishing configuration is required.

## Publication prerequisites

- Protect the `release` GitHub environment and restrict it to approved release tags. The workflow only triggers on `v*` tags and checks the tag against every product version.
- Every release tag independently runs the five-platform native `verify` matrix at that tag's SHA, including CLI/compatibility tests and Windows native bridge tests. Build and publication jobs explicitly depend on this gate; previous PR/main CI results are not used as substitutes.
- macOS uses the existing imported signing identity and conditionally submits the desktop and CLI for notarization. Windows reuses desktop signing configuration when present and otherwise emits an explicit unsigned warning; updater signatures are not Windows Authenticode signatures.

CLI and desktop notarization run independently and each must report `Accepted`. The desktop ticket is stapled and validated before regenerating the updater archive and signature. Standalone Mach-O executables cannot be stapled; verify the CLI code signature and binary hash instead.

## Validation

The compatibility test starts with a frozen v0.10.1 shared-schema fixture, performs CLI install/tag/deploy, reopens the desktop service with identical runtime paths, and reads the result with v0.10.1-compatible SQL. It verifies schema 6 stays unchanged and unknown newer schemas fail closed without writes. Full frontend/Rust checks, native CLI smoke, and desktop development startup are recorded in the task report.

References: [GitHub native runners](https://docs.github.com/en/actions/reference/runners/github-hosted-runners).

[Standalone CLI installation](cli-installation.md): one-command setup and upgrades without npm.

## CLI resource publication

Five CLI targets publish binary, SHA-256, and final-byte manifest files to the public `qufei1993/skills-hub-cli` repository. The desktop builds embed matching manifests and exclude CLI binaries; desktop publication waits for successful anonymous fixed-version downloads. `GH_RELEASE_TOKEN` must have Contents read/write access to the resource repository and is supplied only through Actions secrets. Existing version assets must match exactly; the pipeline never overwrites a conflicting version.

The existing v0.11.0 desktop draft and tag refer to the previous bundled build. These changes do not publish that draft or rewrite the tag. A release owner must choose the replacement candidate/tag before shipping this implementation.

[On-demand CLI validation](cli-on-demand-download.md)
