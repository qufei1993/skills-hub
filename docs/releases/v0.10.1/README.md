# Skills Hub v0.10.1

v0.10.1 是面向 Windows 用户的 hotfix 版本，修复 Skills Hub v0.10.0 运行期间反复闪现命令窗口的问题。

## 修复内容

- 自动更新进度仍每 5 秒刷新，但只读取本地运行数据，不再反复查询操作系统计划任务。
- 启动阶段不再重复读取完整自动更新配置。
- 手动触发自动更新后的进度等待不再调用系统计划任务命令。
- Windows 计划任务和系统 Git 子进程使用无控制台窗口方式启动。
- macOS 与 Linux 同时避免高频启动 `launchctl` 或 `systemctl`，但保持原有计划任务行为。

## Issue

- [#150：启动软件会有大量命令行窗口闪现](https://github.com/qufei1993/skills-hub/issues/150)

## 验证

- 前端集成测试覆盖启动、5 秒定时刷新和窗口重新聚焦。
- Rust 测试覆盖统一后台进程启动封装。
- Windows CI 使用独立运行探针，通过 `GetConsoleWindow` 验证子进程没有控制台窗口。
- Windows x86_64 目标交叉编译、完整前端与 Rust 检查通过。

## 详细文档

- [Windows 命令窗口闪烁修复](bugfix-windows-command-window-flashing.md)

## 更新日志

- [中文更新日志](../../CHANGELOG.zh.md)
- [英文更新日志](../../../CHANGELOG.md)
