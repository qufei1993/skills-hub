# Skills Hub Agent-first CLI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a native Rust `skillshub-cli` and official `skills-hub` Agent Skill that manage the same local library as the desktop app, then distribute that CLI with the desktop app, standalone releases, and npm.

**Architecture:** Tauri commands and the CLI become thin adapters over `SkillsHubService`. The service owns validation, operation planning, cross-process locking, and calls the existing SQLite/installer/sync modules. npm only selects and launches a prebuilt platform-specific Rust binary.

**Tech Stack:** Rust 2021/MSRV 1.77.2, Tauri 2, rusqlite, fs2, clap 4, serde, React 19, TypeScript 5.9, Vitest, npm workspaces, GitHub Actions.

**Spec:** `docs/superpowers/specs/2026-09-19-agent-first-cli-design.md`

## Global Constraints

- Names: product `Skills Hub`, npm package and command `skillshub-cli`, official Agent Skill `skills-hub`.
- Secure the `skillshub-app` npm organization/scope with required 2FA and trusted publishing before any public platform package is released; do not substitute a personal scope in the release workflow.
- Target v0.11.0; keep root, Cargo, Tauri, npm platform packages, lockfiles, changelogs, and release records synchronized.
- Install is library-only. Deploy requires one or more explicit Agents. Do not expose a CLI command named `sync`.
- No CLI for device sync, schedules, credential configuration, proxy changes, storage migration, app updates, custom tools, permanent trash deletion, or force overwrite.
- Production, development, and test data roots remain isolated. Desktop and production CLI share one production DB and central library.
- Credentials remain in system secure storage and never appear in CLI arguments, files, logs, URLs, or JSON.
- All outbound HTTP/Git traffic continues through `src-tauri/src/core/network_proxy.rs`; run `npm run network:check` after network changes.
- User-visible desktop and CLI text requires English, Simplified Chinese, and Korean.
- Do not raise shared `PRAGMA user_version` solely for this feature. Include upgrade and v0.10.1 compatibility tests for any migration.
- Implement with TDD, commit each task, and run `npm run check` before completion.

---

### Task 1: Create a Tauri-independent runtime context

**Files:**
- Create: `src-tauri/src/core/runtime_paths.rs`
- Create: `src-tauri/src/core/tests/runtime_paths.rs`
- Modify: `src-tauri/src/core/mod.rs`
- Modify: `src-tauri/src/core/skill_store.rs`
- Modify: `src-tauri/src/core/central_repo.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Produces `RuntimeProfile`, `RuntimePaths::{from_roots,for_cli,from_tauri}`, and `open_store(&RuntimePaths)`.
- Later tasks consume one explicit path object instead of `AppHandle`.

- [ ] **Step 1: Write the failing isolation test**

```rust
#[test]
fn production_and_development_paths_do_not_overlap() {
    let prod = RuntimePaths::from_roots(RuntimeProfile::Production, "/home/may", "/data");
    let dev = RuntimePaths::from_roots(RuntimeProfile::Development, "/home/may", "/data");
    assert_eq!(prod.database_path, PathBuf::from("/data/com.qufei1993.skillshub/skills_hub.db"));
    assert_eq!(dev.database_path, PathBuf::from("/data/com.qufei1993.skillshub.dev/skills_hub.db"));
    assert_eq!(prod.default_central_repo, PathBuf::from("/home/may/.skillshub"));
    assert_eq!(prod.cli_bridge_dir, PathBuf::from("/home/may/.skills-hub/bin"));
}
```

- [ ] **Step 2: Run it before implementation**

Run: `cd src-tauri && cargo test runtime_paths -- --nocapture`

Expected: compile failure because `runtime_paths` does not exist.

- [ ] **Step 3: Implement the path object and shared store opener**

```rust
pub const PRODUCT_IDENTIFIER: &str = "com.qufei1993.skillshub";
pub enum RuntimeProfile { Production, Development, Test }
pub struct RuntimePaths {
    pub profile: RuntimeProfile,
    pub app_data_dir: PathBuf,
    pub database_path: PathBuf,
    pub default_central_repo: PathBuf,
    pub git_cache_dir: PathBuf,
    pub recycle_bin_dir: PathBuf,
    pub cli_bridge_dir: PathBuf,
}
```

`open_store` runs legacy migration, `ensure_schema`, and the existing safe startup migrations. `from_tauri` accepts Tauri-resolved app data; `for_cli` uses `dirs` and the same identifiers; tests inject roots.

- [ ] **Step 4: Convert DB and central-path callers**

Change central resolution to `resolve_central_repo_path(paths, store)` and default to `paths.default_central_repo`. Construct/manage `RuntimePaths` and the store once in `lib.rs`; keep window/plugin code in Tauri.

- [ ] **Step 5: Verify and commit**

Run: `cd src-tauri && cargo test runtime_paths && cargo check --all-targets`

```bash
git add src-tauri/src/core src-tauri/src/lib.rs
git commit -m "refactor: share runtime paths outside Tauri"
```

### Task 2: Add a cross-process operation lock

**Files:**
- Create: `src-tauri/src/services/mod.rs`
- Create: `src-tauri/src/services/operation_lock.rs`
- Create: `src-tauri/src/services/tests/operation_lock.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Produces `OperationKind`, `OperationLockError`, and `OperationLock::acquire(&RuntimePaths, OperationKind)`.
- Top-level services own the lock; core helpers never reacquire it.

- [ ] **Step 1: Write the failing contention test**

```rust
let first = OperationLock::acquire(&paths, OperationKind::Install).unwrap();
let error = OperationLock::acquire(&paths, OperationKind::Deploy).unwrap_err();
assert!(matches!(error, OperationLockError::Busy(OperationKind::Deploy)));
drop(first);
OperationLock::acquire(&paths, OperationKind::Deploy).unwrap();
```

- [ ] **Step 2: Run it before implementation**

Run: `cd src-tauri && cargo test operation_lock -- --nocapture`

Expected: compile failure for missing lock types.

- [ ] **Step 3: Implement a non-blocking `fs2` lock**

Open `<app_data_dir>/operation.lock`, call `try_lock_exclusive`, map `WouldBlock` to `OperationLockError::Busy(kind)`, and unlock on drop. Do not sleep or retry automatically. Task 3 maps this typed lock error to the public `OPERATION_BUSY` service code.

- [ ] **Step 4: Apply it to existing top-level writers**

Guard desktop install, update, deploy/undeploy, delete/restore, storage migration, auto-update execution, and device-sync execution. Preserve internal locks temporarily but enforce one fixed outer lock order.

- [ ] **Step 5: Verify and commit**

Run: `cd src-tauri && cargo test operation_lock && cargo test installer && cargo test recycle_bin`

```bash
git add src-tauri/src/services src-tauri/src/lib.rs src-tauri/src/commands src-tauri/src/core
git commit -m "feat: serialize Skills Hub write operations"
```

### Task 3: Define service errors, selectors, and read operations

**Files:**
- Create: `src-tauri/src/services/error.rs`
- Create: `src-tauri/src/services/types.rs`
- Create: `src-tauri/src/services/skills_hub.rs`
- Create: `src-tauri/src/services/tests/skills_hub_read.rs`
- Modify: `src-tauri/src/services/mod.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/commands/mod.rs`

**Interfaces:**
- Produces `ServiceError`, `ErrorCode`, `SkillSelector`, service DTOs, and `SkillsHubService`.
- Produces `list_skills`, `show_skill`, `skill_status`, `list_agents`, and `doctor`.

- [ ] **Step 1: Write failing selector/schema tests**

```rust
let error = service.show_skill(SkillSelector::Name("REACT".into())).unwrap_err();
assert_eq!(error.code, ErrorCode::AmbiguousSkill);
assert_eq!(error.details["candidates"].as_array().unwrap().len(), 2);
```

Add a DB fixture with `user_version = 99` and assert business writes return `INCOMPATIBLE_DATABASE`.

- [ ] **Step 2: Run failing tests**

Run: `cd src-tauri && cargo test skills_hub_read -- --nocapture`

- [ ] **Step 3: Implement stable error contracts**

```rust
pub enum ErrorCode {
    InvalidArgument, InvalidSource, SkillNotFound, AmbiguousSkill, MultiSkills,
    AgentNotFound, ProjectScopeUnsupported, TargetConflict, UpdateHeldBack,
    ConfirmationRequired, PlanStale, OperationBusy, IncompatibleDatabase, AuthRequired,
    NetworkError, InternalError,
}
pub struct ServiceError { pub code: ErrorCode, pub message: String, pub details: Value }
```

Resolve exact ID first, then case-insensitive name; never guess among duplicates.
Map `OperationLockError::Busy` to `ErrorCode::OperationBusy` with only the non-sensitive operation kind in `details`.

- [ ] **Step 4: Implement read DTOs and service opening**

`SkillsHubService::open(paths)` opens the shared store. DTOs include source, tags, targets, scope, project path, content status, and detected Agent state, but no secret settings.

- [ ] **Step 5: Route desktop read commands through the service**

Keep existing IPC names/DTO shapes and convert service DTOs in the command adapter.

- [ ] **Step 6: Verify and commit**

Run: `cd src-tauri && cargo test skills_hub_read && cargo test commands`

```bash
git add src-tauri/src/services src-tauri/src/lib.rs src-tauri/src/commands
git commit -m "refactor: add shared Skills Hub service"
```

### Task 4: Share install, search, check, and update operations

**Files:**
- Create: `src-tauri/src/services/install.rs`
- Create: `src-tauri/src/services/tests/install.rs`
- Modify: `src-tauri/src/core/installer.rs`
- Modify: `src-tauri/src/core/cache_cleanup.rs`
- Modify: `src-tauri/src/core/temp_cleanup.rs`
- Modify: `src-tauri/src/commands/mod.rs`

**Interfaces:**
- Produces `InstallSource`, `InstallRequest`, `InstallOutcome`, `UpdateCheck`, and `UpdateOutcome`.
- Produces `SkillsHubService::{install,search,check_updates,update}`.

- [ ] **Step 1: Write the cross-entry failing test**

```rust
let installed = service.install(InstallRequest::local(skill_dir)).unwrap();
assert!(installed.targets.is_empty());
let reopened = fixture.reopen_service();
assert_eq!(reopened.show_skill(installed.id.into()).unwrap().source.kind, "local");
```

Add Git subpath, multi-Skill candidates, proxy boundary, and held-back update cases.

- [ ] **Step 2: Run failing tests**

Run: `cd src-tauri && cargo test services::tests::install -- --nocapture`

- [ ] **Step 3: Remove `AppHandle` from installer/cache signatures**

Use `&RuntimePaths` for central directory and cache paths:

```rust
pub fn install_git_skill(
    paths: &RuntimePaths, store: &SkillStore, repo_url: &str,
    name: Option<String>, cancel: Option<&CancelToken>,
) -> Result<InstallResult>
```

- [ ] **Step 4: Implement deterministic source parsing and update protection**

Recognize explicit local prefixes, Git syntax, and supported marketplace shorthand. Return `MULTI_SKILLS` with candidates. Install never deploys. Map destructive removals to `UPDATE_HELD_BACK`; do not add `--force`.

- [ ] **Step 5: Route desktop install/update commands through the service**

Preserve current Tauri command names and frontend DTOs. Use one service error formatter.

- [ ] **Step 6: Verify network and regression tests, then commit**

Run: `cd src-tauri && cargo test installer && cargo test services::tests::install && cargo test commands`

Run: `npm run network:check`

```bash
git add src-tauri/src/services/install.rs src-tauri/src/services/tests/install.rs src-tauri/src/core/installer.rs src-tauri/src/core/cache_cleanup.rs src-tauri/src/core/temp_cleanup.rs src-tauri/src/commands/mod.rs
git commit -m "refactor: share skill install and update operations"
```

### Task 5: Add atomic deploy and undeploy services

**Files:**
- Create: `src-tauri/src/services/deployment.rs`
- Create: `src-tauri/src/services/tests/deployment.rs`
- Modify: `src-tauri/src/core/sync_engine.rs`
- Modify: `src-tauri/src/core/tool_distribution.rs`
- Modify: `src-tauri/src/commands/mod.rs`

**Interfaces:**
- Produces `DeploymentScope`, `DeploymentRequest`, `DeploymentPlan`, and `DeploymentOutcome`.
- Produces `plan_deploy`, `deploy`, `plan_undeploy`, and `undeploy`.

- [ ] **Step 1: Write failing all-or-nothing tests**

```rust
fixture.create_unmanaged_target("cursor");
let error = service.deploy(DeploymentRequest::global("demo", ["codex", "cursor"])).unwrap_err();
assert_eq!(error.code, ErrorCode::TargetConflict);
assert!(!fixture.target_exists("codex"));
assert!(service.skill_targets("demo").unwrap().is_empty());
```

Add dry-run, project-scope, shared-directory, and safe undeploy cases.

- [ ] **Step 2: Run failing tests**

Run: `cd src-tauri && cargo test services::tests::deployment -- --nocapture`

- [ ] **Step 3: Implement complete preflight planning**

Require at least one explicit Agent. Resolve every target, scope, shared directory, overlap, conflict, and permission before writing. Return conflict details with Agent, path, and reason.

- [ ] **Step 4: Implement staged activation and rollback**

Prepare all target replacements, activate them, commit target rows in one transaction, and keep backups until commit succeeds. Roll back activated paths on error.

- [ ] **Step 5: Route desktop deployment commands through the service**

Keep IPC names `sync_skill_to_tool` and `unsync_skill_from_tool` for frontend compatibility; the CLI later exposes only deploy/undeploy.

- [ ] **Step 6: Verify and commit**

Run: `cd src-tauri && cargo test services::tests::deployment && cargo test sync_engine && cargo test commands`

```bash
git add src-tauri/src/services/deployment.rs src-tauri/src/services/tests/deployment.rs src-tauri/src/core/sync_engine.rs src-tauri/src/core/tool_distribution.rs src-tauri/src/commands/mod.rs
git commit -m "feat: add atomic agent deployment service"
```

### Task 6: Share tags, adopt, and recycle-bin removal

**Files:**
- Create: `src-tauri/src/services/library.rs`
- Create: `src-tauri/src/services/tests/library.rs`
- Modify: `src-tauri/src/core/onboarding.rs`
- Modify: `src-tauri/src/core/recycle_bin.rs`
- Modify: `src-tauri/src/commands/mod.rs`

**Interfaces:**
- Produces `AdoptPlan`, `TagAction`, `RemovePlan`, and corresponding outcomes.
- Produces `plan_adopt`, `adopt`, `apply_tag_action`, `plan_remove`, and `remove`.

- [ ] **Step 1: Write failing preview/confirmation tests**

```rust
let plan = service.plan_remove("demo".into()).unwrap();
let error = service.remove(RemoveRequest::unconfirmed(plan.id)).unwrap_err();
assert_eq!(error.code, ErrorCode::ConfirmationRequired);
assert!(fixture.central_skill_exists("demo"));
```

Add adopt exclusion, stale-plan, tag rename/delete impact, and recycle metadata cases.

- [ ] **Step 2: Run failing tests**

Run: `cd src-tauri && cargo test services::tests::library -- --nocapture`

- [ ] **Step 3: Implement immutable preview plans**

Plan IDs include hashes of relevant DB/filesystem state. Confirmed execution recomputes the state and returns `PLAN_STALE` if the preview is no longer valid.

- [ ] **Step 4: Implement tag/adopt/remove through existing core**

Tags remain case-insensitive. Adopt excludes existing records/targets and treats unknown sources as local. Remove only calls `RecycleBinService::archive`; never permanently deletes.

- [ ] **Step 5: Route desktop tags/import/delete through the service**

Preserve frontend behavior and remove duplicate command-layer filesystem orchestration.

- [ ] **Step 6: Verify and commit**

Run: `cd src-tauri && cargo test onboarding && cargo test recycle_bin && cargo test skill_store && cargo test services::tests::library && cargo test commands`

```bash
git add src-tauri/src/services/library.rs src-tauri/src/services/tests/library.rs src-tauri/src/core/onboarding.rs src-tauri/src/core/recycle_bin.rs src-tauri/src/commands/mod.rs
git commit -m "feat: share tags import and safe removal"
```

### Task 7: Build the Rust CLI parser, protocol, and localization

**Files:**
- Create: `src-tauri/src/bin/skillshub-cli.rs`
- Create: `src-tauri/src/cli/{mod.rs,args.rs,output.rs,locale.rs,tests.rs}`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/Cargo.lock`

**Interfaces:**
- Produces clap `Cli`, `run_with_io`, localized human output, `JsonEnvelope<T>`, and exit-code mapping.
- Consumes `SkillsHubService` and service errors.

- [ ] **Step 1: Write failing parser/output tests**

```rust
let error = Cli::try_parse_from(["skillshub-cli", "skills", "deploy", "demo"]).unwrap_err();
assert_eq!(error.kind(), clap::error::ErrorKind::MissingRequiredArgument);
```

Assert JSON errors go only to stderr and `SKILL_NOT_FOUND` exits 3.

- [ ] **Step 2: Run failing tests**

Run: `cd src-tauri && cargo test cli:: -- --nocapture`

- [ ] **Step 3: Add `clap = { version = "4.5", features = ["derive"] }` and the binary**

```rust
fn main() -> std::process::ExitCode { app_lib::cli::run(std::env::args_os()) }
```

Export `cli`, `core`, and `services` from `app_lib`. Do not apply the desktop Windows-subsystem attribute to the CLI.

- [ ] **Step 4: Define the complete approved command tree**

Add `skills list/show/search/status/check/install/deploy/undeploy/update/adopt/tag/remove`, `agents list`, `doctor`, `version`, and `setup`. Support `--json`, `--lang`, `--dry-run`, and `--yes` as specified. Hide the bridge maintenance subcommand.

- [ ] **Step 5: Implement JSON and locale contracts**

```rust
pub enum JsonEnvelope<T> {
    Success { ok: bool, command: &'static str, data: T },
    Failure { ok: bool, command: &'static str, code: ErrorCode, message: String, details: Value },
}
```

Provide complete EN/ZH-CN/KO Rust catalogs; resolution is `--lang`, supported locale environment, then English. JSON field names and codes never localize.

- [ ] **Step 6: Verify and commit**

Run: `cd src-tauri && cargo test cli:: && cargo run --bin skillshub-cli -- --help`

```bash
git add src-tauri/src/bin/skillshub-cli.rs src-tauri/src/cli src-tauri/src/lib.rs src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "feat: add agent-friendly Rust CLI protocol"
```

### Task 8: Wire all CLI workflows and process-level tests

**Files:**
- Create: `src-tauri/src/cli/handlers.rs`
- Create: `src-tauri/tests/cli_integration.rs`
- Modify: `src-tauri/src/cli/mod.rs`
- Modify: `src-tauri/src/cli/output.rs`

**Interfaces:**
- Consumes all service methods from Tasks 3–6.
- Produces the full public CLI behavior and `CARGO_BIN_EXE_skillshub-cli` integration coverage.

- [ ] **Step 1: Write the failing end-to-end test**

```rust
fixture.run_json(["skills", "install", fixture.skill_path()]).assert_ok();
fixture.run_json(["skills", "tag", "add", "demo", "frontend"]).assert_ok();
fixture.run_json(["skills", "deploy", "demo", "--agent", "codex"]).assert_ok();
let skill = fixture.reopen_service().show_skill("demo".into()).unwrap();
assert_eq!(skill.tags, vec!["frontend"]);
assert_eq!(skill.targets[0].agent, "codex");
```

Use a test-only injected root that cannot be enabled in release builds.

- [ ] **Step 2: Run it before handlers exist**

Run: `cd src-tauri && cargo test --test cli_integration -- --nocapture`

- [ ] **Step 3: Implement handlers as pure adapters**

Each handler constructs one service request and renders its result. It must not query SQLite, resolve Agent paths, copy files, or interpret credentials.

- [ ] **Step 4: Add negative protocol tests**

Cover `MULTI_SKILLS`, `AMBIGUOUS_SKILL`, `TARGET_CONFLICT`, `UPDATE_HELD_BACK`, `CONFIRMATION_REQUIRED`, `OPERATION_BUSY`, `INCOMPATIBLE_DATABASE`, project-scope errors, and secret redaction. Assert JSON shape and exit code.

- [ ] **Step 5: Verify and commit**

Run: `cd src-tauri && cargo test --all && cargo clippy --all-targets --all-features -- -D warnings`

```bash
git add src-tauri/src/cli src-tauri/tests/cli_integration.rs
git commit -m "feat: expose Skills Hub workflows through CLI"
```

### Task 9: Add the official `skills-hub` Skill and setup service

**Files:**
- Create: `skills/skills-hub/SKILL.md`
- Create: `src-tauri/src/services/agent_access.rs`
- Create: `src-tauri/src/services/tests/agent_access.rs`
- Modify: `src-tauri/src/services/mod.rs`
- Modify: `src-tauri/src/cli/handlers.rs`
- Modify: `src-tauri/src/core/onboarding.rs`

**Interfaces:**
- Produces `AgentAccessStatus`, `SetupAgentRequest`, `agent_access_status`, and `setup_agent_access`.
- Setup installs a `bundled` source through normal library/deployment services.

- [ ] **Step 1: Write failing setup/conflict tests**

```rust
let result = service.setup_agent_access(SetupAgentRequest::install("codex")).unwrap();
assert!(result.deployed);
assert!(!fixture.agent_has_skill("cursor"));
assert_eq!(fixture.managed_source("skills-hub"), "bundled");
```

Add unmanaged same-name conflict and modified bundled copy protection.

- [ ] **Step 2: Run failing tests**

Run: `cd src-tauri && cargo test agent_access -- --nocapture`

- [ ] **Step 3: Write the official Skill**

Include trusted CLI resolution, mandatory `--json`, library/deploy distinction, all approved workflows, confirmation requirements, stable error handling, and prohibitions on direct SQL/filesystem work, credential exposure, device sync, and force overwrite.

- [ ] **Step 4: Embed and install through normal services**

```rust
const OFFICIAL_SKILL_MD: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"), "/../skills/skills-hub/SKILL.md"
));
```

Stage the bundled content, record `source_type = "bundled"`, then deploy only to requested Agents. Exclude its managed targets from onboarding discoveries.

- [ ] **Step 5: Wire `setup`, verify, and commit**

Run: `cd src-tauri && cargo test agent_access && cargo test onboarding && cargo test cli::`

```bash
git add skills/skills-hub/SKILL.md src-tauri/src/services/agent_access.rs src-tauri/src/services/tests/agent_access.rs src-tauri/src/services/mod.rs src-tauri/src/cli/handlers.rs src-tauri/src/core/onboarding.rs
git commit -m "feat: add official Skills Hub agent skill"
```

### Task 10: Bundle and publish the verified desktop CLI bridge

**Files:**
- Create: `src-tauri/src/core/cli_bridge.rs`
- Create: `src-tauri/src/core/tests/cli_bridge.rs`
- Create: `scripts/prepare-cli-sidecar.mjs`
- Create: `scripts/prepare-cli-sidecar.test.mjs`
- Modify: `src-tauri/src/core/mod.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/tauri.conf.json`
- Modify: `scripts/build-desktop.mjs`
- Modify: `package.json`

**Interfaces:**
- Produces `publish_cli_bridge` and `CliBridgeStatus`.
- Publishes binary plus `.version`/`.sha256` only after verification.

- [ ] **Step 1: Write failing atomic-publication tests**

```rust
let error = publish_cli_bridge(source, destination, "0.11.0", "wrong-hash").unwrap_err();
assert!(error.to_string().contains("CLI_BRIDGE_HASH_MISMATCH"));
assert!(!fixture.version_stamp().exists());
```

Add successful replacement, executable mode, interrupted temp file, and dev/prod isolation.

- [ ] **Step 2: Run failing tests**

Run: `cd src-tauri && cargo test cli_bridge -- --nocapture`

- [ ] **Step 3: Implement atomic bridge publication**

Delete stamps, copy to sibling temp, set Unix executable mode, hash, atomically rename, then atomically write stamps. Bridge paths are `~/.skills-hub/bin` and `~/.skills-hub-dev/bin`.

- [ ] **Step 4: Build and bundle the Tauri sidecar**

The script takes an explicit target, builds `--bin skillshub-cli`, and copies it to `src-tauri/binaries/skillshub-cli-<target>[.exe]`. Add `bundle.externalBin = ["binaries/skillshub-cli"]`; reject unsupported targets.

- [ ] **Step 5: Integrate desktop startup and verify**

Desktop startup publishes the bundled same-version sidecar and reports failure without aborting the app. Run:

`node --test scripts/prepare-cli-sidecar.test.mjs scripts/build-desktop.test.mjs`

`cd src-tauri && cargo test cli_bridge && cargo check --all-targets`

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/core/cli_bridge.rs src-tauri/src/core/tests/cli_bridge.rs src-tauri/src/core/mod.rs src-tauri/src/lib.rs src-tauri/tauri.conf.json scripts/prepare-cli-sidecar.mjs scripts/prepare-cli-sidecar.test.mjs scripts/build-desktop.mjs package.json
git commit -m "feat: publish a verified desktop CLI bridge"
```

### Task 11: Add the desktop Agent Access page

**Files:**
- Create: `src/components/skills/AgentAccessPage.tsx`
- Create: `src/components/skills/AgentAccessPage.test.tsx`
- Modify: `src/components/skills/{Header.tsx,types.ts}`
- Modify: `src/{App.tsx,App.css}`
- Modify: `src/i18n/{resources.ts,ko.ts}`
- Modify: `src-tauri/src/{commands/mod.rs,lib.rs}`

**Interfaces:**
- Produces IPC `get_agent_access_status` and `set_agent_access`.
- Adds management tab `agents`; never exposes arbitrary command execution.

- [ ] **Step 1: Write failing UI behavior tests**

```tsx
await userEvent.click(await screen.findByRole('button', { name: 'agentAccess.install' }))
expect(invoke).toHaveBeenCalledWith('set_agent_access', {
  agent: 'codex', action: 'install',
})
```

Add bridge damaged, repair, remove confirmation, copy npm command, keyboard, and collapsed-sidebar cases.

- [ ] **Step 2: Run failing test**

Run: `npm test -- AgentAccessPage.test.tsx`

- [ ] **Step 3: Add DTOs/commands and management-tab UI**

Commands call the service in `spawn_blocking`. The page shows CLI version/path/check status and a structured Agent list with install/repair/remove. It contains no terminal console and no database watcher.

- [ ] **Step 4: Add EN/ZH-CN/KO copy and guideline-compliant styles**

Use semantic tokens, monospace paths/commands, text plus icons for state, existing button hierarchy, visible focus, and constrained-window layout.

- [ ] **Step 5: Verify UI and manually inspect**

Run: `npm test -- AgentAccessPage.test.tsx && npm run lint && npm run build`

Run: `cd src-tauri && cargo test commands`

Run: `npm run tauri:dev`

Confirm install/repair/remove affects only the selected Agent and the bridge status is accurate.

- [ ] **Step 6: Commit**

```bash
git add src/components/skills/AgentAccessPage.tsx src/components/skills/AgentAccessPage.test.tsx src/components/skills/Header.tsx src/components/skills/types.ts src/App.tsx src/App.css src/i18n/resources.ts src/i18n/ko.ts src-tauri/src/commands/mod.rs src-tauri/src/lib.rs
git commit -m "feat: add desktop agent access management"
```

### Task 12: Package the Rust binary for npm

**Files:**
- Create: `packages/skillshub-cli/package.json`
- Create: `packages/skillshub-cli/README.md`
- Create: `packages/skillshub-cli/bin/skillshub-cli.cjs`
- Create: `packages/skillshub-cli/lib/platform.cjs`
- Create: `packages/cli-darwin-arm64/package.json`
- Create: `packages/cli-darwin-x64/package.json`
- Create: `packages/cli-win32-x64/package.json`
- Create: `packages/cli-linux-x64/package.json`
- Create: `packages/cli-linux-arm64/package.json`
- Create: `scripts/package-cli.mjs`
- Create: `scripts/package-cli.test.mjs`
- Modify: `package.json`
- Modify: `package-lock.json`
- Modify: `scripts/version.mjs`
- Modify: `.gitignore`

**Interfaces:**
- Produces public `skillshub-cli` and internal `@skillshub-app/cli-*` packages.
- Launcher selects by platform/arch and spawns with `shell: false`.

- [ ] **Step 1: Write failing platform selection tests**

```js
assert.equal(
  resolveBinaryPackage('darwin', 'arm64'),
  '@skillshub-app/cli-darwin-arm64/skillshub-cli',
)
assert.throws(() => resolveBinaryPackage('freebsd', 'x64'), /unsupported platform/i)
```

Add Windows extension, Linux arm64, missing optional dependency, argument passthrough, and exit-status cases.

- [ ] **Step 2: Run failing tests**

Run: `node --test scripts/package-cli.test.mjs`

- [ ] **Step 3: Add workspaces and exact package manifests**

The root stays private. Public package exposes `skillshub-cli`, requires Node >=20, and lists exact-version platform packages as `optionalDependencies`. Platform manifests declare exact `os`/`cpu` and contain no lifecycle scripts.

- [ ] **Step 4: Implement the thin launcher and deterministic staging**

Resolve with `require.resolve` and use:

```js
spawnSync(binary, process.argv.slice(2), { shell: false, stdio: 'inherit' })
```

`package-cli.mjs` requires explicit version/target/source/output, verifies `version --json`, copies, hashes, and runs `npm pack --json`. It never downloads in `postinstall`.

- [ ] **Step 5: Extend version sync and smoke-test tarballs**

Make `scripts/version.mjs` validate every CLI manifest and optional dependency version. Install host tarballs in a temporary directory and run `node_modules/.bin/skillshub-cli version --json`.

- [ ] **Step 6: Verify and commit**

Run: `node --test scripts/package-cli.test.mjs && npm run version:check`

```bash
git add packages scripts/package-cli.mjs scripts/package-cli.test.mjs package.json package-lock.json scripts/version.mjs .gitignore
git commit -m "feat: package the Rust CLI for npm"
```

### Task 13: Complete CI, release, compatibility, and v0.11.0 records

**Files:**
- Create: `src-tauri/tests/cli_desktop_compatibility.rs`
- Create: `docs/releases/v0.11.0/{README.md,agent-first-cli.md}`
- Modify: `.github/workflows/{ci.yml,release.yml}`
- Modify: `CHANGELOG.md`
- Modify: `docs/CHANGELOG.zh.md`
- Modify: `package.json`, `package-lock.json`
- Modify: `src-tauri/{Cargo.toml,Cargo.lock,tauri.conf.json}`

**Interfaces:**
- Consumes all prior tasks.
- Produces supported binaries, checksums, npm publication, compatibility proof, and release documentation.

- [ ] **Step 1: Add v0.10.1 compatibility tests**

Open a v0.10.1 fixture DB, perform v0.11 CLI install/tag/deploy, assert shared `user_version` is unchanged, then read it through a previous-stable-compatible reader. Also assert a higher unknown shared schema blocks v0.11 writes.

- [ ] **Step 2: Run compatibility tests**

Run: `cd src-tauri && cargo test --test cli_desktop_compatibility -- --nocapture`

Expected: both upgrade and previous-stable reads pass.

- [ ] **Step 3: Extend CI and release matrices**

Build/smoke-test CLI targets:

```text
aarch64-apple-darwin
x86_64-apple-darwin
x86_64-pc-windows-msvc
x86_64-unknown-linux-gnu
aarch64-unknown-linux-gnu
```

Create standalone `skillshub-cli-<version>-<platform>-<arch>` assets and SHA-256 files. Publish internal npm platform packages first and `skillshub-cli` last, only after every advertised platform succeeds. Use OIDC provenance with `id-token: write`; do not add a persistent npm token.

Reuse the existing Apple certificate import for the macOS CLI, verify its code signature, and include the standalone CLI in the configured notarization flow when notarization credentials are present. Reuse the project's Windows signing path when configured; unsigned artifacts must retain the existing explicit release warning.

- [ ] **Step 4: Set v0.11.0 and update release records**

Run: `npm run version:set -- 0.11.0`

Document the CLI, official Skill, Agent Access page, supported platforms, safety confirmations, desktop-only exclusions, and lack of live desktop refresh in English/Chinese changelogs and `docs/releases/v0.11.0/`.

- [ ] **Step 5: Run full verification and manual smoke test**

Run: `npm run check`

Run: `npm run version:check`

Run the host npm tarball smoke test and `npm run tauri:dev`. Install/tag/deploy a fixture through CLI, reopen/reload desktop data, and verify identical source, tags, scope, and targets.

- [ ] **Step 6: Audit secrets, scope, and final diff**

Run: `git diff --check origin/main...HEAD`

Run: `rg -n "(token|password|private[_-]?key|authorization)" packages src-tauri/src/cli skills/skills-hub .github/workflows/release.yml`

Review every match as a prohibition, redaction test, public client ID, or secure-store call. Confirm no secret value or CLI credential argument exists.

- [ ] **Step 7: Commit the release integration**

```bash
git add .github/workflows src-tauri/tests/cli_desktop_compatibility.rs CHANGELOG.md docs/CHANGELOG.zh.md docs/releases/v0.11.0 package.json package-lock.json src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/tauri.conf.json
git commit -m "release: prepare Skills Hub 0.11.0"
```

## Final Review Gate

- Every product feature in the spec's “本版本功能清单” has a service test and CLI integration test.
- Every excluded feature has no public clap command and no Agent Skill bypass.
- The official Skill never edits SQLite or Agent directories directly.
- A valid desktop bridge wins over PATH; a broken bridge does not fall back to an older CLI.
- Unknown newer schemas block writes; v0.10.1-compatible data remains readable.
- `npm run check`, version checks, package smoke tests, and manual desktop startup all pass.
