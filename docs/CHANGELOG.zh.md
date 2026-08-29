# 更新日志

本文件记录项目的重要变更（中文版本）。

## [Unreleased]

## [0.9.1] - 2026-08-28

### 变更
- **双语 GitHub Release 说明**：发布工作流现在会同时截取中英文更新日志，并在 GitHub Release 与应用更新元数据中依次提供中文、英文更新内容及对应的平台安装提示。应用内更新弹窗只展示与当前界面语言匹配的分段，同时继续完整展示旧格式或不完整的发布说明。
- **应用更新完成流程**：发现新版本时，标题栏会显示更醒目的更新入口，并在鼠标悬停或键盘聚焦时从图标展开为本地化文案。安装完成后可以在完成弹窗中立即重启，也可以先关闭弹窗，稍后通过标题栏持续显示的“重启”入口完成重启。

### 修复
- **旧版 macOS 的 Skill 详情兼容性**：修复不支持正则后行断言的 WKWebView 在打开 Skill Markdown 文件时导致应用整页白屏的问题。详情渲染器现在会检测该能力，在不支持时降级为标准 Markdown，并在支持的系统上继续提供完整 GFM 渲染（[#108](https://github.com/qufei1993/skills-hub/issues/108)）。
- **Tauri WebView 标签重命名弹窗**：标签重命名改用应用内输入弹窗，不再调用 macOS 上无法显示的浏览器 `window.prompt`；剩余的共享目录确认也改用现有应用内弹窗，不再调用 `window.confirm`（[#109](https://github.com/qufei1993/skills-hub/issues/109)）。
- **响应式 Skill 详情工作区**：将固定宽度的详情抽屉改为随主窗口伸缩的完整工作区，保证文件树与 Markdown 内容拥有可读空间，并在紧凑的响应式头部展示范围、已同步工具、标签、来源、更新时间和文件数。来源路径支持直接复制，Markdown 元数据表不再重复展示 `name` 和 `description` 字段（[#111](https://github.com/qufei1993/skills-hub/issues/111)）。
- **Windows 目录联接清理与已有 Skill 导入**：停用、取消同步或删除 Skill 时，现在可以正确移除 Windows 目录联接且不会影响其目标目录。发现流程也可以复用中央存储中内容完全相同的托管记录，同时继续拒绝普通本地重复安装和内容冲突（[#110](https://github.com/qufei1993/skills-hub/pull/110)）。
- **Windows 自动更新计划**：默认 24 小时间隔及其他整天间隔现在会通过 `schtasks` 注册为按天执行，不再生成超过 Windows 上限的小时修饰值。受支持的分钟和小时间隔继续使用原有计划类型，无法精确表达的非整天长间隔会在创建任务前返回明确错误（[#102](https://github.com/qufei1993/skills-hub/issues/102)）。

## [0.9.0] - 2026-08-23

### 新增
- **DeepSeek Harness 工具适配**：新增通过 `~/.dsh/skills` 和 `.dsh/skills` 进行全局及项目级 Skill 同步的支持。Skills Hub 会通过默认的 `~/.dsh` 目录检测 DeepSeek Harness，并在工具管理和同步流程中展示 DeepSeek 产品图标。
- **发现扫描设置**：新增按实际目录持久化的扫描来源开关，在设置页提供永久入口，并在发现提示旁提供快捷入口。保存后会刷新发现结果，不改变已托管或已同步的 Skills。

### 变更
- **内置工具目录**：内置适配器数量更新为 47，并在中英文支持矩阵中补充 DeepSeek Harness 路径规则。
- **发现结果校验**：工具目录下的子目录只有包含 `SKILL.md` 时才会作为可导入 Skill 展示，减少无关扫描结果。

## [0.8.1] - 2026-08-06

### 修复
- **中心存储目录提示**：从本地目录添加 Skill 时，如果与用户直接放入 Skills Hub 中心存储目录的文件夹冲突，错误提示现在会说明该目录由应用自动管理，并指引用户将 Skill 移出后重新导入（[#103](https://github.com/qufei1993/skills-hub/issues/103)）。

## [0.8.0] - 2026-07-17

### 新增
- **卡片与列表视图**：My Skills 支持在响应式卡片和紧凑列表布局之间切换，并在切换后保留选择和筛选状态。
- **可折叠桌面导航**：侧边栏可以折叠为纯图标导航，在较小窗口尺寸下为工作区释放更多空间。
- **易识别的工具身份**：内置 AI 编程工具在工具管理、安装流程和 Skill 同步状态中统一使用对应产品图标。
- **自定义工具身份与同步模式**：自定义工具支持头像，并可选择自动、软链接、Windows 目录联接或复制模式；旧配置自动沿用无头像和自动模式。
- **自定义工具编辑**：创建后可以修改名称、头像、全局/项目目录和同步模式，已有同步目标会迁移到新目录或按新模式重建。
- **标题栏更新状态**：应用启动后在标题栏检查新版本，发现更新时显示可点击图标并复用现有更新说明与安装流程。

### 变更
- **桌面 UI 重构**：My Skills、Explore、标签、工具、更新、设置和安装流程统一桌面布局、间距体系、滚动行为和响应式规则。
- **安装流程重构**：在线、Git 仓库和本地目录安装使用统一结构，标签、工具、范围、摘要和操作区域始终清晰可见。
- **Skills Hub 品牌视觉**：替换应用 Logo 并重新生成桌面图标，使新 Skills Hub 标志在应用、安装包和操作系统启动器中保持一致。
- **工具卡片操作层级**：自定义工具卡片右上角只保留启停开关，编辑和删除移动到底部管理操作区。
- **GitHub 项目入口**：开源项目链接移动到设置页“GitHub 与网络”区域，并通过系统默认浏览器打开。
- **Skills 卡片列宽**：卡片视图使用稳定列宽，筛选结果只有一个时不再横向撑满内容区。

### 修复
- **首次启动加载遮罩**：可导入 Skills 的启动扫描改为后台静默执行，不再短暂显示“正在安装技能”弹窗。
- **自定义工具目标安全迁移**：目录和同步模式变化时保护共享物理目标，并在新路径冲突或迁移失败时拒绝保存无效配置。
- **外链打开权限**：补充限制到 Skills Hub 仓库的 Tauri opener 权限，修复 GitHub 项目页面无法打开的问题。

## [0.7.1] - 2026-07-10

### 修复
- **统一网络代理**：应用更新检查、更新下载与安装、更新说明以及 Explore 在线搜索现在都会使用已配置的网络代理（[PR #91](https://github.com/qufei1993/skills-hub/pull/91)）。

## [0.7.0] - 2026-07-05

### 新增
- **安装范围选择**：安装 Skill 前即可选择全局同步或项目级同步，本地目录和 Git 仓库的批量安装流程也会复用该范围选择。
- **批量 Skill 管理**：在 My Skills 中选择多个 Skill 后，可批量设置标签、目标工具、启用/停用和删除。
- **Skill 启用/停用**：可以临时停用 Skill 而不删除配置；重新启用后会按之前的工具同步设置恢复。
- **管理中心**：将标签、工具配置和 Skills 自动更新集中到独立的管理区域。
- **工具管理**：可启用或停用内置工具目标；停用后的工具不会继续出现在 Skill 卡片、安装流程和同步操作中。
- **自定义工具目录**：支持添加自定义工具目标，配置全局 skills 目录和可选的项目级 skills 目录。
- **Skills 定时自动更新**：支持配置系统级后台更新任务，手动触发立即更新，并在管理中心查看运行结果。
- **GitHub 网络代理设置**：可为 GitHub API、精选 Skills、GitHub 下载和 Git 更新流程配置本地代理。

### 变更
- **设置页组织方式**：设置页聚焦应用偏好，包括界面语言、外观、存储、缓存、GitHub 访问、网络代理和应用更新。
- **管理页面布局**：标签、工具和更新页面统一工作区边距和 tab 切换体验。
- **批量工具同步行为**：批量设置工具时，以当前勾选的工具列表作为所选 Skills 的最终目标状态。
- **自动更新结果展示**：更新状态、检查数量、更新数量、失败数量、开始时间、完成时间和耗时改为页面内展示，不再依赖独立进度弹窗。
- **安装弹窗布局**：新增 Skill 流程使用更紧凑的安装范围选择器，候选列表较长时底部操作仍保持可见。

### 修复
- **本地批量安装审核**：修复本地 Skill 目录批量安装和候选选择中的边界问题。
- **项目范围分组**：共享同一项目 skills 目录的工具按目标目录分组，保持项目同步状态一致。
- **弹窗遮挡**：修复弹窗内容过长时可能遮挡或隐藏底部操作的问题。
- **定时任务 lint 稳定性**：清理平台相关的调度代码路径，确保 Rust lint 在各目标平台检查通过。

## [0.6.3] - 2026-06-27

### 修复
- **Antigravity 全局同步路径**：将 Antigravity 2.0 的全局 Skill 同步目录更新为 `~/.gemini/config/skills`，确保当前版本 Antigravity 可以发现已同步的 Skill（[#79](https://github.com/qufei1993/skills-hub/issues/79)）。
- **Windows 自动更新元数据**：为 Windows 发布产物补充 updater 签名，并在 `updater.json` 中新增 `windows-x86_64` / `windows-aarch64` 平台条目，修复 Windows 检查更新失败的问题（[#78](https://github.com/qufei1993/skills-hub/issues/78)、[PR #82](https://github.com/qufei1993/skills-hub/pull/82)）。

## [0.6.2] - 2026-06-19

### 新增
- **WorkBuddy 工具适配**：新增全局 Skill 同步支持，目录为 `~/.workbuddy/skills/`（[PR #73](https://github.com/qufei1993/skills-hub/pull/73)）。
- **CodeWhale 工具适配**：新增全局和项目级 Skill 同步支持，目录分别为 `~/.codewhale/skills/` 和 `.codewhale/skills/`（[#70](https://github.com/qufei1993/skills-hub/issues/70)、[PR #74](https://github.com/qufei1993/skills-hub/pull/74)）。
- **Claude Code 插件 Skill 自动发现**：支持发现并导入用户级 Claude Code 插件中的 Skill，包括 `.claude-plugin/plugin.json` 声明的自定义 Skill 路径（[#69](https://github.com/qufei1993/skills-hub/issues/69)、[PR #75](https://github.com/qufei1993/skills-hub/pull/75)）。

## [0.6.1] - 2026-05-16

### 修复
- **窗口关闭行为**：点击主窗口关闭按钮现在会直接退出应用，不再隐藏到后台，修复应用仍在运行但无法从 Dock 或任务栏重新打开的问题（[PR #68](https://github.com/qufei1993/skills-hub/pull/68)）。

## [0.6.0] - 2026-05-05

### 新增
- **Skill 标签**：可为已托管 Skill 添加自定义标签，方便整理和筛选。
- **标签页面**：新增独立 Tags / 标签页面，支持新建、重命名、删除标签，并可快速跳回已筛选的 My Skills 视图。
- **标签筛选**：My Skills 支持按一个或多个标签筛选，使用 OR 匹配；同时提供虚拟 `Untagged` / `无标签` 筛选项。
- **单个 Skill 标签编辑**：可直接从 Skill 卡片打开标签编辑入口，调整该 Skill 的标签关联。
- **导入搜索**：从本地目录或 Git 仓库导入前，可按名称、描述或路径搜索候选 Skill。

### 变更
- **My Skills 筛选栏**：移除手动刷新按钮；安装、删除、同步和编辑标签等流程已自动刷新列表。

### 修复
- **中文筛选栏布局**：移除刷新按钮后，修复中文界面下按钮区域拥挤和样式错乱问题。
- **发现 Skill 审核弹窗**：查看已发现 Skills 时支持搜索，并让选择数量与筛选结果保持一致。

## [0.5.0] - 2026-04-16

### 新增
- **项目级 Skill 同步**：Skill 现在可以同步到指定项目目录，不再只支持同步到各工具的全局目录。
- **同步范围控制**：My Skills 卡片新增范围徽标（全局 / 项目数量），并提供范围弹窗用于切换全局同步和项目同步。
- **范围筛选**：My Skills 支持按全部 / 全局 / 项目范围筛选。
- **Hermes Agent 工具适配**：新增 Hermes Agent 全局同步支持，目录为 `~/.hermes/skills`（[#54](https://github.com/qufei1993/skills-hub/issues/54)）。

### 变更
- **My Skills 筛选栏**：标题现在显示 Skill 总数，搜索框更紧凑，默认窗口下筛选控件保持单行展示。
- **默认窗口尺寸**：桌面端默认窗口从 `800x600` 调整为 `960x680`。
- **macOS 关闭行为**：点击主窗口关闭按钮现在隐藏窗口而不是退出应用；从 Dock 重新打开时会恢复并聚焦窗口。
- **项目级同步支持矩阵**：项目级同步改为按工具显式声明；未确认项目级 skills 目录的工具仅作为全局同步目标。

### 修复
- **同名同内容 Skill 导入接管**：导入已有 Skill 时，如果目标同名目录内容一致，现在可以安全接管同步状态。
- **取消同步后的工具重新启用入口**：从 Skill 取消同步的工具按钮会继续显示，便于重新启用。
- **SKILL.md 元数据解析**：正确解析 frontmatter 中的 YAML block scalar 描述，并在卡片和详情页正常展示。

## [0.4.3] - 2026-04-11

### 新增
- **Copaw 工具适配**：新增 Copaw AI 编程工具支持（感谢 @LeonDevLifeLog [PR#50](https://github.com/qufei1993/skills-hub/pull/50)）。

### 修复
- **Git 技能安装与 frontmatter 渲染**：修复 Git 技能安装及 frontmatter 元数据渲染问题。
- **Git 技能发现（容器路径）**：修复仓库使用容器风格目录路径时技能发现失败的问题。

## [0.4.2] - 2026-04-06

### 修复
- **检测到新工具弹窗样式**：「New tools detected」弹窗改用与其他弹窗一致的 `modal-header` + `modal-footer` 结构，修复标题缺少内边距和分隔线的问题（[#46](https://github.com/qufei1993/skills-hub/issues/46)）。
- **Git 技能名称推导**：从仓库根目录（subpath 为 `"."`）安装 Git 技能时，现在正确从仓库 URL 推导名称，不再以 `"."` 作为展示名称。

## [0.4.1] - 2026-03-21

### 新增
- **Frontmatter 元数据表格**：包含 YAML frontmatter 的 Markdown 文件在技能详情页顶部以 GitHub 风格的表格展示元数据。

## [0.4.0] - 2026-03-20

### 新增
- **应用内检查更新**：在设置页内直接检查新版本，支持下载安装，无需手动访问 GitHub Releases（[#33](https://github.com/qufei1993/skills-hub/issues/33)）。
- **QoderWork 工具适配**：新增 QoderWork 桌面 AI 代理支持（`~/.qoderwork/skills/`）（[#34](https://github.com/qufei1993/skills-hub/issues/34)）。

### 变更
- **设置页面化**：设置从模态弹窗升级为独立页面视图，与 My Skills / Explore 导航风格一致。
- **精选技能聚合**：Explore 数据源改为 7 个精选高质量仓库。

### 修复
- 切换语言时 Explore 页面短暂闪现「Installing Skills...」加载遮罩。

## [0.3.0] - 2026-03-15

### 新增
- **Explore 页面**：探索功能从弹窗提升为独立页面，顶部导航新增 My Skills / Explore 两个页面级 Tab 切换。
- **精选技能推荐**：Explore 页展示由 ClawHub API 预生成的热门技能列表（GitHub Actions 每日更新），支持前端筛选和一键安装。
- **在线技能搜索**：输入 ≥ 2 字符后通过 skills.sh API 实时搜索，500ms 防抖，搜索结果与精选列表自动去重、分区展示。
- **技能详情页**：点击技能名称进入详情视图，支持文件树浏览、Markdown 渲染（GFM + frontmatter 剥离）和代码语法高亮（40+ 语言，亮/暗主题自适应）。
- **技能描述字段**：安装时从 SKILL.md frontmatter 提取 description 存入数据库，My Skills 卡片展示描述文本。
- **GitHub Token 配置**：设置页新增可选的 GitHub Token 输入，认证后 API 限额从 60 提升至 5000 次/小时。
- **MoltBot 工具适配**：OpenClaw 更名拆分后新增独立的 MoltBot 工具支持。

### 修复
- Git 安装时 skill 名称为 "skills" 导致同步路径重复（[#28](https://github.com/qufei1993/skills-hub/issues/28)）。
- GitHub API 限流错误未提示重置时间，现在显示具体重置时间。
- Windows 同步时拒绝访问 OS error 5（[#20](https://github.com/qufei1993/skills-hub/issues/20)）。
- Git 仓库目录结构无法被正确识别为 skill（[#18](https://github.com/qufei1993/skills-hub/issues/18)、[#8](https://github.com/qufei1993/skills-hub/issues/8)）。
- 不支持 `.claude/skills/` 目录格式的仓库（[#27](https://github.com/qufei1993/skills-hub/issues/27)）。
- OpenClaw 路径更新（`.moltbot/skills` → `.openclaw/skills`）（[#29](https://github.com/qufei1993/skills-hub/issues/29)）。

### 变更
- My Skills 列表优化：工具徽章只显示已同步的工具，超过 5 个折叠为 `+N more`。
- 添加技能弹窗（Manual Add）精简为仅保留 Local Directory / Git Repository 两个 Tab。
- 多技能仓库在线安装时支持自动匹配（精确 → 唯一包含 → 回退手动选择）。

## [0.2.0] - 2026-02-01
### 新增
- **Windows 平台支持**：支持 Windows 构建与发布（感谢 @jrtxio [PR#6](https://github.com/qufei1993/skills-hub/pull/6)）。
- 新增多款工具适配与显示（如 Kimi Code CLI、Augment、OpenClaw、Cline、CodeBuddy、Command Code、Continue、Crush、Junie、iFlow CLI、Kiro CLI、Kode、MCPJam、Mistral Vibe、Mux、OpenClaude IDE、OpenHands、Pi、Qoder、Qwen Code、Trae/Trae CN、Zencoder、Neovate、Pochi、AdaL 等）。
- 前端新增共享技能目录提示与联动选择：同一全局 skills 目录的工具勾选/同步/取消同步会一起生效，并弹窗确认。
- 本地导入对齐 Git 规则的 multi-skill 发现，支持批量选择并展示无效项原因。
- 新增本地导入候选列表/按子路径安装的命令，并在安装前校验 SKILL.md。

### 变更
- Antigravity 默认全局技能目录更新为 `~/.gemini/antigravity/global_skills`。
- OpenCode 全局技能目录修正为 `~/.config/opencode/skills`。
- 工具状态接口增加 `skills_dir` 字段，前端列表与同步逻辑改为后端驱动并按目录去重。
- 同一 skills 目录的工具在同步/取消同步时统一写入与清理记录，避免重复文件操作与状态不一致。
- 本地导入流程改为先扫描候选：单个有效候选直接安装，多个候选进入选择列表。

## [0.1.1] - 2026-01-26

### 变更
- GitHub Actions 发版工作流：macOS 打包并上传 `updater.json`（`.github/workflows/release.yml`）。
- Cursor 同步固定使用 Copy：因为 Cursor 在发现 skills 时不会跟随 symlink：https://forum.cursor.com/t/cursor-doesnt-follow-symlinks-to-discover-skills/149693/4
- 托管技能更新时：对 copy 模式目标使用“纯 copy 覆盖回灌”；并对 Cursor 目标强制回灌为 copy，避免误创建软链导致不可用。

## [0.1.0] - 2026-01-24

### 新增
- Skills Hub 桌面应用（Tauri + React）初始发布。
- Skills 中心仓库：统一托管并同步到多种 AI 编程工具（优先 symlink/junction，失败回退 copy）。
- 本地导入：支持从本地文件夹导入 Skill。
- Git 导入：支持仓库 URL/文件夹 URL（`/tree/<branch>/<path>`），支持多 Skill 候选选择与批量安装。
- 同步与更新：copy 模式目标支持回灌更新；托管技能支持从来源更新。
- 迁移接管：扫描工具目录中已有 Skills，导入中心仓库并可一键同步。
- 新工具检测并可选择同步。
- 基础设置：存储路径、界面语言、主题模式。
- Git 缓存：支持按天清理与新鲜期（秒）配置。

### 构建与发布
- 本地打包脚本：macOS（dmg）、Windows（msi/nsis）、Linux（deb/appimage）。
- GitHub Actions 跨平台构建验证与 tag 发布 Draft Release（从 `CHANGELOG.md` 自动提取发布说明）。

### 性能
- Git 导入/批量安装优化：缓存 clone 减少重复拉取；增加超时与无交互提示提升稳定性。

[Unreleased]: https://github.com/qufei1993/skills-hub/compare/v0.9.1...HEAD
[0.9.1]: https://github.com/qufei1993/skills-hub/compare/v0.9.0...v0.9.1
[0.9.0]: https://github.com/qufei1993/skills-hub/compare/v0.8.1...v0.9.0
[0.8.1]: https://github.com/qufei1993/skills-hub/compare/v0.8.0...v0.8.1
[0.8.0]: https://github.com/qufei1993/skills-hub/compare/v0.7.1...v0.8.0
[0.7.1]: https://github.com/qufei1993/skills-hub/compare/v0.7.0...v0.7.1
