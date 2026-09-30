# Verified desktop CLI bridge

[中文](verified-cli-bridge.zh.md) · [Release overview](README.md)

Production desktop installers contain the matching CLI manifest, not the CLI executable. Enabling or updating AI management downloads the fixed version and platform file from the same Skills Hub release. Embedded size and SHA-256 values are verified before installation. A verified matching bridge can be reused offline; startup and status reads do not download or update components.

Bridge installation uses a sibling temporary file and protected atomic replacement, followed by version and SHA-256 stamps. Failed replacement restores the previously verified CLI when possible. Status inspection only reads bridge files; it never executes the CLI or accesses credentials. Existing destination ancestors must pass symlink/Windows reparse-point checks before directories are created, and subsequent operations use validated directories.

Production uses `~/.skills-hub/bin`; development and local test builds use `~/.skills-hub-dev/bin`. The binary is `skillshub-cli` (`skillshub-cli.exe` on Windows). Stamps are `skillshub-cli.version` and `skillshub-cli.sha256`.

## Build modes

- Production packaging consumes a release CLI manifest matching the version, source commit, and target. CLI bytes are not bundled.
- Development startup prepares a local debug CLI and verifies its build profile and bytes.
- [Local installation test builds](local-test-builds.md) embed a local release CLI, use development credentials and bridge directories, and disable production updater endpoints. Shared Skill data and tool directories are still affected.

Custom Cargo profiles and conflicting `debug_assertions` are rejected. CLI preparation supports `darwin-arm64`, `darwin-x64`, `win32-x64`, `linux-x64`, `linux-arm64`, or the corresponding Rust triples. Toolchains must already be installed; macOS universal preparation is unsupported.

For local Rust checks on macOS arm64:

```sh
npm run cli:prepare -- --target darwin-arm64 --debug
npm run check
```

`tauri:dev` and `tauri:build:local` prepare their local CLI automatically. Regular `tauri:build` requires the matching release manifest; see the [build guide](../../../README.md#build).

## Release validation

The workflow signs and, when configured, notarizes the standalone CLI before generating its final-byte manifest. Production desktop builds embed that manifest and exclude the CLI executable. Package checks inspect macOS app contents and the actual Windows NSIS instructions; missing NSIS instructions or any CLI payload fail validation.

CLI and desktop assets enter the same original-repository release draft. Remote CLI sizes and digests must match before publication; anonymous fixed-version downloads are checked after the complete release is published. Conflicting version assets are never overwritten. CLI and desktop signing/notarization are validated independently. See [current validation and release gates](cli-on-demand-download.md).
