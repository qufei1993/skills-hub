# 桌面 CLI 桥接

[English](verified-cli-bridge.md) · [版本概览](README.zh.md)

桌面构建内置同版本的原生 `skillshub-cli`。仅在用户主动启用 AI 管理时，先使用构建时嵌入的元数据验证二进制，再使旧校验戳失效，将二进制复制到同目录临时文件并原子替换，最后发布 SHA-256 和版本校验戳。普通启动不会安装、更新或修复桥接文件；安装失败会在启用操作中提示，可由用户再次点击重试。状态检查只读取桥接文件，不执行 CLI，也不读取凭据或复制数据库、配置。

正式版发布到 `~/.skills-hub/bin`，开发版发布到 `~/.skills-hub-dev/bin`。二进制名为 `skillshub-cli`，Windows 使用 `skillshub-cli.exe`；校验戳始终为 `skillshub-cli.version` 和 `skillshub-cli.sha256`。

内置 CLI 元数据记录实际开发或正式构建模式，并根据 Cargo 编译产物验证。桌面构建会拒绝任一方向的模式不匹配，包括冲突的 `debug_assertions`；不支持自定义 Cargo 构建配置。桥接发布在创建目录前拒绝目标上级目录中的符号链接或 Windows 重解析点，后续文件操作绑定到已验证的目录。

## 本地验证

执行本地 Rust 检查前，先准备与当前机器匹配的 CLI。例如，在 macOS arm64 上：

```sh
npm run cli:prepare -- --target darwin-arm64 --debug
npm run check
```

支持显式指定 `darwin-arm64`、`darwin-x64`、`win32-x64`、`linux-x64`、`linux-arm64`，或对应的 Rust 目标三元组。`tauri:dev` 和 `tauri:build` 会自动准备内置 CLI。交叉编译工具链必须预先安装，脚本不会下载工具链；此准备步骤不支持 macOS 通用二进制构建。

## 发布验证

发布工作流先对 macOS CLI 签名，再重新计算 SHA-256 元数据并编译桌面应用。已签名 CLI 通过 `bundle.macOS.files` 放入 `Contents/MacOS/skillshub-cli`，该次发布构建禁用 `externalBin`，避免再次签名改变文件字节。

最终应用包必须恰好包含一个可执行 CLI，其哈希与元数据一致；配置签名身份时，应用和 CLI 的代码签名都必须有效。可选公证同时提交应用与独立 CLI。原生签名发布验证仍依赖受保护的 CI 环境，这里的准备与本地检查不代表已签名或已发布。
