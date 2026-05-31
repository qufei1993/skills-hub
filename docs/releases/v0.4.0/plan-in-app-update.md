# 需求：应用内检查更新（Issue #33）

## Context

来源：https://github.com/AlwaysCodeHQ/skills-syncer/issues/33

用户每次跟进版本都需要手动进入 GitHub releases 页面下载，体验不便。需要在软件内支持手动检查更新功能。

## 现状分析

后端 Tauri updater 插件已完全就绪：
- `tauri-plugin-updater` v2 已安装（Cargo.toml + package.json）
- `tauri.conf.json` 已配置更新端点（GitHub releases）+ 公钥签名验证
- `lib.rs` 已注册插件
- i18n 翻译键已全部就绪（EN/ZH）

唯一缺失：前端没有调用更新 API 的 UI。

## 实施方案

在 SettingsModal 底部版本信息区域扩展为"检查更新"功能块：

1. **SettingsModal.tsx** — 添加更新状态管理 + UI
   - 状态：idle → checking → up-to-date / available → downloading → done / error
   - 使用 `@tauri-apps/plugin-updater` 的 `check()` 和 `downloadAndInstall()` API
   - 保存 update 对象引用避免重复请求
   - 弹窗关闭时重置状态

2. **App.css** — 添加更新区块样式
   - 版本号 + 按钮水平排列
   - 更新可用时显示高亮区块
   - 错误/成功状态样式

3. **版本号** — 0.3.0 → 0.3.1

## 涉及文件

| 文件 | 改动 |
|------|------|
| `src/components/skills/modals/SettingsModal.tsx` | 添加更新检查 UI + 逻辑 |
| `src/App.css` | 添加更新区块样式 |
| `package.json` | 版本号 → 0.3.1 |
| `src-tauri/tauri.conf.json` | 版本号 → 0.3.1 |
| `src-tauri/Cargo.toml` | 版本号 → 0.3.1 |
