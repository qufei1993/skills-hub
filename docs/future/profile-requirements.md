# Skill Profile / 配置方案后续需求记录

## 背景

GitHub Issue #23 提到“技能分组”能力：

- Skill 作为底层资产存在。
- 单个 Skill 可以被划分到多个组中复用。
- 组支持快速挂载与切换。
- 可根据不同项目和不同工具组合不同 Skill。

这个方向和 v0.6.0 的标签功能有关联，但不属于同一个需求。

标签解决的是：

```text
如何查找和整理 Skill。
```

Profile / 配置方案解决的是：

```text
如何让一套 Skill 作为当前工作场景实际生效。
```

因此 Profile 不纳入 v0.6.0，单独记录为后续评估需求。

---

## 建议命名

不建议使用 `Group / 分组` 作为主 UI 名称。

原因：

- “分组”容易被用户理解成分类展示。
- 它和 `Tag / 标签` 的语义过近。
- 如果同时出现“标签”和“分组”，用户容易混淆。

建议名称：

```text
Profile / 配置方案
```

中文 UI 可使用：

```text
配置方案
```

英文 UI 可使用：

```text
Profile
```

一句话定义：

```text
Profile 是一套可应用的 Skill 同步配置。
```

---

## 与 Tag 的区别

| 概念 | 目的 | 是否影响同步 | 是否可多选 |
|------|------|--------------|------------|
| Tag | 查找、筛选、整理 Skill | 否 | 是 |
| Profile | 应用一套 Skill 配置 | 是 | 建议否 |

核心区分：

```text
Tag 用于找 Skill。
Profile 用于用 Skill。
```

---

## 初步产品规则

### 1. Profile 建议单选激活

第一版建议只允许一个 Active Profile。

原因：

- Profile 表示当前工作场景配置。
- 多个 Profile 同时启用会让语义接近“标签组叠加”。
- 多 Profile 会引入工具目标合并、冲突处理和预览复杂度。

建议模型：

```text
Current Profile: SkillSyncer Dev
```

切换 Profile 是替换当前同步配置，不是叠加。

### 2. Profile 内部可以包含多个 Skill

一个 Profile 可以包含多个 Skill：

```text
SkillSyncer Dev
- react
- tauri-desktop
- test-driven-development
- frontend-design
```

### 3. 一个 Skill 可以属于多个 Profile

Skill 是可复用资产。

例如：

```text
frontend-design
- SkillSyncer Dev
- Review Flow
- Docs Writing
```

同一个 Profile 内不能重复包含同一个 Skill。

数据层建议使用：

```sql
UNIQUE(profile_id, skill_id)
```

### 4. Profile 可以配置目标工具

Profile 可能需要记录目标工具：

```text
Profile: SkillSyncer Dev
Skills: react, tauri-desktop, test-driven-development
Tools: Cursor, Codex, Claude Code
```

是否在第一版实现目标工具配置，需要后续结合现有同步模型评估。

### 5. 需要处理未加入任何 Profile 的 Skill

类似标签中的 `Untagged`，Profile 维度也可能存在：

```text
Unassigned
```

含义：

```text
没有加入任何 Profile 的 Skill。
```

`Unassigned` 不是真实 Profile，而是系统虚拟状态。

后续 UI 可在 Profiles 页面顶部提示：

```text
3 skills are not in any profile        [Review]
```

---

## 关键交互问题

后续实现前需要确认以下问题。

### 1. Profile 是否直接影响同步结果

需要明确：

- 应用 Profile 时是否会新增同步。
- 应用 Profile 时是否会移除不在 Profile 中的旧同步。
- 是否只影响当前 Profile 的目标工具。
- 是否影响全局同步和项目级同步。

### 2. 切换 Profile 是否需要预览

建议需要。

示例：

```text
Apply Docs Writing?

+ 2 skills will be added
- 3 skills will be removed
= 1 skill will stay active

[Cancel] [Apply Profile]
```

切换 Profile 涉及实际同步变更，不应该静默执行。

### 3. Profile 是否绑定项目

Issue #23 提到“针对不同项目”。

需要评估：

- Profile 是否绑定项目路径。
- 进入某项目时是否自动推荐 Profile。
- Profile 与 v0.5.0 项目级同步如何协作。
- 项目切换是否自动应用 Profile。

第一版建议不要自动应用，先做手动切换。

### 4. 是否允许多个 Profile 同时启用

当前建议不允许。

如果后续确实需要多 Profile，应明确合并规则：

- Skill 集合是并集还是交集。
- 工具集合如何合并。
- 同步移除如何判断。
- 冲突时谁优先。

在没有清晰规则前，不建议支持多 Profile 同时启用。

---

## 可能的 UI 方向

### My Skills 顶部

仅展示当前配置：

```text
Current Profile: SkillSyncer Dev ▾
```

选择其他 Profile 后弹出变更预览。

### Profiles 页面

```text
Profiles                                      [+ New Profile]

左侧列表：
- SkillSyncer Dev      Active
- Docs Writing
- Review Flow

右侧详情：
SkillSyncer Dev
React + Tauri + Rust workspace

Tools
[Cursor] [Codex] [Claude Code]

Skills
[✓] react
[✓] tauri-desktop
[✓] test-driven-development
[ ] youtube-transcript

[Preview Changes] [Apply Profile]
```

### Unassigned 处理

Profiles 页面顶部：

```text
3 skills are not in any profile        [Review]
```

点击 `Review` 后展示未分配 Skill，并允许加入某个 Profile。

---

## 数据模型草案

仅供后续评估，不作为当前实现承诺。

```sql
CREATE TABLE skill_profiles (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL UNIQUE,
  description TEXT,
  is_active INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE skill_profile_links (
  profile_id INTEGER NOT NULL,
  skill_id INTEGER NOT NULL,
  created_at TEXT NOT NULL,
  PRIMARY KEY (profile_id, skill_id),
  FOREIGN KEY (profile_id) REFERENCES skill_profiles(id) ON DELETE CASCADE,
  FOREIGN KEY (skill_id) REFERENCES skills(id) ON DELETE CASCADE
);

CREATE TABLE skill_profile_tools (
  profile_id INTEGER NOT NULL,
  tool TEXT NOT NULL,
  created_at TEXT NOT NULL,
  PRIMARY KEY (profile_id, tool),
  FOREIGN KEY (profile_id) REFERENCES skill_profiles(id) ON DELETE CASCADE
);
```

如果坚持单一 Active Profile，需要在应用层保证同一时间只有一个 `is_active = 1`。

---

## 暂不实现内容

在需求没有进一步确认前，暂不实现：

- Profile 创建 / 编辑。
- Profile 切换。
- Profile 自动应用到项目。
- 多 Profile 同时启用。
- Profile 与同步目标的合并规则。
- Profile 未分配 Skill 的批量处理。

---

## 后续评估结论要求

进入实现前，至少需要明确：

- Profile 是否直接改变同步结果。
- 切换 Profile 的删除策略。
- 是否绑定项目路径。
- 是否配置目标工具。
- 是否只允许一个 Active Profile。
- 未加入任何 Profile 的 Skill 如何处理。

这些问题明确前，Profile 不应和 Tag 放在同一版本交付。
