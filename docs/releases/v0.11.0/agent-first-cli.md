# Agent-first CLI / Agent 优先的命令行入口

## English

Deployment previews expose `affected_agents` for all tools sharing a physical target directory, separately from the managed deployment targets. Disabled tools do not gain new deployment records. One-click AI management rejects scope expansion before installing the official Skill and prompts the user to review tool configuration.

Desktop refreshes Skills and tags together on opening tag filters, entering library/tag pages, and window focus. Existing search/tag filters remain selected and stale read responses are ignored. This is interaction-triggered refresh, not continuous CLI-change polling.

AI installation through `manage-skills-hub` now defaults to installing and deploying to all detected, enabled tools, including eligible custom tools. Explicit tool selection, library-only requests, and project scope override the defaults. No eligible tools, shared-directory scope expansion, conflicts, and deployment failures are reported without claiming full success. At the CLI protocol level, `skills install` still only adds to the library; the official Skill follows it with explicit `skills deploy` calls.

Installation status no longer includes a manual refresh button. Automatic checks and retry after an initial check failure remain available.

View Skill remains available during background status checks without disabled-state flicker.

Reopening Settings preserves the last AI management status while refreshing locally in the background. The loading label appears only before a first successful check; refresh failures retain the previous result with an error message.

Project deployment is supported on macOS and Linux for Agents with a project skills directory. Windows exposes global scope only in CLI capabilities and desktop selection; project deployment or undeployment returns `PROJECT_SCOPE_UNSUPPORTED` (exit 4), including previews, without changing the library or Agent targets.

`skillshub-cli` provides local Skill management through the same Rust services and database as Skills Hub desktop: list/show/search, source update checks, local/Git installation and multi-Skill selection, import, deployment to explicit Agents, supported global/project scope, safe single/batch updates, tags, recoverable removal, status, and diagnostics. Installing a Skill only adds it to the central library; deployment is a separate explicit operation.

After publication, CLI-only users can install `skillshub-cli` from npm or use a standalone release binary and verify its `.sha256` file. npm selects one of five native platform packages: macOS arm64/x64, Windows x64, Linux GNU arm64/x64. Other platforms are unsupported. `skillshub-cli setup --agent codex` installs and deploys the official `manage-skills-hub` Skill only to the selected Agent. Use the detected Agent ID for other tools.

The official Skill prefers the verified desktop CLI bridge, then PATH only when the desktop bridge directory is absent. A damaged or mismatched bridge stops and asks the user to repair it by opening the desktop. Settings now includes an AI management card: one click verifies/prepares the bundled native CLI and installs the official Skill through the shared installer into detected, enabled tools. No Node.js or npm installation is required for this desktop flow. Installed Skills use the normal library controls; technical status stays collapsed. The dedicated Agent Access page is removed. Network and storage cards collapse while retaining summaries.

Use `--json` for stable structured results and `--dry-run` for supported mutation previews. Destructive operations require explicit user authorization and `--yes` confirmation; the flag does not grant the Agent authority beyond the user's request. Unowned target directories, ambiguous selections, unsupported scopes, simultaneous writers, and potentially destructive updates return actionable errors. There is no force-overwrite bypass. The official Skill never manipulates SQLite or Agent directories directly and never accepts credentials in arguments, URLs, or logs.

Device synchronization, automation/scheduled tasks, OAuth and credential management, proxy/storage/custom-tool configuration, application updates, recycle-bin restore, and permanent deletion remain desktop-only. An explicitly requested authenticated repository operation may use credentials already configured in the system secure store; ordinary local reads and startup do not access credentials.

Desktop and CLI share storage. The desktop refreshes Skills and tags on window focus, library/tag navigation, and opening tag filters; no continuous polling or filesystem watcher is added. Development and production desktop/CLI share the same database, central library, settings, cache, recycle bin, and write lock, affecting real Agent directories. Development credentials and CLI bridge binaries remain separate; authenticated operations require credentials in the development namespace. Old development data is preserved without automatic merging. Tests use temporary data and Agent directories and never import or clean production legacy state. Shared schema 6 remains readable by v0.10.1-compatible readers; newer unknown schemas are rejected before legacy migration, preserving database, WAL/SHM, backup, and legacy file bytes.

For legacy multi-Skill records without a source subpath, checks and updates require a unique source match. Missing or ambiguous matches return `INVALID_SOURCE` with reason `source_selection_required` (exit 2), preserving installed content and the JSON error protocol.

## 中文

部署预览通过 `affected_agents` 披露共用同一物理目标目录的全部工具，与实际托管的同步目标分开；不会给停用工具新增同步记录。一键启用 AI 管理遇到共享目录扩大影响范围时，在安装官方 Skill 前停止，并提示检查工具配置。

桌面端在打开标签筛选、进入技能列表/标签管理以及窗口重新获得焦点时统一读取 Skills 和标签，保留已有搜索与标签筛选，忽略过期读取结果。这是交互触发刷新，不是持续轮询 CLI 修改。

通过 `manage-skills-hub` 发起的 AI 安装默认继续同步到已检测且已启用的所有工具（含符合条件的自定义工具）。明确指定工具、只入库及项目范围优先；无可用工具、共享目录扩大影响范围、冲突或同步失败时明确报告，不宣称全部成功。CLI 底层 `skills install` 仍只入库，由官方 Skill 继续调用指定目标的 `skills deploy` 完成流程。

安装状态移除手动刷新按钮，保留自动检查和首次检查失败后的重试入口。

后台检查时「查看 Skill」保持可用，不再因切换禁用状态而闪烁。

再次进入设置时保留上次 AI 管理状态并在后台检查，仅首次成功读取前显示检查提示；刷新失败保留旧结果并提示错误。

AI 管理卡片改为紧凑横向布局，取消标题与操作间的分隔线；已启用状态与标题同行，启用前后统一展示「通过与 AI 对话，安装、更新和整理 Skills，与桌面端统一管理。」移除使用示例和复制入口，保留「查看 Skill」，「安装状态」默认折叠。窄窗口自动换行，安装与同步行为不变。

macOS 和 Linux 支持向已配置项目目录的 Agent 进行项目级部署。Windows 的 CLI 能力及桌面选择仅提供全局范围；项目部署、移除部署及其预览均返回 `PROJECT_SCOPE_UNSUPPORTED`（退出码 4），不修改中央库或 Agent 目标。

`skillshub-cli` 通过与桌面相同的 Rust 服务和数据库提供本机 Skill 管理：列表、详情、搜索、检查更新、本地/Git 安装、多 Skill 选择、导入、向明确 Agent 部署、支持的全局/项目范围、安全的单个/批量更新、标签、可恢复删除、状态和诊断。安装只进入中央库，部署需要另行明确指定。

发布后，CLI-only 用户可从 npm 安装 `skillshub-cli`，也可下载独立二进制并核对 `.sha256`。npm 自动选择 macOS arm64/x64、Windows x64 或 Linux GNU arm64/x64 平台包；其他平台不支持。`skillshub-cli setup --agent codex` 只向指定 Agent 安装和部署官方 `manage-skills-hub` Skill；其他工具使用检测到的 Agent ID。

官方 Skill 优先使用校验后的桌面 CLI 桥接，仅当桥接目录不存在时才查找 PATH。桥接损坏或版本不符时停止，并引导用户打开桌面修复。桌面入口移入「设置 → AI 管理」：一键校验并准备内置 CLI，通过共用安装流程将官方 Skill 安装并同步到已检测、已启用的工具。此桌面流程不依赖用户安装 Node.js 或 npm。安装后沿用普通 Skill 管理，技术状态默认折叠；移除独立「Agent 接入」页面。网络与存储卡片支持折叠并保留摘要。

`--json` 提供稳定结构化输出，`--dry-run` 预览支持的写操作。危险操作要求用户明确授权，并用 `--yes` 确认；该参数不能扩大用户授权。未托管目标冲突、选择不明确、不支持的范围、并发写入或可能损害用户文件的更新会返回可处理的错误，不允许强制覆盖。官方 Skill 不直接操作 SQLite 或 Agent 目录，不把凭据放进参数、URL 或日志。

设备同步、自动/定时任务、OAuth 与凭据管理、代理/存储/自定义工具配置、应用更新、回收站恢复和永久删除仍只能在桌面管理。用户明确请求的认证仓库操作可以使用已配置的系统安全凭据；普通本地查询和启动不会读取凭据。

桌面和 CLI 共用存储。切回窗口、进入技能列表/标签管理或打开标签筛选时刷新 Skills 与标签，不增加持续轮询或文件监听。开发版和正式版桌面/CLI 共用数据库、中央库、配置、缓存、回收站和写锁，操作会影响真实 Agent 目录。开发凭据和 CLI bridge 可执行文件仍独立；需要认证的操作须在开发凭据命名空间配置凭据。旧开发数据保留，不自动合并。自动化测试使用临时数据及 Agent 目录，不导入或清理生产旧库。共享 schema 6 继续兼容 v0.10.1 读取方式，遇到未知新版 schema 时在旧库迁移前拒绝操作，数据库、WAL/SHM、备份及旧库文件的字节保持不变。

缺少来源子路径的旧版多 Skill 记录在检查和更新时必须唯一匹配来源。无匹配或存在多个匹配时，返回 `INVALID_SOURCE`、原因 `source_selection_required`（退出码 2），保留已安装内容及 JSON 错误格式。
