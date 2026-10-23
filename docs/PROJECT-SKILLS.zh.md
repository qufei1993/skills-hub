# 项目 Skills（中文对照）

本文件对应[英文说明](../skills/README.md)，修改时同步维护。

仓库的 `skills/` 保存受版本管理的 Skill 源文件。贡献者可以通过 Skills Hub 导入，再部署到自己使用的 AI 编程工具。是否使用 AI 由贡献者自行决定；所有贡献都遵循相同的[项目规则](AGENTS.zh.md)和[质量检查](../CONTRIBUTING.md#quality-checks)。

## 包含内容

- `dsh-*` 目录保存开发、审查、文档和验证所用的 DSH Engineering 工作流。按任务选择技能，见 [AI 工程工作流](AI-ENGINEERING.zh.md#使用顺序)。
- [manage-skills-hub](../skills/manage-skills-hub/SKILL.md) 是应用自带的技能，用于管理本地技能库，由本仓库维护；贡献代码不要求安装它。

## 导入与使用

1. 克隆仓库。在 Skills Hub 桌面端选择“从本地导入”，选中当前检出的 `skills/` 目录。
2. 勾选需要的 `dsh-*` 技能。已有同名技能时，先核对来源，不覆盖无关技能或本地修改。
3. 在安装设置中选择已启用的 AI 工具和分发范围。工具及平台支持项目范围时，选择当前检出作为项目目录；否则，根据需要明确选择全局范围。
4. 完成安装，检查同步结果。按所用 AI 工具的说明加载已部署技能，再使用工作流指南中的任务名称。

作为本地来源使用期间，保留仓库检出的原路径。拉取仓库更新只会改变源文件；还需在桌面端更新对应的已导入技能，才能刷新中央副本和部署文件。此前从其他位置安装的技能仍使用原来的来源，直到你明确调整其安装配置。

`.agents/skills/`、`.claude/skills/` 等工具部署目录保留在本机，由 Git 忽略。提交的是仓库中的源文件，不是个人部署结果或指向本机技能库的链接。

## 维护 DSH 副本

[dsh-engineering.source.json](../skills/dsh-engineering.source.json) 记录导入的 DSH Engineering 版本、准确源码提交及技能列表。每个技能包保留 `LICENSE`、`NOTICE` 和 `UPSTREAM.json`，包含 DeepSeek Harness 来源署名。导入的技能包保留原有中文指令。

通用工作流在 DSH Engineering 中维护，再从已审查的源码提交复制完整的选定技能包，同时更新来源记录。核对复制内容、必需资源和本地链接，并运行仓库要求的检查。Skills Hub 特有规则写在项目规范中，不直接修改导入的副本。更新副本不会改写项目规则，也不会自动更新贡献者已经安装的技能。
