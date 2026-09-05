# v0.10.0 嵌套 Git Skill 发现修复

关联问题：[Issue #129](https://github.com/qufei1993/skills-hub/issues/129)。修复 PR：[PR #131](https://github.com/qufei1993/skills-hub/pull/131)。

## 问题与原因

从 `https://github.com/mattpocock/skills` 添加技能时，仓库使用 `skills/engineering/code-review/SKILL.md` 等分类结构。旧扫描只检查标准容器的直接子目录，无法发现分类中的 Skill，前端因此收到空候选列表。

此前的分层发现方案刻意限制扫描范围与深度，以控制大型仓库的扫描成本，见 [v0.4.3 设计记录](../v0.4.3/bugfix-github-install-and-frontmatter.md)。本次保留范围限制，为标准 `skills/` 容器补充分类层支持。

## 修复与边界

- 从仓库根 URL 或标准 `skills/` 目录 URL 添加时，在标准容器内最多扫描四层子目录；例如 `skills/a/b/c/my-skill/SKILL.md` 可被识别。
- 发现 `SKILL.md` 后停止进入该 Skill 的子目录，避免继续发现其内部示例或资源。
- 跳过目录符号链接、无关隐藏目录，以及 `node_modules`、`target`、`dist`；保留标准容器下 `.curated`、`.experimental`、`.system` 入口，分类目录也计入深度限制。
- 根级自定义 `*skill*` 容器及 `.claude/skills/` 维持原有规则。本次不改变本地导入扫描规则。
- 候选项保留完整仓库相对路径，用于选择安装和保存来源子路径。

## 验证

新增四项回归测试，使用临时目录及本地 Git 仓库，无网络依赖：

- 两个分类下三个 Skill 均能从仓库根发现，空分类不作为候选；多技能仓库要求选择后安装，逐项验证复制内容和来源子路径。
- 单个分类下 Skill 的候选元数据和安装结果正确。
- 四层深度边界、已识别 Skill 内部资源、隐藏目录、构建目录和自定义容器边界。
- 跳过外部目录链接、隐藏兼容入口链接和循环链接（Unix 测试）。

`npm run check` 已通过：90 项前端测试、279 项 Rust 测试，以及构建、格式检查和 Clippy。
