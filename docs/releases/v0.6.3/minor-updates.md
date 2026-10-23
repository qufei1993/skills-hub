# v0.6.3 小需求与体验优化记录

本文件保留 v0.6.3 发布周期的历史记录，不再追加后续版本的功能或修复。新变更遵循[变更与发布规则](../../../CONTRIBUTING.md#change-records-and-releases)。

## 2026-06-27

### 修复 Antigravity 2.0 全局 Skill 同步路径

- 修复将 Skill 同步到 Antigravity 时使用旧目录的问题（Issue [#79](https://github.com/qufei1993/skills-hub/issues/79)，PR [#80](https://github.com/qufei1993/skills-hub/pull/80)）。
- 全局 Skill 同步目录从 `~/.gemini/antigravity/skills/` 更新为 `~/.gemini/config/skills/`，匹配 Antigravity 2.0 当前文档。
- Antigravity 安装检测目录同步调整为 `~/.gemini/config/`。
- 项目级 Skill 同步目录保持为 `<project>/.agents/skills/`。
- 英文和中文工具支持列表已同步更新 Antigravity 路径。
- 增加 Rust 回归测试，覆盖 Antigravity 全局 Skill 目录、检测目录和项目级目录映射。
- 修复验证：`npm run check`。

### 修复 Windows 自动更新平台元数据缺失

- 修复 Windows 检查更新时报 `windows-x86_64` 平台缺失的问题（Issue [#78](https://github.com/qufei1993/skills-hub/issues/78)，PR [#82](https://github.com/qufei1993/skills-hub/pull/82)）。
- Windows 发布构建恢复生成 Tauri updater 签名文件，NSIS 安装包会随 release assets 一起上传对应的 `.exe.sig`。
- `updater.json` 生成逻辑新增 `windows-x86_64` 和 `windows-aarch64` 平台条目，分别指向 x64 与 arm64 Windows 安装包。
- 增加发布流水线校验：缺少 Windows updater 签名文件时直接失败，避免发布缺失平台信息的 `updater.json`。
- 修复验证：`npm run check`，并本地模拟生成 updater JSON 确认包含 macOS 与 Windows 平台键。
