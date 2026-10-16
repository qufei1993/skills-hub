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
- Include applicable changelog and documentation updates following [Change records and releases](#change-records-and-releases)

For AI-assisted work, follow [the project rules](AGENTS.md) and [AI engineering workflow](docs/AI-ENGINEERING.md). The repository includes [project Skills and desktop import instructions](skills/README.md); choose your own AI tools and deployment scope. AI assistance is optional and does not change the contribution requirements.

## Change Records and Releases

- Record user-visible features, improvements, and bug fixes in the [English changelog](CHANGELOG.md) and [Chinese changelog](docs/CHANGELOG.zh.md) in the same task. Until a target release is explicitly confirmed, add entries under `Unreleased`. The version currently in package files does not establish the target release for new work.
- A small fix normally needs only a concise changelog entry and any affected usage documentation. Add detailed topic documents when a complex feature, compatibility change, or migration needs explanation. Before a release is assigned, update the existing owner document or a version-independent topic under `docs/`; do not invent a future version directory or create one document per bug.
- During release preparation, once the target version and release scope are confirmed, move only the selected entries into that version's section in both changelogs. Leave other pending work under `Unreleased`. Create or update the English `README.md` and Chinese `README.zh.md` release indexes under `docs/releases/v<version>/` using the format below. Separate topic documents remain optional; fix links when moving them.
- Published release sections and directories describe what shipped. Do not append later features or fixes to them. Corrections must clarify the behavior of that historical release, not attribute later changes to it. Forward-looking instructions in old release documents apply only to their original release cycle.
- Change version fields only when a version bump is requested, and verify consistency with `npm run version:check`. Preparing change records does not authorize publishing a release.

### Release Index Format

Use the [English template](docs/releases/_template/README.md) and [Chinese template](docs/releases/_template/README.zh.md), based on the [v1.0.0 index](docs/releases/v1.0.0/README.md). Apply this format to future releases; historical indexes do not need reformatting.

- Start with the version and publication status. Before publication, label it as unreleased; after publication, add the actual release date and release-page link.
- Keep two sections in order: **Features and improvements** (`Topic`, `User-visible changes`, `Details and pull requests`) and **Bug fixes** (`Problem`, `Issue`, `Pull request`, `Details`). Omit sections with no entries and use `—` when a link is unavailable.
- Describe each change in one concise sentence and link to existing issues, pull requests, or topic documents. Keep implementation details, test logs, and publishing procedures in their owning documents. Link to the version's changelog section for the full list; do not create a topic document just to fill a table cell.
- Keep the English and Chinese indexes aligned in release scope, status, rows, and references. Replace template placeholders, remove template instructions, and verify local links and changelog anchors before delivery. Preparing an index does not authorize publication.

## Reporting Issues

Please include the following in your issue report:

- OS version (macOS/Windows/Linux)
- Skills Hub version
- Steps to reproduce and expected vs. actual behavior
- Relevant logs (please redact local paths and any sensitive information)
