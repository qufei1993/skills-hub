# Skills Hub v0.11.0

v0.11.0 adds an Agent-first CLI, the official `skills-hub` Skill, and the desktop Agent Access page. Agents can manage the same local library as the desktop; the desktop need not be running or installed. See [Agent-first CLI](agent-first-cli.md) and [verified desktop bridge](verified-cli-bridge.md).

v0.11.0 新增 Agent-first CLI、官方 `skills-hub` Skill 和桌面「Agent 接入」页面。Agent 与桌面共用本地库，CLI 可在桌面未启动或未安装时工作。详细说明见 [Agent-first CLI](agent-first-cli.md) 和 [桌面桥接](verified-cli-bridge.md)。

## Release status / 发布状态

This is release preparation, not evidence of publication. The release workflow advertises exactly macOS arm64/x64, Windows x64, and Linux GNU arm64/x64, each on a matching native runner. This work was validated locally on macOS arm64; the other native jobs and configured signing/notarization must pass in CI before release. No release, registry publication, signing, or notarization was performed during preparation.

当前是发布准备记录，不代表已发布。发布矩阵严格覆盖 macOS arm64/x64、Windows x64、Linux GNU arm64/x64，各自使用匹配架构的原生 runner。本次本地验证仅覆盖 macOS arm64；其余原生平台和已配置的签名、公证仍须在 CI 通过。本次准备未执行发布、上传、签名或公证。

## Publication prerequisites / 发布前置条件

- Protect the `release` GitHub environment and restrict it to approved release tags. The workflow only triggers on `v*` tags and checks the tag against every product version.
- Protect/register the `skillshub-app` npm scope and configure Trusted Publishing for all six packages, this repository's `release.yml`, and the `release` environment. Use npm account 2FA; no long-lived npm token is supported by this workflow. Initial package/trust setup is an external prerequisite, not performed here.
- All five platform builds and native offline npm smoke tests must succeed. The five platform tarballs publish first using OIDC/provenance; only successful completion permits publishing `skillshub-cli`. A partial platform publication cannot publish the main package; release operators must resolve a failed publication before retrying immutable versions.
- Every release tag independently runs the five-platform native `verify` matrix at that tag's SHA, including CLI/compatibility tests and Windows native bridge tests. Build and publication jobs explicitly depend on this gate; previous PR/main CI results are not used as substitutes.
- macOS uses the existing imported signing identity and conditionally submits the desktop and CLI for notarization. Windows reuses desktop signing configuration when present and otherwise emits an explicit unsigned warning; updater signatures are not Windows Authenticode signatures.

需保护 GitHub `release` 环境、限制 release tag，保护 npm 组织及六个包并预先配置 Trusted Publishing/2FA。五个平台全部成功后先发布平台包，再发布主包；任一失败都不会发布主包。macOS 使用已有证书能力，并按凭据配置执行可选公证；Windows 无签名配置时明确提示未签名。环境规则、npm 权限与首发包建立需要发布负责人配置，本次未更改远端设置。

每个 release tag 都在自身 SHA 上独立执行五平台原生 `verify`，包含 CLI/兼容性与 Windows 原生桥接测试；构建、产物和发布必须等待全部成功，不能以此前 PR/main CI 代替。macOS 两份公证响应均须解析为 `Accepted`，之后才给 app 附加并验证公证票据，重新生成 updater 归档及签名；独立 Mach-O CLI 不支持附加票据，继续校验其代码签名与字节一致性。

## Validation / 验证

The compatibility test starts with a frozen v0.10.1 shared-schema fixture, performs CLI install/tag/deploy, reopens the desktop service with identical runtime paths, and reads the result with v0.10.1-compatible SQL. It verifies schema 6 stays unchanged and unknown newer schemas fail closed without writes. Full frontend/Rust checks, host npm tarball smoke, and desktop development startup are recorded in the task report.

兼容性测试从冻结的 v0.10.1 共享表结构开始，经 CLI 安装、打标签、部署，再用相同运行路径重开桌面服务并通过旧版兼容 SQL 读取。验证共享 schema 6 不升级、未知新 schema 拒绝写入。完整检查、主机离线 npm 包冒烟和桌面开发版启动证据记录在任务报告。

References: [GitHub native runners](https://docs.github.com/en/actions/reference/runners/github-hosted-runners), [npm Trusted Publishing](https://docs.npmjs.com/trusted-publishers/).
