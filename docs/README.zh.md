# SkillSyncer（Tauri Desktop）

一个跨平台桌面应用（Tauri + React），用于统一管理 Agent Skills，并把它们同步到多种 AI 编程工具的全局或项目级 skills 目录（优先 symlink/junction，失败回退 copy），实现 “Install once, sync everywhere”。

> English documentation: [`README.md`](../README.md)

## 主要功能

- **Explore 探索页**：独立页面浏览精选技能推荐和在线搜索，一键安装并同步到所有已检测工具
- **Tags 标签页**：在独立页面中新建、重命名、删除自定义标签，并快速跳转到对应的 Skill 列表
- **标签筛选**：为 Skill 添加多个标签，并在 My Skills 中按标签筛选，包括查看 `无标签` Skill
- **全局 / 项目级同步**：Skill 可同步到全局目录，在所有项目中生效；也可限定到指定项目目录中生效
- **同步范围控制**：在全局和项目范围之间切换 Skill，管理项目目录，并按范围筛选 My Skills
- **技能详情页**：点击技能名称查看完整文件内容，支持文件树浏览、Markdown 渲染和代码语法高亮（40+ 语言）
- **统一视图**：查看 SkillSyncer 托管的 skills 总数、范围徽标及其在各工具的生效状态
- **迁移接管**：扫描本机工具目录已有 skills，导入到中心仓库并可一键同步
- **多来源导入**：本地目录 / Git 仓库 URL（含可搜索的 multi-skill 候选选择、`.claude/skills/` 目录格式支持）
- **更新**：从原来源更新中心仓库内容，并回灌 copy 模式的目标
- **新工具检测**：发现新安装工具时提示是否同步所有已托管 skills

### My Skills — 技能管理列表
![My Skills](./assets/my-skills.png)

### Explore — 探索与在线搜索
![Explore](./assets/explore-search.png)

### Manual Add — 手动添加技能
![Manual Add](./assets/manual-add.png)

### Skill Detail — 技能详情与文件浏览
![Skill Detail](./assets/skill-detail.png)

## 支持的 AI 编程工具

项目级 skills 目录相对所选项目根目录。标记为“不支持”的工具尚未确认项目级 skills 目录，仅支持全局同步。

| tool key | 工具 | 全局 skills 目录（相对 `~`） | 项目级 skills 目录（相对项目根目录） | 存在即视为已安装（相对 `~`） |
| --- | --- | --- | --- | --- |
| `cursor` | Cursor | `.cursor/skills` | `.agents/skills` | `.cursor` |
| `claude_code` | Claude Code | `.claude/skills` | `.claude/skills` | `.claude` |
| `codex` | Codex | `.codex/skills` | `.agents/skills` | `.codex` |
| `opencode` | OpenCode | `.config/opencode/skills` | `.agents/skills` | `.config/opencode` |
| `antigravity` | Antigravity | `.gemini/antigravity/skills` | `.agents/skills` | `.gemini/antigravity` |
| `amp` | Amp | `.config/agents/skills` | `.agents/skills` | `.config/agents` |
| `kimi_cli` | Kimi Code CLI | `.config/agents/skills` | `.agents/skills` | `.config/agents` |
| `augment` | Augment | `.augment/skills` | `.augment/skills` | `.augment` |
| `openclaw` | OpenClaw | `.openclaw/skills` | `skills` | `.openclaw` |
| `copaw` | Copaw | `.copaw/skill_pool` | `.copaw/skill_pool` | `.copaw` |
| `cline` | Cline | `.agents/skills` | `.agents/skills` | `.agents` |
| `codebuddy` | CodeBuddy | `.codebuddy/skills` | `.codebuddy/skills` | `.codebuddy` |
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

- Skill 存在哪里？中心仓库（Central Repo）默认是 `~/.skillsyncer`，可在设置里修改。
- 标签用于什么？标签只用于查找和整理 Skill，不会改变 Skill 的同步目录，也不会改变哪些工具可以使用它。
- 什么是项目级同步？Skill 仍然只在中心仓库保存一份，但同步目标变为指定项目目录，例如 `<project>/.agents/skills`、`<project>/.claude/skills` 或其它工具对应的项目级 skills 路径。
- Cursor 为什么强制 Copy？Cursor 当前不支持软链（symlink/junction）形式的技能目录，因此同步到 Cursor 时会固定使用目录复制（copy）。
- 为什么有时会变成 Copy？默认优先 symlink/junction，但在某些系统（尤其 Windows）可能因为权限/策略导致无法创建链接，会自动回退到目录复制。
- `TARGET_EXISTS|...` 是什么意思？目标目录已存在且默认不覆盖（为了安全）。你需要先清理目标目录，或在“接管/覆盖”的明确流程里重试。
- macOS Gatekeeper 备注（未签名/未公证构建，不同 macOS 版本表现可能不同）：如提示“已损坏/无法验证开发者”，可执行 `xattr -cr "/Applications/SkillSyncer.app"`（https://v2.tauri.app/distribute/#macos）。

## 支持的系统

- macOS（已验证）
- Windows（按架构应支持，未做本地验证）
- Linux（按架构应支持，未做本地验证）

## License

MIT License（见 `LICENSE`）。
