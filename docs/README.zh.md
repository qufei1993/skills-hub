# Skills Hub（Tauri Desktop）

一个跨平台桌面应用（Tauri + React），用于集中安装、整理、更新 Agent Skills，并把它们同步到多个 AI 编程工具的全局或项目级 skills 目录。Skills Hub 会优先使用 symlink/junction，同步失败时自动回退到 copy，实现 “Install once, sync everywhere”。

> English documentation: [`README.md`](../README.md)

## 为什么使用 Skills Hub

AI 编程工具越来越多，每个工具都有自己的 skills 目录和安装方式。手动维护这些目录会带来几个问题：同一个 Skill 要复制多份、更新来源不清楚、不同工具启用状态不一致、批量整理成本高。

Skills Hub 的做法是：把 Skill 统一安装到中心仓库，再按你的选择同步到 Claude Code、Codex、Cursor、OpenCode、Antigravity 等工具。你可以为 Skill 打标签、选择全局或项目范围、批量调整工具目标，也可以让系统定时帮你更新 Git 和本地来源的 Skill。

## 主要功能

- **集中托管**：把 Skill 安装到中心仓库，避免分散在多个工具目录里。
- **探索安装**：从精选列表、在线搜索、本地目录或 Git 仓库安装 Skill。
- **多工具同步**：按全局或项目范围同步到不同 AI 编程工具。
- **批量管理**：批量设置标签、工具、启用状态或删除 Skill。
- **标签整理**：用标签筛选、归类和维护 Skill。
- **工具管理**：启用内置工具，也可以添加自定义工具目录。
- **自动更新**：定时更新 Git 和本地来源的 Skill，并查看失败原因。
- **详情查看**：浏览 Skill 文件树、Markdown 内容和代码片段。
- **迁移接管**：扫描并导入本机已有 Skills，统一纳入管理。

## 界面预览

### My Skills — 托管技能与批量管理

My Skills 展示已托管 Skill 的来源、标签、同步范围、目标工具和启用状态。顶部可以筛选范围、排序、进入批量模式、按标签筛选或搜索。

![My Skills](./assets/my-skills.png)

### Explore — 精选 Skill 与在线搜索

Explore 汇总精选仓库中的 Skill，并支持在线搜索。点击 Install 后可以继续选择标签、安装范围和目标工具。

![Explore](./assets/explore-search.png)

### Add Skill — 安装前设置标签、范围和工具

手动添加支持本地目录和 Git 仓库。安装前可以设置标签，选择全局或项目范围，并选择要同步到哪些工具。

![Add Skill](./assets/add-skill-modal.png)

### Management Center — 标签、工具和更新集中管理

管理中心收拢标签、工具和更新能力。更新页支持系统定时更新、立即更新、运行结果统计和失败原因查看。

![Management Center Updates](./assets/management-updates.png)

### Settings — 应用级设置

设置页只保留应用偏好：界面语言、外观、存储与缓存、GitHub Token、网络代理和应用版本更新。

![Settings](./assets/settings-page.png)

## 工作方式

1. 从 Explore、本地目录或 Git 仓库安装 Skill。
2. 安装前选择标签、同步范围和目标工具。
3. Skills Hub 将 Skill 保存到中心仓库，默认目录为 `~/.skillshub`。
4. 按工具规则同步到全局 skills 目录或项目级 skills 目录。
5. 后续可以在 My Skills 中批量整理、启停、删除，或在管理中心配置自动更新和工具目标。

## 支持的 AI 编程工具

当前内置 46 个工具适配，并支持通过管理中心添加自定义工具目录。项目级 skills 目录相对所选项目根目录；标记为“不支持”的工具尚未确认项目级 skills 目录，仅支持全局同步。

| tool key | 工具 | 全局 skills 目录（相对 `~`） | 项目级 skills 目录（相对项目根目录） | 存在即视为已安装（相对 `~`） |
| --- | --- | --- | --- | --- |
| `cursor` | Cursor | `.cursor/skills` | `.agents/skills` | `.cursor` |
| `claude_code` | Claude Code | `.claude/skills` | `.claude/skills` | `.claude` |
| `codex` | Codex | `.codex/skills` | `.agents/skills` | `.codex` |
| `opencode` | OpenCode | `.config/opencode/skills` | `.agents/skills` | `.config/opencode` |
| `antigravity` | Antigravity | `.gemini/config/skills` | `.agents/skills` | `.gemini/config` |
| `amp` | Amp | `.config/agents/skills` | `.agents/skills` | `.config/agents` |
| `kimi_cli` | Kimi Code CLI | `.config/agents/skills` | `.agents/skills` | `.config/agents` |
| `augment` | Augment | `.augment/skills` | `.augment/skills` | `.augment` |
| `openclaw` | OpenClaw | `.openclaw/skills` | `skills` | `.openclaw` |
| `copaw` | Copaw | `.copaw/skill_pool` | `.copaw/skill_pool` | `.copaw` |
| `cline` | Cline | `.agents/skills` | `.agents/skills` | `.agents` |
| `codebuddy` | CodeBuddy | `.codebuddy/skills` | `.codebuddy/skills` | `.codebuddy` |
| `codewhale` | CodeWhale | `.codewhale/skills` | `.codewhale/skills` | `.codewhale` |
| `workbuddy` | WorkBuddy | `.workbuddy/skills` | `不支持` | `.workbuddy` |
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
| `github_copilot` | GitHub Copilot | `.copilot/skills` | `.agents/skills` | `.copilot` |
| `clawdbot` | Clawdbot | `.clawdbot/skills` | `.clawdbot/skills` | `.clawdbot` |
| `droid` | Droid | `.factory/skills` | `.factory/skills` | `.factory` |
| `windsurf` | Windsurf | `.codeium/windsurf/skills` | `.windsurf/skills` | `.codeium/windsurf` |
| `moltbot` | MoltBot | `.moltbot/skills` | `.moltbot/skills` | `.moltbot` |
| `hermes_agent` | Hermes Agent | `.hermes/skills` | 不支持 | `.hermes` |

完整路径规则与检测逻辑见 [`src-tauri/src/core/tool_adapters/mod.rs`](../src-tauri/src/core/tool_adapters/mod.rs)。

## 开发

### 环境要求

- Node.js 18+（建议 20+）
- Rust（stable）
- Tauri 系统依赖（按官方文档安装）

### 启动（桌面端）

```bash
npm install
npm run tauri:dev
```

### 构建

```bash
npm run lint
npm run build
npm run tauri:build
```

#### 各系统构建命令（来自 `package.json`）

- macOS（dmg）：`npm run tauri:build:mac:dmg`
- macOS（universal dmg）：`npm run tauri:build:mac:universal:dmg`
- Windows（MSI）：`npm run tauri:build:win:msi`
- Windows（NSIS exe）：`npm run tauri:build:win:exe`
- Windows（MSI+NSIS）：`npm run tauri:build:win:all`
- Linux（deb）：`npm run tauri:build:linux:deb`
- Linux（AppImage）：`npm run tauri:build:linux:appimage`
- Linux（deb+AppImage）：`npm run tauri:build:linux:all`

### 测试（Rust）

```bash
cd src-tauri
cargo test
```

## FAQ / 备注

- Skill 存在哪里？中心仓库（Central Repo）默认是 `~/.skillshub`，可在设置里修改。
- 标签用于什么？标签只用于查找和整理 Skill，不会改变 Skill 的同步目录，也不会改变哪些工具可以使用它。
- 管理中心用于什么？管理中心负责标签、工具目标和 Skills 自动更新；设置页只保留应用级配置。
- 停用 Skill 会删除文件吗？不会。停用只会移除工具侧同步，中心仓库中的 Skill 和配置仍保留，重新启用后可按原工具设置恢复。
- 批量设置工具是什么意思？对选中的 Skill 应用当前勾选的工具列表；未勾选的工具会从这些 Skill 的同步目标中移除。
- 什么是项目级同步？Skill 仍然只在中心仓库保存一份，但同步目标变为指定项目目录，例如 `<project>/.agents/skills`、`<project>/.claude/skills` 或其它工具对应的项目级 skills 路径。
- 自定义工具目录是什么？如果某个内部工具或二次封装 Agent 使用自己的 skills 目录，可以在管理中心添加为自定义同步目标。
- 自动更新会更新什么？自动更新会按配置更新 Git 和本地来源的 Skill，并把更新结果同步到对应工具目标。
- 网络代理影响哪些请求？它会影响 GitHub API、精选 Skills、GitHub Contents 下载和 Git clone/fetch/update 流程。
- Cursor 为什么强制 Copy？Cursor 当前不支持软链（symlink/junction）形式的技能目录，因此同步到 Cursor 时会固定使用目录复制（copy）。
- 为什么有时会变成 Copy？默认优先 symlink/junction，但在某些系统（尤其 Windows）可能因为权限/策略导致无法创建链接，会自动回退到目录复制。
- `TARGET_EXISTS|...` 是什么意思？目标目录已存在且默认不覆盖（为了安全）。你需要先清理目标目录，或在“接管/覆盖”的明确流程里重试。
- macOS Gatekeeper 备注（未签名/未公证构建，不同 macOS 版本表现可能不同）：如提示“已损坏/无法验证开发者”，可执行 `xattr -cr "/Applications/Skills Hub.app"`（https://v2.tauri.app/distribute/#macos）。

## 支持的系统

- macOS（已验证）
- Windows（按架构应支持，未做本地验证）
- Linux（按架构应支持，未做本地验证）

## License

MIT License（见 `LICENSE`）。
