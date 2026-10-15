# Contributing

Thanks for taking the time to contribute to Skills Hub!

See [Development and builds](docs/development.en.md) for OAuth configuration, local test packages, and release build requirements. User documentation is maintained on the [Skills Hub website](https://aiskillshub.link/docs).

## Development Requirements

- Node.js 18+ (recommended: 20+)
- Rust (stable)
- Tauri system dependencies (install per the official Tauri docs for macOS/Windows/Linux)

## Run Locally

```bash
npm install
npm run tauri:dev
```

## Quality Checks

Run the complete gate on the final changes before committing:

```bash
npm run check
```

The exact sequence is maintained in `package.json`. Targeted checks help during development but do not replace the full gate. Report checks that could not run rather than treating them as passing.

Rust checks need a debug CLI sidecar and manifest matching the current commit. If `CLI_MANIFEST_REQUIRED` or a source mismatch is reported, prepare them with `npm run cli:prepare -- --target <host-target> --debug`, using the `host` value from `rustc -vV` (for example, `aarch64-apple-darwin`), then rerun the full gate. This prepares local build artifacts; it does not install or enable the CLI bridge.

## Run Unit Tests

Rust unit tests live under `src-tauri/src/core/tests/`.

```bash
cd src-tauri
cargo test
```

## Before Submitting a PR

- Ensure `npm run check` passes on the final changes
- Keep changes small and focused (do not commit local configs/caches/build artifacts)
- For UI changes, include screenshots or a short recording

For AI-assisted work, follow [the project rules](AGENTS.md) and [AI engineering workflow](docs/AI-ENGINEERING.md).

## Reporting Issues

Please include the following in your issue report:

- OS version (macOS/Windows/Linux)
- Skills Hub version
- Steps to reproduce and expected vs. actual behavior
- Relevant logs (please redact local paths and any sensitive information)
