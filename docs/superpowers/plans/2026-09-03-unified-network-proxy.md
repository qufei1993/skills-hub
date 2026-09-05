# Skills Hub Unified Network and Proxy Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 Skills Hub 的 HTTP、OAuth、Git 和应用更新统一接入设置页代理策略，并让设备同步具备可取消、有限等待、完整错误和失败清理能力。

**Architecture:** Rust 侧新增 `core/network` 作为唯一网络边界，由 `NetworkPolicy` 读取并迁移代理设置，`HttpTransport` 与 `GitTransport` 执行代理优先、一次静默直连回退、超时和脱敏。设备同步通过独立运行时发布阶段与取消状态；前端只消费后端状态，Tauri Updater 通过一个前端适配器复用后端给出的策略。

**Tech Stack:** Rust 2021、reqwest blocking + rustls、git2/libgit2、rusqlite、Tauri 2 IPC、React 19、TypeScript 5.9、Vitest、mockito。

**Spec:** `docs/superpowers/specs/2026-09-03-unified-network-proxy-design.md`

## Global Constraints

- Skills Hub 设置页的“网络代理”是所有应用内网络请求的唯一配置来源。
- 代理失败后只对安全操作静默直连一次，不显示代理回退提示，不循环重试。
- `401`、`403`、限流、参数错误和其他业务响应不得触发直连回退。
- GitHub、GitLab、Gitee 的 HTTP API、OAuth 和 Git clone/fetch/push 必须使用同一策略。
- 连接阶段超时为 30 秒；网络传输连续 5 分钟无进展时中止。
- Token、Authorization、OAuth code、refresh token、Cookie 和含凭证 URL 不得进入日志或错误文本。
- 现有 `github_proxy_url` 必须无感兼容，用户不需要重新配置。
- 浏览器 OAuth 授权页继续使用浏览器自己的网络设置。
- macOS、Windows、Linux 使用同一策略，平台差异只存在于底层执行适配器。
- Rust MSRV 保持 `1.77.2`；不引入异步 HTTP 运行时或新的状态管理库。
- 每个任务遵循 TDD：先写失败测试，再做最小实现，再运行相关测试并独立提交。

---

## File Map

| 文件 | 责任 |
| --- | --- |
| `src-tauri/src/core/network/mod.rs` | 统一导出网络接口和固定超时 |
| `src-tauri/src/core/network/policy.rs` | 代理设置兼容、规范化、端点预检和路由选择 |
| `src-tauri/src/core/network/error.rs` | 网络错误分类、完整错误链和敏感信息脱敏 |
| `src-tauri/src/core/network/http.rs` | HTTP 客户端构建、代理优先和一次直连回退 |
| `src-tauri/src/core/network/git.rs` | Git 远程操作代理、超时、进度、取消和 push 核验 |
| `src-tauri/src/core/network_proxy.rs` | 旧接口的短期兼容转发；迁移完成后不再含实现 |
| `src-tauri/src/core/device_sync/runtime.rs` | 同步阶段、耗时、取消状态和线程安全快照 |
| `src-tauri/src/core/device_sync/git_repo.rs` | 设备同步本地 Git 操作，远程操作委托 `GitTransport` |
| `src-tauri/src/core/device_sync/{providers,oauth,credentials}.rs` | 通过 `HttpTransport` 执行 API 和 OAuth |
| `src-tauri/src/core/device_sync/mod.rs` | 同步编排、临时目录守卫、完整错误记录 |
| `src-tauri/src/core/device_sync/types.rs` | 同步进度、体积摘要和历史 DTO |
| `src-tauri/src/core/git_fetcher.rs` | Skill 安装/更新迁入统一 Git 入口 |
| `src-tauri/src/core/{github_search,github_download,featured_skills,skills_search}.rs` | 现有 HTTP 调用迁入统一入口 |
| `src-tauri/src/core/skill_store.rs` | 恢复遗留运行记录 |
| `src-tauri/src/lib.rs` | 初始化 libgit2 超时、注册同步运行时、启动恢复 |
| `src-tauri/src/commands/mod.rs` | 注入策略/运行时，新增进度和取消命令 |
| `src-tauri/src/commands/tests/commands.rs` | IPC 行为与错误链测试 |
| `src/lib/updaterNetwork.ts` | Tauri Updater 的代理/直连一次回退适配器 |
| `src/lib/updaterNetwork.test.ts` | Updater 适配器测试 |
| `src/components/skills/{DeviceSyncPage,deviceSyncState}.tsx` | 同步阶段、耗时、取消、大体积说明和错误详情 |
| `src/components/skills/types.ts` | 与 Rust DTO 对齐 |
| `src/i18n/resources.ts` | 中英文用户文案 |
| `src/App.tsx`、`src/components/skills/SettingsPage.tsx` | 删除重复 Updater 代理逻辑，调用统一适配器 |
| `scripts/check-network-boundaries.mjs` | 阻止业务代码直接创建网络客户端或读取代理来源 |
| `scripts/check-network-boundaries.test.mjs` | 架构检查器自身测试 |
| `package.json` | 将架构检查接入 `npm run check` |
| `AGENTS.md` | 固化所有新增联网功能必须通过统一模块的规则 |

### Task 0: 恢复 v0.9.1 数据库兼容性（已完成前置修复）

**Files:**
- Modify: `src-tauri/src/core/skill_store.rs`
- Test: `src-tauri/src/core/tests/skill_store.rs`
- Modify: `AGENTS.md`

**Interfaces:**
- Preserves: 共享主数据库 `PRAGMA user_version = 6`。
- Produces: `settings.schema.device_sync = 1` 作为设备同步独立 schema 标记。
- Repairs: v0.10.0 开发版产生的主数据库 schema 7，保留设备同步数据后恢复为 6。

- [x] **Step 1: 添加 v0.10 数据库可被 v0.9.1 接受的失败测试**

Run: `cd src-tauri && cargo test core::skill_store::tests::device_sync_schema_keeps_v091_database_compatibility -- --nocapture`

Observed before fix: FAIL，实际主版本为 7，v0.9.1 只接受到 6。

- [x] **Step 2: 添加开发版 schema 7 的无损恢复测试**

测试先保存设备同步配置、模拟 `PRAGMA user_version = 7`、再次初始化，再断言主版本为 6 且配置内容不变。

- [x] **Step 3: 将设备同步 schema 与共享版本解耦**

`SCHEMA_VERSION` 恢复为 6；设备同步表每次通过 `CREATE TABLE IF NOT EXISTS` 幂等确保，并将自身版本写入 `settings.schema.device_sync`。只把已知的开发版版本 7 恢复为 6，未知的更高版本仍拒绝打开。

- [x] **Step 4: 运行数据库定向测试**

Run: `cd src-tauri && cargo test core::skill_store::tests:: -- --nocapture`

Observed after fix: 19 项数据库测试全部 PASS。

- [ ] **Step 5: 在最终发布检查中验证真实升级/降级路径**

用数据库副本执行 v0.9.1 → v0.10.0 → v0.9.1 往返启动验证，断言 Skill、目标、标签和设置数量保持一致；不得直接修改用户真实数据库作为测试手段。

### Task 1: NetworkPolicy 与旧代理配置迁移

**Files:**
- Create: `src-tauri/src/core/network/mod.rs`
- Create: `src-tauri/src/core/network/policy.rs`
- Create: `src-tauri/src/core/network/error.rs`
- Modify: `src-tauri/src/core/mod.rs`
- Modify: `src-tauri/src/core/network_proxy.rs`
- Test: `src-tauri/src/core/network/policy.rs`
- Test: `src-tauri/src/core/network/error.rs`

**Interfaces:**
- Produces: `NetworkPolicy::load(&SkillStore) -> Result<NetworkPolicy>`
- Produces: `NetworkPolicy::routes() -> Vec<NetworkRoute>`
- Produces: `NetworkPolicy::preferred_route() -> NetworkRoute`
- Produces: `NetworkPolicy::for_test(proxy_url, probe) -> NetworkPolicy`，仅在 `#[cfg(test)]` 下可用。
- Produces: `NetworkPolicy::direct_for_test() -> NetworkPolicy`，仅在 `#[cfg(test)]` 下可用。
- Produces: `NetworkRoute::{Proxy(String), Direct}`
- Produces: `RequestKind::{ReadOnly, IdempotentWrite, NonIdempotentWrite}`
- Produces: `sanitize_network_text(&str) -> String`
- Preserves: `get_github_proxy_config`、`set_github_proxy_config` 等现有命令接口。

- [ ] **Step 1: 写代理迁移、预检和脱敏的失败测试**

在同一测试模块增加 `test_store()`，用 `tempfile::tempdir()` 创建数据库并调用 `ensure_schema()`；`for_test` 接收端点探测闭包，使测试不访问真实代理端口。

```rust
#[test]
fn loads_legacy_proxy_and_migrates_without_user_action() {
    let store = test_store();
    store.set_setting("github_proxy_url", "http://127.0.0.1:7890").unwrap();
    let policy = NetworkPolicy::load(&store).unwrap();
    assert_eq!(policy.proxy_url(), Some("http://127.0.0.1:7890"));
    assert_eq!(store.get_setting("network_proxy_url").unwrap().as_deref(), Some("http://127.0.0.1:7890"));
}

#[test]
fn unreachable_local_proxy_returns_only_direct_route() {
    let policy = NetworkPolicy::for_test(Some("http://127.0.0.1:9"), |_| false);
    assert_eq!(policy.routes(), vec![NetworkRoute::Direct]);
}

#[test]
fn redacts_tokens_headers_and_url_credentials() {
    let text = "Authorization: Bearer abc https://user:secret@example.com?access_token=xyz";
    let redacted = sanitize_network_text(text);
    assert!(!redacted.contains("abc"));
    assert!(!redacted.contains("secret"));
    assert!(!redacted.contains("xyz"));
}
```

- [ ] **Step 2: 运行定向测试并确认失败**

Run: `cd src-tauri && cargo test core::network -- --nocapture`

Expected: FAIL，原因是 `core::network`、`NetworkPolicy` 和脱敏函数尚不存在。

- [ ] **Step 3: 实现最小统一策略接口**

```rust
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
pub const STALL_TIMEOUT: Duration = Duration::from_secs(300);
pub const NETWORK_PROXY_URL_KEY: &str = "network_proxy_url";
pub const LEGACY_PROXY_URL_KEY: &str = "github_proxy_url";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NetworkRoute { Proxy(String), Direct }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RequestKind { ReadOnly, IdempotentWrite, NonIdempotentWrite }

#[derive(Clone, Debug)]
pub struct NetworkPolicy {
    proxy_url: Option<String>,
    proxy_reachable: bool,
}

impl NetworkPolicy {
    pub fn load(store: &SkillStore) -> Result<Self> {
        let current = store.get_setting(NETWORK_PROXY_URL_KEY)?;
        let legacy = if current.is_none() {
            store.get_setting(LEGACY_PROXY_URL_KEY)?
        } else {
            None
        };
        if let Some(value) = legacy.as_deref() {
            let _ = store.set_setting(NETWORK_PROXY_URL_KEY, value);
        }
        let proxy_url = current.or(legacy).map(|value| normalize_proxy_url(&value));
        let proxy_url = proxy_url.filter(|value| !value.is_empty());
        if let Some(value) = proxy_url.as_deref() {
            validate_proxy_url(value)?;
        }
        let proxy_reachable = proxy_url.as_deref().is_some_and(proxy_endpoint_reachable);
        Ok(Self { proxy_url, proxy_reachable })
    }
    pub fn proxy_url(&self) -> Option<&str> { self.proxy_url.as_deref() }
    pub fn routes(&self) -> Vec<NetworkRoute> {
        match (&self.proxy_url, self.proxy_reachable) {
            (Some(url), true) => vec![NetworkRoute::Proxy(url.clone()), NetworkRoute::Direct],
            _ => vec![NetworkRoute::Direct],
        }
    }
}
```

`network_proxy.rs` 只调用 `NetworkPolicy` 并保持现有 DTO 形状，避免当前设置页和旧调用同时失效。

- [ ] **Step 4: 运行网络策略测试和格式检查**

Run: `cd src-tauri && cargo test core::network core::network_proxy && cargo fmt --all -- --check`

Expected: 新旧配置、端点预检、路由和脱敏测试全部 PASS。

- [ ] **Step 5: 提交网络策略基础层**

```bash
git add src-tauri/src/core/network src-tauri/src/core/network_proxy.rs src-tauri/src/core/mod.rs
git commit -m "feat: add unified network policy"
```

### Task 2: HttpTransport 的安全回退与错误分类

**Files:**
- Create: `src-tauri/src/core/network/http.rs`
- Modify: `src-tauri/src/core/network/mod.rs`
- Test: `src-tauri/src/core/network/http.rs`

**Interfaces:**
- Consumes: `NetworkPolicy`、`NetworkRoute`、`RequestKind`、`sanitize_network_text`。
- Produces: `HttpTransport::new(NetworkPolicy) -> Result<HttpTransport>`
- Produces: `HttpTransport::send(operation, kind, build) -> Result<Response>`，其中 `build: Fn(&Client) -> RequestBuilder`。
- Produces: `HttpTransport::send_direct(operation, build) -> Result<Response>`，只供完成远端核对后的显式重试使用。
- Produces: `is_ambiguous_transport_error(&anyhow::Error) -> bool`。
- Produces: `is_retryable_reqwest_error(&reqwest::Error) -> bool`。

- [ ] **Step 1: 写代理成功、静默直连和禁止业务错误回退的失败测试**

使用两个 `mockito::Server` 分别代表代理路径与直连路径，并在 `#[cfg(test)]` 中定义 `RecordingClientFactory`：第一个 sender 返回传输失败，第二个 sender 请求 mockito 直连地址；`attempted_routes()` 只记录 `proxy`/`direct`，不保存 URL 或请求头。

```rust
#[test]
fn read_only_request_falls_back_direct_once_after_proxy_transport_error() {
    let transport = HttpTransport::for_test(policy_with_broken_proxy(), test_client_factory());
    let response = transport.send("list repositories", RequestKind::ReadOnly, |client| {
        client.get(direct_server_url())
    }).unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(transport.attempted_routes(), vec!["proxy", "direct"]);
}

#[test]
fn unauthorized_response_does_not_fall_back() {
    let response = transport.send("validate token", RequestKind::ReadOnly, |client| {
        client.get(unauthorized_server_url())
    }).unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(transport.attempted_routes(), vec!["proxy"]);
}

#[test]
fn non_idempotent_write_does_not_retry_automatically() {
    let _ = transport.send("create repository", RequestKind::NonIdempotentWrite, |client| {
        client.post(direct_server_url())
    });
    assert_eq!(transport.attempted_routes(), vec!["proxy"]);
}
```

- [ ] **Step 2: 运行测试并确认失败**

Run: `cd src-tauri && cargo test core::network::http -- --nocapture`

Expected: FAIL，原因是 `HttpTransport` 尚未实现。

- [ ] **Step 3: 实现两个显式客户端和一次回退**

```rust
pub struct HttpTransport {
    policy: NetworkPolicy,
    proxy: Option<Client>,
    direct: Client,
}

impl HttpTransport {
    pub fn new(policy: NetworkPolicy) -> Result<Self> {
        let direct = Client::builder()
            .no_proxy()
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(STALL_TIMEOUT)
            .build()
            .context("build direct HTTP client")?;
        let proxy = policy.proxy_url().map(|url| {
            Client::builder()
                .no_proxy()
                .proxy(reqwest::Proxy::all(url)?)
                .connect_timeout(CONNECT_TIMEOUT)
                .timeout(STALL_TIMEOUT)
                .build()
                .context("build proxy HTTP client")
        }).transpose()?;
        Ok(Self { policy, proxy, direct })
    }

    pub fn send<F>(&self, operation: &str, kind: RequestKind, build: F) -> Result<Response>
    where F: Fn(&Client) -> RequestBuilder {
        let mut routes = self.policy.routes();
        if kind == RequestKind::NonIdempotentWrite {
            routes.truncate(1);
        }
        let mut failures = Vec::new();
        for route in routes {
            let client = match route {
                NetworkRoute::Proxy(_) => self.proxy.as_ref().context("proxy client missing")?,
                NetworkRoute::Direct => &self.direct,
            };
            match build(client).send() {
                Ok(response) => return Ok(response),
                Err(err) if is_retryable_reqwest_error(&err) => {
                    failures.push(sanitize_network_text(&err.to_string()));
                }
                Err(err) => return Err(err).with_context(|| operation.to_string()),
            }
        }
        bail!("{}: {}", operation, failures.join("; "))
    }
}
```

两个客户端均禁用环境代理自动发现；代理客户端显式配置代理，直连客户端显式 `no_proxy()`，保证行为不受启动终端影响。

- [ ] **Step 4: 验证传输、超时和脱敏测试**

Run: `cd src-tauri && cargo test core::network::http core::network::error -- --nocapture`

Expected: 所有测试 PASS，代理与直连均失败时错误链包含两条路径但不含敏感值。

- [ ] **Step 5: 提交 HTTP 传输层**

```bash
git add src-tauri/src/core/network
git commit -m "feat: add proxy fallback http transport"
```

### Task 3: 迁移 Provider API、OAuth 与凭证刷新

**Files:**
- Modify: `src-tauri/src/core/device_sync/providers.rs`
- Modify: `src-tauri/src/core/device_sync/oauth.rs`
- Modify: `src-tauri/src/core/device_sync/credentials.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Test: `src-tauri/src/core/device_sync/providers.rs`
- Test: `src-tauri/src/core/device_sync/oauth.rs`
- Test: `src-tauri/src/core/device_sync/credentials.rs`
- Test: `src-tauri/src/commands/tests/commands.rs`

**Interfaces:**
- Consumes: `HttpTransport::send` 和 `NetworkPolicy::load`。
- Changes: `provider(id, transport: Arc<HttpTransport>) -> Box<dyn GitProvider>`。
- Changes: `oauth::start(provider, &HttpTransport) -> Result<OAuthStartResult>`。
- Changes: `oauth::poll(flow_id, credentials, &HttpTransport) -> Result<OAuthPollResult>`。
- Changes: `resolve_access_token(store, key, &HttpTransport) -> Result<Option<String>>`。

- [ ] **Step 1: 写三平台 API 与 OAuth 回退的失败测试**

继续使用各文件现有 `mockito` 测试形式，并新增测试内的 `RecordingTransport` 与 `AmbiguousCreateProvider`：前者记录 operation 名称，后者第一次创建返回传输中断、随后仓库查询返回同名仓库。fixture token 固定为 `token`/`refresh-secret`，断言错误输出不包含它们。

```rust
#[test]
fn all_providers_use_injected_transport() {
    for id in [ProviderId::Github, ProviderId::Gitlab, ProviderId::Gitee] {
        let transport = recording_transport();
        provider(id, Arc::new(transport.clone())).list_repositories("token").unwrap();
        assert_eq!(transport.last_operation(), Some("list provider repositories"));
    }
}

#[test]
fn create_repository_reconciles_ambiguous_failure_before_retry() {
    let provider = provider_with_ambiguous_create_then_existing_repo();
    let repo = provider.create_private_repository("token", "skills-hub-sync").unwrap();
    assert_eq!(repo.name, "skills-hub-sync");
    assert_eq!(provider.create_call_count(), 1);
}

#[test]
fn oauth_refresh_never_logs_refresh_token() {
    let error = refresh_with_broken_proxy("refresh-secret").unwrap_err();
    assert!(!format!("{error:#}").contains("refresh-secret"));
}
```

- [ ] **Step 2: 运行设备同步 HTTP 测试并确认失败**

Run: `cd src-tauri && cargo test core::device_sync::providers core::device_sync::oauth core::device_sync::credentials -- --nocapture`

Expected: FAIL，旧代码仍自行创建 `Client`，也没有非幂等操作核对。

- [ ] **Step 3: 注入统一传输并实现副作用核对**

```rust
pub struct ApiProvider {
    id: ProviderId,
    base_url: String,
    transport: Arc<HttpTransport>,
}

fn create_private_repository(&self, token: &str, name: &str) -> Result<RemoteRepository> {
    match self.create_once(token, name) {
        Ok(repo) => Ok(repo),
        Err(err) if is_ambiguous_transport_error(&err) => {
            if let Some(existing) = self.find_owned_repository(token, name)? {
                Ok(existing)
            } else {
                self.create_once_direct(token, name)
            }
        }
        Err(err) => Err(err),
    }
}
```

OAuth 设备码开始按 `IdempotentWrite` 执行；设备码换取 Token 的轮询和 refresh token 刷新按 `NonIdempotentWrite` 执行。后两者遇到模糊传输失败时保留或终止当前授权流程，但不立即盲目直连重放。命令层从 `SkillStore` 创建 `NetworkPolicy`，再把同一个 `Arc<HttpTransport>` 传给 Provider/OAuth/凭证刷新。

- [ ] **Step 4: 运行三平台与 OAuth 测试**

Run: `cd src-tauri && cargo test core::device_sync commands::tests:: -- --nocapture`

Expected: GitHub、GitLab、Gitee 认证头与响应归一化测试 PASS；代理回退和敏感信息测试 PASS。

- [ ] **Step 5: 提交设备同步 HTTP/OAuth 迁移**

```bash
git add src-tauri/src/core/device_sync/providers.rs src-tauri/src/core/device_sync/oauth.rs src-tauri/src/core/device_sync/credentials.rs src-tauri/src/commands/mod.rs src-tauri/src/commands/tests/commands.rs
git commit -m "refactor: route device sync api through network transport"
```

### Task 4: GitTransport、超时、取消与远端核验

**Files:**
- Create: `src-tauri/src/core/network/git.rs`
- Modify: `src-tauri/src/core/network/mod.rs`
- Modify: `src-tauri/src/core/device_sync/git_repo.rs`
- Modify: `src-tauri/src/core/git_fetcher.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/core/network/git.rs`
- Test: `src-tauri/src/core/device_sync/git_repo.rs`
- Test: `src-tauri/src/core/tests/git_fetcher.rs`

**Interfaces:**
- Consumes: `NetworkPolicy::routes`、`CancelToken`、`CONNECT_TIMEOUT`、`STALL_TIMEOUT`。
- Produces: `configure_libgit2_timeouts() -> Result<()>`，只允许在 `run()` 最开始调用一次。
- Produces: `GitProgress { phase, received_objects, total_objects, bytes }`。
- Produces: `GitPhase::{Connecting, Downloading, Uploading}`。
- Produces: `GitTransport::clone`、`fetch`、`push_verified`。

- [ ] **Step 1: 写代理路由、取消、超时和 push 核验失败测试**

在 `git.rs` 的测试模块实现 `ScriptedGitRunner`：它按队列返回成功、传输错误或远端 ref，并记录每次 `NetworkRoute`；`FakeClock` 允许测试推进 300 秒而不真实等待。`noop_progress()` 返回空闭包。

```rust
#[test]
fn git_proxy_failure_retries_direct_once() {
    let runner = scripted_runner([proxy_transport_error(), success()]);
    GitTransport::for_test(policy_with_proxy(), runner.clone()).fetch(&repo, None, noop_progress()).unwrap();
    assert_eq!(runner.routes(), vec![NetworkRoute::Proxy(PROXY.into()), NetworkRoute::Direct]);
}

#[test]
fn stalled_transfer_returns_timeout() {
    let clock = fake_clock();
    let error = stalled_transport(clock.advance(STALL_TIMEOUT)).unwrap_err();
    assert!(format!("{error:#}").contains("5 minutes without network progress"));
}

#[test]
fn ambiguous_push_checks_remote_ref_before_retrying() {
    let runner = push_succeeded_but_response_lost();
    runner.transport.push_verified(&repo, target_oid, None, noop_progress()).unwrap();
    assert_eq!(runner.push_count(), 1);
}
```

- [ ] **Step 2: 运行 Git 测试并确认失败**

Run: `cd src-tauri && cargo test core::network::git core::device_sync::git_repo core::git_fetcher -- --nocapture`

Expected: FAIL，统一 `GitTransport` 和核验逻辑尚不存在。

- [ ] **Step 3: 实现 libgit2 固定超时和回调取消**

在 `run()` 创建任何后台任务之前调用：

```rust
pub fn configure_libgit2_timeouts() -> Result<()> {
    unsafe {
        git2::opts::set_server_connect_timeout_in_milliseconds(30_000)?;
        git2::opts::set_server_timeout_in_milliseconds(300_000)?;
    }
    Ok(())
}
```

为 fetch/push callbacks 增加 `transfer_progress`、`push_transfer_progress` 和 `sideband_progress`，每次进展更新计时与 UI 快照；检测 `CancelToken` 后返回 `false` 终止。所有凭证继续通过 libgit2 credential callback 提供，不拼入 remote URL。

- [ ] **Step 4: 实现代理优先、直连一次与远端 ref 核验**

```rust
pub fn push_verified(
    &self,
    repo: &Repository,
    config: &DeviceSyncConfig,
    token: Option<&str>,
    target: Oid,
    progress: &dyn Fn(GitProgress),
) -> Result<()> {
    match self.push_once(repo, config, token, target, self.policy.preferred_route(), progress) {
        Ok(()) => Ok(()),
        Err(err) if is_ambiguous_git_transport_error(&err) => {
            if self.remote_ref_oid(config, token)? == Some(target) { Ok(()) }
            else { self.push_once(repo, config, token, target, NetworkRoute::Direct, progress) }
        }
        Err(err) => Err(err),
    }
}
```

`device_sync/git_repo.rs` 保留 index、commit、manifest 等本地操作；clone/fetch/push 委托 `GitTransport`。`git_fetcher.rs` 的系统 Git 与 libgit2 回退都从同一策略生成显式代理/直连参数，删除环境变量代理读取。

- [ ] **Step 5: 验证 Git 测试和本地 bare repository 往返**

Run: `cd src-tauri && cargo test core::network::git core::device_sync::git_repo core::git_fetcher -- --nocapture`

Expected: 代理、直连、取消、超时、push 核验和 bare repository 往返全部 PASS。

- [ ] **Step 6: 提交统一 Git 传输层**

```bash
git add src-tauri/src/core/network src-tauri/src/core/device_sync/git_repo.rs src-tauri/src/core/git_fetcher.rs src-tauri/src/core/tests/git_fetcher.rs src-tauri/src/lib.rs
git commit -m "feat: add resilient git transport"
```

### Task 5: 设备同步运行时、失败清理和遗留状态恢复

**Files:**
- Create: `src-tauri/src/core/device_sync/runtime.rs`
- Modify: `src-tauri/src/core/device_sync/mod.rs`
- Modify: `src-tauri/src/core/device_sync/types.rs`
- Modify: `src-tauri/src/core/skill_store.rs`
- Modify: `src-tauri/src/core/tests/skill_store.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/core/device_sync/mod.rs`
- Test: `src-tauri/src/core/device_sync/runtime.rs`

**Interfaces:**
- Produces: `DeviceSyncRuntime::begin() -> Result<SyncRunGuard>`。
- Produces: `DeviceSyncRuntime::snapshot() -> Option<SyncProgress>`。
- Produces: `DeviceSyncRuntime::cancel()`。
- Produces: `SyncPhase::{Preparing, Connecting, Downloading, Merging, Uploading, Applying}`。
- Produces: `SyncProgress::starting(started_at: i64) -> SyncProgress`。
- Produces: `SkillStore::interrupt_running_device_sync_runs(finished_at: i64) -> Result<usize>`。

- [ ] **Step 1: 写临时目录、完整错误链和遗留运行记录失败测试**

在设备同步测试模块增加 `SyncFixture`：使用临时数据库、中央 Skill 目录和 bare Git 仓库；可注入 `GitTransport`，使 push 返回 `anyhow!("transport closed").context("push device sync repository")`。fixture 提供 `export_directories()` 与 `latest_history()`，只读取临时目录和测试数据库。

```rust
#[test]
fn failed_sync_removes_export_directory_and_persists_full_chain() {
    let fixture = sync_fixture_failing_at_push("transport closed");
    let error = fixture.service.sync().unwrap_err();
    assert!(format!("{error:#}").contains("transport closed"));
    assert!(fixture.export_directories().is_empty());
    assert!(fixture.latest_history().error.unwrap().contains("transport closed"));
}

#[test]
fn startup_marks_running_sync_as_interrupted() {
    store.start_device_sync_run("stale", 1).unwrap();
    assert_eq!(store.interrupt_running_device_sync_runs(2).unwrap(), 1);
    let row = store.list_device_sync_history(1).unwrap().remove(0);
    assert_eq!(row.status, "interrupted");
    assert_eq!(row.finished_at, Some(2));
}
```

- [ ] **Step 2: 运行同步服务测试并确认失败**

Run: `cd src-tauri && cargo test core::device_sync core::skill_store::tests::startup_marks -- --nocapture`

Expected: FAIL，当前失败分支遗留 `export-*`，历史只保存 `err.to_string()`。

- [ ] **Step 3: 实现 RAII 清理与同步运行时**

```rust
struct TempExport(PathBuf);
impl Drop for TempExport {
    fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); }
}

pub struct DeviceSyncRuntime {
    cancel: CancelToken,
    progress: Mutex<Option<SyncProgress>>,
}

impl DeviceSyncRuntime {
    pub fn begin(&self) -> Result<SyncRunGuard<'_>> {
        let mut progress = self.progress.lock().unwrap();
        if progress.is_some() {
            bail!("device sync is already running");
        }
        self.cancel.reset();
        *progress = Some(SyncProgress::starting(now_ms()));
        Ok(SyncRunGuard { runtime: self })
    }
    pub fn snapshot(&self) -> Option<SyncProgress> { self.progress.lock().unwrap().clone() }
    pub fn cancel(&self) { self.cancel.cancel(); }
}
```

`DeviceSyncService` 接收 `&DeviceSyncRuntime`，在准备、连接、下载、合并、上传、应用阶段更新快照；失败历史改为保存 `sanitize_network_text(&format!("{err:#}"))`。启动初始化数据库后立即调用 `interrupt_running_device_sync_runs(now_ms())`。

- [ ] **Step 4: 加入同步内容体积摘要**

在导出完成后统计 `file_count` 与 `total_bytes`，写入 `SyncProgress`。阈值固定为 100 MiB，仅用于非阻塞说明，不阻止同步、不删除内容。

- [ ] **Step 5: 运行服务、数据库和全量 Rust 测试**

Run: `cd src-tauri && cargo test core::device_sync core::skill_store -- --nocapture`

Expected: 成功/失败/取消均无临时目录；遗留记录恢复；完整错误链已脱敏；现有冲突合并测试保持 PASS。

- [ ] **Step 6: 提交同步可靠性改造**

```bash
git add src-tauri/src/core/device_sync src-tauri/src/core/skill_store.rs src-tauri/src/core/tests/skill_store.rs src-tauri/src/lib.rs
git commit -m "fix: bound and recover device sync runs"
```

### Task 6: IPC 与设备同步进度界面

**Files:**
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/components/skills/types.ts`
- Modify: `src/components/skills/deviceSyncState.ts`
- Modify: `src/components/skills/deviceSyncState.test.ts`
- Modify: `src/components/skills/DeviceSyncPage.tsx`
- Modify: `src/App.css`
- Modify: `src/i18n/resources.ts`
- Test: `src-tauri/src/commands/tests/commands.rs`
- Reference before UI edit: `docs/UI-DESIGN-GUIDELINES.md`

**Interfaces:**
- Consumes: `DeviceSyncRuntime::snapshot/cancel`。
- Produces IPC: `get_device_sync_progress() -> Option<SyncProgress>`。
- Produces IPC: `cancel_device_sync() -> ()`。
- Changes: `DeviceSyncStatus`/TypeScript DTO 增加可选 `progress`，保持旧数据兼容。

- [ ] **Step 1: 阅读项目 UI 规范并写状态映射失败测试**

Run: `sed -n '1,260p' docs/UI-DESIGN-GUIDELINES.md`

```ts
it('maps upload progress to localized presentation data', () => {
  expect(getSyncProgressPresentation({ phase: 'uploading', elapsed_seconds: 42, total_bytes: 0, file_count: 0 }))
    .toEqual({ labelKey: 'deviceSync.phase.uploading', elapsedSeconds: 42, cancellable: true })
})

it('exposes failed history error only when present', () => {
  expect(getHistoryError({ status: 'failed', error: 'push: timeout' })).toBe('push: timeout')
})
```

- [ ] **Step 2: 运行前端状态测试并确认失败**

Run: `npm test -- src/components/skills/deviceSyncState.test.ts`

Expected: FAIL，进度展示函数尚不存在。

- [ ] **Step 3: 新增 IPC 命令并注册独立运行时**

```rust
#[tauri::command]
pub fn get_device_sync_progress(runtime: State<'_, Arc<DeviceSyncRuntime>>) -> Option<SyncProgress> {
    runtime.snapshot()
}

#[tauri::command]
pub fn cancel_device_sync(runtime: State<'_, Arc<DeviceSyncRuntime>>) {
    runtime.cancel();
}
```

`run_device_sync` 和后台自动同步使用同一个 `Arc<DeviceSyncRuntime>`；不要复用安装 Skill 的全局取消令牌。

- [ ] **Step 4: 实现同步中的阶段、耗时和取消界面**

`DeviceSyncPage` 在 `busy === 'sync'` 时每秒读取一次进度，结束时清除 timer。按钮显示具体阶段而不是永久“同步中”，旁边提供“取消同步”。超过 100 MiB 时显示非阻塞说明。历史失败项提供可展开的脱敏错误详情。

```tsx
{progress ? (
  <div className="device-sync-progress" role="status" aria-live="polite">
    <LoaderCircle className="spin" size={16} />
    <span>{t(`deviceSync.phase.${progress.phase}`)}</span>
    <time>{formatElapsed(progress.elapsed_seconds)}</time>
    <button type="button" onClick={cancelSync}>{t('deviceSync.cancelSync')}</button>
  </div>
) : null}
```

所有新增文案同时添加英文和中文；样式遵循现有圆角、颜色变量和深色主题，不新增全屏阻塞弹窗。

- [ ] **Step 5: 运行前后端定向测试**

Run: `npm test -- src/components/skills/deviceSyncState.test.ts && cd src-tauri && cargo test commands::tests:: -- --nocapture`

Expected: 进度映射、取消命令、历史错误和 DTO 序列化测试 PASS。

- [ ] **Step 6: 提交同步反馈界面**

```bash
git add src-tauri/src/commands/mod.rs src-tauri/src/commands/tests/commands.rs src-tauri/src/lib.rs src/components/skills/types.ts src/components/skills/deviceSyncState.ts src/components/skills/deviceSyncState.test.ts src/components/skills/DeviceSyncPage.tsx src/App.css src/i18n/resources.ts
git commit -m "feat: show cancellable device sync progress"
```

### Task 7: 迁移其余 HTTP 与应用更新路径

**Files:**
- Modify: `src-tauri/src/core/github_search.rs`
- Modify: `src-tauri/src/core/github_download.rs`
- Modify: `src-tauri/src/core/featured_skills.rs`
- Modify: `src-tauri/src/core/skills_search.rs`
- Modify: `src-tauri/src/core/auto_update.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Create: `src/lib/updaterNetwork.ts`
- Create: `src/lib/updaterNetwork.test.ts`
- Modify: `src/App.tsx`
- Modify: `src/components/skills/SettingsPage.tsx`
- Test: existing Rust module tests plus `src/lib/updaterNetwork.test.ts`

**Interfaces:**
- Consumes: `HttpTransport` 和 `NetworkPolicy`。
- Produces IPC: `get_updater_network_policy() -> UpdaterNetworkPolicyDto`。
- Produces: `checkWithNetworkFallback(policy, check) -> Promise<Update | null>`。
- Produces: `downloadWithNetworkFallback(update, policy) -> Promise<void>`。

- [ ] **Step 1: 写 Updater 一次回退和业务错误不回退测试**

```ts
it('checks through proxy then direct after transport failure', async () => {
  const check = vi.fn().mockRejectedValueOnce(new TypeError('network error')).mockResolvedValueOnce(null)
  await checkWithNetworkFallback({ proxy_url: 'http://127.0.0.1:7890', proxy_reachable: true }, check)
  expect(check).toHaveBeenNthCalledWith(1, { proxy: 'http://127.0.0.1:7890' })
  expect(check).toHaveBeenNthCalledWith(2, undefined)
})

it('does not retry non-network updater errors', async () => {
  const check = vi.fn().mockRejectedValue(new Error('signature invalid'))
  await expect(checkWithNetworkFallback(
    { proxy_url: 'http://127.0.0.1:7890', proxy_reachable: true },
    check,
  )).rejects.toThrow('signature invalid')
  expect(check).toHaveBeenCalledTimes(1)
})

it('rechecks the same version directly before retrying a failed download', async () => {
  const proxyUpdate = fakeUpdate('0.10.1', new TypeError('proxy disconnected'))
  const directUpdate = fakeUpdate('0.10.1')
  const check = vi.fn().mockResolvedValueOnce(proxyUpdate).mockResolvedValueOnce(directUpdate)
  await downloadWithNetworkFallback(
    proxyUpdate,
    { proxy_url: 'http://127.0.0.1:7890', proxy_reachable: true },
    check,
  )
  expect(check).toHaveBeenLastCalledWith(undefined)
  expect(directUpdate.downloadAndInstall).toHaveBeenCalledTimes(1)
})
```

- [ ] **Step 2: 运行 Updater 测试并确认失败**

Run: `npm test -- src/lib/updaterNetwork.test.ts`

Expected: FAIL，适配器尚不存在。

- [ ] **Step 3: 迁移 Rust HTTP 调用**

所有业务函数从命令层或 `SkillStore` 获得 `NetworkPolicy`/`HttpTransport`，移除直接调用 `app_http_client`、`github_http_client` 和 `Client::new`。测试中的 mock server 通过 `NetworkPolicy::direct_for_test()` 注入，不访问真实网络。

- [ ] **Step 4: 实现 UpdaterAdapter 并删除两份重复逻辑**

```ts
export type UpdaterNetworkPolicy = {
  proxy_url?: string | null
  proxy_reachable: boolean
}

export async function checkWithNetworkFallback(
  policy: UpdaterNetworkPolicy,
  check: (options?: { proxy?: string }) => Promise<Update | null>,
): Promise<Update | null> {
  const proxy = policy.proxy_reachable ? policy.proxy_url?.trim() : ''
  if (!proxy) return check(undefined)
  try {
    return await check({ proxy })
  } catch (error) {
    if (!isUpdaterTransportError(error)) throw error
    return check(undefined)
  }
}

export const isUpdaterTransportError = (error: unknown): boolean => {
  const message = String(error).toLowerCase()
  return error instanceof TypeError ||
    ['network', 'connect', 'proxy', 'dns', 'timeout'].some((part) => message.includes(part))
}

export async function downloadWithNetworkFallback(
  update: Update,
  policy: UpdaterNetworkPolicy,
  check: (options?: { proxy?: string }) => Promise<Update | null>,
): Promise<void> {
  const proxy = policy.proxy_reachable ? policy.proxy_url?.trim() : ''
  try {
    await update.downloadAndInstall(undefined, proxy ? { proxy } : undefined)
  } catch (error) {
    if (!proxy || !isUpdaterTransportError(error)) throw error
    const directUpdate = await check(undefined)
    if (!directUpdate || directUpdate.version !== update.version) throw error
    await directUpdate.downloadAndInstall(undefined, undefined)
  }
}
```

测试模块的 `fakeUpdate(version, error?)` 返回只实现 `version` 与 `downloadAndInstall` 的类型安全 fixture，`policy` 固定为可达的 `http://127.0.0.1:7890`。`App.tsx` 和 `SettingsPage.tsx` 统一调用该文件，不再各自定义 `buildUpdaterProxyOptions`。下载失败后先用直连重新检查同一版本，只有版本仍可用时才继续下载。

- [ ] **Step 5: 运行所有联网模块测试**

Run: `npm test -- src/lib/updaterNetwork.test.ts && cd src-tauri && cargo test core::github core::featured_skills core::skills_search core::auto_update -- --nocapture`

Expected: 搜索、下载、推荐、版本说明和更新适配器测试全部 PASS。

- [ ] **Step 6: 提交剩余网络路径迁移**

```bash
git add src-tauri/src/core src-tauri/src/commands/mod.rs src/lib/updaterNetwork.ts src/lib/updaterNetwork.test.ts src/App.tsx src/components/skills/SettingsPage.tsx
git commit -m "refactor: route all outbound traffic through network policy"
```

### Task 8: 自动架构守卫与项目规则

**Files:**
- Create: `scripts/check-network-boundaries.mjs`
- Create: `scripts/check-network-boundaries.test.mjs`
- Modify: `package.json`
- Modify: `AGENTS.md`

**Interfaces:**
- Produces CLI: `node scripts/check-network-boundaries.mjs`，违规时退出码为 1。
- Produces npm script: `npm run check:network`。
- Changes: `npm run check` 在 lint 之前执行架构检查。

- [ ] **Step 1: 写检查器 fixture 测试并确认失败**

```js
test('rejects direct reqwest client outside network module', () => {
  const result = checkSource('src-tauri/src/core/example.rs', 'reqwest::blocking::Client::new()')
  assert.deepEqual(result, ['direct reqwest client'])
})

test('allows network module and cfg test blocks', () => {
  assert.deepEqual(checkSource('src-tauri/src/core/network/http.rs', 'Client::new()'), [])
  assert.deepEqual(checkSource('src-tauri/src/core/example.rs', '#[cfg(test)] mod tests { Client::new(); }'), [])
})
```

Run: `node --test scripts/check-network-boundaries.test.mjs`

Expected: FAIL，检查器尚不存在。

- [ ] **Step 2: 实现可测试的静态扫描器**

检查生产 Rust/TypeScript 文件中的以下模式：

```js
const forbidden = [
  ['direct reqwest client', /(?:Client|ClientBuilder)::(?:new|builder)\s*\(/],
  ['direct remote git', /(?:RemoteCallbacks|FetchOptions|PushOptions|ProxyOptions|Command::new\(\"git\")/],
  ['proxy environment read', /(?:HTTP_PROXY|HTTPS_PROXY|ALL_PROXY|NO_PROXY)/],
  ['legacy proxy setting read', /github_proxy_url/],
]
```

扫描器使用花括号平衡移除 `#[cfg(test)] mod tests`，允许 `src-tauri/src/core/network/**`、`src/lib/updaterNetwork.ts` 和集中维护的兼容文件白名单。输出精确文件与行号。

- [ ] **Step 3: 接入项目检查并加入永久规则**

```json
{
  "scripts": {
    "check:network": "node scripts/check-network-boundaries.mjs",
    "check": "npm run check:network && npm run lint && npm run test && npm run build && npm run rust:fmt:check && npm run rust:clippy && npm run rust:test"
  }
}
```

在 `AGENTS.md` 增加：

> 所有新增出站 HTTP、OAuth、Git 和更新请求必须通过 `core/network` 或 `src/lib/updaterNetwork.ts`；代理配置只来自 Skills Hub 设置。联网操作必须定义超时、取消、错误脱敏及代理失败后的安全回退测试。业务模块不得直接创建网络客户端、读取代理环境变量或自行配置远程 Git 代理。

- [ ] **Step 4: 运行检查器测试和真实仓库扫描**

Run: `node --test scripts/check-network-boundaries.test.mjs && npm run check:network`

Expected: fixture 测试 PASS，当前生产代码零违规。

- [ ] **Step 5: 提交架构守卫**

```bash
git add scripts/check-network-boundaries.mjs scripts/check-network-boundaries.test.mjs package.json AGENTS.md
git commit -m "chore: enforce unified network boundary"
```

### Task 9: 跨场景验收与发布文档

**Files:**
- Modify: `docs/releases/v0.10.0/README.md`（若该版本目录使用其他入口文件，则修改现有入口，不新增重复说明）
- Modify: `CHANGELOG.md`
- Modify: `docs/CHANGELOG.zh.md`
- Test: 全项目检查及人工网络矩阵

**Interfaces:**
- Consumes: 前八个任务的最终接口。
- Produces: v0.10.0 用户可见变更说明和验收记录。

- [ ] **Step 1: 执行自动化全量检查**

Run: `npm run check`

Expected: 架构检查、lint、54 项以上前端测试、build、Rust fmt、clippy 和 170 项以上 Rust 测试全部 PASS；测试数量可以增加，不得减少现有覆盖。

- [ ] **Step 2: 执行代理行为矩阵**

依次验证以下三种设置，每种覆盖 GitHub、GitLab、Gitee 的授权、仓库列表、创建专用私有仓库、首次同步和再次同步：

1. 代理关闭：所有操作直连成功。
2. 代理开启且 `127.0.0.1:7890` 可用：所有应用内请求经代理成功。
3. 代理开启但端口不可达：界面无回退提示，操作自动直连成功。

Expected: 三种设置均无重复仓库、重复版本或凭证泄漏；浏览器授权页行为不受应用强制控制。

- [ ] **Step 3: 执行失败与恢复矩阵**

验证：代理在请求中途关闭、直连也不可用、上传停滞超过 5 分钟、同步中取消、应用在同步中退出并重启、超过 100 MiB 的同步内容。

Expected: 最多一次回退；无数小时挂起；可取消；`export-*` 被清理；遗留记录标为 `interrupted`；历史错误包含完整脱敏原因；大内容只显示非阻塞说明。

- [ ] **Step 4: 更新中英文发布说明**

英文与中文说明必须包含：统一使用设置页网络代理、代理失效静默直连、设备同步可取消和超时、失败诊断与临时文件清理。不要暴露内部凭证存储或实现细节。

- [ ] **Step 5: 检查最终差异并提交**

```bash
git diff --check
git status --short
git add CHANGELOG.md docs/CHANGELOG.zh.md docs/releases/v0.10.0
git commit -m "docs: document resilient network and sync behavior"
```

- [ ] **Step 6: 最终提交前再次验证**

Run: `npm run check && npm run version:check`

Expected: 两条命令退出码均为 0，版本仍为 `0.10.0`，工作区只剩用户原有且与本计划无关的改动。
