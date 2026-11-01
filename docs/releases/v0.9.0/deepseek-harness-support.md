# v0.9.0 DeepSeek Harness 工具适配

## 背景

DeepSeek Harness 提供本地文件系统 Skill provider，并按项目专属、项目共享、自定义、用户专属和用户共享的顺序发现 Skill。Skills Hub v0.9.0 将 DeepSeek Harness 加入内置工具列表，让用户可以沿用现有的 Skill 管理和同步流程。

官方参考：

- [DeepSeek Harness Skills 子系统](https://github.com/deepseek-ai/deepseek-harness/blob/master/docs/subsystems/skills.md)
- [DeepSeek Harness filesystem Skill provider](https://github.com/deepseek-ai/deepseek-harness/blob/master/packages/skill/skill-filesystem/README.md)
- [DeepSeek Harness home path 规则](https://github.com/deepseek-ai/deepseek-harness/blob/master/packages/util/home-paths/README.md)

## 官方目录规则

DeepSeek Harness 默认发现以下目录：

| 范围 | 目录 | 发现顺序 |
| --- | --- | --- |
| 项目专属 | `<project>/.dsh/skills` | 1 |
| 项目共享 | `<project>/.agents/skills` | 2 |
| 自定义 | `customSkillDirs` | 3 |
| 用户专属 | `$DSH_HOME/skills`，默认 `~/.dsh/skills` | 4 |
| 用户共享 | `$DSH_AGENTS_HOME/skills`，默认 `~/.agents/skills` | 5 |

项目目录以当前工作目录最近的 `.git` 祖先为根；没有 `.git` 时使用当前工作目录。同名 Skill 会优先使用发现顺序更靠前的版本。

## Skills Hub 映射

| 字段 | 配置 |
| --- | --- |
| 工具 key | `deepseek_harness` |
| 显示名称 | `DeepSeek Harness` |
| 全局 skills 目录 | `.dsh/skills` |
| 项目 skills 目录 | `.dsh/skills` |
| 安装检测目录 | `.dsh` |
| 项目级同步 | 支持 |

Skills Hub 选择 DeepSeek Harness 的专属 `.dsh/skills` 目录作为内置目标。该目录比对应的 `.agents/skills` 目录优先级更高，并且不会把 DeepSeek Harness 的工具启停和同步状态与其他使用 `.agents/skills` 的工具绑定。

## Skill 格式兼容

DeepSeek Harness 支持单层目录包 `<name>/SKILL.md` 和平铺文件 `<name>.md`，不递归发现更深层的 `SKILL.md`。其 frontmatter 要求 `name` 和 `description`，且名称必须为 kebab-case。

Skills Hub 管理的标准目录型 Skill 会同步为 `<skills-dir>/<skill-name>/SKILL.md`，与 DeepSeek Harness 的目录包格式兼容。当前版本不把平铺 `<name>.md` 文件作为独立的 Skills Hub 托管单元导入。

## 同步与刷新

DeepSeek Harness 的本地 provider 默认监视 Skill 根目录并跟随符号链接。Skills Hub 继续使用现有的软链接、Windows 目录联接和复制三级回退机制；同步目标创建或发生目录成员变化后，DeepSeek Harness 可以刷新 Skill catalog。

## 自定义目录边界

以下为 v0.9.0 的适配范围。v0.11.0 已补齐 `DSH_HOME` 支持及旧部署恢复步骤，见[当前说明](../v0.11.0/README.zh.md#deepseek-harness-自定义目录)。

DeepSeek Harness 的 home path 优先级为显式 `dshHome` 配置、`DSH_HOME` 环境变量、默认 `~/.dsh`。内置适配器面向默认目录 `~/.dsh/skills`；使用自定义 `DSH_HOME`、`DSH_AGENTS_HOME` 或 `customSkillDirs` 的用户，可以在 Skills Hub 管理中心创建自定义工具并填写实际全局及项目目录。

## 实现范围

- Rust 内置工具注册表新增 `DeepSeekHarness`。
- 全局发现、首次导入扫描和同步复用现有通用工具适配流程。
- 项目范围同步使用 `.dsh/skills`。
- 前端中英文工具名称统一为 `DeepSeek Harness`。
- 工具图标复用现有图标依赖提供的 DeepSeek 彩色图标。
- Rust 测试覆盖工具 key、全局目录、项目目录、检测目录和项目范围能力。
- 中英文 README、CHANGELOG 和 v0.9.0 Roadmap 同步更新。

## 验证

```bash
npm run version:check
npm run check
```
