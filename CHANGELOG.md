# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

## [0.9.1] - 2026-08-28

### Changed
- **Bilingual GitHub release notes**: The release workflow now extracts both Chinese and English changelog entries, including localized platform installation notes in GitHub Releases and app updater metadata.
- **App update completion flow**: Available updates now use a prominent title-bar action that expands from an icon to a localized label on hover or keyboard focus. After installation, users can restart immediately from the completion dialog or close it and restart later from the persistent title-bar action.

### Fixed
- **Skill detail compatibility on older macOS versions**: Opening a Skill Markdown file no longer blanks the entire app in WKWebView versions that do not support regular-expression lookbehind. The detail renderer now detects this capability and falls back to standard Markdown while preserving full GFM rendering on supported systems ([#108](https://github.com/qufei1993/skills-hub/issues/108)).
- **Tag rename dialogs in Tauri WebView**: Tag rename now uses an in-app input dialog instead of the unsupported browser `window.prompt`, restoring tag renaming on macOS. The remaining shared-directory confirmation also uses the existing in-app dialog instead of `window.confirm` ([#109](https://github.com/qufei1993/skills-hub/issues/109)).
- **Responsive Skill detail workspace**: Replaced the fixed-width detail drawer with a full workspace view that adapts to the main window, keeps the file tree and Markdown content readable, and presents scope, synced tools, tags, source, update time, and file count in a compact responsive header. Source paths can be copied directly, while duplicated `name` and `description` frontmatter fields are omitted from the Markdown metadata table ([#111](https://github.com/qufei1993/skills-hub/issues/111)).
- **Accurate Skill status**: Skill details, cards, and dashboard summaries now distinguish source errors, healthy syncs, partial failures, failed syncs, and not-yet-synced states. Source update and target sync failures are persisted, the dashboard links active issues to their update details, and clicking a failed tool retries the sync instead of attempting to disable it. Skill cards and details now use the same currently detected tool set, with cards showing the synchronized count explicitly.
- **Windows junction cleanup and existing Skill imports**: Disabling, unsyncing, or deleting a Skill now removes Windows directory junctions without touching their targets. Discovery can also import byte-identical copies that already exist in central storage by reusing the managed record, while normal duplicate local installs and conflicting content remain rejected ([#110](https://github.com/qufei1993/skills-hub/pull/110)).
- **Windows automatic update schedules**: The default 24-hour interval and other whole-day intervals now register with `schtasks` as daily schedules instead of invalid hourly modifiers above 23. Supported minute and hourly intervals retain their native schedule types, while unsupported long non-whole-day intervals are rejected before task creation ([#102](https://github.com/qufei1993/skills-hub/issues/102)).

## [0.9.0] - 2026-08-23

### Added
- **DeepSeek Harness tool adapter**: Added global and project-level Skill sync through `~/.dsh/skills` and `.dsh/skills`. Skills Hub detects a default DeepSeek Harness installation through `~/.dsh` and displays the DeepSeek product icon throughout tool management and sync flows.
- **Discovery scan settings**: Added persistent per-directory scan source controls with a permanent Settings entry and a contextual shortcut beside discovered Skills. Saving refreshes discovery results without changing managed or synced Skills.

### Changed
- **Built-in tool catalog**: Increased the built-in adapter count to 47 and documented DeepSeek Harness path rules in the English and Chinese support matrices.
- **Discovery validation**: Tool directories are now reported as importable Skills only when they contain a `SKILL.md` file, reducing unrelated discovery results.

## [0.8.1] - 2026-08-06

### Fixed
- **Central storage guidance**: When a local Skill import conflicts with a folder placed directly in the Skills Hub-managed central storage directory, the error now explains that the directory is managed by the app and tells the user how to move and re-import the Skill ([#103](https://github.com/qufei1993/skills-hub/issues/103)).

## [0.8.0] - 2026-07-17

### Added
- **Card and list views**: My Skills can switch between responsive card and compact list layouts while preserving selection and filtering state.
- **Collapsible desktop navigation**: The sidebar can collapse to icon-only navigation, giving the workspace more room at smaller window sizes.
- **Recognizable tool identities**: Built-in AI coding tools use their product icons consistently across tool management, installation, and Skill sync status.
- **Custom tool identity and sync modes**: Custom tools support avatars and explicit automatic, symbolic link, Windows junction, or copy modes. Legacy configurations default to no avatar and automatic mode.
- **Custom tool editing**: Existing custom tools can update their name, avatar, global/project directories, and sync mode. Active targets migrate to the new directory or representation.
- **Title bar update status**: The app checks for updates after startup and shows an actionable icon when a new version is available, reusing the existing release notes and installation flow.

### Changed
- **Desktop UI redesign**: My Skills, Explore, Tags, Tools, Updates, Settings, and installation flows now share a consistent desktop layout, spacing system, scrolling behavior, and responsive rules.
- **Installation workflow**: Online, Git repository, and local folder installation use a unified structure with visible tag, tool, scope, summary, and action areas.
- **Skills Hub branding**: Replaced the application logo and generated desktop icons with the new balanced Skills Hub mark for consistent use in the app, installer, and operating system launcher.
- **Tool card action hierarchy**: Custom tool cards keep enable/disable in the top-right corner and move edit/delete actions into a separate footer.
- **GitHub project entry**: The open-source project link moved to the GitHub & network settings section and opens in the system default browser.
- **Skill card column sizing**: Card view keeps a stable column width instead of stretching a single filtered result across the content area.

### Fixed
- **Startup loading overlay**: Initial discovery of importable skills now runs silently in the background instead of briefly showing the “Installing Skills” dialog.
- **Safe custom target migration**: Directory and sync-mode changes protect shared physical targets and reject invalid configuration when a destination conflicts or migration fails.
- **External link permission**: Added a Tauri opener permission scoped to the Skills Hub repository, fixing the GitHub project page failing to open.

## [0.7.1] - 2026-07-10

### Fixed
- **Unified network proxy**: App update checks, update downloads and installation, release notes, and Explore online search now honor the configured network proxy ([PR #91](https://github.com/qufei1993/skills-hub/pull/91)).

## [0.7.0] - 2026-07-05

### Added
- **Install scope selection**: Choose global or project-level sync before installing a skill, including batch installs from local folders and Git repositories.
- **Bulk skill management**: Select multiple skills from My Skills and apply batch operations for tags, target tools, enable/disable, and delete.
- **Skill enable/disable controls**: Temporarily disable a skill without deleting it, then re-enable it later with its previous tool sync settings restored.
- **Management Center**: Manage tags, tool configuration, and Skills auto update from one dedicated area.
- **Tool management**: Enable or disable built-in tool targets so unused tools no longer appear in skill cards, install flows, or sync actions.
- **Custom tool directories**: Add custom tool targets with global skills directories and optional project-level skills directories.
- **Scheduled Skills auto update**: Configure system-level background updates for managed skills, trigger an immediate update, and review run results in the Management Center.
- **GitHub network proxy settings**: Configure a local proxy for GitHub API requests, featured skills, GitHub downloads, and Git update flows.

### Changed
- **Settings page organization**: Settings now focuses on application preferences such as interface language, appearance, storage, cache, GitHub access, network proxy, and app updates.
- **Management UI layout**: Tags, tools, and update pages now share consistent workspace spacing and tab navigation.
- **Bulk tool sync behavior**: Batch tool changes now apply the selected tool list as the final target state for the selected skills.
- **Auto update results**: Update status, counts, failures, start time, finish time, and duration are now shown inline instead of relying on a separate progress dialog.
- **Install modal layout**: The add-skill flow uses a compact install scope selector and keeps modal actions visible when candidate lists are long.

### Fixed
- **Local batch install review**: Fixed local skill directory batch installation and candidate selection edge cases.
- **Project scope grouping**: Tools that share the same project skills directory are grouped by target directory so project sync remains consistent.
- **Modal overlap**: Fixed cases where long modal content could overlap or hide footer actions.
- **Scheduler lint stability**: Cleaned up platform-specific scheduler code paths so all Rust lint checks pass across targets.

## [0.6.3] - 2026-06-27

### Fixed
- **Antigravity global sync path**: Updated Antigravity 2.0 global skill sync to `~/.gemini/config/skills` so synced skills are discovered by current Antigravity versions ([#79](https://github.com/qufei1993/skills-hub/issues/79)).
- **Windows updater metadata**: Added Windows updater signatures and `windows-x86_64` / `windows-aarch64` platform entries to `updater.json`, fixing update checks on Windows ([#78](https://github.com/qufei1993/skills-hub/issues/78), [PR #82](https://github.com/qufei1993/skills-hub/pull/82)).

## [0.6.2] - 2026-06-19

### Added
- **WorkBuddy tool adapter**: Added global skill sync support via `~/.workbuddy/skills/` ([PR #73](https://github.com/qufei1993/skills-hub/pull/73)).
- **CodeWhale tool adapter**: Added global and project-level skill sync via `~/.codewhale/skills/` and `.codewhale/skills/` ([#70](https://github.com/qufei1993/skills-hub/issues/70), [PR #74](https://github.com/qufei1993/skills-hub/pull/74)).
- **Claude Code plugin skill discovery**: Existing user-level Claude Code plugin skills can now be discovered and imported, including custom skill paths declared in `.claude-plugin/plugin.json` ([#69](https://github.com/qufei1993/skills-hub/issues/69), [PR #75](https://github.com/qufei1993/skills-hub/pull/75)).

## [0.6.1] - 2026-05-16

### Fixed
- **Window close behavior**: Closing the main window now exits the app instead of hiding it in the background, fixing cases where the app kept running but could not be reopened from the Dock or taskbar ([PR #68](https://github.com/qufei1993/skills-hub/pull/68)).

## [0.6.0] - 2026-05-05

### Added
- **Skill tags**: Add custom tags to managed skills for easier organization and filtering.
- **Tags page**: Manage tags from a dedicated Tags page, including create, rename, delete, and quick navigation back to filtered My Skills views.
- **Tag filtering**: Filter My Skills by one or more tags with OR matching, including a virtual `Untagged` filter for skills without tags.
- **Per-skill tag editor**: Edit a skill's tag assignments directly from the skill card.
- **Import search**: Search discovered skill candidates by name, description, or path before importing from a local directory or Git repository.

### Changed
- **My Skills filter bar**: Removed the manual refresh button; install, delete, sync, and tag-edit flows already refresh the list automatically.

### Fixed
- **Chinese filter bar layout**: Removing the refresh button fixes the cramped button layout in Chinese.
- **Discovered skills review**: The discovered skills review dialog now supports search and keeps selection counts aligned with filtered results.

## [0.5.0] - 2026-04-16

### Added
- **Project-level skill sync**: Skills can now be synced to selected project directories instead of only global tool directories.
- **Skill scope controls**: My Skills cards now show a scope badge (`Global` / project count) and include a scope modal for switching between global and project sync.
- **Scope filtering**: My Skills can be filtered by All / Global / Project scope.
- **Hermes Agent adapter**: Added global sync support for Hermes Agent via `~/.hermes/skills` ([#54](https://github.com/qufei1993/skills-hub/issues/54)).

### Changed
- **My Skills filter bar**: The section title now displays the total skill count, search is more compact, and filter controls stay on one line in the default window.
- **Default window size**: Increased the default desktop window from `800x600` to `960x680`.
- **macOS close behavior**: Closing the main window now hides it instead of quitting the app; reopening from the Dock restores and focuses the window.
- **Project sync support matrix**: Project-level sync is now treated as an explicit per-tool capability; tools without a confirmed project skills directory are global-only.

### Fixed
- **Import takeover for identical skills**: Importing an existing skill can now safely take over same-name targets when the content hash matches.
- **Unsynced tool re-enable entry**: Tool buttons that were unsynced from a skill remain visible so they can be re-enabled.
- **SKILL.md metadata parsing**: YAML block scalar descriptions in frontmatter now render correctly in skill cards and detail views.

## [0.4.3] - 2026-04-11

### Added
- **Copaw tool adapter**: Support for Copaw AI coding tool (thanks @LeonDevLifeLog [PR#50](https://github.com/qufei1993/skills-hub/pull/50)).

### Fixed
- **Git skill install & frontmatter rendering**: Fixed issues with Git-based skill installation and frontmatter metadata rendering.
- **Git skill discovery for container paths**: Fixed skill discovery failing when repository uses container-style directory paths.

## [0.4.2] - 2026-04-06

### Fixed
- **New tools modal style**: "New tools detected" dialog now uses consistent header/footer structure (`modal-header` + `modal-footer`) matching all other modals, fixing missing padding and border separators ([#46](https://github.com/qufei1993/skills-hub/issues/46)).
- **Git skill name derivation**: Installing a Git skill from a repo root (subpath `"."`) now correctly derives the name from the repository URL instead of using `"."` as the display name.

## [0.4.1] - 2026-03-21

### Added
- **Frontmatter metadata table**: Markdown files with YAML frontmatter now render a GitHub-style metadata table at the top of the skill detail view.

## [0.4.0] - 2026-03-20

### Added
- **In-app update check**: Check for updates directly within Settings, download and install without leaving the app ([#33](https://github.com/qufei1993/skills-hub/issues/33)).
- **QoderWork tool adapter**: Support for QoderWork desktop AI agent (`~/.qoderwork/skills/`) ([#34](https://github.com/qufei1993/skills-hub/issues/34)).

### Changed
- **Settings promoted to full page**: Settings moved from a modal dialog to a dedicated page view, consistent with My Skills / Explore navigation pattern.
- **Curated skills aggregation**: Explore page now sources skills from a curated list of 7 high-quality repositories.

### Fixed
- Language toggle briefly flashing "Installing Skills..." loading overlay on Explore page.

## [0.3.0] - 2026-03-15

### Added
- **Explore page**: Explore promoted from a modal tab to an independent page with My Skills / Explore top-level navigation.
- **Featured skills**: Explore page displays curated skills from ClawHub API (updated daily via GitHub Actions) with frontend filtering and one-click install.
- **Online skill search**: Real-time search via skills.sh API (triggered at 2+ characters, 500ms debounce), results deduplicated against the featured list and shown in separate sections.
- **Skill detail view**: Click a skill name to browse its files with a file tree, Markdown rendering (GFM + frontmatter stripping), and syntax highlighting (40+ languages, light/dark theme adaptive).
- **Skill description field**: Description extracted from SKILL.md frontmatter at install time, stored in database, and displayed on My Skills cards.
- **GitHub Token setting**: Optional GitHub Token input in settings to increase API rate limit from 60 to 5,000 requests/hour.
- **MoltBot tool adapter**: Added standalone MoltBot tool support after OpenClaw rename/split.

### Fixed
- Git install deriving skill name as "skills" when URL points to a `skills/` subdirectory, causing duplicated sync paths ([#28](https://github.com/qufei1993/skills-hub/issues/28)).
- GitHub API rate-limit errors now display the exact reset time instead of a generic message.
- Windows "Access Denied" OS error 5 when syncing to tools ([#20](https://github.com/qufei1993/skills-hub/issues/20)).
- Git repo directory structures not correctly recognized as skills ([#18](https://github.com/qufei1993/skills-hub/issues/18), [#8](https://github.com/qufei1993/skills-hub/issues/8)).
- Repos using `.claude/skills/` directory format not detected ([#27](https://github.com/qufei1993/skills-hub/issues/27)).
- OpenClaw path updated from `.moltbot/skills` to `.openclaw/skills` ([#29](https://github.com/qufei1993/skills-hub/issues/29)).

### Changed
- My Skills list: tool badges now only show synced tools, collapsing to `+N more` beyond 5.
- Manual Add modal simplified to Local Directory / Git Repository tabs only (Explore tab removed).
- Multi-skill repo online install now auto-matches target skill (exact → unique-contains → fallback to manual picker).

## [0.2.0] - 2026-02-01

### Added
- **Windows platform support**: Full support for Windows build and release (thanks @jrtxio [PR#6](https://github.com/qufei1993/skills-hub/pull/6)).
- Support and display for many new tools (e.g., Kimi Code CLI, Augment, OpenClaw, Cline, CodeBuddy, Command Code, Continue, Crush, Junie, iFlow CLI, Kiro CLI, Kode, MCPJam, Mistral Vibe, Mux, OpenClaude IDE, OpenHands, Pi, Qoder, Qwen Code, Trae/Trae CN, Zencoder, Neovate, Pochi, AdaL).
- UI confirmation and linked selection for tools that share the same global skills directory.
- Local import multi-skill discovery aligned with Git rules, with a selection list and invalid-item reasons.
- New local import commands for listing candidates and installing a selected subpath with SKILL.md validation.

### Changed
- Antigravity global skills directory updated to `~/.gemini/antigravity/global_skills`.
- OpenCode global skills directory corrected to `~/.config/opencode/skills`.
- Tool status now includes `skills_dir`; frontend tool list/sync is driven by backend data and deduped by directory.
- Sync/unsync now updates records across tools sharing a skills directory to avoid duplicate filesystem work and inconsistent state.
- Local import flow now scans candidates first; single valid candidate installs directly, multi-candidate opens selection.

## [0.1.1] - 2026-01-26

### Changed
- GitHub Actions release workflow for macOS packaging and uploading `updater.json` (`.github/workflows/release.yml`).
- Cursor sync now always uses directory copy due to Cursor not following symlinks when discovering skills: https://forum.cursor.com/t/cursor-doesnt-follow-symlinks-to-discover-skills/149693/4
- Managed skill update now re-syncs copy-mode targets using copy-only overwrite, and forces Cursor targets to copy to avoid accidental relinking.

## [0.1.0] - 2026-01-25

### Added
- Initial release of Skills Hub desktop app (Tauri + React).
- Central repository for Skills; sync to multiple AI coding tools (symlink/junction preferred, copy fallback).
- Local import from folders.
- Git import via repository URL or folder URL (`/tree/<branch>/<path>`), with multi-skill selection and batch install.
- Sync and update: copy-mode targets can be refreshed; managed skills can be updated from source.
- Migration intake: scan existing tool directories, import into central repo, and one‑click sync.
- New tool detection and optional sync.
- Basic settings: storage path, language, and theme.
- Git cache with cleanup (days) and freshness window (seconds).

### Build & Release
- Local packaging scripts for macOS (dmg), Windows (msi/nsis), Linux (deb/appimage).
- GitHub Actions build validation and tag-based draft releases (release notes pulled from `CHANGELOG.md`).

### Performance
- Git import and batch install optimizations: cached clones reduce repeated fetches; timeouts and non‑interactive git improve stability.

[Unreleased]: https://github.com/qufei1993/skills-hub/compare/v0.9.1...HEAD
[0.9.1]: https://github.com/qufei1993/skills-hub/compare/v0.9.0...v0.9.1
[0.9.0]: https://github.com/qufei1993/skills-hub/compare/v0.8.1...v0.9.0
[0.8.1]: https://github.com/qufei1993/skills-hub/compare/v0.8.0...v0.8.1
[0.8.0]: https://github.com/qufei1993/skills-hub/compare/v0.7.1...v0.8.0
[0.7.1]: https://github.com/qufei1993/skills-hub/compare/v0.7.0...v0.7.1
