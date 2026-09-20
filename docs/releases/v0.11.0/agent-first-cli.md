# Agent-first CLI / Agent 优先的命令行入口

## English

Project deployment is supported on macOS and Linux for Agents with a project skills directory. Windows exposes global scope only in CLI capabilities and desktop selection; project deployment or undeployment returns `PROJECT_SCOPE_UNSUPPORTED` (exit 4), including previews, without changing the library or Agent targets.

`skillshub-cli` provides local Skill management through the same Rust services and database as Skills Hub desktop: list/show/search, source update checks, local/Git installation and multi-Skill selection, import, deployment to explicit Agents, supported global/project scope, safe single/batch updates, tags, recoverable removal, status, and diagnostics. Installing a Skill only adds it to the central library; deployment is a separate explicit operation.

After publication, CLI-only users can install `skillshub-cli` from npm or use a standalone release binary and verify its `.sha256` file. npm selects one of five native platform packages: macOS arm64/x64, Windows x64, Linux GNU arm64/x64. Other platforms are unsupported. `skillshub-cli setup --agent codex` installs and deploys the official `skills-hub` Skill only to the selected Agent. Use the detected Agent ID for other tools.

The official Skill prefers the verified desktop CLI bridge, then PATH only when the desktop bridge directory is absent. A damaged or mismatched bridge stops and asks the user to repair it by opening the desktop. The Agent Access page displays this status and provides setup/removal/refresh controls; it does not run a shell console.

Use `--json` for stable structured results and `--dry-run` for supported mutation previews. Destructive operations require explicit user authorization and `--yes` confirmation; the flag does not grant the Agent authority beyond the user's request. Unowned target directories, ambiguous selections, unsupported scopes, simultaneous writers, and potentially destructive updates return actionable errors. There is no force-overwrite bypass. The official Skill never manipulates SQLite or Agent directories directly and never accepts credentials in arguments, URLs, or logs.

Device synchronization, automation/scheduled tasks, OAuth and credential management, proxy/storage/custom-tool configuration, application updates, recycle-bin restore, and permanent deletion remain desktop-only. An explicitly requested authenticated repository operation may use credentials already configured in the system secure store; ordinary local reads and startup do not access credentials.

Desktop and CLI changes share storage immediately, but an already open desktop may retain its current snapshot. Changes appear on its next normal data read or restart; this release adds no live refresh or background polling for CLI changes. Development and test profiles never import or clean production legacy data, including stored central paths and Agent targets. Shared schema 6 remains readable by v0.10.1-compatible readers; newer unknown schemas are rejected before legacy migration, preserving database, WAL/SHM, backup, and legacy file bytes.

For legacy multi-Skill records without a source subpath, checks and updates require a unique source match. Missing or ambiguous matches return `INVALID_SOURCE` with reason `source_selection_required` (exit 2), preserving installed content and the JSON error protocol.

## 中文

macOS 和 Linux 支持向已配置项目目录的 Agent 进行项目级部署。Windows 的 CLI 能力及桌面选择仅提供全局范围；项目部署、移除部署及其预览均返回 `PROJECT_SCOPE_UNSUPPORTED`（退出码 4），不修改中央库或 Agent 目标。

`skillshub-cli` 通过与桌面相同的 Rust 服务和数据库提供本机 Skill 管理：列表、详情、搜索、检查更新、本地/Git 安装、多 Skill 选择、导入、向明确 Agent 部署、支持的全局/项目范围、安全的单个/批量更新、标签、可恢复删除、状态和诊断。安装只进入中央库，部署需要另行明确指定。

发布后，CLI-only 用户可从 npm 安装 `skillshub-cli`，也可下载独立二进制并核对 `.sha256`。npm 自动选择 macOS arm64/x64、Windows x64 或 Linux GNU arm64/x64 平台包；其他平台不支持。`skillshub-cli setup --agent codex` 只向指定 Agent 安装和部署官方 `skills-hub` Skill；其他工具使用检测到的 Agent ID。

官方 Skill 优先使用校验后的桌面 CLI 桥接，仅当桥接目录不存在时才查找 PATH。桥接损坏或版本不符时停止，并引导用户打开桌面修复。桌面「Agent 接入」页展示状态并提供接入、移除和刷新操作，不提供命令控制台。

`--json` 提供稳定结构化输出，`--dry-run` 预览支持的写操作。危险操作要求用户明确授权，并用 `--yes` 确认；该参数不能扩大用户授权。未托管目标冲突、选择不明确、不支持的范围、并发写入或可能损害用户文件的更新会返回可处理的错误，不允许强制覆盖。官方 Skill 不直接操作 SQLite 或 Agent 目录，不把凭据放进参数、URL 或日志。

设备同步、自动/定时任务、OAuth 与凭据管理、代理/存储/自定义工具配置、应用更新、回收站恢复和永久删除仍只能在桌面管理。用户明确请求的认证仓库操作可以使用已配置的系统安全凭据；普通本地查询和启动不会读取凭据。

桌面和 CLI 共用存储，但已打开的桌面可能暂时保留当前快照；下次正常读取或重启后显示 CLI 修改。本版本不添加实时刷新或后台轮询。开发和测试环境不导入或清理生产旧库，包括旧库保存的中央目录和 Agent 目标。共享 schema 6 继续兼容 v0.10.1 读取方式，遇到未知新版 schema 时在旧库迁移前拒绝操作，数据库、WAL/SHM、备份及旧库文件的字节保持不变。

缺少来源子路径的旧版多 Skill 记录在检查和更新时必须唯一匹配来源。无匹配或存在多个匹配时，返回 `INVALID_SOURCE`、原因 `source_selection_required`（退出码 2），保留已安装内容及 JSON 错误格式。
