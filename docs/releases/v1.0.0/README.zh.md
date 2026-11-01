# Skills Hub v1.0.0

[English](README.md)

本版本已于 2026 年 10 月 11 日发布。安装包与中英文发布说明见 [Skills Hub v1.0.0 发布页](https://github.com/qufei1993/skills-hub/releases/tag/v1.0.0)。

## 功能与优化

| 主题 | 用户可见变化 | 说明与 PR |
| --- | --- | --- |
| 所选 Skills 批量更新 | 更新选中的多个 Skills，复用 Git／本地来源，保留工具选择和分发范围。 | 逐项进度、当前项结束后取消、结果与错误详情保留、失败项重试；停用或未绑定来源的项跳过。 |
| 合集探索 | 浏览、搜索官网合集，滚动加载并复用版本化缓存。 | [探索与搜索](collection-discovery.zh.md)；[PR #187](https://github.com/qufei1993/skills-hub/pull/187) |
| 合集安装 | 官网唤起后选择 Skills，设置标签与分发范围，重试失败项。 | [官网唤起](website-collection-install.zh.md)、[安装工作区](collection-install-workspace.zh.md)；[PR #185](https://github.com/qufei1993/skills-hub/pull/185)、[PR #187](https://github.com/qufei1993/skills-hub/pull/187) |
| 统一安装 | Git、本地和合集共用选择与设置两步流程。 | [安装流程](unified-installation.zh.md)；[PR #187](https://github.com/qufei1993/skills-hub/pull/187) |
| 官网版本数据 | 更新下载链接与中英文日志，无需重新构建官网。 | [独立发布](website-release-data.zh.md)；[PR #184](https://github.com/qufei1993/skills-hub/pull/184) |

## Bug 修复

| 问题 | Issue | PR | 说明 |
| --- | --- | --- | --- |
| 停用后启用被已关闭工具阻断，其他工具未恢复同步 | — | — | [恢复原有工具选择](skill-enable-recovery.zh.md) |
| 后台更新创建额外窗口或 macOS Dock 图标 | [#152](https://github.com/qufei1993/skills-hub/issues/152) | [#193](https://github.com/qufei1993/skills-hub/pull/193) | [后台无窗口更新](headless-updates.zh.md) |
| 来源删除文件导致技能更新被拦截 | — | — | [与来源保持一致](source-mirror-updates.zh.md) |
| Windows 定时任务本地语言错误乱码 | [#181](https://github.com/qufei1993/skills-hub/issues/181) | [#188](https://github.com/qufei1993/skills-hub/pull/188) | [错误解码](windows-scheduler-output.zh.md) |
| Windows 子目录更新报 `path not found in repo` | [#182](https://github.com/qufei1993/skills-hub/issues/182) | [#189](https://github.com/qufei1993/skills-hub/pull/189) | [路径兼容](windows-subpath-updates.zh.md) |
| Skill 更新中断后一直显示运行中、无法重试 | [#183](https://github.com/qufei1993/skills-hub/issues/183) | [#192](https://github.com/qufei1993/skills-hub/pull/192) | [中断恢复](auto-update-recovery.zh.md) |
| CLI 导入含排除项的目录时误报 `PLAN_STALE` | [#186](https://github.com/qufei1993/skills-hub/issues/186) | [#191](https://github.com/qufei1993/skills-hub/pull/191) | [来源路径规范化](cli-adopt-excluded-entries.zh.md) |

合集相关修复还包括大型清单、固定版本仓库根链接、冲突项选择、完成提示、取消检测、确认状态保留、官网唤起、Linux 协议注册、Windows 候选路径及项目目录对齐。完整列表见[中文更新日志](../../CHANGELOG.zh.md)的「修复」部分及各主题说明。
