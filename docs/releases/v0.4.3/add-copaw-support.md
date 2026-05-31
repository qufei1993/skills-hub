# 新增：支持 Copaw 工具

> 贡献者：[@LeonDevLifeLog](https://github.com/LeonDevLifeLog)，PR [AlwaysCodeHQ/skills-syncer#50](https://github.com/AlwaysCodeHQ/skills-syncer/pull/50)

## 更新内容

新增对 [CoPaw](https://github.com/agentscope-ai/CoPaw) 的支持。CoPaw 是 AgentScope 出品的 AI 个人助理，支持多端接入（钉钉、飞书、微信、Discord 等）、技能扩展与多智能体协作。

安装 skill 后，SkillSyncer 可自动将其同步到 CoPaw 的本地技能池。

## 技术细节

CoPaw 的技能池路径与大多数工具不同，使用 `skill_pool` 而非 `skills`：

| 字段 | 值 |
|------|-----|
| Tool ID | `copaw` |
| 显示名称 | Copaw |
| 技能目录 | `~/.copaw/skill_pool/` |
| 检测目录 | `~/.copaw/` |

## 修改文件

- `src-tauri/src/core/tool_adapters/mod.rs`：新增 `ToolId::Copaw` 枚举变体及对应的 `ToolAdapter`
- `README.md`：工具支持列表新增 Copaw
- `docs/README.zh.md`：同步更新中文版工具支持列表
