# Development and builds

[Project overview](../README.md) · [简体中文](development.zh.md)

User documentation is maintained on the [Skills Hub website](https://aiskillshub.link/docs). This document covers building and contributing to the desktop project.

## Prerequisites

- Node.js 18+ (recommended: 20+)
- Rust (stable)
- Tauri system dependencies (follow Tauri official docs for your OS)

Create the local OAuth configuration once before starting the desktop app or building an installer:

```bash
cp .env.example .env
```

Replace both placeholders with the team's GitHub and GitLab OAuth public Client IDs. `npm run tauri:dev` and local `npm run tauri:build*` commands load only these two allowlisted values from the root `.env`; each value already present in the process environment takes precedence. Both IDs are required so browser authorization is available for both providers. Never add a client secret or user token, and do not commit `.env`.

```bash
npm install
npm run tauri:dev
```

### Isolated desktop acceptance testing

Normal development startup shares application data with production. For desktop tests that write skills or settings, start a debug build with an empty temporary directory:

```bash
skills_hub_desktop_test_root="$(mktemp -d)"
SKILLSHUB_DESKTOP_TEST_ROOT="$skills_hub_desktop_test_root" npm run tauri:dev
```

The directory must already exist and use an absolute path; invalid values stop startup. This debug-only override selects the test runtime profile and places the database, default central library, Git cache, recycle bin, and CLI bridge directory under its `home/` and `data/` subdirectories. It does not import production or legacy data. Release builds, including local test packages, do not read this variable. Without the variable, startup behavior is unchanged.

Use explicit source, project, and custom tool paths within the temporary directory for write tests. This override does not change the operating system home directory, built-in tool discovery, system credentials, application logs, or external integration targets; do not use it as isolation for authorization, AI management enablement, or terminal PATH changes. Keep the temporary directory while reviewing results, then remove only that test directory after stopping its application process.

## Build

For local installation testing, build on the target operating system:

```bash
# Automatically selects the host platform and architecture.
npm run tauri:build:local
# Or select an installer format.
npm run tauri:build:local:mac:dmg
npm run tauri:build:local:win:exe
npm run tauri:build:local:win:msi
npm run tauri:build:local:win:all
npm run tauri:build:local:linux:deb
npm run tauri:build:local:linux:appimage
npm run tauri:build:local:linux:all
```

Local test packaging compiles one CLI for the selected target (host by default, or an explicit supported `--target`). It embeds those exact bytes in the release-optimized desktop executable. The installed **Skills Hub Local Test** app verifies and installs that local CLI when you enable AI management; it does not download the CLI or depend on the source checkout. App identity and installer name are distinct, production updater endpoints and updater artifacts are disabled, and both desktop and CLI use the development credential and CLI bridge namespaces. Skill data, settings, caches, recycle bin and write locks are still shared with production; enabling AI management can change real Agent directories and terminal PATH. Do not combine this mode with `--cli-manifest`, `SKILLS_HUB_CLI_MANIFEST_PATH`, `--dev`, or debug flags. Universal macOS and Windows ARM targets are not currently supported; build the supported native target instead.

For regular release packaging, provide the manifest from the published CLI for the same version, source commit and target:

```bash
# Example: macOS ARM, with the official release manifest downloaded separately.
npm run tauri:build:mac:dmg -- --target aarch64-apple-darwin --cli-manifest /path/to/cli-manifest-aarch64-apple-darwin.json
```

All platform commands below require `-- --cli-manifest <path>`, or `SKILLS_HUB_CLI_MANIFEST_PATH`. A locally generated manifest may differ from the signed published CLI and fail download verification; use the local test mode for unpublished changes. Missing manifests produce platform-specific guidance and never silently switch to a test build. `npm run tauri:dev` continues to prepare a local debug CLI automatically. Local test packages cover installation and local CLI integration; verify the official CLI download, checksum and installation flow separately in a release package. Both build modes require the public OAuth Client IDs described above.

### Platform build commands (from `package.json`)

- macOS (dmg): `npm run tauri:build:mac:dmg`
- macOS (universal dmg): `npm run tauri:build:mac:universal:dmg`
- Windows (MSI): `npm run tauri:build:win:msi`
- Windows (NSIS exe): `npm run tauri:build:win:exe`
- Windows (MSI+NSIS): `npm run tauri:build:win:all`
- Linux (deb): `npm run tauri:build:linux:deb`
- Linux (AppImage): `npm run tauri:build:linux:appimage`
- Linux (deb+AppImage): `npm run tauri:build:linux:all`

## Tests (Rust)

```bash
cd src-tauri
cargo test
```

