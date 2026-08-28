# v0.9.1 标签重命名弹窗修复

## 问题

在 macOS 的 Tauri WebView 中，用户进入标签管理并点击重命名按钮后，没有出现输入框，因而无法修改标签名称。

该问题由 [Issue #109](https://github.com/qufei1993/skills-hub/issues/109) 报告，用户环境为 macOS 12.7.6 Monterey、Intel Mac 和 Skills Hub v0.9.0。

## 根因

标签页面直接调用浏览器原生 `window.prompt` 获取新名称。当前 Tauri 使用的 macOS WKWebView UI delegate 没有实现 JavaScript 文本输入对话框，因此调用不会显示可交互界面，也不会产生可供后续重命名逻辑使用的输入值。

安装流程中另有一处 `window.confirm` 依赖相同的浏览器原生对话框能力，也存在静默失效风险。

## 修复

- 新增与现有界面一致的应用内标签重命名弹窗。
- 支持自动聚焦、Enter 保存、Esc 或取消关闭。
- 空名称或未修改的名称不能提交。
- 保存继续复用现有 `rename_tag` 后端命令和成功提示。
- 共享目录确认改用项目已有的应用内确认弹窗。
- 前端不再使用 `window.prompt`、`window.confirm` 或 `window.alert`。

该修复不改变数据库结构、标签关联关系或后端命令协议。

## 验收

1. 在 Tauri 桌面窗口中进入标签管理并点击重命名，应用内输入弹窗正常显示。
2. 输入新名称并保存后，标签列表与关联 Skill 信息刷新，显示重命名成功提示。
3. 按 Esc、点击取消或点击遮罩可关闭弹窗，原名称保持不变。
4. 名称为空或与原名称相同时，保存按钮保持禁用。
5. 共享目录工具切换需要确认时，应用内确认弹窗正常显示。
6. `npm run version:check` 和 `npm run check` 均通过。
