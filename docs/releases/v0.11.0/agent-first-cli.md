# Agent-first CLI

[中文](agent-first-cli.zh.md) · [Release overview](README.md)

Before the official Skill is installed, My Skills shows a dismissible AI management notice. Go to setup opens Settings, focuses the AI management card, and briefly highlights it. Dismissal is remembered on this device; installed, inactive, or damaged Skills do not show this first-use notice.

Deployment previews expose `affected_agents` for all tools sharing a physical target directory, separately from the managed deployment targets. Disabled tools do not gain new deployment records. One-click AI management rejects scope expansion before installing the official Skill and prompts the user to review tool configuration.

Desktop refreshes Skills and tags together on opening tag filters, entering library/tag pages, and window focus. Existing search/tag filters remain selected and stale read responses are ignored. This is interaction-triggered refresh, not continuous CLI-change polling.

AI installation through `manage-skills-hub` now defaults to installing and deploying to all detected, enabled tools, including eligible custom tools. Explicit tool selection, library-only requests, and project scope override the defaults. No eligible tools, shared-directory scope expansion, conflicts, and deployment failures are reported without claiming full success. At the CLI protocol level, `skills install` still only adds to the library; the official Skill follows it with explicit `skills deploy` calls.

Installation status no longer includes a manual refresh button. Automatic checks and retry after an initial check failure remain available.

View Skill remains available during background status checks without disabled-state flicker.

Reopening Settings preserves the last AI management status while refreshing locally in the background. The loading label appears only before a first successful check; refresh failures retain the previous result with an error message.

The AI management card uses a compact horizontal layout, with enabled status beside the title and no divider separating actions. Both states explain shared AI and desktop management. Usage examples and copy actions are removed; View Skill remains available, and Installation status is collapsed by default. Actions wrap in narrow windows without changing installation or sync behavior.

Project deployment is supported on macOS and Linux for Agents with a project skills directory. Windows exposes global scope only in CLI capabilities and desktop selection; project deployment or undeployment returns `PROJECT_SCOPE_UNSUPPORTED` (exit 4), including previews, without changing the library or Agent targets.

`skillshub-cli` provides local Skill management through the same Rust services and database as Skills Hub desktop: list/show/search, source update checks, local/Git installation and multi-Skill selection, import, deployment to explicit Agents, supported global/project scope, safe single/batch updates, tags, recoverable removal, status, and diagnostics. Installing a Skill only adds it to the central library; deployment is a separate explicit operation.

Enable AI management in Settings to download and verify the native CLI matching the production desktop version. First setup requires a network connection; a verified matching installation is reused offline and works after the desktop closes. CLI and official Skill updates require an explicit click in Settings; startup and status reads do not install or update components. No Node.js or npm installation is required.

The official Skill prefers the verified desktop CLI bridge, then PATH only when the desktop bridge directory is absent. A damaged or mismatched bridge stops and asks the user to repair it through Settings → AI management → Enable in one click. Settings now includes an AI management card: one click downloads or reuses the verified native CLI and installs the official Skill through the shared installer into detected, enabled tools. No Node.js or npm installation is required for this desktop flow. Installed Skills use the normal library controls; technical status stays collapsed. The dedicated Agent Access page is removed. Network and storage cards collapse while retaining summaries.

Use `--json` for stable structured results and `--dry-run` for supported mutation previews. Destructive operations require explicit user authorization and `--yes` confirmation; the flag does not grant the Agent authority beyond the user's request. Unowned target directories, ambiguous selections, unsupported scopes, simultaneous writers, and potentially destructive updates return actionable errors. There is no force-overwrite bypass. The official Skill never manipulates SQLite or Agent directories directly and never accepts credentials in arguments, URLs, or logs.

Device synchronization, automation/scheduled tasks, OAuth and credential management, proxy/storage/custom-tool configuration, application updates, recycle-bin restore, and permanent deletion remain desktop-only. An explicitly requested authenticated repository operation may use credentials already configured in the system secure store; ordinary local reads and startup do not access credentials.

Desktop and CLI share storage. Development, local test, and production desktop/CLI share the same database, central library, settings, cache, recycle bin, and write lock, affecting real Agent directories. Development credentials and CLI bridge binaries remain separate; authenticated operations require credentials in the development namespace. Old development data is preserved without automatic merging. Tests use temporary data and Agent directories and never import or clean production legacy state. Shared schema 6 remains readable by v0.10.1-compatible readers; newer unknown schemas are rejected before legacy migration, preserving database, WAL/SHM, backup, and legacy file bytes.

For legacy multi-Skill records without a source subpath, checks and updates require a unique source match. Missing or ambiguous matches return `INVALID_SOURCE` with reason `source_selection_required` (exit 2), preserving installed content and the JSON error protocol.
