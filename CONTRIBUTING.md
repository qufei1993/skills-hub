# Contributing

Thanks for taking the time to contribute to SkillSyncer!

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

```bash
npm run lint
npm run build
```

## Run Unit Tests

Rust unit tests live under `src-tauri/src/core/tests/`.

```bash
cd src-tauri
cargo test
```

## Before Submitting a PR

- Ensure `npm run lint` and `npm run build` pass
- Ensure `cd src-tauri && cargo test` pass
- Keep changes small and focused (do not commit local configs/caches/build artifacts)
- For UI changes, include screenshots or a short recording

## Reporting Issues

Please include the following in your issue report:

- OS version (macOS/Windows/Linux)
- SkillSyncer version
- Steps to reproduce and expected vs. actual behavior
- Relevant logs (please redact local paths and any sensitive information)
