# Skills Hub Project Rules

This English file is the canonical project instruction entry. Keep the [Chinese reference](docs/AGENTS.zh.md) aligned when updating it.

## Communication and Scope

- Write project rule documents in English and maintain corresponding Chinese references under `docs/`. Use one prose language per file; preserve commands, paths, code identifiers, and proper names.
- Before implementation, briefly describe the approach and affected files. Continue work the user has already authorized without repeated confirmation. Ask when requirements, scope, or authorization for irreversible actions are unclear. Keep review and advice requests read-only.
- Change only what the task requires, preserve existing user changes, and avoid unrelated refactoring or cleanup.

## Project Entry Points

Skills Hub is a cross-platform desktop app for managing and distributing AI skills, built with React, TypeScript, Tauri, and Rust. Dependency versions, compiler options, and check commands are maintained in [package.json](package.json), [Cargo.toml](src-tauri/Cargo.toml), and their associated configuration files.

| Area | Entry points |
| --- | --- |
| Frontend UI and interactions | `src/App.tsx`, `src/components/` |
| Desktop commands and registration | `src-tauri/src/commands/`, `src-tauri/src/lib.rs` |
| Shared business logic | `src-tauri/src/core/` |
| Command-line interface | `src-tauri/src/cli/`, `src-tauri/src/bin/skillshub-cli.rs` |
| Development, builds, and verification | [Contributing](CONTRIBUTING.md), [Development and builds](docs/development.en.md) |
| Selecting skills for a task | [AI engineering workflow](docs/AI-ENGINEERING.md) |
| Importing project Skills into your tools | [Project Skills](skills/README.md) |

## Implementation Conventions

- The frontend calls desktop commands through Tauri `invoke`. Define new commands in the command layer and register them in `lib.rs`. Keep `src/components/skills/types.ts` aligned with backend transport types; use `camelCase` command parameters.
- The command layer handles parameters, transport types, and error conversion; business logic belongs in `core/`. Use `tauri::async_runtime::spawn_blocking` for blocking work inside async commands. Export new core modules from `core/mod.rs`; the Rust library is named `app_lib`.
- Follow the responsibilities of existing frontend components, local state, and hooks. Do not force all state into `App.tsx` or introduce a state management framework for a local change.
- Handle state reset, focus, and resource cleanup according to the dialog's interaction requirements. A component returning `null` does not mean that component has unmounted.
- Preserve existing error categories and conversion conventions, using `anyhow::Context` to retain causes. User feedback must explain failure state and recovery actions; transient notifications must not be the sole source of essential information.
- All UI copy must use localization resources and include English, Simplified Chinese, and Korean. English and Chinese resources live in `src/i18n/resources.ts`; Korean resources live in `src/i18n/ko.ts`.
- Follow the existing style organization in the affected area, with semantic class names and theme variables from `src/index.css`. Read the [UI design guidelines](docs/UI-DESIGN-GUIDELINES.md) before changing layout, visuals, shared components, navigation, overlays, responsiveness, or interaction feedback. Other tasks need not load them.
- Preserve `~` path expansion and the skill distribution fallback order: symlink, Windows junction, then copy. When adding a tool adapter, update both the tool identifier and adapter registration in `tool_adapters/mod.rs`.

## Development and Delivery

1. **Working branch**: Continue existing tasks on their current branch. For a new independent development task without a user-specified baseline, fetch `origin/main` and create a `codex/` branch from it. Preserve existing work, verify the baseline, and sync dependencies with the lockfile after switching.
2. **Complete implementation**: For features spanning frontend and backend, deliver command registration, transport types, UI, and copy in all three languages together.
3. **Verification**: Bug fixes require regression tests that detect the original defect. Targeted checks are useful during development; run `npm run check` after completing changes and ensure results cover the final changes before committing. Follow the full sequence and build prerequisites in the contribution guide. Do not substitute targeted checks for the full gate or report blocked or unrun checks as passing.
4. **Desktop review**: After user-facing fixes, run `npm run tauri:dev` from the working branch and confirm startup unless the user requests otherwise. Reuse or restart only this checkout's development processes. A browser preview does not verify native desktop behavior.
5. **Documentation and versions**: Follow the [change record and release rules](CONTRIBUTING.md#change-records-and-releases), including the standard bilingual release index. Until a target release is confirmed, record user-visible changes under `Unreleased` in both changelogs. Keep later changes out of published releases; a small fix does not require a separate topic document. Required multilingual updates are part of the task; do not translate unrelated documents. Release notes do not require Korean. Do not bump the version unless requested; verify consistency with `npm run version:check`.
6. **Commits and publishing**: Follow the user's instructions on commits, pushes, and publishing. When asked to create a pull request, target `main`, prefer updating an existing request for the current branch, include code, tests, and applicable changelog and documentation updates, and verify the link. Do not merge without authorization.

## Security Boundaries

- Store tokens, passwords, and private keys only in the system credential store. Never put them in databases, configuration files, logs, URLs, or synchronization repositories.
- Read credentials only for user-initiated operations or explicitly enabled background features, never during page load, tab changes, status display, or ordinary startup.
- Development builds use an independent credential namespace. Changes involving authorization, credentials, or synchronization must pass leakage-prevention and access-boundary tests.
- Development desktop and CLI builds share the production database, central skill library, settings, caches, recycle bin, and write lock; operations affect real tool directories. Credentials and the CLI bridge use independent development namespaces. Automated tests must use temporary data and tool directories. Never automatically merge or overwrite production data with an old development database.

## Network and Data Compatibility

- Create or configure production HTTP clients, OAuth requests, and remote Git operations through `src-tauri/src/core/network_proxy.rs`, explicitly using application proxy settings. Do not rely on process proxy environment variables or global Git proxy settings, or construct transport clients directly in business modules.
- Run `npm run network:check` after adding or changing outbound network code. Do not bypass it through paths, naming, or test markers.
- Follow the existing database migration mechanism. New feature tables that older versions can safely ignore must use a feature-specific schema marker in `settings`, without increasing the shared `PRAGMA user_version`.
- Every database migration requires upgrade tests and compatibility tests against the previous stable version. Define a compatibility plan before making incompatible shared-schema changes.

## AI Engineering

- DSH Engineering supplies task methods; this file and the contribution guide retain authority over project constraints and the full gate. See the [AI engineering workflow](docs/AI-ENGINEERING.md) for usage order, evidence requirements, and guidance updates.
- Completion reports should briefly describe changes, actual checks, and remaining issues. Maintain facts in their existing owner documents and link to them elsewhere. Updating skill packages does not automatically rewrite project rules.
