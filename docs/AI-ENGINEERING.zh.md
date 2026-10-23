# AI 工程工作流（中文对照）

本文件是[英文工作流](AI-ENGINEERING.md)的中文对照，修改时同步维护两份文件。项目规则的中文说明见[规则对照](AGENTS.zh.md)。

Skills Hub 采用 DSH Engineering 辅助日常开发。[项目规则](../AGENTS.md)是约束入口，项目导航、架构、安全和网络边界继续以它为准。共享技能源文件保存在 [skills/](PROJECT-SKILLS.zh.md)，贡献者通过桌面端导入，自行选择 AI 工具和分发范围；个人工具部署不纳入版本管理。

## 使用顺序

首次接入已完成规则整理，日常开发无需重复初始化，也无需每次运行全部 Skill。

| 任务阶段 | 按需使用的 Skill |
| --- | --- |
| 功能开发、问题修复、实现设计，以及检查或迁移项目规范 | `dsh-engineering` |
| 开发中的错误处理、并行测试、简化、性能问题 | `dsh-error-handling`、`dsh-ci-test-reliability`、`dsh-find-simplifications`、`dsh-speed-up-perf` |
| 界面交互 | `dsh-client-ui-ux`，先读项目 UI 指南 |
| 更新文档 | `dsh-doc`；明确需要翻译时调用 `dsh-translate-docs` |
| 检查本次改动 | `dsh-code-review` |
| 最终检查 | `dsh-pre-push-checks`，执行 `npm run check` |
| 展示需要录制的浏览器行为 | `dsh-record-browser-gif` |

可以直接说：“用 dsh-engineering 实现这个功能”“用 dsh-code-review 审查当前改动，只报告问题”或“用 dsh-pre-push-checks 验证当前改动”。未发现技能时先检查安装与项目部署状态。命名调用成功不等于所有客户端都能自动触发。

浏览器录制只证明浏览器中实际运行的页面；Tauri IPC、系统凭据及桌面窗口行为仍须按项目规则验证，不能以网页预览代替。检查与录制不自动授权提交、推送、发布或操作真实 Skill 数据。

## 文档与证据

- 开发与检查入口：[贡献指南](../CONTRIBUTING.md)；构建配置：[开发与构建](development.zh.md)。
- UI 规范：[UI-DESIGN-GUIDELINES.md](UI-DESIGN-GUIDELINES.md)，仅在相关界面任务加载。
- 变更记录遵循[变更与发布规则](../CONTRIBUTING.md#change-records-and-releases)：目标发布版本未确定时，写入 [CHANGELOG.md](../CHANGELOG.md) 和[中文更新日志](CHANGELOG.zh.md)的 `Unreleased`；准备已确定的版本发布时，按[版本清单模板](releases/_template/README.zh.md)在 `docs/releases/` 下创建中英文清单，仅在需要时增加独立专题文档。
- 项目规定的三语界面文案和中英文发布记录属于实现任务的配套工作，无需再单独请求翻译；其他文档的翻译按明确范围调用 `dsh-translate-docs`。
- 完成报告说明修改、实际运行的检查及剩余问题。性能须测真实入口；并行故障须保留真实并行条件；错误处理不能用重试掩盖结果未知。

## 更新与撤销

采用版本为 DSH Engineering 1.0.0；上游固定提交和接入前后文档见 [采用记录](../.dsh-engineering/adoption.json)。记录包含文档全文，发布前需要检查是否含私有内容。

源文件副本维护和桌面端更新遵循[项目 Skills 指南](PROJECT-SKILLS.zh.md#维护-dsh-副本)。更新包后，项目文档不会自动重写；显式调用 `dsh-engineering` 更新或撤销时，先比较记录、当前文档与建议，保留项目后续修改。缺失或损坏的基线不能用来推测原文。
