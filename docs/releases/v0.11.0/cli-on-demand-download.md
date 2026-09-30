# CLI on-demand download validation

The desktop installer excludes CLI executables. Settings downloads the fixed desktop version only when enabling or updating AI management, verifies the embedded expected size and SHA-256, then uses the existing protected atomic bridge publisher. Startup and status reads do not update the CLI or official Skill. Existing valid matching CLI installations are reused offline.

## Local evidence (2026-09-30)

- `npm run check` and `npm run version:check` passed; download failure, preflight, UI progress/retry, immutable resource publication and actual bundle checks are covered.
- Release/CI workflows passed actionlint 1.7.12.
- Development process started successfully; GUI inventory could not identify the development window, so visual startup inspection is not claimed.
- macOS arm64 DMG: **10,728,379 bytes (10.23 MiB)** compared with the previous v0.11.0 asset's **16,280,098 bytes (15.53 MiB)**: **34.1% smaller**. The actual app bundle has only the desktop executable and no CLI.
- Local packaging disabled updater artifact signing for this size check. This is a development verification artifact, not a published release or evidence of platform signing/notarization.

## Build constraints

CLI targets require the non-default Cargo `cli` feature; `npm run cli:prepare` and full Rust tests enable it. Desktop builds do not enable it. Tauri CLI 2.12.0 is required because the previously locked 2.9.6 reintroduced disabled secondary binaries during discovery. Generated bundle staging is cleaned before packaging and inspected afterward.

## Remote state and remaining release gates

The public [CLI resource repository](https://github.com/qufei1993/skills-hub-cli) has been created, but no CLI release was published. The existing desktop v0.11.0 release remains a draft and its tag is unchanged.

Five-platform native verification, Windows PowerShell tests, configured platform signing/notarization, the resource token's cross-repository permissions, and anonymous production CLI downloads must pass in CI before desktop publication. This local implementation does not substitute for those gates. A release owner must choose the candidate/tag that contains these changes before publication; the old draft must not be published as this implementation.

## Final review

The Windows packaging gate now requires and checks the actual sibling `release/nsis/x64/installer.nsi`; missing instructions or CLI payloads fail validation. Default builds explicitly pass their resolved target so staging cleanup and output agree. Both READMEs document the required matching release manifest. Regression tests failed before these fixes and passed afterward. Final full check passed: 282 frontend, 623 Rust unit, 2 compatibility and 19 CLI integration tests; workflow lint and version checks also passed. One final fix pass was performed, without a second independent review. No minor review findings remain deferred.

The complete implementation decision ledger and tradeoffs are recorded in [the Chinese validation record](cli-on-demand-download.zh.md#最终审查与实现决定). The user-deferred missing central Skill directory issue is unchanged. Native CI, signing, notarization, public download and candidate-tag decisions remain release prerequisites.
