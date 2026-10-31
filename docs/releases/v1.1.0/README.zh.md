# Skills Hub v1.1.0

[English](README.md)

未发布。本索引汇总本次功能，软件版本仍为 1.0.1。当前行为、兼容约束与验证证据统一维护在[实施说明](implementation.zh.md)，固定设计稿保留为历史基线。

## 功能与优化

| 主题 | 用户可见变化 | 详情与拉取请求 |
| --- | --- | --- |
| 项目管理 | 按项目筛选和管理已安装 Skills，预设配置集中到“技能预设”，保留无关部署。 | [技能库与项目](implementation.zh.md#library-and-project-management) |
| 技能预设与历史 | 列表直接编辑和应用预设，在“更多操作”中创建副本，采用紧凑选择与结果视图；保留冲突检查、历史、重试与撤销。 | [技能预设](implementation.zh.md#skill-presets) |
| 更新与恢复 | 主动检查来源，更新全部或所选待更新项，按需查看差异，恢复更新前内容并修复来源或缺失部署。 | [更新与恢复](implementation.zh.md#updates-and-recovery) |
| 界面组织 | 当前更新问题只展示一份，运行记录采用紧凑摘要与日志复制，相关流程支持键盘操作及三语言文案。 | [当前行为](implementation.zh.md#library-and-project-management) |

## Bug 修复

| 问题 | Issue | PR | 说明 |
| --- | --- | --- | --- |
| 冲突、重试和部分失败时，受影响目标或已完成进度不够明确。 | — | — | 保留选择和执行结果，显示可处理的冲突位置，并保护撤销及可恢复备份，见[完整性规则](implementation.zh.md#data-integrity-and-compatibility)。 |
| 技能库操作误入预设或增加确认，共享工具提示后只登记一个工具。 | — | — | 恢复工具切换与更新的直接操作，项目管理限定范围，已确认的共享工具一起处理，见[技能库行为](implementation.zh.md#library-and-project-management)。 |
| Git 检查可能因辅助进程卡住，或由单个故障来源阻塞其他结果。 | — | — | 超时或取消时终止辅助进程，隔离失败并排除内部缓存元数据，见[更新检查](implementation.zh.md#updates-and-recovery)。 |

合并后的变更列表统一保留在 [Unreleased](../../CHANGELOG.zh.md#unreleased)；已完成检查与待验证范围见[验证记录](implementation.zh.md#verification-record)。
