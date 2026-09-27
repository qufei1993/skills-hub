# Skills Hub Agent-first CLI 设计

> 2026-09-26 UI revision: the Settings AI management card replaces the separate Agent Access page. One action prepares the native CLI and installs `manage-skills-hub` using the shared installer for detected, enabled tools. Normal Skill controls handle subsequent management. Earlier per-Agent page descriptions below are superseded; CLI explicit-agent setup remains supported.

- 状态：已确认
- 日期：2026-09-19
- 目标版本：v0.11.0
- npm 主包：`skillshub-cli`
- 终端命令：`skillshub-cli`
- 官方 Agent Skill：`skills-hub`

## 本版本功能清单

本节是产品评审入口。后续章节用于说明这些功能如何安全实现。

### 查找 Skill

- 查看当前安装的 Skills；
- 查看某个 Skill 的详细信息；
- 搜索新的 Skill；
- 查看 Skill 已部署到哪些 Agent；
- 检查 Skill 是否有更新。

### 安装和导入

- 从 Git 仓库安装 Skill；
- 从本地目录安装 Skill；
- 从包含多个 Skills 的仓库中选择安装；
- 把 Codex、Claude Code、Cursor 等 Agent 目录中的已有 Skill 导入 Skills Hub。

### 分配给 Agent

- 把 Skill 部署到明确指定的 Agent；
- 同时部署到多个 Agent；
- 部署到全局目录或指定项目；
- 从某个 Agent 卸载，但保留中央库副本。

### 更新

- 检查和更新单个 Skill；
- 检查和批量更新全部 Skills；
- 更新可能覆盖或删除用户文件时停止，由用户处理。

### 标签管理

- 添加、移除或重设 Skill 标签；
- 查看和重命名标签；
- 预览并删除标签。

### 删除

- 预览删除会影响的中央文件和 Agent 目标；
- 删除 Skill，并进入 Skills Hub 回收站；
- 通过桌面端恢复，不允许 Agent 永久清空回收站。

### 检查和排错

- 检查 Skills Hub 数据状态和 Agent 检测状态；
- 检查 Skill 是否正确部署；
- 识别目录冲突并给出具体路径和恢复建议。

### CLI 辅助能力

- 把官方 `skills-hub` Skill 安装到指定 Agent；
- 通过 `--json` 向 Agent 提供稳定结果；
- 通过 `--dry-run` 预览操作；
- 通过 `--yes` 确认危险操作；
- 查看 CLI 版本和诊断信息。

### 明确不包含

- Preset、Packs 和 Skills 合集网站；
- 设备同步、自动同步和定时任务；
- Token、OAuth、账号、代理、存储目录和应用设置管理；
- 永久清空回收站；
- 强制覆盖冲突目录。

一句话定义：Agent 可以通过官方 `skills-hub` Skill 完成 Skill 的查找、安装、部署、更新、导入、标签、删除和排错；设备、账号和自动化继续由桌面端管理。

## 背景

Skills Hub 当前是一个 Tauri 2 + React 19 桌面应用。Skill 的安装、更新、标签、目标部署、SQLite 元数据和中央目录均由 Rust 后端管理，用户通过桌面界面触发这些能力。

下一版本需要把同一套管理能力开放给 Codex、Claude Code、Cursor 等 Agent，使用户可以直接表达“安装这个 Skill 到 Codex”“给它加上 frontend 标签”之类的意图，由 Agent 调用官方 CLI 完成操作。CLI 必须在桌面应用未启动、甚至未安装时独立工作；CLI 完成的操作必须能被之后启动的桌面应用直接读取。

本设计不把桌面功能整体搬到终端，也不允许 Agent 直接修改文件夹或 SQLite。CLI 是 Skills Hub Rust 业务核心的第二个受控入口，桌面端与 CLI 共享数据库、中央目录、工具适配器、安全规则和错误语义。

## 目标

1. 提供原生 Rust CLI，使 Agent 能查询、安装、更新、导入、部署、卸载、删除和标记本机 Skills。
2. 桌面端与 CLI 调用同一套 Rust Service，不复制安装、同步或数据库业务逻辑。
3. CLI 在桌面关闭或未安装时独立运行；通过 CLI 写入的数据在桌面下次读取时直接可见。
4. 通过 npm 提供简单安装入口，同时发布可独立下载的原生二进制。
5. 提供官方 `skills-hub` Skill，规定 Agent 如何安全调用 CLI。
6. 对危险操作提供预览、显式确认、冲突拒绝和稳定的机器可读错误。
7. 防止桌面端、CLI、自动更新和设备同步同时写入中央目录造成竞态。
8. 保持现有生产数据库、中央目录和工具部署状态兼容，不要求用户迁移或重新导入。

## 非目标

- 不实现 Preset、Pack、合集或网站目录能力。
- 不把设备同步、自动同步、定时更新、冲突解决开放给 CLI。
- 不提供凭据配置、OAuth 登录、Token 管理、代理修改、存储迁移、应用更新或自定义工具配置命令。
- 不允许 CLI 永久清空回收站。
- 不让 Agent 直接执行 SQL、编辑 Skills Hub 数据库或绕过 CLI 写 Agent 目录。
- 不要求桌面端实时监听 CLI 变更。桌面已打开时可以保持当前内存快照；重新读取、手动刷新或下次启动后显示 CLI 的修改。
- 不把 CLI 业务逻辑重写为 Node.js。
- 不在本版本提供远程控制、守护进程、HTTP API 或 MCP Server。

## 核心决策

### 共享 Rust 核心

```text
React UI
  └─ Tauri commands ─┐
                     ├─ SkillsHubService ─ Core ─ SQLite + central library + agent targets
Agent                │
  └─ skillshub-cli ──┘
```

桌面端和 CLI 只是适配层：

- Tauri commands 负责参数转换、阻塞任务调度和桌面 DTO。
- CLI 负责参数解析、人类输出、JSON 输出和进程退出码。
- `SkillsHubService` 负责完整业务操作、安全预检、事务边界和跨进程锁。
- `core` 继续负责存储、安装、Git、内容哈希、路径解析、工具适配和文件同步。

不得在 CLI 中重新实现安装、更新、部署、删除或标签规则。现有 Tauri command 中与这些操作有关的业务逻辑需要有针对性地迁入 Service，随后 Tauri command 与 CLI 都调用 Service。与 CLI 无关的桌面功能不做无关重构。

### 共用同一份数据

正式 CLI 与正式桌面应用使用相同的：

- 应用数据目录；
- `skills_hub.db`；
- 用户配置的中央目录，未配置时为 `~/.skillshub`；
- `skills`、`skill_targets`、`skill_tags`、`skill_tag_links` 和相关设置；
- Git 缓存、回收站和内容哈希规则；
- 48+ 工具适配器与全局/项目级目标路径。

CLI 安装完成后不执行“导入到桌面”的第二步。桌面端之后调用现有查询入口时，直接从同一数据库读取结果。

开发版桌面、调试构建 CLI 与正式版共用上述数据目录、数据库、中央库、配置、缓存、回收站和写锁；开发操作会影响真实 Skills 和 Agent 部署目录。开发凭据命名空间与 CLI bridge 可执行文件目录仍独立，避免使用正式凭据或覆盖正式命令。旧开发库及中央目录保留，不自动导入或合并进正式库；正式历史数据库迁移规则对开发和正式构建一致。自动化测试通过注入临时数据和 Agent 目录运行，不提供面向普通用户的任意数据库路径参数。

### 桌面关闭不是前提

CLI 不通过 IPC 连接桌面应用，也不尝试启动桌面应用。只要当前平台存在受支持的 CLI 二进制，它就可以初始化或打开 Skills Hub 数据，执行允许的本机操作。

桌面已经打开时不增加文件监听、后台轮询或 CLI 事件桥。窗口重新获得焦点、进入技能列表/标签管理及打开标签筛选时刷新 Skills 与标签，保留筛选条件并忽略过期响应。

## 后端模块设计

### RuntimePaths

新增不依赖 Tauri 的运行路径对象，集中描述：

```text
RuntimePaths
├── app_data_dir
├── database_path
├── central_repo_default
├── git_cache_dir
├── recycle_bin_dir
├── cli_bridge_dir
└── profile: production | development | test
```

桌面端从 Tauri 解析路径后构造 `RuntimePaths`；CLI 使用同一产品标识和平台规则构造。安装器、中央目录、Git 缓存和回收站接收明确路径，不再通过 `AppHandle` 自行寻找目录。

现有 `default_db_path()`、`resolve_central_repo_path()`、安装器缓存路径、回收站路径等 Tauri 依赖逐步收口到运行时路径边界。需要窗口、事件或插件的桌面能力继续留在 Tauri 层。

### SkillsHubService

Service 提供面向两个入口的稳定操作：

```text
list_skills
show_skill
search_skills
install_skill
check_updates
update_skill
adopt_skills
deploy_skills
undeploy_skills
remove_skills
list_agents
manage_tags
doctor
setup_agent_access
```

每个写操作遵循统一顺序：

1. 校验数据库兼容性。
2. 获取跨进程操作锁。
3. 解析 Skill、Agent、scope 和路径。
4. 完成全量预检并形成操作计划。
5. `--dry-run` 只返回计划，不修改状态。
6. 使用现有 core 执行文件与数据库操作。
7. 回读数据库和目标目录，返回最终状态。

Service 返回结构化领域结果和稳定错误，不返回已本地化的 UI 字符串。桌面端和 CLI 各自负责呈现。

### 复用范围

以下现有模块继续作为唯一实现：

- `skill_store.rs`：SQLite、Skills、目标、标签和设置；
- `installer.rs`：本地/Git 安装、多 Skill 检测、更新；
- `sync_engine.rs`：symlink、junction、copy 回退和安全替换；
- `tool_adapters/`：Agent 检测、全局路径和项目路径；
- `git_fetcher.rs`、`github_download.rs`：来源获取；
- `content_hash.rs`：变更与冲突哈希；
- `central_repo.rs`：中央目录；
- `recycle_bin.rs`：可恢复删除；
- `network_proxy.rs`：应用代理边界。

抽取 Service 后，桌面端现有行为必须由回归测试证明未改变。

## 并发与原子性

### 跨进程操作锁

当前部分更新已有文件锁，但设备同步和回收站仍包含进程内锁。新增 CLI 后，所有会同时修改数据库和文件系统的顶层操作必须共用一把跨进程锁，锁文件放在稳定的应用数据目录，而不是可迁移的中央目录。

需要获取锁的操作包括：

- 安装、导入、更新和删除 Skill；
- deploy/undeploy；
- 批量标签修改；
- 桌面端存储迁移；
- 桌面端设备同步、冲突解决和回收站恢复；
- 自动更新执行。

读取命令不获取独占锁。锁被占用时不无限等待，返回 `OPERATION_BUSY`，包含非敏感的操作类型；Agent 应报告并让用户稍后重试，不得循环高频重试。

Service 是锁的所有者，底层函数不重复获取同一锁，避免嵌套死锁。现有更新锁、设备同步锁和回收站锁在迁移期间可以保留内部防护，最终由测试保证锁顺序固定。

### 预检与批量操作

多 Agent deploy/undeploy 和批量修改先检查所有目标。任何目标存在未托管目录、路径重叠、项目 scope 不受支持或权限问题时，整批拒绝，不留下部分 Agent 已成功、部分失败的状态。

文件替换继续采用 staging、备份、哈希校验和原子重命名。数据库写入使用事务。若文件已完成但数据库提交失败，Service 必须按操作计划回滚文件；无法完整回滚时返回可诊断错误并保留备份，不静默删除用户数据。

## CLI 命令面

### 全局约定

```text
skillshub-cli [--json] [--lang <en|zh-CN|ko>] <command>
```

- 默认输出适合人在终端阅读的文本。
- `--json` 输出稳定 JSON，官方 Skill 始终使用该模式。
- 人类输出支持英文、简体中文和韩文，默认跟随系统 locale，可用 `--lang` 覆盖。
- JSON 字段名、错误码和枚举值始终使用稳定英文标识；`message` 可以本地化，Agent 只解析 `code` 和 `details`。
- 所有路径在输出前规范化；不得在错误、日志或 JSON 中包含 Token、Authorization、OAuth code 或带凭据 URL。

### 查询

```bash
skillshub-cli skills list
skillshub-cli skills show <name-or-id>
skillshub-cli skills search <query> [--limit <n>]
skillshub-cli skills status <name-or-id>
skillshub-cli skills check <name-or-id>
skillshub-cli skills check --all
skillshub-cli agents list
skillshub-cli doctor
skillshub-cli version
```

`list` 支持标签、来源、部署 Agent、未标记和状态过滤。Skill 标识优先按完整 ID，其次按名称解析；多个名称候选返回 `AMBIGUOUS_SKILL`，不猜测。

### 安装

```bash
skillshub-cli skills install <local-path-or-git-ref>
skillshub-cli skills install <repo-ref> --subpath <skill-path>
```

来源识别规则固定：

1. `./`、`../`、绝对路径或 `~/` 开头为本地路径；
2. 包含协议、以 `.git` 结尾或以 `git@` 开头为 Git 来源；
3. 支持的 marketplace shorthand 按明确语法解析；
4. 无法确定时返回 `INVALID_SOURCE`，不根据偶然存在的同名本地路径猜测。

安装默认只进入中央库，不部署到任何 Agent。仓库包含多个 Skill 且用户未指定子路径时返回 `MULTI_SKILLS` 及候选列表；Agent 展示候选并按用户意图再次调用，不自动全装。

安装成功后 Service 回读 Skill、来源、内容哈希和目标状态。CLI 返回这些数据，官方 Skill 不通过扫描文件夹自行判断成功。

### 部署

```bash
skillshub-cli skills deploy <skill> --agent codex
skillshub-cli skills deploy <skill> --agent codex --agent cursor
skillshub-cli skills deploy <skill> --agent codex --project /path/to/project
skillshub-cli skills undeploy <skill> --agent codex
```

CLI 使用 `deploy/undeploy` 表达“让某个 Agent 能否看到 Skill”，不暴露名为 `sync` 的命令，避免与设备同步混淆。

- 至少明确指定一个 Agent；不提供“默认部署到全部 Agent”。
- 默认是 global scope；`--project` 明确进入项目 scope。
- 目标存在未托管内容时返回 `TARGET_CONFLICT`，不覆盖、不删除。
- 共享同一 Skills 目录的工具沿用现有适配器规则，并在结果中列出所有受影响 Agent。
- undeploy 只移除受 Skills Hub 管理的目标，不删除中央库副本。

### 更新

```bash
skillshub-cli skills update <skill>
skillshub-cli skills update --all
```

官方 Skill 在批量更新前必须先调用 `check --all` 并向用户说明范围。若更新会移除中央库或已部署副本中的额外文件，CLI 保持旧版本不动并返回 `UPDATE_HELD_BACK`。CLI 不提供覆盖这一保护的 `--force`；需要人工判断时引导用户使用桌面端。

更新可在明确的用户请求下使用已存于系统安全凭据存储中的仓库凭据，但 CLI 不提供凭据查看、写入或删除命令。普通查询、页面加载、状态展示和本地操作不得读取凭据。

### 导入

```bash
skillshub-cli skills adopt <agent-skills-dir> --dry-run
skillshub-cli skills adopt <agent-skills-dir> --yes
```

导入先扫描候选，排除已在数据库或已作为部署目标管理的目录。正式导入要求 `--yes`。外部目录默认记录为本地来源；CLI 不推测未知 Git 来源。

### 标签

```bash
skillshub-cli skills tag add <skill> <tag>...
skillshub-cli skills tag remove <skill> <tag>...
skillshub-cli skills tag set <skill> <tag>...
skillshub-cli skills tag list [<skill>]
skillshub-cli skills tag rename <old> <new>
skillshub-cli skills tag delete <tag> --dry-run
skillshub-cli skills tag delete <tag> --yes
```

标签操作复用现有大小写不敏感约束。删除标签先返回受影响 Skill 数量，正式删除要求 `--yes`。

### 删除

```bash
skillshub-cli skills remove <skill> --dry-run
skillshub-cli skills remove <skill> --yes
```

删除计划列出中央目录、全部部署目标和将生成的回收站记录。正式删除进入现有 Skills Hub 回收站，不永久清除内容。`--json` 不等于确认，缺少 `--yes` 必须拒绝。

### 官方 Skill 接入

```bash
skillshub-cli setup --agent codex
skillshub-cli setup --agent claude-code
skillshub-cli setup --agent codex --remove --yes
```

`setup` 把随当前版本发布的官方 `skills-hub` Skill 安装到中央库并部署到明确指定的 Agent。它遵循普通目标冲突规则，不覆盖同名的未托管目录。官方 Skill 作为 `bundled` 来源记录在现有 `skills` 表中，因此桌面端可见，版本升级也能使用相同安全更新机制。

## JSON 协议与退出码

成功：

```json
{
  "ok": true,
  "command": "skills.install",
  "data": {}
}
```

失败：

```json
{
  "ok": false,
  "command": "skills.deploy",
  "code": "TARGET_CONFLICT",
  "message": "The target contains content not managed by Skills Hub.",
  "details": {
    "path": "/Users/example/.codex/skills/example"
  }
}
```

JSON 成功写 stdout；JSON 错误对象写 stderr。日志不得混入 stdout。退出码固定为：

| 退出码 | 含义 |
| --- | --- |
| `0` | 成功，包含成功的 dry-run |
| `2` | 参数或命令无效 |
| `3` | 目标不存在或标识不明确 |
| `4` | 路径、内容或安全冲突 |
| `5` | 操作锁被占用 |
| `6` | 数据库或版本不兼容 |
| `7` | 网络、认证或远端来源失败 |
| `10` | 未分类内部错误 |

稳定错误码至少包括：

```text
INVALID_ARGUMENT
INVALID_SOURCE
SKILL_NOT_FOUND
AMBIGUOUS_SKILL
MULTI_SKILLS
AGENT_NOT_FOUND
PROJECT_SCOPE_UNSUPPORTED
TARGET_CONFLICT
UPDATE_HELD_BACK
CONFIRMATION_REQUIRED
PLAN_STALE
OPERATION_BUSY
INCOMPATIBLE_DATABASE
AUTH_REQUIRED
NETWORK_ERROR
INTERNAL_ERROR
```

新增错误码允许向后兼容增加，已有错误码的语义和 `details` 核心字段不得在补丁版本中改变。

## 官方 `skills-hub` Skill

官方 Skill 的职责是把自然语言意图转换为 CLI 命令，不包含业务实现。它必须规定：

1. 先定位可信 CLI。
2. Agent 解析结果时始终使用 `--json`。
3. 安装与部署是两个独立状态。AI 安装默认查询 agents list，安装后向 detected=true 且 enabled=true 的工具继续 deploy；明确指定工具、只入库及项目范围优先。不得自动启用工具或默认创建未检测工具的目录。底层 install 命令仍只入库、deploy 仍显式传 Agent；同步失败和无可用工具须与安装结果分别报告。
4. 查询命令可直接执行。
5. 用户明确请求的单个安装、部署和标签操作可执行，完成后回读验证。
6. 删除、批量更新、adopt、批量 undeploy 和标签删除必须先 dry-run，再获得确认。
7. 永远不传不存在的强制覆盖参数，不自行删除冲突目录。
8. 遇到 `TARGET_CONFLICT`、`UPDATE_HELD_BACK` 或 `OPERATION_BUSY` 时报告实际路径和恢复方式，不绕过 CLI。
9. 不直接访问 SQLite，不使用 `cp`、`rm`、`ln` 等命令模拟 Skills Hub 操作。
10. 不读取、打印或请求用户把 Token 放进命令、文件、URL 或日志。

### CLI 解析顺序

官方 Skill 按以下顺序寻找 CLI：

1. 桌面应用发布并校验过的 `~/.skills-hub/bin/skillshub-cli`（Windows 为 `.exe`）；
2. 仅当桌面发布目录完全不存在时，查找 PATH 中的 `skillshub-cli`；
3. 都不存在时提示安装桌面应用、全局 npm 包或独立二进制。

桌面发布目录存在但校验戳缺失、版本不匹配或二进制缺失时返回“桥接损坏”，不得静默回退到可能更旧的 PATH 版本。用户打开桌面应用一次即可重新发布。

## 桌面端接入

### CLI 发布桥

桌面安装包内附同版本 `skillshub-cli` 原生二进制。应用正常启动时：

1. 读取内置 CLI 版本与校验值；
2. 删除旧校验戳；
3. 把二进制复制到临时路径；
4. 校验 SHA-256 和可执行权限；
5. 原子替换到 `~/.skills-hub/bin/skillshub-cli`；
6. 最后写入版本与校验戳。

发布失败不阻止桌面应用启动，但 Agent 接入状态显示可恢复错误。不得把应用凭据、配置或数据库复制到 bridge 目录。

bridge 使用独立于中央 Skills 目录的固定位置，避免与用户 Skill 名称或自定义中央目录冲突。开发版发布到独立的 `~/.skills-hub-dev/bin`，不得覆盖正式 bridge。

### Agent 接入页面

在“管理中心”增加 `Agent 接入` 标签页，不新增营销式首页，也不提供命令控制台。页面保持现有紧凑管理界面，包含：

- CLI 状态：版本、路径、校验状态；
- 官方 Skill 版本；
- 已检测 Agent 的结构化列表；
- 每个 Agent 的“安装”“修复”“移除”操作；
- CLI-only 用户可复制的 npm 安装命令；
- 明确说明设备同步、自动任务和凭据仍由桌面端管理。

页面只在进入或用户点击刷新时读取状态，不监听 CLI 操作。所有新增文案提供英文、简体中文和韩文翻译；路径、版本和命令使用等宽字体；状态不能只依赖颜色表达。

## npm 与原生发布

### 对外命名

```text
产品：Skills Hub
npm 主包：skillshub-cli
终端命令：skillshub-cli
官方 Skill：skills-hub
```

内部平台包：

```text
@skillshub-app/cli-darwin-arm64
@skillshub-app/cli-darwin-x64
@skillshub-app/cli-win32-x64
@skillshub-app/cli-linux-x64
@skillshub-app/cli-linux-arm64
```

发布前必须注册并保护 `skillshub-app` npm 组织/作用域；若无法取得该作用域，必须先重新确认统一的组织名，不能临时改用个人用户名发布正式包。

### 包结构

`skillshub-cli` 是很薄的跨平台入口：

- 使用 npm `bin` 暴露 `skillshub-cli` 命令；
- 通过 `optionalDependencies` 声明平台包；
- 根据 `process.platform` 和 `process.arch` 定位二进制；
- 使用 `spawnSync`/`spawn` 且 `shell: false` 原样传递参数和退出码；
- 不实现 Skill 业务，不读取数据库，不解释 JSON；
- 不使用安装后脚本从任意 URL 下载文件。

平台包只包含该平台编译好的 Rust 二进制、许可证和最小元数据，并通过 `os`、`cpu` 限制安装。用户只安装 `skillshub-cli`，npm 自动下载当前平台包。

首个版本支持 macOS arm64/x64、Windows x64、Linux x64/arm64。其他平台返回明确的不支持提示；无 Node/npm 的用户可从 GitHub Release 下载相同版本的独立二进制。

### 供应链与版本

- 主包、所有平台包、桌面版本和 CLI `--version` 使用同一产品版本。
- 平台包先发布，主包最后发布；任一平台包失败则不发布主包。
- 使用 npm Trusted Publishing/OIDC、provenance 和强制 2FA，不保存长期 npm Token。
- GitHub Release 为每个二进制提供 SHA-256；npm 平台包中的二进制必须与对应 Release 资产哈希一致。
- macOS 二进制进入现有签名/公证发布链；Windows 按桌面发布能力签名。
- 发布工作流先在临时 registry 或打包产物上完成安装和执行冒烟测试。

## 数据库兼容

本功能使用现有 Skills、目标、标签、设置和回收站结构，不因 CLI 引入新的共享业务表，也不提高共享 `PRAGMA user_version`。

CLI 启动时先检查：

- 主 schema 版本是否在当前二进制支持范围内；
- CLI 所需的 feature schema marker 是否存在或可安全幂等创建；
- 开发和正式构建是否使用同一数据库路径，测试是否使用临时路径；CLI bridge 是否匹配当前构建模式。

遇到高于当前二进制支持范围的共享 schema 时，CLI 可以执行不会误读结构的 `version` 和有限诊断，但拒绝所有业务写入并返回 `INCOMPATIBLE_DATABASE`。不得用旧 CLI 自动降级数据库。

桌面内置 CLI 与桌面版本完全一致。npm CLI 可能与桌面版本不同，因此官方 Skill 优先使用校验过的桌面 bridge；CLI-only 环境使用 PATH 版本。

所有未来迁移继续遵守项目兼容规则：可被旧稳定版安全忽略的功能表使用独立 marker；不兼容共享结构必须先提供明确兼容设计、升级测试和上一稳定版兼容测试。

## 网络与凭据边界

- CLI 的所有 HTTP 和远端 Git 操作继续通过 `network_proxy.rs` 及应用代理设置，禁止读取进程代理环境变量或 Git 全局代理形成第二套行为。
- CLI 可以在用户明确执行安装、搜索或更新时使用核心层安全解析出的凭据，但不能管理或展示凭据。
- 本地查询、标签、状态、版本、doctor 和普通启动不得读取系统凭据。
- CLI-only 环境没有已配置安全凭据时，公开来源仍可使用；需要认证的私有来源返回 `AUTH_REQUIRED`，引导用户在桌面端配置，不接受命令行 Token 参数。
- 日志、JSON、错误链和测试快照必须通过现有脱敏边界。

## 安全权限矩阵

| 操作 | CLI 是否提供 | 官方 Skill 行为 |
| --- | --- | --- |
| list/show/status/search/check | 是 | 可直接执行 |
| 安装单个 Skill | 是 | 用户明确请求后执行并验证 |
| deploy/undeploy 单个明确目标 | 是 | 用户明确请求后执行并验证 |
| 添加/移除单个 Skill 标签 | 是 | 用户明确请求后执行 |
| update 单个 Skill | 是 | 先 check；危险变更由 CLI 阻止 |
| update --all | 是 | 先展示 check 结果并确认 |
| adopt | 是 | 必须 dry-run 并确认 |
| remove | 是 | 必须 dry-run 和 `--yes` |
| 标签删除 | 是 | 必须 dry-run 和 `--yes` |
| 强制覆盖冲突目录 | 否 | 引导用户人工处理 |
| 永久清空回收站 | 否 | 仅桌面端 |
| 设备同步/冲突解决 | 否 | 仅桌面端 |
| 自动任务配置 | 否 | 仅桌面端 |
| 凭据、代理、存储、工具配置 | 否 | 仅桌面端 |

## 错误处理

- 所有错误在 Service 层分类，适配层只负责本地化和序列化。
- 冲突错误必须携带具体路径，但不得携带敏感查询参数或凭据。
- 多步骤操作失败时返回已经回滚、保留备份或需要人工恢复的明确状态。
- CLI 人类输出说明下一步；JSON 输出提供稳定 `code` 和结构化 `details`。
- Agent 不根据错误字符串猜测，不自动重试破坏性操作。
- 网络读取可以使用现有安全重试；写文件、部署和删除不做无界重试。

## 测试策略

### Service 单元与回归测试

- 桌面与 CLI 入口调用同一 Service，并得到相同数据库和文件结果。
- 本地/Git/子目录/多 Skill 安装沿用现有行为。
- install 只进入中央库，deploy 后才产生目标记录。
- global/project scope、共享目录 Agent 和三重同步回退正确。
- 标签大小写、重命名、删除和过滤保持现有语义。
- 更新保护、目标冲突和本地来源重叠保护不能被 CLI 绕过。
- remove 进入回收站并保留可恢复元数据。

### CLI 合约测试

- 每个命令的参数、帮助、文本输出、JSON 输出和退出码。
- JSON stdout 不混入日志；失败 JSON 写 stderr。
- `--json` 不隐式确认删除。
- 模糊 Skill、多 Skill 来源和不支持 scope 返回稳定错误。
- EN、ZH-CN、KO 人类输出存在且 JSON 协议不随语言变化。
- 输出和错误不泄露 Token、凭据 URL 或安全存储内容。

### 跨入口集成测试

1. 使用 CLI 临时运行时安装、标记并部署 Skill。
2. 使用桌面 Core/command 查询同一数据库。
3. 验证 Skill、来源、标签、scope、目标路径和状态一致。
4. 反向由桌面安装，再由 CLI list/show/status 读取。

还需覆盖：

- 桌面进程持锁时 CLI 返回 `OPERATION_BUSY`；
- CLI 持锁时设备同步、自动更新和回收站操作不并发写入；
- 进程异常退出后文件锁自动释放；
- 旧稳定版数据库升级后仍能被上一稳定版按兼容设计安全打开；
- 更高未知 schema 阻止旧 CLI 写入。

### 发布与平台测试

- macOS arm64/x64、Windows x64、Linux x64/arm64 构建。
- 每个平台验证直接下载的二进制和 npm 安装的二进制哈希一致。
- `npm install -g skillshub-cli` 后执行 `version`、`doctor` 和临时目录安装冒烟测试。
- 桌面安装包启动后 bridge 二进制、版本戳和 SHA-256 校验正确。
- bridge 半完成、旧版本或二进制缺失时官方 Skill 拒绝回退。
- `npm run network:check`、`npm run version:check` 和完整 `npm run check` 通过。

## 预期代码边界

实现时预期增加或调整以下区域，具体文件拆分由实施计划确定：

```text
src-tauri/src/
├── bin/skillshub-cli.rs          # Rust CLI 入口
├── services/                     # 共享业务操作与安全边界
├── core/runtime_paths.rs         # 无 Tauri 的路径上下文
├── commands/mod.rs               # 改为调用 Service
└── lib.rs                        # 桌面 Runtime、CLI bridge 初始化

src/components/skills/
└── AgentAccessPage.tsx           # 管理中心 Agent 接入页

skills/manage-skills-hub/
└── SKILL.md                      # 官方 Agent Skill

packages/
├── skillshub-cli/                # npm 总入口
└── cli-*/                        # 平台包定义

scripts/
└── CLI 构建、打包、校验和发布脚本
```

如果现有模块已经承担相同职责，优先扩展现有模块，不为目录结构本身进行无关迁移。

## 验收标准

1. 未安装或未启动桌面应用时，npm/独立 CLI 可以在支持平台初始化并管理 Skills Hub。
2. CLI 安装、标签和部署的 Skill 能被之后启动的桌面端完整读取，无导入步骤。
3. 桌面端执行相同操作继续使用共享 Service，行为与当前版本一致。
4. Agent 可以通过官方 Skill 完成查询、安装、部署、更新、导入、标签和安全删除。
5. install 不隐式部署，deploy 必须明确 Agent，CLI 不包含设备同步命令。
6. 危险更新、未托管目标、缺少确认和数据库不兼容都被可靠阻止。
7. 桌面与 CLI 并发写不会造成部分文件、脏数据库或静默覆盖。
8. npm 用户只需安装 `skillshub-cli`；平台包选择透明完成。
9. npm、GitHub Release、桌面内置 CLI 和产品版本一致且可验证。
10. 所有新增用户文案具备英文、简体中文和韩文翻译，完整检查通过。

## 已确认的产品边界

- 当前优先级是 Agent-first 管理入口，不是 Skills 合集网站。
- Preset/Packs 暂不开发。
- CLI 管理本机 Skill 生命周期；设备、账号和自动化归桌面端。
- CLI 使用 Rust 并复用现有 Core；npm 只是分发渠道。
- 对外 npm 包和命令都使用 `skillshub-cli`。
- 内部平台包使用 `@skillshub-app/cli-*`，普通用户不直接接触。
- 桌面已打开时不要求实时显示 CLI 修改。
