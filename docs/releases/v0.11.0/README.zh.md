# Skills Hub v0.11.0

[English](README.md)

v0.11.0 支持通过 AI 对话管理 Skills，由官方 `manage-skills-hub` Skill 和 `skillshub-cli` 执行。在「设置 → AI 管理」一键启用后，AI 安装默认同步到已检测、已启用的工具；切换相关页面或切回应用即可刷新列表。CLI 与桌面共用本地库，随桌面端打包，仅在主动启用 AI 管理时安装或更新；普通启动不安装，关闭桌面端后仍可使用。

详细说明见 [Agent 优先的命令行入口](agent-first-cli.zh.md) 和 [桌面 CLI 桥接](verified-cli-bridge.zh.md)。

## Git 仓库增量安装

再次安装多 Skill 仓库时，按仓库、分支选择和子路径识别同源技能，原位更新已有项并安装新增项；内容无变化则保留原记录。桌面选择列表展示新增和更新数量，以绿色、蓝色、橙色标签区分新技能、检查更新和冲突跳过，单项失败不影响其他选中项继续执行。

更新保留技能 ID、标签、启用状态及原有同步配置。本地修改以及会删除受管理文件的更新会被拦住。旧记录缺少内容指纹时，根据原 Git 提交验证；若缓存中已无法找回原提交，则暂停该项，交由用户检查。本次不包含跨来源覆盖或交互式重命名；仓库内容遵循已配置的 Git 缓存有效期。

## Cline 桌面版支持

更新现有 Cline 适配：通过 `~/.cline` 检测安装状态，全局同步到 `~/.cline/skills`，项目同步到 `.cline/skills`，与 [Cline 官方目录说明](https://docs.cline.bot/customization/skills)一致。共享目录预览不再将 Cline 与 `.agents/skills` 工具归为一组。已有 `.agents/skills` 文件不会迁移或删除；请将所需 Skills 重新同步到 Cline，写入其原生目录。

## 发布状态

当前是发布准备记录，不代表已发布。发布矩阵覆盖 macOS arm64/x64、Windows x64、Linux GNU arm64/x64，各自使用匹配架构的原生运行环境。本次本地验证仅覆盖 macOS arm64；其余原生平台和已配置的签名、公证仍须在 CI 通过。本次准备未执行正式发布、签名或公证。

## 发布前置条件

- 保护 GitHub 的 `release` 环境，将其限制为经过批准的版本标签。工作流仅由 `v*` 标签触发，并检查标签与全部产品版本一致。
- 每个版本标签都在自身提交上独立运行五平台原生 `verify`，包括 CLI、兼容性及 Windows 原生桥接测试。构建和发布必须等待全部通过，不能用此前 PR 或主分支的检查结果替代。
- macOS 使用已有导入证书签名，并按凭据配置对桌面应用和 CLI 进行公证。Windows 使用已有桌面签名配置；未配置时明确警告未签名。应用更新签名不等同于 Windows Authenticode 签名。

macOS 两份公证响应都必须为 `Accepted`，才能给应用附加并验证公证票据，重新生成更新归档及签名。独立 Mach-O CLI 不支持附加票据，仍须校验代码签名与二进制哈希。

环境保护规则由发布负责人管理，本次未修改远端设置。CLI 不再发布到 npm，无需配置 npm 作用域、包权限、Token 或 Trusted Publishing。

## 验证

兼容性测试从冻结的 v0.10.1 共享表结构开始，经 CLI 安装、打标签、部署，再用相同运行路径重开桌面服务，并通过旧版兼容 SQL 读取。测试验证共享数据库结构版本 6 不升级，未知新版本拒绝写入。

完整前端与 Rust 检查、本机内置 CLI 冒烟测试和桌面开发版启动证据记录在任务报告中。

参考：[GitHub 原生运行环境](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)。

[独立 CLI 安装](cli-installation.md)：通过一条命令安装和升级，无需 npm。
