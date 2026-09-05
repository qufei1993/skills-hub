# Skills Hub - Project Rules

Reply in Chinese; write PR titles and descriptions in English.

## Overview

Skills Hub is a cross-platform desktop app (Tauri 2 + React 19) for managing AI Agent Skills and syncing them to 47+ AI coding tools. Core concept: "Install once, sync everywhere."

## Tech Stack

- **Frontend**: React 19 + TypeScript 5.9 (strict) + Vite 7 + Tailwind CSS 4
- **Backend**: Rust (Edition 2021, MSRV 1.77.2) + Tauri 2
- **Database**: SQLite (rusqlite, bundled)
- **Git**: libgit2 (git2 crate, vendored-openssl)
- **HTTP**: reqwest (rustls-tls, blocking)
- **i18n**: i18next (English / Chinese bilingual)
- **Notifications**: sonner (toast)
- **Icons**: lucide-react

## Common Commands

```bash
npm run dev              # Vite dev server (port 5173)
npm run tauri:dev        # Tauri dev window (frontend + backend)
npm run build            # tsc + vite build
npm run check            # Full check: lint + test + build + rust:fmt:check + rust:clippy + rust:test
npm test                 # Frontend regression tests
npm run lint             # ESLint (flat config v9)
npm run rust:test        # cargo test
npm run rust:clippy      # Rust lint
npm run rust:fmt         # Rust format
npm run rust:fmt:check   # Rust format check
```

## Directory Structure

```
src/                          # React frontend
├── App.tsx                   # Root component (centralized state, all modal states)
├── App.css                   # Global styles (all component styles live here)
├── index.css                 # CSS variables (theming) + Tailwind entry
├── components/
│   ├── Layout.tsx            # Main layout (sidebar + content area)
│   └── skills/               # Skills feature module
│       ├── Header.tsx        # Top bar (branding + language toggle + new button)
│       ├── FilterBar.tsx     # Filter/sort bar
│       ├── SkillsList.tsx    # Skills list container
│       ├── SkillCard.tsx     # Individual skill card
│       ├── LoadingOverlay.tsx
│       ├── types.ts          # Shared DTO type definitions (frontend ↔ backend)
│       └── modals/           # Modal components (8 total)
└── i18n/
    ├── index.ts              # i18next initialization
    └── resources.ts          # Translation resources (EN/ZH)

src-tauri/src/                # Rust backend
├── main.rs                   # Entry point (calls app_lib::run)
├── lib.rs                    # App initialization (plugin registration, DB, cleanup tasks)
├── commands/
│   ├── mod.rs                # Tauri command layer (23 commands + DTOs)
│   └── tests/
└── core/                     # Core business logic
    ├── skill_store.rs        # SQLite ORM (4 tables: skills, skill_targets, settings, discovered_skills)
    ├── installer.rs          # Skill installation (local/git, with multi-skill detection)
    ├── sync_engine.rs        # Sync engine (symlink/junction/copy triple fallback)
    ├── git_fetcher.rs        # Git clone/pull (with cache and TTL)
    ├── tool_adapters/mod.rs  # Tool adapter registry (47 AI tools)
    ├── onboarding.rs         # Existing skill scanning/discovery
    ├── github_search.rs      # GitHub API search
    ├── central_repo.rs       # Central repository path management
    ├── content_hash.rs       # SHA256 directory content hashing
    ├── cache_cleanup.rs      # Git cache cleanup
    ├── temp_cleanup.rs       # Temp directory cleanup
    └── tests/                # One test file per module (10 total)
```

## Architecture

### Frontend ↔ Backend Communication
- Uses Tauri IPC (`invoke`) to call backend commands
- Frontend call pattern: `const result = await invoke('command_name', { param })`
- Backend commands are defined in `commands/mod.rs` and registered in `lib.rs` via `generate_handler!`
- New commands must be registered in both places

### Frontend State Management
- **No state management library** — all state is centralized in `App.tsx` via `useState`
- Passed to child components via props drilling (modals receive many props)
- Data refresh pattern: call `invoke('get_managed_skills')` after operations to re-fetch the list

### Backend Layering
- `commands/` layer: Tauri command definitions, DTO conversions, error formatting (no business logic)
- `core/` layer: Pure business logic, independently testable
- Async commands use `tauri::async_runtime::spawn_blocking` to wrap synchronous operations
- Shared state injected via `app.manage(store)` + `State<'_, SkillStore>`

### Error Handling
- Backend uses `anyhow::Result<T>`, converted to string via `format_anyhow_error()` for the frontend
- Special error prefixes for frontend identification: `MULTI_SKILLS|`, `TARGET_EXISTS|`, `TOOL_NOT_INSTALLED|`
- Frontend catches with try-catch and displays errors via sonner toast

## Coding Conventions

### TypeScript
- Strict mode: `noUnusedLocals` and `noUnusedParameters` are enabled — unused variables/params cause compile errors
- Component files: PascalCase (`SkillCard.tsx`)
- Props types: `ComponentNameProps` (`SkillCardProps`)
- CSS class names: kebab-case (`modal-backdrop`, `skill-card`)
- Modal conditional rendering: `if (!open) return null` (full unmount, not display:none)
- Wrap presentational components with `memo()`
- All user-visible text must use i18n (`t('key')`), translation keys defined in `src/i18n/resources.ts`
- When adding new text, always provide both English and Chinese translations
- DTO types are defined in `src/components/skills/types.ts` and must stay in sync with the Rust DTOs in `commands/mod.rs`

### Rust
- Functions/methods: snake_case
- Constants: SCREAMING_SNAKE_CASE
- Tauri command parameters use camelCase (to match frontend JS calling convention)
- Use `anyhow::Context` to add context to errors
- New core modules must be exported in `core/mod.rs`
- Tests use `tempfile` crate for temp directories and `mockito` for HTTP mocking

### Styling
- Component styles go in `src/App.css` (not CSS Modules), using semantic CSS class names
- Theming via CSS variables + `[data-theme="dark"]` selector, variables defined in `src/index.css`
- Tailwind utility classes and custom CSS classes can be mixed

### UI Design Reference
- Before frontend work that changes layout, visual styling, shared components, navigation, overlays, responsive behavior, or interaction feedback, load only `docs/UI-DESIGN-GUIDELINES.md`.
- Do not load the UI guidelines for backend-only, data-only, test-only, release, or documentation tasks unless they also change product UI.

## Development Workflow

1. **Branch baseline**: Unless specified otherwise, fetch `origin/main` and create a `codex/` branch from it, preserving existing work. Verify the base commit; after changing the baseline, sync dependencies with the lockfile.
2. **Before implementing**: Briefly describe the approach and list the files to be modified. Wait for confirmation before writing code.
3. **Implement completely**: For features involving both frontend and backend, modify both sides in one pass — including Tauri command registration, DTO types, i18n translations (both EN and ZH), and UI.
4. **Keep changes minimal**: Only modify what is necessary for the requirement. Do not refactor, add comments, or "improve" unrelated code.
5. **Verify**: Bug fixes must include regression tests that fail before the fix and pass afterward. Run `npm run check` on the final changes before committing; resolve all failures.
6. **Manual review**: After user-facing bug fixes, run `npm run tauri:dev` from the working branch and confirm startup, unless requested otherwise. Reuse or restart only this checkout's development processes.
7. **Release records**: Record user-visible fixes under the current project version in `CHANGELOG.md`, `docs/CHANGELOG.zh.md`, and `docs/releases/v<version>/`; do not bump the version unless requested.
8. **Pull requests**: When requested, submit code, tests, and release records against `main`, updating an existing branch PR when available. Describe the problem, fix, and validation; verify the PR and return its link. Do not merge without authorization.

## Important Notes

- Path handling must support `~` expansion (backend has `expand_home_path()`)
- Sync strategy uses triple fallback: symlink → junction (Windows) → copy
- Git uses vendored-openssl, HTTP uses rustls-tls — avoids system SSL issues
- Version numbers must stay in sync between `package.json` and `src-tauri/tauri.conf.json` (validate with `npm run version:check`)
- Rust crate is named `app_lib` (not the default package name) — use `app_lib::...` for imports
- Database has a schema migration mechanism (`migrate_legacy_db_if_needed`) — consider migrations when modifying table structures
- Additive, feature-only database tables must use a feature-specific schema marker in `settings`; do not raise the shared `PRAGMA user_version` when the previous stable release can safely ignore the change
- Every database migration must include both an upgrade test and a previous-stable-version compatibility test; incompatible shared-schema changes require an explicit compatibility design before implementation
- Tool adapter list is in `tool_adapters/mod.rs` — adding a new AI tool requires both a `ToolId` enum variant and an adapter instance
