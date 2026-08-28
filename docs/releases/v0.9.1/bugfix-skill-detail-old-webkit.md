# v0.9.1 Skill 详情旧版 WebKit 兼容性修复

## 问题

在不支持正则后行断言的 macOS WKWebView 中，用户从 My Skills 点击任意 Skill 并打开 Markdown 详情时，GFM 自动链接解析会抛出 `SyntaxError`。由于异常发生在 React 渲染阶段，应用根组件会被卸载，最终表现为整页白屏，只能重启恢复。

该问题由 [Issue #108](https://github.com/qufei1993/skills-hub/issues/108) 报告，用户环境为 macOS 12.7.6 Monterey、Intel Mac 和 Skills Hub v0.9.0。

## 根因

`remark-gfm` 的自动链接解析使用正则后行断言 `(?<=...)`。Safari 16.4 之前的 WebKit 不支持该语法，而降低 Vite 构建目标不会转译依赖中的这类正则表达式。

## 修复

Skill 详情页在启用 GFM 插件前检测当前 WebView 是否支持正则后行断言：

- 支持时继续使用 frontmatter 与完整 GFM 渲染。
- 不支持时保留 frontmatter 处理并降级为标准 Markdown，避免执行不兼容的 GFM 自动链接解析。

该修复不修改 Skill 文件、中心仓库或后端数据。

## 验收

1. 在正常 WebKit 能力下打开 Skill 详情，Markdown 与 GFM 正常渲染。
2. 模拟 WebView 拒绝后行断言后打开同一详情，标准 Markdown 正常显示，应用根节点保持挂载。
3. 页面无 `Invalid regular expression` 异常，也不再出现整页白屏。
4. 兼容性检测的支持和不支持分支均有自动化测试。
5. `npm run version:check`、发布说明提取和 `npm run check` 均通过。
