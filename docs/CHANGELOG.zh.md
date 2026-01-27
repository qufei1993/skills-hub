# 更新日志

本文件记录项目的重要变更（中文版本）。

## [Unreleased]

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
