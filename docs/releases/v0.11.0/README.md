# Skills Hub v0.11.0

[中文](README.zh.md)

v0.11.0 lets you manage Skills through AI conversations using the official `manage-skills-hub` Skill and `skillshub-cli`. Enable AI management in Settings with one click. AI installations sync to detected, enabled tools by default, and desktop lists refresh on navigation and window focus. The CLI shares the desktop library and works without the desktop running or installed. See [Agent-first CLI](agent-first-cli.md) and [verified desktop bridge](verified-cli-bridge.md).

## Release status

This is release preparation, not evidence of publication. The release workflow advertises exactly macOS arm64/x64, Windows x64, and Linux GNU arm64/x64, each on a matching native runner. This work was validated locally on macOS arm64; the other native jobs and configured signing/notarization must pass in CI before release. No release, registry publication, signing, or notarization was performed during preparation.

## Publication prerequisites

- Protect the `release` GitHub environment and restrict it to approved release tags. The workflow only triggers on `v*` tags and checks the tag against every product version.
- Protect/register the `skillshub-app` npm scope and configure Trusted Publishing for all six packages, this repository's `release.yml`, and the `release` environment. Use npm account 2FA; no long-lived npm token is supported by this workflow. Initial package/trust setup is an external prerequisite, not performed here.
- All five platform builds and native offline npm smoke tests must succeed. The five platform tarballs publish first using OIDC/provenance; only successful completion permits publishing `skillshub-cli`. A partial platform publication cannot publish the main package; release operators must resolve a failed publication before retrying immutable versions.
- Every release tag independently runs the five-platform native `verify` matrix at that tag's SHA, including CLI/compatibility tests and Windows native bridge tests. Build and publication jobs explicitly depend on this gate; previous PR/main CI results are not used as substitutes.
- macOS uses the existing imported signing identity and conditionally submits the desktop and CLI for notarization. Windows reuses desktop signing configuration when present and otherwise emits an explicit unsigned warning; updater signatures are not Windows Authenticode signatures.

Both macOS notarization responses must report `Accepted` before stapling and validating the app ticket and regenerating the updater archive and signature. Standalone Mach-O executables cannot be stapled; verify the CLI code signature and binary hash instead.

## Validation

The compatibility test starts with a frozen v0.10.1 shared-schema fixture, performs CLI install/tag/deploy, reopens the desktop service with identical runtime paths, and reads the result with v0.10.1-compatible SQL. It verifies schema 6 stays unchanged and unknown newer schemas fail closed without writes. Full frontend/Rust checks, host npm tarball smoke, and desktop development startup are recorded in the task report.

References: [GitHub native runners](https://docs.github.com/en/actions/reference/runners/github-hosted-runners), [npm Trusted Publishing](https://docs.npmjs.com/trusted-publishers/).
