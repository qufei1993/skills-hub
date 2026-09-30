# 桌面 CLI 桥接

[English](verified-cli-bridge.md) · [版本概览](README.zh.md)

正式桌面安装包只携带匹配的 CLI 清单，不携带 CLI 可执行文件。点击启用或更新 AI 管理时，从 Skills Hub 同一个 Release 下载固定版本、对应平台的文件，验证内嵌的长度与 SHA-256 后安装。已校验的匹配桥接可离线复用；启动和状态查询不下载或更新组件。

桥接安装通过同目录临时文件和受保护的原子替换发布，随后写入版本和 SHA-256 校验戳。替换失败时尽可能恢复原来已校验的 CLI。状态检查只读取桥接文件，不执行 CLI、不读取凭据。创建目录前检查目标上级目录中的符号链接或 Windows 重解析点，后续操作绑定到已验证的目录。

正式版使用 `~/.skills-hub/bin`，开发版和本地测试版使用 `~/.skills-hub-dev/bin`。二进制为 `skillshub-cli`，Windows 为 `skillshub-cli.exe`；校验戳为 `skillshub-cli.version` 和 `skillshub-cli.sha256`。

## 构建模式

- 正式打包使用版本、源提交及平台匹配的 release CLI 清单，不将 CLI 文件放入安装包。
- 开发启动自动准备本地 debug CLI，并校验构建模式和文件字节。
- [本地安装测试构建](local-test-builds.zh.md)内嵌本地 release CLI，使用开发凭据和桥接目录，禁用正式更新地址。技能数据与工具目录仍与正式版共享，操作会影响真实数据。

不支持自定义 Cargo profile 或冲突的 `debug_assertions`。CLI 准备支持 `darwin-arm64`、`darwin-x64`、`win32-x64`、`linux-x64`、`linux-arm64` 及对应 Rust 三元组。编译工具链须提前安装，不支持 macOS universal 准备。

在 macOS arm64 上执行本地 Rust 检查：

```sh
npm run cli:prepare -- --target darwin-arm64 --debug
npm run check
```

`tauri:dev` 和 `tauri:build:local` 自动准备本地 CLI；常规 `tauri:build` 必须提供匹配的 release 清单，详见[构建说明](../../README.zh.md#构建)。

## 发布验证

工作流先对独立 CLI 签名，并按配置完成公证，再根据最终字节生成清单。正式桌面构建嵌入清单，排除 CLI 可执行文件。实物检查覆盖 macOS 应用目录和实际 Windows NSIS 安装指令；缺少安装指令或发现 CLI 载荷均失败。

CLI 与桌面产物进入原仓库同一个 Release 草稿，公开前核对 CLI 长度和摘要，完整发布后验证固定版本匿名下载。相同版本的冲突资源不覆盖。CLI 与桌面的签名、公证分别验证，详见[当前验证结论与发布前置条件](cli-on-demand-download.zh.md)。
