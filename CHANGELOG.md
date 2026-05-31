# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

## [0.6.1] - 2026-05-16

### Fixed
- **Window close behavior**: Closing the main window now exits the app instead of hiding it in the background, fixing cases where the app kept running but could not be reopened from the Dock or taskbar ([PR #68](https://github.com/AlwaysCodeHQ/skills-syncer/pull/68)).

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
- **Hermes Agent adapter**: Added global sync support for Hermes Agent via `~/.hermes/skills` ([#54](https://github.com/AlwaysCodeHQ/skills-syncer/issues/54)).

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
- **Copaw tool adapter**: Support for Copaw AI coding tool (thanks @LeonDevLifeLog [PR#50](https://github.com/AlwaysCodeHQ/skills-syncer/pull/50)).

### Fixed
- **Git skill install & frontmatter rendering**: Fixed issues with Git-based skill installation and frontmatter metadata rendering.
- **Git skill discovery for container paths**: Fixed skill discovery failing when repository uses container-style directory paths.

## [0.4.2] - 2026-04-06

### Fixed
- **New tools modal style**: "New tools detected" dialog now uses consistent header/footer structure (`modal-header` + `modal-footer`) matching all other modals, fixing missing padding and border separators ([#46](https://github.com/AlwaysCodeHQ/skills-syncer/issues/46)).
- **Git skill name derivation**: Installing a Git skill from a repo root (subpath `"."`) now correctly derives the name from the repository URL instead of using `"."` as the display name.

## [0.4.1] - 2026-03-21

### Added
- **Frontmatter metadata table**: Markdown files with YAML frontmatter now render a GitHub-style metadata table at the top of the skill detail view.

## [0.4.0] - 2026-03-20

### Added
- **In-app update check**: Check for updates directly within Settings, download and install without leaving the app ([#33](https://github.com/AlwaysCodeHQ/skills-syncer/issues/33)).
- **QoderWork tool adapter**: Support for QoderWork desktop AI agent (`~/.qoderwork/skills/`) ([#34](https://github.com/AlwaysCodeHQ/skills-syncer/issues/34)).

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
- Git install deriving skill name as "skills" when URL points to a `skills/` subdirectory, causing duplicated sync paths ([#28](https://github.com/AlwaysCodeHQ/skills-syncer/issues/28)).
- GitHub API rate-limit errors now display the exact reset time instead of a generic message.
- Windows "Access Denied" OS error 5 when syncing to tools ([#20](https://github.com/AlwaysCodeHQ/skills-syncer/issues/20)).
- Git repo directory structures not correctly recognized as skills ([#18](https://github.com/AlwaysCodeHQ/skills-syncer/issues/18), [#8](https://github.com/AlwaysCodeHQ/skills-syncer/issues/8)).
- Repos using `.claude/skills/` directory format not detected ([#27](https://github.com/AlwaysCodeHQ/skills-syncer/issues/27)).
- OpenClaw path updated from `.moltbot/skills` to `.openclaw/skills` ([#29](https://github.com/AlwaysCodeHQ/skills-syncer/issues/29)).

### Changed
- My Skills list: tool badges now only show synced tools, collapsing to `+N more` beyond 5.
- Manual Add modal simplified to Local Directory / Git Repository tabs only (Explore tab removed).
- Multi-skill repo online install now auto-matches target skill (exact → unique-contains → fallback to manual picker).

## [0.2.0] - 2026-02-01

### Added
- **Windows platform support**: Full support for Windows build and release (thanks @jrtxio [PR#6](https://github.com/AlwaysCodeHQ/skills-syncer/pull/6)).
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
- Initial release of SkillSyncer desktop app (Tauri + React).
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
