# Skills Hub v1.0.1

[English](README.md)

本版本已于 2026 年 10 月 16 日发布。安装包与中英文发布说明见 [Skills Hub v1.0.1 发布页](https://github.com/qufei1993/skills-hub/releases/tag/v1.0.1)。

## 功能与优化

| 主题 | 用户可见变化 | 说明与 PR |
| --- | --- | --- |
| 状态与工具筛选 | 按 Skills 启用状态和当前同步工具筛选，计数随其他条件联动，清除筛选时一起重置。 | [PR #197](https://github.com/qufei1993/skills-hub/pull/197) |
| 工具图标即时提示 | 悬停或键盘聚焦时立即显示工具名称与同步状态，图标不再重叠。 | [PR #198](https://github.com/qufei1993/skills-hub/pull/198) |

## Bug 修复

| 问题 | Issue | PR | 说明 |
| --- | --- | --- | --- |
| 切换工具同步状态后，已打开的提示可能仍显示旧状态。 | — | [#202](https://github.com/qufei1993/skills-hub/pull/202) | 即时更新提示，停用 Skill 或移除工具时隐藏提示。 |
| 批量更新全部成功或无变化后，结果面板仍需手动关闭。 | — | [#203](https://github.com/qufei1993/skills-hub/pull/203) | 改为短暂完成提示，需要处理的结果继续保留。 |
| 合集安装改用默认分支最新提交，同批安装及重试保持版本一致，新安装项支持后续更新。 | — | [#199](https://github.com/qufei1993/skills-hub/pull/199) | 保留已安装的同源 Skills，兼容官网链接和现有合集数据。 |
| Windows junction 部署可能在预览成功后失败，CLI 本地路径可能丢失原有字符。 | [#196](https://github.com/qufei1993/skills-hub/issues/196) | [#201](https://github.com/qufei1993/skills-hub/pull/201) | 修复规范化路径处理，补充安全的部署失败诊断。 |
| 无关 Git LFS 资源可能阻断子目录 Skill 扫描和安装。 | [#195](https://github.com/qufei1993/skills-hub/issues/195) | [#200](https://github.com/qufei1993/skills-hub/pull/200) | 仅检出所需子目录，持续显示扫描错误。 |

完整变更列表见[中文更新日志](../../CHANGELOG.zh.md#101)。
