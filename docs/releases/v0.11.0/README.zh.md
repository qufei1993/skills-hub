# Skills Hub v0.11.0

[English](README.md)

v0.11.0 支持通过 AI 对话管理 Skills，由官方 `manage-skills-hub` Skill 和 `skillshub-cli` 执行。在「设置 → AI 管理」一键启用后，AI 安装默认同步到已检测、已启用的工具；切换相关页面或切回应用即可刷新列表。CLI 与桌面共用本地库，可在桌面未启动或未安装时工作。

详细说明见 [Agent 优先的命令行入口](agent-first-cli.zh.md) 和 [桌面 CLI 桥接](verified-cli-bridge.zh.md)。

## 发布状态

当前是发布准备记录，不代表已发布。发布矩阵覆盖 macOS arm64/x64、Windows x64、Linux GNU arm64/x64，各自使用匹配架构的原生运行环境。本次本地验证仅覆盖 macOS arm64；其余原生平台和已配置的签名、公证仍须在 CI 通过。本次准备未执行正式发布、npm 发布、签名或公证。

## 发布前置条件

- 保护 GitHub 的 `release` 环境，将其限制为经过批准的版本标签。工作流仅由 `v*` 标签触发，并检查标签与全部产品版本一致。
- 注册并保护 `skillshub-app` npm 作用域，为全部六个包配置与本仓库 `release.yml`、`release` 环境对应的 Trusted Publishing。npm 账号启用双重验证；工作流不使用长期 npm Token。首次建包和信任配置需要发布负责人预先完成。
- 五个平台构建及原生离线 npm 安装冒烟测试必须全部成功。先通过 OIDC 和来源证明发布五个平台包，再发布 `skillshub-cli` 主包。平台包仅部分发布成功时不得发布主包；版本不可覆盖，重试前须处理已发布包的状态。
- 每个版本标签都在自身提交上独立运行五平台原生 `verify`，包括 CLI、兼容性及 Windows 原生桥接测试。构建和发布必须等待全部通过，不能用此前 PR 或主分支的检查结果替代。
- macOS 使用已有导入证书签名，并按凭据配置对桌面应用和 CLI 进行公证。Windows 使用已有桌面签名配置；未配置时明确警告未签名。应用更新签名不等同于 Windows Authenticode 签名。

macOS 两份公证响应都必须为 `Accepted`，才能给应用附加并验证公证票据，重新生成更新归档及签名。独立 Mach-O CLI 不支持附加票据，仍须校验代码签名与二进制哈希。

环境保护规则、npm 权限、首次建包及信任配置由发布负责人管理，本次未修改这些远端设置。

## 验证

兼容性测试从冻结的 v0.10.1 共享表结构开始，经 CLI 安装、打标签、部署，再用相同运行路径重开桌面服务，并通过旧版兼容 SQL 读取。测试验证共享数据库结构版本 6 不升级，未知新版本拒绝写入。

完整前端与 Rust 检查、本机离线 npm 包冒烟测试和桌面开发版启动证据记录在任务报告中。

参考：[GitHub 原生运行环境](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)、[npm Trusted Publishing](https://docs.npmjs.com/trusted-publishers/)。
