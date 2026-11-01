# CLI 导入含排除项的目录

[English](cli-adopt-excluded-entries.md) · 修复 [#186](https://github.com/qufei1993/skills-hub/issues/186) · [PR #191](https://github.com/qufei1993/skills-hub/pull/191)

## 问题与修复

来源路径未规范化，且目录包含空文件夹、无效 Skill 或不安全链接等排除项时，`skills adopt --yes` 可能误报 `PLAN_STALE`。预览保存了按输入根目录生成的排除路径，确认却重新扫描规范化目录，即使文件系统未变化也会产生不同快照。

单项与容器扫描的预览和确认现在统一使用规范化根目录。排除的链接仍按链接自身位置记录，包含断链；真实内容变化及路径越界仍会阻止不安全导入。

## 回归覆盖

服务与 CLI 测试覆盖相对路径、父目录片段和别名，以及越界链接和断链。三个回归在修复前失败、修复后通过。同时修正 Unix CLI 桥接测试中的系统调用声明，兼容新 Rust 编译器，保持同步行为。
