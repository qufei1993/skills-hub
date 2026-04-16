# 更新日志

本文件记录项目的重要变更（中文版本）。

## [Unreleased]

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
- **导入弹窗关闭行为**：本地 Skill 导入出现部分失败时，包括部分同步目标已存在的情况，导入弹窗现在会正确关闭（[#51](https://github.com/qufei1993/skills-hub/issues/51)）。
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
