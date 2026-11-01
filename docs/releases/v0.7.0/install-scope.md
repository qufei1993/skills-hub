# 安装范围选择

## 功能概述

在新增 Skill 弹窗中增加「安装范围」选择，允许用户在安装前一次性指定本批 Skill 按全局范围还是项目范围同步。默认保持全局安装，避免改变现有用户习惯；切换到项目范围后可选择多个项目目录，并自动禁用不支持项目范围的工具。

## 背景与目标

- Skills Hub 之前仅在 Skill 安装完成后，通过「同步范围」弹窗对单个 Skill 调整范围。批量安装时只能先全局安装再逐个修改，成本较高。
- 本功能让全局/项目范围、多个项目目录和目标工具的配置在安装前一次性完成，并自动复用到批量发现的多个 Skill。
- 复用现有 `sync_skill_to_tool` 命令和范围模型，不引入新的后端结构。

## 主要变更

### 新增范围选择能力

- 新增 `src/components/skills/installScope.ts`：定义 `InstallScope` 类型、项目路径归一化、项目模式工具过滤和同步任务展开等纯逻辑，并配套单元测试。
- 新增 `src/components/skills/ScopeSelector.tsx`：抽取共享的范围选择组件，负责全局/项目切换、项目目录列表、最近项目快捷添加和必填状态提示。
- `ScopeSyncModal.tsx` 改用共享 `ScopeSelector`，保留单个已安装 Skill 的范围调整流程。
- `AddSkillModal.tsx` 在「安装到工具」区域之前增加紧凑的范围选择；项目范围下自动禁用不支持项目范围的工具，未选择项目目录时阻止提交。

### 状态管理与统一同步

- `App.tsx` 增加 `installScope` 和 `installProjects` 状态，统一管理新增弹窗的范围和项目目录。
- 范围切换时自动过滤目标工具（项目模式下取消不支持项目范围的工具；切回全局时不自动恢复，避免隐式改变用户选择）。
- 新增 `syncInstalledSkill` 统一执行安装后的同步，覆盖本地、Git、在线搜索和批量候选流程。
- 批量安装多个 Skill 时，首次配置的范围、项目目录和工具选择自动复用到所有候选。

### 体验优化

- 新增 Skill 弹窗增加 `.add-skill-modal` 独立样式，限制最大高度并让主体区域独立滚动，避免内容过多时底部按钮被裁掉。
- 安装范围选择器在新增弹窗中使用紧凑模式，「安装范围」标题与「全局 / 项目」切换控件展示在同一行，减少垂直空间占用。
- 项目目录选择区域保持原有展示方式不变。

## 国际化

新增 `installScope` 文案键：

- `installScope.title`：安装范围
- `installScope.help`：选择本次安装的所有 Skill 在哪里可用。
- `installScope.unsupportedTool`：{{tool}} 不支持项目安装。

## 涉及文件

- `src/components/skills/installScope.ts`
- `src/components/skills/installScope.test.ts`
- `src/components/skills/ScopeSelector.tsx`
- `src/components/skills/modals/AddSkillModal.tsx`
- `src/components/skills/modals/ScopeSyncModal.tsx`
- `src/App.tsx`
- `src/App.css`
- `src/i18n/resources.ts`

## 参考文档

- 设计文档：[安装时选择全局或项目范围](../../superpowers/specs/2026-06-19-install-scope-design.md)
- 原实现计划未随仓库归档。

## 验证

实现完成后运行：

```bash
npm run check
```

覆盖 ESLint、前端测试、TypeScript/Vite 构建、Rust 格式、Clippy 和 Rust 测试。
