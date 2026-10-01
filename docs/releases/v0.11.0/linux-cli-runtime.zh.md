# Linux CLI 运行依赖

[English](linux-cli-runtime.md)

草稿审查发现 ARM64 CLI 直接依赖 GTK、WebKit、JavaScriptCore 等库，而 x64 CLI 没有这些依赖。AppImage 的应用环境不会自动传递到单独启动的终端或 AI Agent，因此缺少系统 WebKit 的 ARM64 机器可能打开桌面，却无法启动已安装的 CLI。问题产物源提交为 `61b11dbdce0d5dfc59fb1ee1d9e4ab6d4fbe0fa8`，实际 ARM64 文件哈希为 `fdd3d55f908beb23f788c48804b5713a1b79c37a943174b909b4e4088465dd7a`。

ARM64 原生构建使用 Clang/LLD，保留桌面所需依赖，并让 CLI 不使用的库能够被移除。x64 在 [Rust 1.90 及以后](https://blog.rust-lang.org/2025/09/01/rust-lld-on-1.90.0-stable/) 已默认使用 LLD。改动仅应用于 Linux ARM64，其他平台的链接配置和缓存输入保持原样。

`verify-linux-cli.mjs` 读取实际 ELF 动态依赖，发现 GUI 库或无效文件即失败。`verify-linux-cli.sh` 将同一个二进制只读挂载到原生 Ubuntu 24.04 容器，仅补充 libdbus 和 zlib，实际运行版本及 doctor 命令。Linux 打包 PR CI、原生 CLI CI、发布验证及最终 CLI 签名阶段均执行此检查；签名环境不再预装 WebKit 来掩盖缺库问题。

桌面仍需要 GTK/WebKit，继续执行 Debian 元数据、安装包内容和 AppImage 窗口启动检查。本修复不承诺兼容旧版 glibc，也不移除 CLI 的基础系统依赖。发布仍保持草稿，直到用户明确确认公开。

回归证据：新增三项依赖测试在检查器实现前失败，之后通过；检查器实际拒绝当前 ARM64 草稿文件，接受 x64 文件。合并前还必须由原生 Linux CI 证明重新链接的产物通过动态依赖检查、无桌面环境运行和桌面打包检查。

本地验证通过：完整 `npm run check`（311 项前端测试、639 项 Rust 测试、2 项兼容性测试、19 项 CLI 测试）、21 项发布脚本回归及桌面开发启动。
