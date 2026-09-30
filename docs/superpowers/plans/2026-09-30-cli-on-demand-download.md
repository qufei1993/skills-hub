# CLI On-Demand Download Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 桌面安装包移除 CLI，在用户点击启用或更新时下载经验证的对应版本，并将 CLI 发布资源移到独立仓库。

**Architecture:** CLI 构建先生成最终二进制的发布清单，桌面只嵌入这份清单。下载模块通过统一代理客户端获取固定版本资源并验证长度和哈希，再复用现有 bridge 原子安装；前端订阅本次操作的进度。发布流水线先发布并验证 CLI，再允许公开桌面安装包。

**Tech Stack:** Tauri 2、Rust 2021、reqwest blocking、sha2、tempfile、React 19、TypeScript、Vitest、Node.js、GitHub Actions；不新增运行时依赖。

**Spec:** `docs/superpowers/specs/2026-09-30-cli-on-demand-download-design.md`

## Global Constraints

- 产品版本保持 `0.11.0`，基线 `origin/main` 的 `24b25e48153fcdda96bbb24e45d020911bc850fb`；不改写 `v0.11.0` 标签、不公开现有草稿。
- 固定公开资源仓库 `qufei1993/skills-hub-cli`；源码与编译仍在当前仓库。
- CLI 支持 darwin arm64/x64、linux arm64/x64、windows x64；桌面正式发布保持 macOS 两种架构与 Windows x64。
- 开发使用本地 debug CLI 和独立开发 bridge；正式桌面只嵌入匹配 release 清单。
- 不迁移数据库，不修复延期的中央目录缺失撤销问题；测试只使用临时数据库和 Agent 目录。
- 启动、页面加载、状态查询不下载、不执行 CLI、不读取凭据；CLI 和官方 Skill 更新由用户点击触发。
- 网络必须通过 `network_proxy.rs`，禁用代理不继承进程代理，公开资源不需要凭据。
- 用户可见文本提供 EN/ZH/KO；进度沿用 AI 管理卡片，遵循已读取的 UI 指南。
- 每项先运行失败测试再实现；最终运行 `npm run check`、`npm run version:check`，检查开发版启动和实际包体积。

## Review Focus

1. 旧 CLI 仍可用时下载超时、截断或替换失败：保留原二进制和有效记录（任务 2）。
2. 本地版本或清单被篡改，或 debug/release 混用：拒绝复用与构建（任务 1、2）。
3. 重复点击、组件卸载、迟到进度：不产生第二次安装，不更新卸载组件，不污染后续操作（任务 3）。
4. GitHub 跳转降级、代理关闭、超长响应：受控拒绝，不泄漏 URL 查询或响应正文（任务 2）。
5. CLI 上传失败、已存在同名版本但字节不同：阻止桌面公开发布，不覆盖不可变资源（任务 4）。

---

### Task 1: 发布清单与不携带 CLI 的桌面构建

**Files:**
- Create: `scripts/cli-manifest.mjs`, `scripts/cli-manifest.test.mjs`
- Modify: `scripts/prepare-cli-sidecar.mjs`, `scripts/prepare-cli-sidecar.test.mjs`, `scripts/build-desktop.mjs`, `scripts/build-desktop.test.mjs`
- Modify: `src-tauri/build.rs`, `src-tauri/tauri.conf.json`, `src-tauri/cli_sidecar_profile.rs`
- Test: `src-tauri/src/core/tests/cli_bridge.rs`（现有构建 profile 与元数据测试）

**Interfaces:**
- Produces: `CliManifest = { version: string, sourceCommit: string, target: string, profile: 'debug'|'release', assetName: string, size: number, sha256: string }`。
- Produces: JS `createCliManifest({binaryPath, version, sourceCommit, target, profile}): CliManifest` 与 `validateCliManifest(manifest, {version, sourceCommit, target, profile}): CliManifest`。
- Produces: 编译环境 `SKILLS_HUB_CLI_MANIFEST`（经验证的 JSON）、debug 专用 `SKILLS_HUB_BUNDLED_CLI_SOURCE_PATH`；保留 CLI 自身构建的 `SKILLS_HUB_PREPARE_CLI_SIDECAR=1` 引导路径。
- Consumes: `cli_sidecar_profile::validate_profile`，五平台现有 target/资源命名映射。

- [ ] **Step 1: 添加失败测试。** 临时文件 `abc` 的清单断言 `size === 3`、`sha256 === 'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad'`；版本 `0.11.0`、darwin arm64、release 对应名称为 `skillshub-cli-0.11.0-darwin-arm64`。逐一改变 version/target/profile/sourceCommit、非整数长度、非十六进制哈希均拒绝。模拟正式构建只有清单且无二进制仍成功；缺清单失败；debug 构建需要本地 debug 二进制且验证其哈希。
- [ ] **Step 2: 运行 `npx vitest run scripts/cli-manifest.test.mjs scripts/prepare-cli-sidecar.test.mjs scripts/build-desktop.test.mjs`。** 预期因新接口不存在或旧构建仍强制准备二进制而失败。
- [ ] **Step 3: 实现清单生成和严格校验。** 对签名后的最终字节生成清单，原子写入；构建时验证产品版本、当前提交、目标、profile、固定名称和长度/哈希格式。`build.rs` 正式模式不读取 CLI 文件，只嵌入清单；debug 模式校验本地产物。移除 `bundle.externalBin`。本地正式构建用明确提供的清单路径；不得静默生成或获取其他版本清单。
- [ ] **Step 4: 重跑上述测试和 `npm run version:check`。** 预期全部通过；准备本机 debug 元数据，运行 Rust profile/bridge 相关测试证明原子发布不退化。
- [ ] **Step 5: 提交任务文件。** Commit: `build: separate CLI manifests from desktop bundles`。

### Task 2: 下载验证、受保护安装与明确触发边界

**Files:**
- Create: `src-tauri/src/core/cli_distribution.rs`, `src-tauri/src/core/tests/cli_distribution.rs`
- Modify: `src-tauri/src/core/mod.rs`, `src-tauri/src/core/cli_bridge.rs`, `src-tauri/src/core/tests/cli_bridge.rs`
- Modify: `src-tauri/src/core/network_proxy.rs`（含现有内联 tests 模块）
- Modify: `src-tauri/src/commands/mod.rs`, `src-tauri/src/commands/tests/commands.rs`, `src-tauri/src/lib.rs`
- Modify: `scripts/cli-on-demand.test.mjs`（替换已不适用的启动自动升级约束）

**Interfaces:**
- Consumes: 任务 1 的嵌入清单，现有 `publish_cli_bridge(source, destination, version, expected_hash)` 与受保护目录/锁/回滚。
- Produces: Rust `CliManifest` 与 JS 相同字段（serde camelCase），`CliPreparationProgress { operation_id: String, phase: CliPreparationPhase, downloaded_bytes: u64, total_bytes: Option<u64> }`；phase 序列 preparing/downloading/verifying/installing/configuring。
- Produces: `embedded_cli_manifest() -> anyhow::Result<CliManifest>`；`prepare_cli(manifest: &CliManifest, destination: &Path, proxy_url: Option<&str>, on_progress: &dyn Fn(CliPreparationPhase, u64, Option<u64>)) -> anyhow::Result<CliBridgeStatus>`。
- Produces: `network_proxy::app_download_client(proxy_url: Option<&str>, timeout_secs: u64) -> anyhow::Result<reqwest::blocking::Client>`，HTTPS-only 跳转、最多 5 次、总超时 120 秒。
- Produces: IPC `enable_ai_management({ operationId })` 保持既有状态 DTO 返回；事件 `ai-management-progress` 携带上面的进度 DTO。错误码区分 CLI_DOWNLOAD_FAILED、CLI_DOWNLOAD_UNAVAILABLE、CLI_INTEGRITY_FAILED；不传远端正文或重定向 URL。

- [ ] **Step 1: 添加失败回归测试。** 本地 HTTP 服务与临时 bridge：匹配现有 CLI 返回 Valid 且请求数为 0；第一次下载正确字节返回 Valid；404、超时、短于/长于预期、哈希不符均失败且旧二进制/记录逐字节不变。篡改本地哈希记录不能使恶意文件成为 Valid。并发调用由现有锁保护；损坏当前 CLI 可重试修复。代理关闭不得使用环境代理；重定向到 HTTP 被拒绝；第六次跳转失败。正式入口固定 HTTPS，测试只注入本地传输入口，不增添生产可配置下载地址。
- [ ] **Step 2: 运行 `cd src-tauri && cargo test cli_distribution`。** 预期模块/接口尚不存在导致失败；现有 bridge 回归保持作为绿色基线。
- [ ] **Step 3: 实现下载与 bridge 集成。** 下载到受控临时文件，流式计数与 SHA-256，超过长度立即中止；校验后调用现有原子发布。debug 分支仅验证本地产物。先执行工具与官方 Skill 的只读前置检查，失败不得联网或安装；随后准备 CLI、安装/更新官方 Skill、配置终端。跨阶段失败不得报告成功。命令层通过 AppHandle 发事件，状态查询仅读清单和本地文件。删除 lib.rs 的启动自动更新块。
- [ ] **Step 4: 添加并执行命令层行为测试。** 无可用工具/官方 Skill 冲突时请求数为 0、无安装；只读状态不创建目录；CLI 失败无 Skill 安装；成功按 CLI→Skill→终端顺序。运行 `cargo test cli_distribution`、`cargo test cli_bridge`、`cargo test ai_management` 和 `npm run network:check`，预期通过。启动入口检查不得保留旧自动更新副作用。
- [ ] **Step 5: 提交任务文件。** Commit: `feat: download and verify CLI on explicit enable`。

### Task 3: 设置卡片进度、更新与失败重试

**Files:**
- Modify: `src/components/skills/AiManagementSettings.tsx`, `src/components/skills/AiManagementSettings.test.tsx`, `src/components/skills/types.ts`
- Modify: `src/i18n/resources.ts`, `src/i18n/ko.ts`, `src/App.css`（仅进度反馈所需）

**Interfaces:**
- Consumes: 任务 2 的 `enable_ai_management({ operationId })`、`ai-management-progress` 事件及错误码。
- Produces: TypeScript `CliPreparationProgress` 对齐 Rust DTO；每次点击创建 operationId，仅接受本次 ID 的事件，组件卸载时清理监听。
- Preserves: 初始只读 get_agent_access_status、现有 onChanged/onStatusChanged 回调与官方 Skill 跳转。

- [ ] **Step 1: 添加失败组件测试。** mount 只调用 get_agent_access_status；点击调用一次启用且传 operationId；事件展示阶段/已下载量/百分比，pending 时按钮 disabled，重复点击不多调用；错误事件 ID 不显示；卸载后解除监听。下载失败展示本地化说明，旧版本仍显示待更新，重试使用新 ID。未知长度只显示阶段与下载量，不显示伪造百分比。
- [ ] **Step 2: 运行 `npx vitest run src/components/skills/AiManagementSettings.test.tsx`。** 预期缺少进度事件及参数导致失败。
- [ ] **Step 3: 实现事件订阅和进度。** 点击前建立监听，完成或失败后清理，使用 busyRef 防重入；支持下载、校验、安装、配置反馈。状态和版本来自后端，更新按钮继续明确触发。补齐 EN/ZH/KO 文案及可访问状态通知，沿用卡片样式。
- [ ] **Step 4: 重跑组件测试、`npm run lint` 和 `npm run build`。** 预期通过；检查三语言与亮暗主题的状态反馈，页面加载没有下载动作。
- [ ] **Step 5: 提交任务文件。** Commit: `feat: show CLI download progress and retry in settings`。

### Task 4: 独立资源发布、安装脚本与整体验收

**Files:**
- Modify: `.github/workflows/release.yml`, `.github/workflows/ci.yml`, `scripts/release-workflow.test.mjs`
- Modify: `scripts/install-cli.sh`, `scripts/install-cli.ps1`, `scripts/install-cli.test.mjs`, `scripts/install-cli.Tests.ps1`
- Create: `scripts/verify-cli-release.mjs`, `scripts/verify-cli-release.test.mjs`
- Modify: `README.md`, `docs/README.zh.md`, `CHANGELOG.md`, `docs/CHANGELOG.zh.md`, `docs/releases/v0.11.0/README.md`, `docs/releases/v0.11.0/README.zh.md`, `docs/releases/v0.11.0/cli-installation.md`

**Interfaces:**
- Consumes: 任务 1 的最终清单生成/验证；五平台资源与 sha256 文件；三个桌面目标对应清单。
- Produces: `verifyCliRelease({ manifests, download }): Promise<void>` 验证匿名固定版本地址、最终长度和 SHA-256；相同版本已存在时逐项核对字节与源提交，拒绝替换不同产物。
- Produces: 工作流依赖 `verify → cli-build → cli-publish → desktop-publish`；桌面构建读取同批 CLI manifest，允许在 CLI 发布验证期间构建，但公开发布必须等待验证成功。

- [ ] **Step 1: 添加失败测试。** 解析工作流并断言五平台 CLI/三平台桌面矩阵、公开发布依赖 CLI 验证、桌面资产集合无 CLI。用假下载器验证五份清单全部成功才通过，任一 404/错哈希/错源提交均失败。安装脚本 fixture 下载来自 CLI 资源仓库且仍验证哈希；既有 symlink/原子替换保护继续通过。
- [ ] **Step 2: 运行上述脚本测试。** 预期现有混合发布任务和旧下载仓库地址导致失败。
- [ ] **Step 3: 重排工作流。** CLI 原生测试、正式编译、已配置签名/公证、最后生成清单；上传 CLI 资源并匿名回读验证。桌面只消费清单，移除 sidecar 复制及混合公证逻辑，保留独立桌面公证和 updater 签名；实物检查 .app/Windows staging 中无 CLI。跨仓库令牌只来自 Actions secret；权限不足时保留已完成本地改动并说明需要管理员配置，不读取或传输密钥。创建公开资源仓库的动作在授权范围内；本次不发布新正式 CLI 或桌面版本。
- [ ] **Step 4: 更新独立安装脚本与发布记录。** 写明首次启用需联网、已验证 CLI 可离线使用、点击更新、资源独立发布；不宣称当前旧草稿已包含改动。
- [ ] **Step 5: 全量验证。** 运行 `npm run version:check`、`npm run check`；在本分支运行 `npm run tauri:dev` 只确认启动，不点击启用或改真实 Agent 目录。用本机匹配清单构建 macOS arm64 DMG，检查 bundle 无 CLI，与旧 0.11.0 的 16,280,098 字节比较。五平台原生/发布验证以 CI 结果为准；没有运行的目标明确记录，不能以本机结果代替。
- [ ] **Step 6: 最终独立审查和提交。** 由新 reviewer 对整个分支核对设计、网络/凭据边界、失败回滚、workflow 发布依赖与包体积证据，修复阻塞项后重跑受影响检查。Commit: `ci: publish CLI separately and shrink desktop installers`。返回实际完成与未运行检查；不合并、不移动标签、不公开草稿。

## Plan Self-Review

- 设计的安装、信任来源、网络、开发隔离、UI、五平台发布、文档与体积验收均有所属任务。
- 清单字段、操作 ID、进度字段在相邻任务间一致；任务依赖按 1→2→3→4 执行。
- 五项 Review Focus 均有行为验证，不依赖新增的源码字符串匹配来证明安全性。
- 发布权限不足与跨平台 CI 未运行会明确列为未完成验证，不扩大本次隐藏版本的发布授权。
