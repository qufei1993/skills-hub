# v0.6.0 小需求与体验优化记录

本文件保留 v0.6.0 发布周期的历史记录，不再追加后续版本的功能或修复。新变更遵循[变更与发布规则](../../../CONTRIBUTING.md#change-records-and-releases)。

## 2026-05-05

### 删除 My Skills 筛选栏刷新按钮

- 删除 My Skills 筛选栏中的 `Refresh` / `刷新` 按钮，解决中文界面下按钮样式错乱问题（GitHub Issue #61，PR #63）。
- 确认该按钮仅手动重新读取当前 Skill 列表和标签，不触发重新扫描工具、Git 更新或重新同步。
- 安装、删除、同步、编辑标签等操作后已有自动刷新，不影响现有数据更新流程。
- 修复验证（PR #63）：`npm run check`。

### 查看并导入 Skill 增加搜索栏

- 在“查看并导入 Skill”弹窗中新增搜索栏，支持按名称、描述和路径筛选候选项（GitHub Issue #57）。
- 搜索结果会同步影响“全选”和已选数量统计，方便在多个 Skill 中快速定位目标。
- 本地目录导入和 Git 仓库导入两个入口都已支持该交互。
- “查看发现的 Skills”弹窗也已补上搜索栏，支持按 Skill 名称或路径筛选已发现条目。
- 修复验证：`npm run lint`、`npx tsc -b`、`npm run rust:fmt:check`、`npm run rust:clippy`、`npm run rust:test`。
