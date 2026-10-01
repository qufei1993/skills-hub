# Skills Hub v0.11.0

[English](README.md)

## 下载安装

| 系统 | 适用电脑 | 安装包 |
| --- | --- | --- |
| macOS | Apple 芯片（M1 / M2 / M3 等） | [下载 .dmg](https://github.com/qufei1993/skills-hub/releases/download/v0.11.0/Skills-Hub-v0.11.0-macOS-aarch64.dmg) |
| macOS | Intel 芯片 | [下载 .dmg](https://github.com/qufei1993/skills-hub/releases/download/v0.11.0/Skills-Hub-v0.11.0-macOS-x86_64.dmg) |
| Windows | Intel / AMD 64 位电脑 | [下载 .exe](https://github.com/qufei1993/skills-hub/releases/download/v0.11.0/Skills-Hub-v0.11.0-Windows-x64.exe) |

桌面用户只需下载安装包，CLI 会在启用 AI 管理时自动下载。

v0.11.0 支持通过 AI 对话管理 Skills，由官方 `manage-skills-hub` Skill 和 `skillshub-cli` 执行。在「设置 → AI 管理」一键启用后，AI 安装默认同步到已检测、已启用的工具；切换相关页面或切回应用即可刷新列表。CLI 与桌面共用本地库，按桌面版本下载安装，安装包不再携带 CLI。首次启用需要联网；已校验通过的对应版本可离线使用，关闭桌面端后仍可运行。CLI 和官方 Skill 更新由设置中的更新按钮触发，启动不自动更新。

详细说明见 [Agent 优先的命令行入口](agent-first-cli.zh.md) 和 [桌面 CLI 桥接](verified-cli-bridge.zh.md)。

## Git 仓库增量安装

再次安装多 Skill 仓库时，按仓库、分支选择和子路径识别同源技能，原位更新已有项并安装新增项；内容无变化则保留原记录。桌面选择列表展示新增和更新数量，以绿色、蓝色、橙色标签区分新技能、检查更新和冲突跳过，单项失败不影响其他选中项继续执行。冲突项显示明确的禁用复选框，描述默认最多三行，超出后可展开或收起。

更新保留技能 ID、标签、启用状态及原有同步配置。本地修改以及会删除受管理文件的更新会被拦住。旧记录缺少内容指纹时，根据原 Git 提交验证；若缓存中已无法找回原提交，则暂停该项，交由用户检查。本次不包含跨来源覆盖或交互式重命名；仓库内容遵循已配置的 Git 缓存有效期。

## Cline 桌面版支持

更新现有 Cline 适配：通过 `~/.cline` 检测安装状态，全局同步到 `~/.cline/skills`，项目同步到 `.cline/skills`，与 [Cline 官方目录说明](https://docs.cline.bot/customization/skills)一致。共享目录预览不再将 Cline 与 `.agents/skills` 工具归为一组。已有 `.agents/skills` 文件不会迁移或删除；请将所需 Skills 重新同步到 Cline，写入其原生目录。

## DeepSeek Harness 自定义目录

Skills Hub 继承的 `DSH_HOME` 非空白时，全局同步和扫描使用 `$DSH_HOME/skills`，安装检测使用 `$DSH_HOME`。未设置或值为空白时回退到 `~/.dsh`；支持将 `~`、`~/` 和 `~\` 展开为当前用户主目录。项目同步仍使用 `<project>/.dsh/skills`。

修改环境变量后，请重启 Skills Hub，必要时也重启启动它的程序，以便读取新值。对于已同步到旧目录的 Skills，先取消 DeepSeek Harness 同步，再重新同步。取消操作使用已保存的部署路径，即使新目录尚不存在也能处理；用户修改过的副本会被保护，请备份并解决提示的冲突后重试。启动时不搬动文件、不丢弃记录；旧记录路径不匹配时仍会阻止直接重新部署。Harness 显式 `dshHome` 配置和 `customSkillDirs` 仍需在 Skills Hub 中配置自定义工具。

## 开发者构建

[本地安装测试构建](local-test-builds.zh.md)内嵌本地 CLI，用于发布前测试。它使用独立应用身份、开发凭据和 CLI 目录，禁用正式版更新；技能数据、配置、缓存、回收站及写锁仍与正式版共享，操作会影响真实工具目录。正式打包仍需匹配的最终 release CLI 清单，不携带 CLI 可执行文件。

## 发布准备与前置条件

本记录不代表已公开。此前内置 CLI 的草稿已替换为 v0.11.0 标签对应的按需下载 CLI 候选版本。新版本保持草稿状态，等待发布负责人明确批准公开。

- 保护 GitHub 的 `release` 环境，仅允许已批准标签。工作流由 `v*` 标签触发，并检查全部产品版本一致。
- 每个标签在自身提交上独立运行五平台原生 CLI 验证：macOS arm64/x64、Windows x64、Linux GNU arm64/x64，包括兼容性和 Windows 桥接测试。此前 PR/main CI 不能替代此门禁。桌面打包覆盖 macOS arm64/x64、Windows x64。
- 先对 CLI 签名并完成已配置的公证，再生成最终字节清单。桌面构建使用匹配清单，不内置 CLI。macOS 使用已配置的签名身份；Windows 配置签名时签名，否则提示未签名。更新签名不等同于 Windows Authenticode 签名。
- CLI 与桌面分别公证，配置公证时各自须返回 `Accepted`。桌面附加并验证票据后，重新生成更新归档和签名。独立 Mach-O CLI 不能附加票据，须校验代码签名和哈希。
- CLI 二进制、校验文件、清单及桌面产物进入原仓库同一个 Release 草稿。公开前核对 CLI 长度与摘要，公开后验证匿名下载，同名冲突资源不覆盖。使用原仓库有 Contents 写权限的 GITHUB_TOKEN，无需独立资源仓库或 npm 发布凭据。

发布工作流只准备草稿，不自动公开。完成 CLI 与桌面资源上传及校验后，等待发布负责人确认；明确批准公开后再发布并执行匿名下载验证。草稿状态下无法验证公开下载。

GitHub 按标签查询返回 404 时，草稿查询改用已认证的分页发布列表，避免创建重复草稿；同一标签存在多个发布记录时停止操作，要求先核对并清理。

## 验证记录

兼容性测试从冻结的 v0.10.1 共享表结构开始，经 CLI 安装、打标签、部署，再用相同运行路径重开桌面服务并通过旧版兼容 SQL 读取。共享 schema 6 不升级，未知新版拒绝写入。

- [当前按需 CLI 验证与剩余发布门禁](cli-on-demand-download.zh.md)
- [本地安装测试构建验证](local-test-builds.zh.md)
- [CLI 桥接与构建模式](verified-cli-bridge.zh.md)
- [独立 CLI 安装](cli-installation.md)

- [设备同步后的 AI 一键管理修复](ai-management-sync-hash.zh.md)

- [发布构建复用](build-performance.zh.md)
