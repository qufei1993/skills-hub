# Skills Hub (Tauri Desktop)

A cross-platform desktop app (Tauri + React) for installing, organizing, updating, and syncing Agent Skills to multiple AI coding tools' global or project-level skills directories. Skills Hub prefers symlink/junction and automatically falls back to copy when needed: "Install once, sync everywhere".

## Documentation

- English (default): `README.md` (this file)
- 中文：[`docs/README.zh.md`](docs/README.zh.md)

## Why Skills Hub

AI coding tools increasingly use their own skills directories and installation flows. Maintaining those directories manually can quickly become messy: the same skill gets copied many times, update sources become unclear, tool activation states drift, and bulk cleanup takes too much effort.

Skills Hub installs skills into one central repository, then syncs them to tools such as Claude Code, Codex, Cursor, OpenCode, and Antigravity based on your choices. You can tag skills, choose global or project scope, update tool targets in bulk, and let the system update Git and local-source skills on a schedule.

## Key Features

- **AI management**: Install, update, and organize skills through conversations with your AI coding tool, using the same library as the desktop app.
- **Centralized library**: Install skills into one central repository instead of scattering copies across tool folders.
- **Explore and install**: Install from curated lists, online search, local folders, or Git repositories.
- **Multi-tool sync**: Sync skills to different AI coding tools by global or project scope.
- **Multi-device library sync**: Keep Skill content, descriptions, and tags aligned across computers through a GitHub, GitLab, or Gitee repository.
- **Local recycle bin**: Recover deleted Skills and their saved local configuration for up to 30 days.
- **Bulk management**: Apply tags, tool targets, enabled state, or delete operations to many skills at once.
- **Tag organization**: Filter, group, and maintain skills with tags.
- **Tool management**: Enable built-in tool targets or add custom skills directories.
- **Automatic updates**: Update Git and local-source skills on a schedule, with visible failure details.
- **Detail view**: Browse skill file trees, Markdown content, and code snippets.
- **Migration**: Scan and import existing local skills into one managed library.
- **Discovery controls**: Choose which installed tool directories participate in import discovery.
- **Multilingual interface**: Use Skills Hub in English, Simplified Chinese, or Korean.

## Interface Preview

### My Skills — Managed Skills and Bulk Actions

My Skills provides card and list views for each managed skill's source, tags, sync scope, target tools, and enabled state. The toolbar supports scope filtering, sorting, tag filtering, search, and bulk actions.

When Skills Hub discovers importable Skills in installed tool directories, the discovery banner lets you review them or open Scan settings. Scan sources are independent from sync targets, persist across restarts, and remain accessible from Settings. Only directories containing `SKILL.md` are shown as importable Skills.

![My Skills card view](docs/assets/skills-library-cards.png)

![My Skills list view with bulk actions](docs/assets/skills-library-bulk-actions.png)

### Skill Details — Inspect Content and Sync Targets

Open a Skill to view its source, tags, scope, and sync status. Browse its file tree and read Markdown or code without leaving the app.

![Skill details with file tree and Markdown preview](docs/assets/skill-details.png)

### Explore — Curated Skills and Online Search

Explore brings together curated repository skills and online search. After clicking Install, you can choose tags, install scope, and target tools.

![Explore online skills](docs/assets/skills-explore.png)

### Add Skill — Set Tags, Scope, and Tools Before Installation

Manual add supports both local folders and Git repositories. Before installing, you can assign tags, choose global or project scope, and choose which tools to sync to.

![Add a skill from a Git repository](docs/assets/skills-install-git.png)

### Device Sync — Keep Your Skill Library Aligned

Device Sync connects Skills Hub to a GitHub, GitLab, or Gitee repository. You can preview changes, synchronize manually, after startup, at an interval, or at a daily time, inspect per-Skill history, and resolve real conflicts without exposing device-local paths, tool targets, or credentials.

Start by authorizing your account or configuring Token/SSH access, then choose a repository and complete the first sync.

![Device Sync provider selection and account authorization](docs/assets/device-sync-authorization.png)

Once connected, review the latest sync status, automatic sync schedule, and per-Skill changes from the dashboard.

![Device Sync dashboard with status, schedule, and change history](docs/assets/device-sync-dashboard.png)

### Recycle Bin — Recover Deleted Skills

Deleted Skills remain in a local recycle bin for 30 days. Restoring a Skill brings back its files and saved local configuration when the original locations are still available.

![Local recycle bin with deleted Skill details and restore action](docs/assets/recycle-bin.png)

### Tools — Built-in and Custom Tool Management

Tools shows detected and enabled AI coding tools with recognizable product icons. You can enable built-in targets or create and edit custom tools with an avatar, skills directories, and an explicit sync mode.

![Built-in and custom tool management](docs/assets/tools-overview.png)

### Updates — Scheduled Runs and Results

Updates can register a system-level schedule that keeps Git and local-source skills current while the app is closed. You can also update immediately and review checked, updated, and failed counts from the latest run.

![Scheduled skill updates and run results](docs/assets/skills-scheduled-updates.png)

### Settings — App-Level Preferences

Settings keeps app-level preferences such as interface language, appearance, AI management, storage and cache, GitHub token, network proxy, and app updates.

![Application preferences](docs/assets/app-settings.png)

### AI Management — Manage Skills Through Conversations (New in v0.11.0)

Click **Settings → AI Management → Enable in one click** to download and verify the matching CLI and install the official Skill and configure the terminal command. Open a new terminal to use `skillshub-cli`; restart the terminal app if needed. No Node.js or npm is required. Terminal configuration supports Bash/Zsh on macOS/Linux and user PATH on Windows. The desktop installer does not include the CLI. First-time setup requires a network connection; a verified matching installation works offline. Updates are applied when you click Update in Settings, rather than during startup. CLI resources are published separately in [skills-hub-cli releases](https://github.com/qufei1993/skills-hub-cli/releases).

![Enable AI management in Settings](docs/assets/ai-management-setup.png)

Find `manage-skills-hub` in My Skills, then ask your AI tool to install, update, or organize Skills using the same library as the desktop app.

![Official management Skill installed and synced to tools](docs/assets/ai-management-installed.png)

For users without the desktop app, see [Standalone CLI Installation](#standalone-cli-installation) (not recommended).

## Workflow

1. Install a skill from Explore, a local folder, or a Git repository.
2. Choose tags, sync scope, and target tools before installation.
3. Skills Hub stores the skill in the central repository, which defaults to `~/.skillshub`.
4. Skills Hub syncs it to global skills directories or project-level skills directories based on each tool's rules.
5. Optionally connect Device Sync to keep the portable Skill library aligned across computers.
6. Later, you can organize, enable/disable, delete, recover, or bulk update skills from My Skills and Management Center.

## Supported AI Coding Tools

Skills Hub includes 48 built-in tool adapters and supports custom skills directories from Management Center. Project skills directories are relative to the selected project root. Tools marked `N/A` do not have a confirmed project-level skills directory and are supported for global sync only.

| tool key | Display name | global skills dir (relative to `~`) | project skills dir (relative to project) | detected if exists (relative to `~`) |
| --- | --- | --- | --- | --- |
| `cursor` | Cursor | `.cursor/skills` | `.agents/skills` | `.cursor` |
| `claude_code` | Claude Code | `.claude/skills` | `.claude/skills` | `.claude` |
| `codex` | Codex | `.codex/skills` | `.agents/skills` | `.codex` |
| `deepseek_harness` | DeepSeek Harness | `.dsh/skills` | `.dsh/skills` | `.dsh` |
| `zcode` | ZCode | `.zcode/skills` | `.zcode/skills` | `.zcode` |
| `opencode` | OpenCode | `.config/opencode/skills` | `.agents/skills` | `.config/opencode` |
| `antigravity` | Antigravity | `.gemini/config/skills` | `.agents/skills` | `.gemini/config` |
| `amp` | Amp | `.config/agents/skills` | `.agents/skills` | `.config/agents` |
| `kimi_cli` | Kimi Code CLI | `.kimi-code/skills` (or `$KIMI_CODE_HOME/skills`) | `.kimi-code/skills` | `.kimi-code` (or `$KIMI_CODE_HOME`) |
| `augment` | Augment | `.augment/skills` | `.augment/skills` | `.augment` |
| `openclaw` | OpenClaw | `.openclaw/skills` | `skills` | `.openclaw` |
| `copaw` | Copaw | `.copaw/skill_pool` | `.copaw/skill_pool` | `.copaw` |
| `cline` | Cline | `.agents/skills` | `.agents/skills` | `.agents` |
| `codebuddy` | CodeBuddy | `.codebuddy/skills` | `.codebuddy/skills` | `.codebuddy` |
| `codewhale` | CodeWhale | `.codewhale/skills` | `.codewhale/skills` | `.codewhale` |
| `workbuddy` | WorkBuddy | `.workbuddy/skills` | `N/A` | `.workbuddy` |
| `command_code` | Command Code | `.commandcode/skills` | `.commandcode/skills` | `.commandcode` |
| `continue` | Continue | `.continue/skills` | `.continue/skills` | `.continue` |
| `crush` | Crush | `.config/crush/skills` | `.crush/skills` | `.config/crush` |
| `junie` | Junie | `.junie/skills` | `.junie/skills` | `.junie` |
| `iflow_cli` | iFlow CLI | `.iflow/skills` | `.iflow/skills` | `.iflow` |
| `kiro_cli` | Kiro CLI | `.kiro/skills` | `.kiro/skills` | `.kiro` |
| `kode` | Kode | `.kode/skills` | `.kode/skills` | `.kode` |
| `mcpjam` | MCPJam | `.mcpjam/skills` | `.mcpjam/skills` | `.mcpjam` |
| `mistral_vibe` | Mistral Vibe | `.vibe/skills` | `.vibe/skills` | `.vibe` |
| `mux` | Mux | `.mux/skills` | `.mux/skills` | `.mux` |
| `openclaude` | OpenClaude IDE | `.openclaude/skills` | `.openclaude/skills` | `.openclaude` |
| `openhands` | OpenHands | `.openhands/skills` | `.openhands/skills` | `.openhands` |
| `pi` | Pi | `.pi/agent/skills` | `.pi/skills` | `.pi` |
| `qoder` | Qoder | `.qoder/skills` | `.qoder/skills` | `.qoder` |
| `qoderwork` | QoderWork | `.qoderwork/skills` | `.qoderwork/skills` | `.qoderwork` |
| `qwen_code` | Qwen Code | `.qwen/skills` | `.qwen/skills` | `.qwen` |
| `trae` | Trae | `.trae/skills` | `.trae/skills` | `.trae` |
| `trae_cn` | Trae CN | `.trae-cn/skills` | `.trae/skills` | `.trae-cn` |
| `zencoder` | Zencoder | `.zencoder/skills` | `.zencoder/skills` | `.zencoder` |
| `neovate` | Neovate | `.neovate/skills` | `.neovate/skills` | `.neovate` |
| `pochi` | Pochi | `.pochi/skills` | `.pochi/skills` | `.pochi` |
| `adal` | AdaL | `.adal/skills` | `.adal/skills` | `.adal` |
| `kilo_code` | Kilo Code | `.kilocode/skills` | `.kilocode/skills` | `.kilocode` |
| `roo_code` | Roo Code | `.roo/skills` | `.roo/skills` | `.roo` |
| `goose` | Goose | `.config/goose/skills` | `.goose/skills` | `.config/goose` |
| `gemini_cli` | Gemini CLI | `.gemini/skills` | `.agents/skills` | `.gemini` |
| `github_copilot` | GitHub Copilot | `.copilot/skills` | `.agents/skills` | `.copilot` |
| `clawdbot` | Clawdbot | `.clawdbot/skills` | `.clawdbot/skills` | `.clawdbot` |
| `droid` | Droid | `.factory/skills` | `.factory/skills` | `.factory` |
| `windsurf` | Windsurf | `.codeium/windsurf/skills` | `.windsurf/skills` | `.codeium/windsurf` |
| `moltbot` | MoltBot | `.moltbot/skills` | `.moltbot/skills` | `.moltbot` |
| `hermes_agent` | Hermes Agent | `.hermes/skills` | N/A | `.hermes` |

See [`src-tauri/src/core/tool_adapters/mod.rs`](src-tauri/src/core/tool_adapters/mod.rs) for the complete path rules and detection logic.

## Standalone CLI Installation

The desktop app is the recommended way to install and manage the CLI. This alternative is only for users who do not use the desktop app.

<details>
<summary>Terminal-only installation (not recommended)</summary>

For terminal-only use without the desktop app, copy the command for your system below. No Node.js, npm, or administrator access is required.

**macOS / Linux** (Intel/AMD x64 or ARM64):

```bash
curl -fsSL https://raw.githubusercontent.com/qufei1993/skills-hub/main/scripts/install-cli.sh | bash
```

**Windows x64** (PowerShell):

```powershell
irm https://raw.githubusercontent.com/qufei1993/skills-hub/main/scripts/install-cli.ps1 | iex
```

The installer selects the latest stable release from the public `qufei1993/skills-hub-cli` resource repository, checks its SHA-256 checksum, and installs to `~/.local/bin` on macOS/Linux or `%LOCALAPPDATA%\SkillsHub\bin` on Windows. It preserves your existing CLI if download or checksum verification fails. The command requires a publicly available CLI release in that repository.

On macOS/Linux, open a new terminal after installation; Bash and Zsh configuration is updated automatically. For other shells, add `~/.local/bin` to PATH yourself. Windows updates both the current PowerShell session and the user PATH.

Verify the installation:

```bash
skillshub-cli version --json
skillshub-cli --help
```

Run the same installation command again to upgrade. This standalone copy is separate from the desktop-managed CLI and does not update automatically with the app. To remove it, delete the installed `skillshub-cli` executable (`skillshub-cli.exe` on Windows); your Skill library is retained.

The CLI shares the local library with Skills Hub. Installing it alone does not install the official AI management Skill. Device sync, scheduled tasks, account authorization, and app settings remain desktop features. Linux releases target GNU/glibc systems, not Alpine/musl.

</details>

## Development

### Prerequisites

- Node.js 18+ (recommended: 20+)
- Rust (stable)
- Tauri system dependencies (follow Tauri official docs for your OS)

Create the local OAuth configuration once before starting the desktop app or building an installer:

```bash
cp .env.example .env
```

Replace both placeholders with the team's GitHub and GitLab OAuth public Client IDs. `npm run tauri:dev` and local `npm run tauri:build*` commands load only these two allowlisted values from the root `.env`; each value already present in the process environment takes precedence. Both IDs are required so browser authorization is available for both providers. Never add a client secret or user token, and do not commit `.env`.

```bash
npm install
npm run tauri:dev
```

### Build

```bash
npm run lint
npm run build
npm run tauri:build
```

#### Platform build commands (from `package.json`)

- macOS (dmg): `npm run tauri:build:mac:dmg`
- macOS (universal dmg): `npm run tauri:build:mac:universal:dmg`
- Windows (MSI): `npm run tauri:build:win:msi`
- Windows (NSIS exe): `npm run tauri:build:win:exe`
- Windows (MSI+NSIS): `npm run tauri:build:win:all`
- Linux (deb): `npm run tauri:build:linux:deb`
- Linux (AppImage): `npm run tauri:build:linux:appimage`
- Linux (deb+AppImage): `npm run tauri:build:linux:all`

### Tests (Rust)

```bash
cd src-tauri
cargo test
```

## Contributing & Security

- Contributing: [`CONTRIBUTING.md`](CONTRIBUTING.md)
- Code of Conduct: [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md)
- Security: [`SECURITY.md`](SECURITY.md)

## FAQ / Notes

- Where are skills stored? The Central Repo defaults to `~/.skillshub` (configurable in Settings).
- What are tags for? Tags help you find and organize skills. They do not change where a skill is synced or which tools can use it.
- What is Management Center for? Management Center handles tags, tool targets, and automatic skill updates. Settings keeps app-level preferences.
- Does disabling a skill delete files? No. Disabling only removes tool-side sync. The skill and its configuration remain in the Central Repo and can be enabled again later.
- What does bulk tool setup mean? Skills Hub applies the currently selected tool list to the selected skills. Unchecked tools are removed from those skills' sync targets.
- What is project-level sync? The skill is still stored once in the Central Repo, but its sync target is a selected project directory such as `<project>/.agents/skills`, `<project>/.claude/skills`, or another tool-specific project skills path.
- What is a custom tool directory? If an internal tool or wrapped agent has its own skills directory, you can add it in Management Center as a custom sync target.
- What does automatic update update? It updates Git and local-source skills according to your schedule, then syncs the result to the configured tool targets.
- Which requests use the network proxy? It is the application-wide route for external HTTP, OAuth, updater, and remote Git operations, including GitHub, GitLab, and Gitee device sync.
- Why is Cursor sync always copy? Cursor currently does not support symlink/junction-based skill directories, so Skills Hub forces directory copy when syncing to Cursor.
- Why does sync sometimes fall back to copy? Skills Hub prefers symlink/junction, but on some systems (especially Windows) symlinks may be restricted; in that case it falls back to directory copy.
- What does `TARGET_EXISTS|...` mean? The target folder already exists and the operation did not overwrite it (default is non-destructive). Remove the existing folder or retry with the appropriate overwrite flow.
- macOS Gatekeeper note (unsigned/notarized builds, may vary by macOS version): if you see “damaged” or “unverified developer”, run `xattr -cr "/Applications/Skills Hub.app"` (https://v2.tauri.app/distribute/#macos).

## Supported Platforms

- macOS
- Windows
- Linux (expected by design; not validated locally)

## License

MIT License — see `LICENSE`.
