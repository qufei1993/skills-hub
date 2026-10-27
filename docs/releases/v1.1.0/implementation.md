# Project Skills, presets and content updates

[Chinese reference](implementation.zh.md) · [Release index](README.md)

This document describes the current unreleased implementation on `feat/v1.1.0`. Package versions remain 1.0.1. The fixed [design](design.md), [review](design-review.md), [handoff](development-handoff.md) and prototype remain the historical `v1.1.0-design-r1` baseline; the behavior below includes subsequent user feedback. Future proposals and `work/prototypes/next/` are outside this implementation.

<a id="library-and-project-management"></a>
## Library and project management

My Skills manages installed content and its deployments. A Skill can be available globally and in several projects at the same time. The All, Global and Project views reflect those deployments; selecting a specific project narrows both the list and the displayed tool states. The searchable project menu includes existing deployments as well as recent projects, showing readable project names while retaining full paths.

Within a project, Manage targets changes where a Skill is deployed. Removing a project deployment preserves the central Skill and its global or other-project deployments. Combination editing and preset saving belong to Skill Presets; My Skills no longer duplicates those actions. AI setup and discovery guidance stay outside the Skill results, can be dismissed independently, and do not count as Skills. Add Skills remains the entry for importing local discoveries.

<a id="skill-presets"></a>
## Skill presets

A preset saves a named combination of installed Skills and the tools assigned to each member. It does not store a project destination. Creating, editing, duplicating or deleting a preset changes the definition, not existing deployments. Editing a preset does not automatically synchronize earlier applications, and removing a member from its definition does not uninstall that Skill. Users apply the revised combination explicitly; unrelated deployments remain in place. Deleted library members remain identifiable as unresolved preset references rather than being silently replaced.

Apply to is available directly on a preset card or its detail page. In one dialog, choose Global or one or more project directories, then apply the saved members and tools. Optional adjustments affect only that application. The backend checks targets before writing; an ordinary conflict-free application continues directly to its result. Conflicts and explicit removals retain a confirmation step. Conflict feedback identifies the affected tool, path and reason. Conflicting content stays unchanged; other items can proceed when any are executable. Returning to adjust the selection preserves the chosen projects and tools.

Application results record what happened at each location, including partial success and interrupted execution. Rechecking works on unfinished items in the same record and does not repeat completed items. Editing or deleting the preset does not rewrite that record. Undo removes only additions still attributable to the application; pre-existing or repaired deployments, later reuse, changed files and replaced targets are retained. A reused physical directory remains while another logical tool target needs it.

History retains the 100 most recently updated confirmed records whose items are all in terminal states. Pending, running, interrupted or otherwise uncertain records are retained separately so recovery remains possible. Cancelling an unexecuted preview removes its temporary record; a confirmed operation or evidence of a write is never removed by that cancellation. Pruning history does not delete installed files or later-use markers, but a pruned record can no longer be undone. Cleanup runs when records are saved or history is read; if another operation owns the write lock, reading remains available and cleanup is deferred. The limit is fixed, with no additional setting.

<a id="updates-and-recovery"></a>
## Updates and recovery

Check for updates explicitly compares the sources of enabled Skills. Entering the page or restarting the app reads the saved summary and timestamp without fetching sources. A check refreshes each Git repository and branch once per run, shares that work across its Skills, and compares content without preparing every full difference. Local sources are checked locally; Git checks depend on the network. Completion remains visible with checked counts, changed and unchanged counts, failures, timestamp and measured duration. The preceding list remains visible during another check, and one failed source does not hide successful results.

The pending list contains detected changes rather than the entire library. Update all acts on the current confirmed candidates; selecting items changes the action to Update selected. Viewing differences is optional: execution prepares and validates the required plans even when that view is skipped. Cached, cancelled or invalidated checks cannot authorize a direct update. A reviewed difference aligns line numbers, highlights additions and removals, folds unchanged context, and separates file-property changes from text changes. Updating supports cancellation and preserves partial outcomes and deployment failures for follow-up.

Content recovery is separate from deployment undo. Each Skill retains its latest successful update's pre-update snapshot, including content and relevant source/version metadata. Restore validates the current state before replacing content, preserves later deployments, and keeps a disabled Skill disabled. Personal edits block unsafe replacement; restored versions are protected from immediate automatic reapplication. Recovery snapshots appear in a separate tab. Scheduled updates remain an explicitly enabled feature with their own plan and execution record.

Source repair can rebind a moved or failed source without replacing central content, Skill identity or deployments. Missing-deployment repair uses the actual failing targets and the same preview and conflict checks. If failures span multiple scopes, the user chooses the intended scope explicitly.

<a id="data-integrity-and-compatibility"></a>
## Data integrity and compatibility

The central Skill UUID identifies content. A logical deployment is the Skill, tool, scope and canonical project path; the target UUID identifies its generation. Registered adapters determine physical paths, and shared physical paths do not imply shared logical ownership. Home expansion, path-alias handling and parent-directory checks preserve the existing deployment boundaries.

Preset definitions, operation journals and check summaries use additive workspace documents with the `schema.workspace` feature marker. The shared database remains at `PRAGMA user_version=6`; existing Skill and target identities are preserved. Presets are local only: this implementation adds neither preset synchronization nor preset CLI commands.

The backend owns preview IDs, expected state and execution receipts. It revalidates source, content, target identity and fingerprints before writes, using the shared cross-process operation lock. Intent and outcomes are journaled per item. Unknown execution results recover through the same journal rather than a new application that could claim earlier additions. Manual, batch, automatic and CLI updates share content-recovery protection; device sync and other writers remain subject to actual-state checks before later recovery or undo. Target-use markers survive history pruning so an older operation cannot regain undo rights.

<a id="implementation-map"></a>
## Implementation map

| Area | Owning implementation |
| --- | --- |
| Storage and compatibility | [core/workspace.rs](../../../src-tauri/src/core/workspace.rs) and [core/skill_store.rs](../../../src-tauri/src/core/skill_store.rs) own workspace documents, retention and target-use metadata. |
| Presets, execution and undo | [services/workspace.rs](../../../src-tauri/src/services/workspace.rs) coordinates plans, journals and recovery through the existing deployment services. |
| Source checks and content recovery | [services/content_check.rs](../../../src-tauri/src/services/content_check.rs), [core/content_recovery.rs](../../../src-tauri/src/core/content_recovery.rs) and [core/installer.rs](../../../src-tauri/src/core/installer.rs) own observations, content validation and replacement. |
| Desktop transport | [commands/workspace.rs](../../../src-tauri/src/commands/workspace.rs), [commands/content_check.rs](../../../src-tauri/src/commands/content_check.rs) and [lib.rs](../../../src-tauri/src/lib.rs) expose registered blocking-task wrappers. |
| User flows | [App.tsx](../../../src/App.tsx), [workspace components](../../../src/components/skills/workspace/) and [UpdatesPage.tsx](../../../src/components/skills/UpdatesPage.tsx) connect project management, presets and updates. UI copy is maintained in English, Simplified Chinese and Korean. |

<a id="verification-record"></a>
## Verification record

The final functional gate on October 27, 2026 passed `npm run check`: 72 frontend test files, 688 tests passed and 1 skipped; 776 Rust unit tests passed and 3 ignored; 2 compatibility tests and 24 CLI integration tests passed. Release-data checks, network boundaries, lint, TypeScript/Vite build, rustfmt and Clippy passed. `npm run version:check` and `git diff --check` also passed. The log is `/tmp/preset-retention-check.log`; the ignored live-network and performance tests are not counted as passes.

The current branch was built and started with `npm run tauri:dev`. Native write tests used disposable data and tool paths through the debug-only isolation entry documented in the [development guide](../../development.en.md#isolated-desktop-acceptance-testing), with matching hashes between the built executable and its temporary application bundle. The following evidence covers the stated flows; earlier layout checks are not presented as a repeated final matrix.

| Coverage | Observed result and evidence |
| --- | --- |
| Preset conflicts and retry | All-conflict and partial-conflict cases preserved modified files, showed the tool/path/reason, retained selections on return and excluded completed items from retry counts. The native pass included a 900-pixel window. Evidence: `/tmp/preset-conflict-feedback-acceptance.md`. |
| History and cancellation | 105 terminal records were reduced to the newest 100 while 3 pending/running/interrupted records remained. Returning or cancelling removed the unexecuted preview; 1,273 fixture files were unchanged. Evidence: `/tmp/preset-retention-acceptance.md` and `/tmp/skills-hub-history-retention-acceptance/native-evidence.json`. |
| Local and GitHub updates | Native review used 13 isolated samples, including 10 GitHub Skills from OpenAI, Anthropic and Vercel. Selected/all updates, optional differences and restoration were verified against file hashes; a real older GitHub revision updated to its default branch while unselected samples stayed unchanged. Evidence: `/tmp/skills-hub-direct-update/acceptance.md` and its verification JSON files. |
| Automated protection | Filesystem, SQLite and component regressions cover compatibility, mixed scopes, shared targets, stale plans, partial failure, restart/retry, later reuse, disabled recovery, permission/directory changes, history retention and cleanup failures. Relevant suites are [workspace services](../../../src-tauri/src/services/tests/workspace.rs), [content checks](../../../src-tauri/src/services/tests/content_check.rs), [installer](../../../src-tauri/src/core/tests/installer.rs) and [preset UI](../../../src/components/skills/workspace/PresetPage.test.tsx). |
| Earlier native coverage | The preceding revisions covered create/edit/duplicate/delete, apply/undo, source/deployment repair, keyboard navigation, 900/1180-pixel layouts, light/dark themes and localized screens. Evidence remains under `/tmp/skills-hub-v110-verification/`, `/tmp/skills-hub-v110-preset-redesign/` and `/tmp/skills-hub-pro-max-redesign/`; it is scoped to those revisions. |

These local logs and screenshots are supporting evidence, not portable repository assets. Windows/Linux native execution and user-task timing targets have not been validated on this macOS host. The exact crash window between a filesystem/target commit and journal persistence was not fault-injected. Fixed design and next-draft hashes remain unchanged.

This verification covers the working branch, not release approval. Before merging, reconcile the branch with the known mainline status/tool filters and immediate tool hints so the new project workflows retain those published behaviors. That integration remains outside this documentation cleanup; no version bump or publication is implied.
