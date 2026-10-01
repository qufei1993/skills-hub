# Release build reuse

[中文](build-performance.zh.md) · [Release overview](README.md)

The [previous candidate run](https://github.com/qufei1993/skills-hub/actions/runs/36714248531) spent 21.3 minutes recompiling the Mac Intel release CLI in `cli-build`, after `verify` had already compiled and smoke-tested it. Mac Intel desktop compilation took another 14.4 minutes. These are baseline observations, not a measured speedup for this change.

Native verification, manual native CI and regular Rust CI now set `CARGO_BUILD_TARGET` to their native target, so CLI preparation, Clippy and tests use the same target directory. Verification, manual native CI and desktop packaging use the same per-target release dependency cache key; manually dispatched native CI on main can seed default-branch release caches for tag runs. Regular Rust CI keeps a separate debug cache so an immutable debug-only cache cannot prevent saving release dependencies. Compiler and dependency identity remain part of the cache key. A cache miss still performs a full build. See [rust-cache inputs](https://github.com/Swatinem/rust-cache#example-usage).

Verification uploads only its release CLI and manifest as a same-run artifact. `cli-build` consumes the matching target artifact, validates commit/version/target/profile, byte size and SHA-256 before restoring executable permission, then performs the existing signing, notarization and smoke checks. It no longer compiles the CLI or installs frontend/native build dependencies. Linux signing jobs still install runtime WebKit, D-Bus and zlib packages before smoke tests; the previous ARM binary links GTK/WebKit libraries, so compilation reuse cannot remove this runtime prerequisite. The final manifest is regenerated from the signed bytes. Archive transport does not preserve executable permissions; see [upload-artifact permissions](https://github.com/actions/upload-artifact#permission-loss).

Desktop packaging downloads only the corresponding final manifest, rather than all five CLI binary sets. Publication still consumes the complete five-target CLI assets, and all native validation jobs must succeed before signing, desktop packaging or staging. The complete release remains a draft. This change does not trigger a tag, replace the existing draft assets, change installer content or weaken release checks.

Cache contents are immutable; see the [GitHub cache reference](https://docs.github.com/en/actions/reference/workflows-and-actions/dependency-caching).

## Validation

Regression coverage exercises the reused binary validation and the workflow handoff script for all five target fixtures on the local host, including tampering and permission restoration. Workflow tests protect platform coverage, shared target/cache configuration, signing order, matching manifest selection, and draft gates. `CARGO_BUILD_TARGET=aarch64-apple-darwin npm run check` passed: 300 frontend tests, 636 Rust unit tests, 2 compatibility tests and 19 CLI integration tests, plus network boundary, lint, build, formatting and Clippy checks. The standalone handoff/workflow suite passed 10 tests; version and whitespace checks passed. No new five-runner release or signing run was executed for this validation. Real cache hits and total build duration must be measured in the next authorized native/release run; no percentage or final duration is promised.
