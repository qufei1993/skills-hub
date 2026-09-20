# Agent-first CLI / Agent 优先的命令行入口

## English

`skillshub-cli` provides local Skill management through the same Rust services and database as Skills Hub desktop: list/show/search, source update checks, local/Git installation and multi-Skill selection, import, deployment to explicit Agents, supported global/project scope, safe single/batch updates, tags, recoverable removal, status, and diagnostics. Installing a Skill only adds it to the central library; deployment is a separate explicit operation.

After publication, CLI-only users can install `skillshub-cli` from npm or use a standalone release binary and verify its `.sha256` file. npm selects one of five native platform packages: macOS arm64/x64, Windows x64, Linux GNU arm64/x64. Other platforms are unsupported. `skillshub-cli setup --agent codex` installs and deploys the official `skills-hub` Skill only to the selected Agent. Use the detected Agent ID for other tools.

The official Skill prefers the verified desktop CLI bridge, then PATH only when the desktop bridge directory is absent. A damaged or mismatched bridge stops and asks the user to repair it by opening the desktop. The Agent Access page displays this status and provides setup/removal/refresh controls; it does not run a shell console.

Use `--json` for stable structured results and `--dry-run` for supported mutation previews. Destructive operations require explicit user authorization and `--yes` confirmation; the flag does not grant the Agent authority beyond the user's request. Unowned target directories, ambiguous selections, unsupported scopes, simultaneous writers, and potentially destructive updates return actionable errors. There is no force-overwrite bypass. The official Skill never manipulates SQLite or Agent directories directly and never accepts credentials in arguments, URLs, or logs.

Device synchronization, automation/scheduled tasks, OAuth and credential management, proxy/storage/custom-tool configuration, application updates, recycle-bin restore, and permanent deletion remain desktop-only. An explicitly requested authenticated repository operation may use credentials already configured in the system secure store; ordinary local reads and startup do not access credentials.

Desktop and CLI changes share storage immediately, but an already open desktop may retain its current snapshot. Changes appear on its next normal data read or restart; this release adds no live refresh or background polling for CLI changes. Development and production runtime/credential/bridge locations remain separate. Shared schema 6 remains readable by v0.10.1-compatible readers; newer unknown schemas refuse CLI writes.

## 中文

`skillshub-cli` 通过与桌面相同的 Rust 服务和数据库提供本机 Skill 管理：列表、详情、搜索、检查更新、本地/Git 安装、多 Skill 选择、导入、向明确 Agent 部署、支持的全局/项目范围、安全的单个/批量更新、标签、可恢复删除、状态和诊断。安装只进入中央库，部署需要另行明确指定。

发布后，CLI-only 用户可从 npm 安装 `skillshub-cli`，也可下载独立二进制并核对 `.sha256`。npm 自动选择 macOS arm64/x64、Windows x64 或 Linux GNU arm64/x64 平台包；其他平台不支持。`skillshub-cli setup --agent codex` 只向指定 Agent 安装和部署官方 `skills-hub` Skill；其他工具使用检测到的 Agent ID。

官方 Skill 优先使用校验后的桌面 CLI 桥接，仅当桥接目录不存在时才查找 PATH。桥接损坏或版本不符时停止，并引导用户打开桌面修复。桌面「Agent 接入」页展示状态并提供接入、移除和刷新操作，不提供命令控制台。

`--json` 提供稳定结构化输出，`--dry-run` 预览支持的写操作。危险操作要求用户明确授权，并用 `--yes` 确认；该参数不能扩大用户授权。未托管目标冲突、选择不明确、不支持的范围、并发写入或可能损害用户文件的更新会返回可处理的错误，不允许强制覆盖。官方 Skill 不直接操作 SQLite 或 Agent 目录，不把凭据放进参数、URL 或日志。

设备同步、自动/定时任务、OAuth 与凭据管理、代理/存储/自定义工具配置、应用更新、回收站恢复和永久删除仍只能在桌面管理。用户明确请求的认证仓库操作可以使用已配置的系统安全凭据；普通本地查询和启动不会读取凭据。

桌面和 CLI 共用存储，但已打开的桌面可能暂时保留当前快照；下次正常读取或重启后显示 CLI 修改。本版本不添加实时刷新或后台轮询。开发版与正式版的运行目录、凭据和桥接保持隔离。共享 schema 6 继续兼容 v0.10.1 读取方式，遇到未知新版 schema 时拒绝 CLI 写入。
