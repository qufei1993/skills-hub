# 安装包 OAuth 配置检查

正式安装版仅使用编译期的 OAuth 配置，不读取用户机器上的 `.env`。开发版能授权不代表打包时配置已经注入。

所有 `npm run tauri:build*` 命令现在先检查 `SKILLS_HUB_GITHUB_CLIENT_ID`。CI 保持从已有构建环境注入；本地显式选择配置文件：

```bash
npm run tauri:build:mac:universal:dmg -- --oauth-env-file /absolute/path/to/.env
```

先只检查配置、不构建：

```bash
node scripts/build-desktop.mjs --oauth-env-file /absolute/path/to/.env --check-oauth-only
```

只提取精确命名的公开 Client ID，不执行文件内容、不展开变量、不导入 Client Secret 或用户 Token、不访问钥匙串；日志不打印配置值。已有显式环境变量优先。不要将 `.env` 复制进安装包。

直接运行 `tauri build` 或 `cargo build` 会绕过 npm 的前置检查；发布和分发包须使用上述受检查的入口。

验证包括缺失与无效值拒绝、重复声明拒绝、环境变量优先、无秘密值日志、实际命令退出码，以及生成包两种架构均包含对应的编译期 Client ID。Client ID 存在不能证明 GitHub 服务、网络或用户授权已成功，仍需在目标设备走一次登录流程。
