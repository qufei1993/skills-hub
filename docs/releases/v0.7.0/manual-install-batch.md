# 手动安装批量支持与弹窗体验优化

## 功能概述

修复本地目录安装时不支持批量选择的问题，统一本地与 Git 安装流程的交互体验，优化候选弹窗高度和层级过渡。

## 背景与目标

- 手动安装本地目录时，如果选择的目录直接包含多个 Skill 子目录（如 `/path/to/skills/skill-a/SKILL.md`），`list_local_skills` 只扫描 `skills/`、`.claude/skills/` 等已知子目录，无法发现根级别的 Skill 目录，导致批量选择弹窗无法出现。
- Git 安装流程使用的 `collect_skill_dirs` 已经支持根级别扫描，本地目录扫描需要对齐。
- 候选选择弹窗（`LocalPickModal` / `GitPickModal`）与外层添加弹窗之间存在高度不一致和层级叠加问题。

## 主要变更

### 本地目录批量选择

- `src-tauri/src/core/installer.rs`：`list_local_skills` 新增根级别目录扫描，与 `collect_skill_dirs` 行为一致：
  - 扫描 `base_path` 下每个直接包含 `SKILL.md` 的子目录
  - 对名称含 "skill" 的容器目录深入扫描其子目录
  - 跳过隐藏目录和已在第一步扫描过的已知目录（`skills`、`.claude`），避免重复

### 弹窗高度对齐

- `LocalPickModal.tsx` / `GitPickModal.tsx`：容器添加 `add-skill-modal` CSS 类，获得与 `AddSkillModal` 一致的 `max-height` flex 布局和独立滚动行为。

### 弹窗层级过渡

- `src/App.tsx`：改为步骤式过渡，避免两层弹窗叠加：
  - 候选选择弹窗出现时关闭添加弹窗（`setShowAddModal(false)`）
  - 点击「取消」回到添加弹窗（`setShowAddModal(true)`），路径/URL 保留可修改重试
  - 点击 X / 背景关闭表示完全退出
  - 安装完成两个弹窗同时关闭

## 涉及文件

- `src-tauri/src/core/installer.rs`
- `src/App.tsx`
- `src/components/skills/modals/LocalPickModal.tsx`
- `src/components/skills/modals/GitPickModal.tsx`

## 验证

实现完成后运行：

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
npx tsc --noEmit
```

所有 95 个 Rust 测试通过，TypeScript 编译无错误。
