# Skills Hub v1.1.0

[English](README.md)

未发布。目标版本和软件包版本已统一为 1.1.0；2026 年 11 月 1 日已获授权开始发布，安装包构建和正式发布尚待完成。当前行为、兼容约束与验证证据统一维护在[实施说明](implementation.zh.md)，固定设计稿保留为历史基线。

## 功能与优化

| 主题 | 用户可见变化 | 详情与拉取请求 |
| --- | --- | --- |
| 关闭到托盘 | 关闭主窗口后继续运行，提供三语言托盘操作及窗口恢复。 | [PR #156](https://github.com/qufei1993/skills-hub/pull/156) |
| 项目管理 | 按项目筛选已安装 Skills，保留完整部署数量和常规操作，通过独立弹窗调整部署勾选。 | [技能库与项目](implementation.zh.md#library-and-project-management) · [PR #209](https://github.com/qufei1993/skills-hub/pull/209) |
| 技能预设与历史 | 列表直接编辑和应用预设，在“更多操作”中创建副本，采用紧凑选择与结果视图；保留冲突检查、历史、重试与撤销。 | [技能预设](implementation.zh.md#skill-presets) · [PR #209](https://github.com/qufei1993/skills-hub/pull/209) · [PR #212](https://github.com/qufei1993/skills-hub/pull/212) |
| 更新与恢复 | 主动检查来源，更新全部或所选待更新项，按需查看差异，恢复更新前内容并修复来源或缺失部署。 | [更新与恢复](implementation.zh.md#updates-and-recovery) · [PR #209](https://github.com/qufei1993/skills-hub/pull/209) |
| 界面组织 | 当前更新问题只展示一份，运行记录采用紧凑摘要与日志复制，相关流程支持键盘操作及三语言文案。 | [当前行为](implementation.zh.md#library-and-project-management) · [PR #212](https://github.com/qufei1993/skills-hub/pull/212) |

## Bug 修复

| 问题 | Issue | PR | 说明 |
| --- | --- | --- | --- |
| 项目筛选改变操作或隐藏部署，目标设置取消勾选后没有移除部署。 | — | — | 恢复纯筛选、完整项目选择及确认移除，新增失败时保留旧部署，见[技能库行为](implementation.zh.md#library-and-project-management)。 |
| 中心目录冲突未区分内容是否已被管理。 | — | [#217](https://github.com/qufei1993/skills-hub/pull/217) · [#121](https://github.com/qufei1993/skills-hub/pull/121) | 保留已有内容，并通过三语言持续提示处理方式。 |
| 收编来源分发回原工具目录时可能冲突。 | [#206](https://github.com/qufei1993/skills-hub/issues/206) | [#214](https://github.com/qufei1993/skills-hub/pull/214) | 内容一致时复用，内容不同时明确选择保留版本，保护已修改的部署。 |
| 中心内容缺失可能被误判为远端删除、显示为正常，或导致管理记录无法删除。 | — | [#215](https://github.com/qufei1993/skills-hub/pull/215) | 阻止设备同步并标明异常，允许用户明确删除记录并保留工具副本和来源，删除冲突使用对应场景提示。见[完整性规则](implementation.zh.md#data-integrity-and-compatibility)；关联报告 [#157](https://github.com/qufei1993/skills-hub/pull/157)、[#158](https://github.com/qufei1993/skills-hub/pull/158)。 |
| 冲突、重试和部分失败时，受影响目标或已完成进度不够明确。 | — | [#209](https://github.com/qufei1993/skills-hub/pull/209) | 保留选择和执行结果，显示可处理的冲突位置，并保护撤销及可恢复备份，见[完整性规则](implementation.zh.md#data-integrity-and-compatibility)。 |
| 技能库操作误入预设或增加确认，共享工具提示后只登记一个工具。 | — | [#212](https://github.com/qufei1993/skills-hub/pull/212) | 恢复工具切换与更新的直接操作，筛选与操作相互独立，已确认的共享工具一起处理，见[技能库行为](implementation.zh.md#library-and-project-management)。 |
| Git 检查可能因辅助进程卡住，或由单个故障来源阻塞其他结果。 | — | [#209](https://github.com/qufei1993/skills-hub/pull/209) | 超时或取消时终止辅助进程，隔离失败并排除内部缓存元数据，见[更新检查](implementation.zh.md#updates-and-recovery)。 |
| 已管理来源可能被重复发现，或被重叠的同步目标覆盖。 | [#208](https://github.com/qufei1993/skills-hub/issues/208) | [#211](https://github.com/qufei1993/skills-hub/pull/211) | 跳过已管理导入和受保护来源目标，继续同步其他可处理工具。 |

完整变更列表见 [v1.1.0](../../CHANGELOG.zh.md#110)；已完成检查与待验证范围见[验证记录](implementation.zh.md#verification-record)。
