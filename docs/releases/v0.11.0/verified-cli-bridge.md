# Verified desktop CLI bridge

[中文](verified-cli-bridge.zh.md) · [Release overview](README.md)

Desktop builds include the same-version native `skillshub-cli`. Normal startup verifies the bundled binary against embedded build metadata, invalidates old stamps, copies to a sibling temporary file, and atomically replaces the bridge binary before publishing SHA-256 and version stamps. Failures do not prevent desktop startup and retain a recoverable damaged status. Status inspection only reads bridge files; it never executes the CLI or accesses credentials.

Production publishes to `~/.skills-hub/bin`; debug builds publish to `~/.skills-hub-dev/bin`. The binary is `skillshub-cli` (`skillshub-cli.exe` on Windows), while stamps are always `skillshub-cli.version` and `skillshub-cli.sha256`.

Sidecar metadata includes its actual debug/release profile, verified from Cargo's compiler artifact. Desktop builds reject either direction of profile mismatch, including conflicting `debug_assertions`; custom Cargo profiles are unsupported. Bridge publication rejects symlinks or Windows reparse points in any existing destination ancestor before creating directories, and pins validated directories for subsequent file operations.

For local Rust checks, first prepare the host sidecar, for example:

```sh
npm run cli:prepare -- --target darwin-arm64 --debug
npm run check
```

Supported explicit targets: `darwin-arm64`, `darwin-x64`, `win32-x64`, `linux-x64`, `linux-arm64`, or their corresponding Rust triples. `tauri:dev` and `tauri:build` prepare the sidecar automatically. Cross-compilation toolchains must already be installed; the script does not download them. Universal macOS builds are not supported by this preparation step.

The release workflow signs the macOS CLI before recomputing its SHA-256 metadata and compiling the desktop. It supplies the already signed binary through `bundle.macOS.files` at `Contents/MacOS/skillshub-cli`, with `externalBin` disabled for that release invocation, to prevent a second signature changing the bytes. Final bundle checks require exactly one executable CLI, the same hash as the metadata, and valid app/CLI signatures when an identity is configured. Optional notarization submits both app and standalone CLI. Native signed release validation still requires the protected CI environment; no local signing or publication is implied.
