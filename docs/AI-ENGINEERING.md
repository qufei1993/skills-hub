# AI 工程工作流

Skills Hub 采用 DSH Engineering 辅助日常开发。现有 [AGENTS.md](../AGENTS.md) 是项目规则入口；技术栈、代码地图、安全和网络边界继续以它为准。

## 使用顺序

首次接入已完成规则整理，日常开发无需重复初始化，也无需每次运行全部 Skill。

| 任务阶段 | 按需使用的 Skill |
| --- | --- |
| 检查、补齐或迁移项目规范 | `dsh-engineering` |
| 开发中的错误处理、并行测试、简化、性能问题 | `dsh-error-handling`、`dsh-ci-test-reliability`、`dsh-find-simplifications`、`dsh-speed-up-perf` |
| 界面交互 | `dsh-client-ui-ux`，先读项目 UI 指南 |
| 更新文档 | `dsh-doc`；明确需要翻译时调用 `dsh-translate-docs` |
| 检查本次改动 | `dsh-code-review` |
| 最终检查 | `dsh-pre-push-checks`，执行 `npm run check` |
| 展示需要录制的浏览器行为 | `dsh-record-browser-gif` |

可以直接说：“用 dsh-code-review 审查当前改动，只报告问题”或“用 dsh-pre-push-checks 验证当前改动”。未发现 Skill 时先检查安装与项目部署状态。命名调用成功不等于所有客户端都能自动触发。

浏览器录制只证明浏览器中实际运行的页面；Tauri IPC、系统凭据及桌面窗口行为仍须按项目规则验证，不能以网页预览代替。检查与录制不自动授权提交、推送、发布或操作真实 Skill 数据。

## 文档与证据

- 开发与检查入口：[贡献指南](../CONTRIBUTING.md)；构建配置：[Development and builds](development.en.md)。
- UI 规范：[UI-DESIGN-GUIDELINES.md](UI-DESIGN-GUIDELINES.md)，仅在相关界面任务加载。
- 发布行为记录：[CHANGELOG.md](../CHANGELOG.md)、[中文记录](CHANGELOG.zh.md)及 `docs/releases/`，遵循现有版本规则。
- 完成报告说明修改、实际运行的检查及剩余问题。性能须测真实入口；并行故障须保留真实并行条件；错误处理不能用重试掩盖结果未知。

## 更新与撤销

采用版本为 DSH Engineering 1.0.0；上游固定提交和接入前后文档见 [采用记录](../.dsh-engineering/adoption.json)。记录包含文档全文，发布前需要检查是否含私有内容。

Skill 包由 Skills Hub 管理。更新包后，项目文档不会自动重写；显式调用 `dsh-engineering` 更新或撤销时，先比较记录、当前文档与建议，保留项目后续修改。缺失或损坏的基线不能用来推测原文。
