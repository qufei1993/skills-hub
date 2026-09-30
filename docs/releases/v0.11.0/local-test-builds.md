# Local installation test builds

Regular release packaging requires a matching published CLI manifest. It now explains that requirement and suggests `tauri:dev` or the current platform's local test build command. It never silently switches build modes.

`npm run tauri:build:local` and the macOS DMG, Windows MSI/NSIS, and Linux DEB/AppImage shortcuts compile one CLI for the selected target and embed its exact bytes in a release-optimized desktop executable. Enabling AI management verifies and publishes those bytes locally. The app no longer needs the checkout or a published CLI to exercise this path. Debug and local-release staging files are separate.

The app is named **Skills Hub Local Test**, uses identifier `com.qufei1993.skillshub.local-test` and executable `skills-hub-local-test`, and disables production updater endpoints and updater artifacts. The distinct executable name avoids Linux package file conflicts. Both CLI and desktop use development credential services and `.skills-hub-dev/bin`; database, Skill library, settings, caches, recycle bin and write locks remain shared. AI setup still affects real Agent directories and terminal PATH. The build layer rejects local-test feature builds without the distinct identity and updater configuration. CLI bootstrap retains its explicit preparation exception.

## Validation

- Script regression tests failed before implementation and passed afterward; cover platform hints, Tauri argument forwarding, invalid mode/manifest combinations, feature bypasses, exact target/profile and separate staging.
- Final `npm run check` passed: 287 frontend tests, 627 Rust unit tests, 2 compatibility tests and 19 CLI integration tests, with network boundaries, lint, frontend build, formatting and Clippy.
- Release `cargo test --release --target aarch64-apple-darwin --all-features --lib` passed 627 tests with the local test config and manifest. This includes installing and executing the actual embedded CLI in a temporary directory, development bridge selection, integrity failure preservation, credential separation and shared data identity.
- macOS ARM local DMG built successfully, 16,819,278 bytes. Mounted the actual DMG read-only, copied the app outside the checkout, verified its identifier/executable and embedded CLI bytes, then confirmed its window loaded at `tauri://localhost`. Development startup also succeeded. Validation apps and development processes were stopped.
- A transient existing file-lock test failure passed isolated recheck and the final full check; no unrelated lock implementation change was made.
- One independent read-only review identified executable-name collision, feature bypass and release-test namespace gaps. All were fixed and revalidated.
- Version check and diff whitespace checks passed; version remains 0.11.0.

Native Windows/Linux installer testing, macOS signing/notarization and the production CLI download flow were not exercised on this macOS host. Real AI setup was not clicked during manual startup verification; installation tests use temporary directories. Universal macOS and Windows ARM remain outside the currently supported CLI target set.
