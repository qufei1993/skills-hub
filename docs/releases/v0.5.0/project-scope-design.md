# 项目级 Skill 同步 — 设计文档

## 背景

SkillSyncer v0.4.x 只支持全局同步：Skill 安装到中央仓库后，同步到各工具的全局目录，在所有项目中均可使用。

v0.5.0 新增**项目级同步**：Skill 可以同步到指定项目中的工具目录，使其只在这些项目中生效。

---

## 核心原则

### 安装位置不变

Skill 文件仍然只安装并维护在中央仓库中：

```text
~/.skillsyncer/<skill-name>/
```

全局 / 项目只决定同步目标，不改变 Hub 中的 Skill 文件。

### 同步范围是 Skill 级别设置

每个 Skill 有一个当前同步范围：

| 范围 | 含义 |
|------|------|
| 全局 | 同步到各工具的全局 Skills 目录 |
| 项目 | 同步到所选项目下各工具的项目级 Skills 目录 |

同步范围不是每个工具单独设置。工具按钮只表示该工具是否参与当前范围的同步。

### 切换范围默认同步当前已安装工具

全局和项目之间切换时，系统会以**当前用户已安装的工具**为准重新同步：

- 全局 → 项目：移除非项目范围的旧同步记录，并将该 Skill 同步到所选项目下所有当前已安装工具。
- 项目 → 全局：移除非全局范围的旧同步记录，并将该 Skill 同步到所有当前已安装工具的全局目录。

这里的“所有工具”不是系统支持的全部工具，而是当前检测到已安装的工具。

---

## 同步路径

### 全局路径

全局同步继续使用各工具 adapter 中已有的全局路径，例如：

```text
~/.claude/skills/<skill-name>
~/.codex/skills/<skill-name>
```

### 项目路径

项目级同步使用独立的项目路径映射。部分工具的项目级路径和全局路径不一致，不能直接复用全局 `relative_skills_dir`。

当前实现中所有工具都允许项目级同步；UI 只展示当前用户已安装的工具。

主要项目级路径如下：

| 工具 | 项目级 Skills 目录 |
|------|--------------------|
| Cursor / Codex / OpenCode / Gemini CLI / GitHub Copilot / Amp / Kimi Code CLI / Antigravity / Cline | `<project>/.agents/skills/` |
| Claude Code | `<project>/.claude/skills/` |
| OpenClaw | `<project>/skills/` |
| Windsurf | `<project>/.windsurf/skills/` |
| Qwen Code | `<project>/.qwen/skills/` |
| OpenHands | `<project>/.openhands/skills/` |
| 其他已映射工具 | 使用 `project_relative_skills_dir()` 中的显式映射 |
| 未显式映射工具 | 回退到该工具的全局 `relative_skills_dir` |

共享同一项目级目录的工具会共用同一个同步目标。例如多个工具都使用 `<project>/.agents/skills/` 时，文件系统只写一份，数据库只为当前已安装且共享该目录的工具记录同步状态。

---

## 同步方式

同步引擎仍沿用现有策略：

```text
symlink -> junction（Windows）-> copy
```

因此文档中“同步目标”不承诺一定是软链接；具体模式由同步引擎决定，并记录在 `skill_targets.mode` 中。

---

## 交互设计

### Skill Card

Skill 卡片 meta 行新增范围徽标：

```text
ux-designer
Expert UX design assistance...
shubhamsaboo/awesome-llm-apps · 10 小时前 · [1 个项目]

[● Cursor] [● Claude Code] [● Codex] [OpenClaw]
```

范围徽标：

| 文案 | 含义 |
|------|------|
| 全局 | 当前 Skill 使用全局同步 |
| N 个项目 | 当前 Skill 使用项目级同步，且已同步到 N 个项目 |

点击范围徽标打开“同步范围”弹窗。

### 工具按钮

工具按钮颜色不区分全局 / 项目，继续沿用原有语义：

| 状态 | 含义 | 样式 |
|------|------|------|
| 已同步 | 该工具已参与当前范围同步 | 原有 active 样式 |
| 未同步 | 该工具未参与当前范围同步 | 原有 inactive 样式 |

全局 / 项目的区别由范围徽标表达，不通过工具按钮颜色表达。

### 同步范围弹窗

弹窗只暴露用户决策需要的信息：

```text
同步范围 · ux-designer

选择这个 Skill 生效的位置。

○ 全局
  在所有项目中可用

● 项目
  仅在选择的项目中可用

项目目录
  /Users/may/Desktop/test/cc-weixin-test    [x]
  [选择项目目录...]

最近使用
  /Users/may/Desktop/test/cursor-browser    [添加]

[取消] [应用]
```

交互规则：

- 切换 radio 后立即展示对应区域，不再弹出额外确认框。
- 选择“项目”时，必须至少选择一个项目目录才能点击“应用”。
- 新增、移除项目目录均为弹窗内草稿状态。
- 点击“取消”不会保存项目列表，不会影响卡片项目数量，也不会触发同步。
- 点击“应用”后才提交范围和项目列表，并执行同步。
- 最近项目只在应用项目范围后保存，最多保留 8 个。

### 筛选栏

范围筛选使用下拉样式，和排序 / 搜索 / 刷新保持同一行布局：

```text
全部 Skills                         [全部 v] [最近更新 ↕] [搜索 skills...] [刷新]
```

筛选项：

| 选项 | 显示内容 |
|------|----------|
| 全部 | 所有 Skill |
| 全局 | 当前范围为全局的 Skill |
| 项目 | 当前范围为项目的 Skill |

筛选在前端完成，不需要新增后端查询接口。

---

## 数据模型

`skill_targets` 增加范围维度：

```text
scope TEXT NOT NULL DEFAULT 'global'
project_path TEXT NULL
```

唯一索引：

```sql
UNIQUE(skill_id, tool, scope, COALESCE(project_path, ''))
```

含义：

- 全局 target：`scope = 'global'`，`project_path = NULL`
- 项目 target：`scope = 'project'`，`project_path = <project root>`

旧数据库升级到 v0.5.0 时，既有同步记录会迁移为：

```text
scope = 'global'
project_path = NULL
```

因此老用户升级后，原有全局同步状态保持不变。

---

## 前后端接口

### `sync_skill_to_tool`

新增可选参数：

```text
scope?: 'global' | 'project'
projectPath?: string
```

规则：

- `scope` 缺省为 `global`，保持向后兼容。
- `scope = project` 时必须传 `projectPath`，且路径必须是已存在目录。
- 全局同步会检查工具是否已安装。
- 项目同步不依赖全局工具安装路径，但最终记录只写入当前已安装工具。
- 同一路径已有有效 target 时视为幂等成功。

### `unsync_skill_from_tool`

同样新增：

```text
scope?: 'global' | 'project'
projectPath?: string
```

规则：

- 按 `skill_id + tool + scope + project_path` 删除目标记录。
- 共享同一 Skills 目录的工具会一起更新数据库状态。
- 文件系统目标只删除一次，避免共享目录重复删除。

### 最近项目

新增命令：

```text
get_recent_projects
save_recent_project(projectPath)
```

最近项目存入 `settings.recent_projects_v1`，用于项目级弹窗快捷添加。

---

## 实现变更概览

| 层 | 文件 | 变更 |
|----|------|------|
| DB | `skill_store.rs` | `skill_targets` 增加 `scope`、`project_path`，V3→V4 重建表迁移 |
| 后端 | `tool_adapters/mod.rs` | 新增项目级路径解析、共享项目目录分组、项目路径映射 |
| 后端 | `commands/mod.rs` | `sync/unsync` 增加 `scope`、`projectPath`；新增最近项目命令 |
| 后端 | `lib.rs` | 注册 `get_recent_projects`、`save_recent_project` |
| 前端 | `types.ts` | DTO 增加 `scope`、`project_path`、`supports_project_scope` |
| 前端 | `FilterBar.tsx` | 新增范围下拉筛选 |
| 前端 | `SkillCard.tsx` | meta 行增加范围徽标；工具按钮保持原有 active/inactive 样式 |
| 前端 | `ScopeSyncModal.tsx` | 新建同步范围弹窗，使用草稿项目列表，应用后提交 |
| 前端 | `App.tsx` | 新增范围状态、筛选、切换、项目同步逻辑 |
| i18n | `resources.ts` | 新增 `scope.*`、`projectSync.*` 翻译键 |

详细实现步骤见：[实现计划](./implementation-plan.md)
