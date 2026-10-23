# AI Engineering Workflow

Keep the [Chinese reference](AI-ENGINEERING.zh.md) aligned when updating this workflow. The Chinese explanation of project rules is available in the [rules reference](AGENTS.zh.md).

Skills Hub uses DSH Engineering for daily development. [Project rules](../AGENTS.md) remain authoritative for project navigation, architecture, security, and network boundaries. Shared Skill sources live in [skills/](../skills/README.md); contributors import them through the desktop app and choose their own AI tools and deployment scope. Personal tool deployments remain outside version control.

## Usage Order

Initial adoption is complete. Daily development does not require reinitialization or running all skills for every task.

| Task | Skills to use as needed |
| --- | --- |
| Feature development, bug fixes, implementation design, and reviewing or migrating project guidance | `dsh-engineering` |
| Error handling, parallel tests, simplification, and performance during development | `dsh-error-handling`, `dsh-ci-test-reliability`, `dsh-find-simplifications`, `dsh-speed-up-perf` |
| UI interactions | `dsh-client-ui-ux`; read the project UI guidelines first |
| Documentation updates | `dsh-doc`; explicitly requested translation uses `dsh-translate-docs` |
| Reviewing changes | `dsh-code-review` |
| Final verification | `dsh-pre-push-checks`, running `npm run check` |
| Demonstrating browser behavior that needs recording | `dsh-record-browser-gif` |

For example: "Use dsh-engineering to implement this feature", "Use dsh-code-review to review the current changes and report findings only", or "Use dsh-pre-push-checks to verify the current changes". If a skill is unavailable, check installation and project deployment status first. Successful explicit invocation does not prove automatic triggering works in every client.

Browser recordings prove only what ran in the browser. Tauri IPC, system credentials, and desktop window behavior still require verification under project rules. A web preview is not a substitute. Verification and recording do not automatically authorize commits, pushes, publishing, or operations on real skill data.

## Documentation and Evidence

- Development and check entry point: [Contributing](../CONTRIBUTING.md); build configuration: [Development and builds](development.en.md).
- UI rules: [UI-DESIGN-GUIDELINES.md](UI-DESIGN-GUIDELINES.md), loaded only for relevant UI tasks.
- Change records follow the [change record and release rules](../CONTRIBUTING.md#change-records-and-releases): use `Unreleased` in [CHANGELOG.md](../CHANGELOG.md) and the [Chinese changelog](CHANGELOG.zh.md) until a target release is confirmed. When preparing a confirmed release, create the bilingual index under `docs/releases/` from the [release template](releases/_template/README.md); add separate topic documents only when needed.
- Required UI copy in three languages and English/Chinese release records are part of implementation work and need no separate translation request. Use `dsh-translate-docs` for explicitly scoped translations of other documents.
- Completion reports describe changes, checks actually run, and remaining issues. Measure performance through real entry points, preserve real parallel conditions when investigating concurrency failures, and do not hide unknown outcomes with retries.

## Updates and Removal

The adopted version is DSH Engineering 1.0.0. The pinned upstream commit and before/after documents are recorded in the [adoption record](../.dsh-engineering/adoption.json). This record contains full document contents; review it for private information before publishing.

Source-copy maintenance and desktop updates follow the [project Skills guide](../skills/README.md#maintain-the-dsh-copies). Updating a package does not rewrite project documents. When explicitly using `dsh-engineering` to update or remove project guidance, compare the record, current documents, and proposed changes first, preserving later project edits. Missing or damaged baselines cannot be used to guess original text.
