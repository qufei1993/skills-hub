# Skills Hub（Tauri Desktop）

一个跨平台桌面应用（Tauri + React），用于统一管理 Agent Skills，并把它们同步到多种 AI 编程工具的全局 skills 目录（优先 symlink/junction，失败回退 copy），实现 “Install once, sync everywhere”。

> English documentation: [`README.md`](../README.md)

## 主要功能

- 统一视图：查看 Hub 托管的 skills 及其在各工具的生效状态
- 迁移接管：扫描本机工具目录已有 skills，导入到中心仓库并可一键同步
- 多来源导入：本地目录 / Git 仓库 URL（含 multi-skill 候选选择）
- 更新：从原来源更新中心仓库内容，并回灌 copy 模式的目标
- 新工具检测：发现新安装工具时提示是否同步所有已托管 skills

![Skills Hub](./assets/home-example.png)

## 支持的 AI 编程工具

| tool key | 工具 | skills 目录（相对 `~`） | detect 目录（相对 `~`） |
| --- | --- | --- | --- |
| `cursor` | Cursor | `.cursor/skills` | `.cursor` |
| `claude_code` | Claude Code | `.claude/skills` | `.claude` |
| `codex` | Codex | `.codex/skills` | `.codex` |
| `opencode` | OpenCode | `.config/opencode/skill` | `.config/opencode` |
| `antigravity` | Antigravity | `.gemini/antigravity/skills` | `.gemini/antigravity` |
| `amp` | Amp | `.config/agents/skills` | `.config/agents` |
| `kilo_code` | Kilo Code | `.kilocode/skills` | `.kilocode` |
| `roo_code` | Roo Code | `.roo/skills` | `.roo` |
| `goose` | Goose | `.config/goose/skills` | `.config/goose` |
| `gemini_cli` | Gemini CLI | `.gemini/skills` | `.gemini` |
| `github_copilot` | GitHub Copilot | `.copilot/skills` | `.copilot` |
| `clawdbot` | Clawdbot | `.clawdbot/skills` | `.clawdbot` |
| `droid` | Droid | `.factory/skills` | `.factory` |
| `windsurf` | Windsurf | `.codeium/windsurf/skills` | `.codeium/windsurf` |

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

## 文档

- 系统设计：[`docs/system-design.zh.md`](system-design.zh.md)

## FAQ / 备注

- Skill 存在哪里？中心仓库（Central Repo）默认是 `~/.skillshub`，可在设置里修改。
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
