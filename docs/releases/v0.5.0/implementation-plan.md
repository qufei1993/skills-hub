# 项目级 Skill 同步功能实现计划

## Context

SkillSyncer v0.4.x 只有全局同步。Skill 安装到中央仓库 `~/.skillsyncer/` 后，再同步到各工具的全局 Skills 目录。

v0.5.0 增加项目级同步能力：同一个 Skill 可以选择同步到全局，或同步到一个或多个项目目录下的工具 Skills 目录。

当前实现已经从原始方案做过几轮交互收敛，本计划按最新代码实现记录。

---

## 已确认的产品规则

### 1. 安装流程不变

Skill 仍然只安装到中央仓库：

```text
~/.skillsyncer/<skill-name>/
```

全局 / 项目只影响同步目标路径。

### 2. Scope 是 Skill 级别设置

每个 Skill 当前只有一个主要 scope：

```text
global | project
```

工具按钮不单独配置 scope，只控制该工具是否参与当前 scope 的同步。

### 3. 切换 scope 后默认同步当前已安装工具

切换范围时，以 `get_tool_status().installed` 返回的当前已安装工具为准：

- 全局 → 项目：同步到所选项目下所有当前已安装工具。
- 项目 → 全局：同步到所有当前已安装工具的全局目录。

不能把系统支持的全部工具展示出来，也不能根据历史同步过的工具推断同步范围。

### 4. 工具按钮样式不区分 scope

工具按钮只表达同步状态：

- active：已同步
- inactive：未同步

项目级状态通过范围徽标表达，例如 `1 个项目`。项目级工具按钮不使用蓝色样式。

### 5. 项目列表是草稿态

同步范围弹窗中的项目目录列表在点击“应用”前都是草稿：

- 添加项目后点取消，不保存、不统计、不同步。
- 删除项目后点取消，不解除同步。
- 只有点击应用后，才提交最终项目列表。

切换到项目范围时，必须至少选择一个项目目录才能应用。

---

## 1. 数据库迁移

**文件**：`src-tauri/src/core/skill_store.rs`

### Schema

`SCHEMA_VERSION` 升级到 4。

`skill_targets` 新增：

```sql
scope TEXT NOT NULL DEFAULT 'global',
project_path TEXT NULL
```

唯一索引：

```sql
CREATE UNIQUE INDEX idx_skill_targets_unique_scope
ON skill_targets(skill_id, tool, scope, COALESCE(project_path, ''));
```

### 迁移规则

V3 → V4 使用重建表迁移：

1. 创建 `skill_targets_new`
2. 复制旧表数据，旧记录统一写为：

   ```text
   scope = 'global'
   project_path = NULL
   ```

3. 删除旧表
4. 重命名新表
5. 创建新的唯一索引

### Rust 结构

`SkillTargetRecord` 增加：

```rust
pub scope: String,
pub project_path: Option<String>,
```

相关方法签名调整：

```rust
get_skill_target(skill_id, tool, scope, project_path)
delete_skill_target(skill_id, tool, scope, project_path)
```

### 兼容性

老用户升级后，既有全局同步记录会保留，并被识别为全局 scope。不会影响原有全局同步状态。

---

## 2. Tool Adapter 扩展

**文件**：`src-tauri/src/core/tool_adapters/mod.rs`

### 新增函数

```rust
resolve_project_path(adapter, project_root)
supports_project_scope(adapter)
project_relative_skills_dir(adapter)
adapters_sharing_project_skills_dir(adapter)
```

### 当前实现规则

- `supports_project_scope()` 当前返回 `true`。
- UI 不根据支持矩阵展示全部工具，只展示当前已安装工具。
- 项目路径不直接复用全局 `relative_skills_dir`，而是先走 `project_relative_skills_dir()` 的显式映射。
- 未显式映射的工具回退到 adapter 自身的 `relative_skills_dir`。

### 关键路径映射

| 工具 | 项目级路径 |
|------|------------|
| Cursor | `.agents/skills` |
| Codex | `.agents/skills` |
| OpenCode | `.agents/skills` |
| Gemini CLI | `.agents/skills` |
| GitHub Copilot | `.agents/skills` |
| Amp | `.agents/skills` |
| Kimi Code CLI | `.agents/skills` |
| Antigravity | `.agents/skills` |
| Cline | `.agents/skills` |
| Claude Code | `.claude/skills` |
| OpenClaw | `skills` |
| Windsurf | `.windsurf/skills` |
| Qwen Code | `.qwen/skills` |

完整映射以 `project_relative_skills_dir()` 为准。

### 共享目录

项目级同步会按项目路径分组：

```rust
adapters_sharing_project_skills_dir(adapter)
```

共享同一目录的工具只写一份文件系统目标，但会为当前已安装的共享工具写入各自的 `skill_targets` 记录。

---

## 3. 后端命令

**文件**：`src-tauri/src/commands/mod.rs`

### DTO

`ToolInfoDto` 增加：

```rust
supports_project_scope: bool
```

`SkillTargetDto` 增加：

```rust
scope: String,
project_path: Option<String>,
```

### `sync_skill_to_tool`

新增可选参数：

```rust
scope: Option<String>,
projectPath: Option<String>,
```

规则：

- `scope` 默认为 `global`。
- 只允许 `global` / `project`。
- `scope = project` 时，`projectPath` 必填，且必须是已存在目录。
- `scope = global` 时继续检查工具是否安装。
- `scope = project` 时使用 `resolve_project_path()` 生成目标目录。
- 如果同 scope / projectPath 下已有有效 target，且目标存在，则幂等返回成功。
- 同步成功后，对共享同一 Skills 目录且当前已安装的工具写入同步记录。

错误前缀沿用：

```text
TARGET_EXISTS|
TOOL_NOT_INSTALLED|
TOOL_NOT_WRITABLE|
```

### `unsync_skill_from_tool`

新增参数同上。

规则：

- 按 `skillId + tool + scope + projectPath` 定位记录。
- 共享目录工具一起更新 DB 记录。
- 文件系统目标只删除一次。
- 全局范围下，如果共享组内没有任何工具已安装，则视为已经无效，直接成功。

### 最近项目

新增命令：

```rust
get_recent_projects()
save_recent_project(projectPath)
```

实现：

- 存储在 settings 表的 `recent_projects_v1`
- JSON 数组
- 新路径插到最前
- 去重
- 最多保留 8 条
- 只在用户点击“应用”提交项目范围后保存

### 命令注册

**文件**：`src-tauri/src/lib.rs`

注册：

```rust
commands::get_recent_projects
commands::save_recent_project
```

---

## 4. 前端类型

**文件**：`src/components/skills/types.ts`

`ManagedSkill.targets` 增加：

```ts
scope: 'global' | 'project' | string
project_path?: string | null
```

`ToolInfoDto` 增加：

```ts
supports_project_scope: boolean
```

---

## 5. 前端 UI

### FilterBar

**文件**：`src/components/skills/FilterBar.tsx`

新增 scope 下拉筛选：

```text
全部 / 全局 / 项目
```

布局要求：

- 靠右，和排序、搜索、刷新同一组。
- 下拉样式参考排序按钮。
- 不显示额外“范围”文案。
- 排序按钮不显示“排序：”前缀。

### SkillCard

**文件**：`src/components/skills/SkillCard.tsx`

新增范围徽标：

```text
全局
N 个项目
```

项目数量从后端真实 `project` target 中统计，不使用弹窗草稿或本地缓存。

工具按钮：

- 只展示当前用户已安装工具。
- active / inactive 样式保持此前一致。
- 不因为项目级 scope 改成蓝色。

### ScopeSyncModal

**文件**：`src/components/skills/modals/ScopeSyncModal.tsx`

新增同步范围弹窗。

文案：

```text
选择这个 Skill 生效的位置。

全局
在所有项目中可用

项目
仅在选择的项目中可用
```

交互：

- radio 切换后直接展示对应内容，不弹额外确认框。
- 项目模式下展示项目目录列表、选择项目目录按钮、最近项目。
- 项目目录列表使用组件内部 `draftProjects`。
- 点击取消丢弃草稿。
- 点击应用调用 `onScopeChange(draftScope, draftProjects)`。
- 项目模式下 `draftProjects.length === 0` 时禁用应用按钮并显示提示。

### App

**文件**：`src/App.tsx`

新增状态：

```ts
scopeFilter
scopeModalSkill
recentProjects
skillScopeState
```

关键逻辑：

- `getSkillScope(skill)` 以后端实际 target 为主，本地缓存只作兜底。
- `getSkillProjects(skill)` 只从后端 project target 统计项目路径。
- `handleScopeChange(nextScope, nextProjects)`：
  - 清理目标 scope 之外的旧 target。
  - 项目 scope 下，同时清理不在最终项目列表中的旧项目 target。
  - 切到项目时，同步到 `nextProjects × installedToolIds`。
  - 切到全局时，同步到 `installedToolIds`。
  - 应用成功后才写入最近项目和本地 scope 缓存。
- `handlePickProject()` 只返回文件夹选择结果，不直接写入 Skill 状态。

---

## 6. i18n

**文件**：`src/i18n/resources.ts`

新增或更新 key：

| Key | EN | ZH |
|-----|----|----|
| `scope.all` | All | 全部 |
| `scope.global` | Global | 全局 |
| `scope.project` | Project | 项目 |
| `scope.globalBadge` | Global | 全局 |
| `scope.projectCount` | `{{count}} projects` | `{{count}} 个项目` |
| `projectSync.title` | Sync Scope | 同步范围 |
| `projectSync.help` | Choose where this Skill is available. | 选择这个 Skill 生效的位置。 |
| `projectSync.globalDesc` | Available in all projects | 在所有项目中可用 |
| `projectSync.projectDesc` | Available only in selected projects | 仅在选择的项目中可用 |
| `projectSync.projectRequired` | Select at least one project directory to apply project scope. | 请至少选择一个项目目录后再应用项目范围。 |

历史确认弹窗相关 key 可保留，但当前交互不再使用。

---

## 7. 关键文件清单

| 文件 | 改动类型 |
|------|----------|
| `src-tauri/src/core/skill_store.rs` | V4 迁移、结构体、查询方法 |
| `src-tauri/src/core/tool_adapters/mod.rs` | 项目路径映射、共享项目目录分组 |
| `src-tauri/src/commands/mod.rs` | 命令参数、DTO、最近项目命令 |
| `src-tauri/src/lib.rs` | 注册新命令 |
| `src/components/skills/types.ts` | DTO 类型更新 |
| `src/components/skills/FilterBar.tsx` | 范围筛选下拉 |
| `src/components/skills/SkillCard.tsx` | 范围徽标、工具展示 |
| `src/components/skills/modals/ScopeSyncModal.tsx` | 新增范围弹窗 |
| `src/App.tsx` | 状态、筛选、切换、同步逻辑 |
| `src/i18n/resources.ts` | 翻译 |
| `src/App.css` | 新增弹窗、范围徽标、筛选样式 |

---

## 8. 验证方式

1. `npm run check` 通过。
2. 旧数据库升级后，旧 `skill_targets` 均为 `scope = global`，`project_path = NULL`。
3. 全局同步一个 Skill，确认工具按钮保持原 active 样式，范围徽标为“全局”。
4. 打开同步范围弹窗，选择“项目”但不选项目时，“应用”不可点。
5. 选择项目后点取消，卡片项目数量不变化，目录不被同步，最近项目不保存。
6. 选择项目后点应用，确认同步到项目目录下当前已安装工具。
7. 再次打开弹窗，删除项目后点取消，原项目同步仍保留。
8. 删除项目后点应用，确认该项目 target 被清理。
9. 项目切回全局后，确认项目 target 被清理，全局 target 写入当前已安装工具。
10. FilterBar 的“全部 / 全局 / 项目”筛选结果正确。
