# 官网桌面发布数据

正式 GitHub Release 发布后，`Publish desktop release data` 工作流从 GitHub 读取最新稳定版本、真实安装包以及该版本 tag 的中英文 Changelog，发布独立数据源。无需重建官网，也不触发官网部署。预发布和草稿不触发。

## 一次性配置

1. Cloudflare R2 创建专用 bucket：`skills-hub-release-data`，绑定自定义域名 `releases.aiskillshub.link`。不要使用合集 bucket。
2. CORS 允许 `https://aiskillshub.link`、`https://skills-hub-7rq.pages.dev` 执行 GET/HEAD。需要本地联调时增加对应 localhost origin。配置示例：
   ```json
   [{"AllowedOrigins":["https://aiskillshub.link","https://skills-hub-7rq.pages.dev"],"AllowedMethods":["GET","HEAD"]}]
   ```
3. 创建仅此 bucket 的 Object Read & Write 凭据。只放入本仓库 GitHub Actions Secrets：`RELEASES_R2_ACCESS_KEY_ID`、`RELEASES_R2_SECRET_ACCESS_KEY`。禁止提交到代码、日志或聊天。账户 ID 在工作流中配置，是公开标识。
4. 手动运行该工作流一次，初始化现有最新稳定版本。确认公开 `current.json`、快照与中英文日志均可访问，再部署官网读取改动。

[R2 自定义域名](https://developers.cloudflare.com/r2/buckets/public-buckets/) · [CORS 配置](https://developers.cloudflare.com/r2/buckets/cors/)

## 发布与回滚

- 仅安装包索引和日志上传 R2，安装包仍从 GitHub 下载。无需 Rust 构建，Node 工作流限时 10 分钟。
- 每次生成 `snapshots/<SHA256>/`。依次上传全部文件、完成标记，最后切换 `current.json`。上传失败保留旧指针；修好后手动重跑。
- `current.json` 使用 `no-store`，不要对它设置强制缓存规则。快照长期缓存，不覆盖或删除。
- 回滚：手动运行工作流，在 `rollback_snapshot` 填此前成功日志中的 SHA256。只允许激活有完成标记的快照，保留全部历史文件。
- 发布流程读取 GitHub 当前 latest，重跑历史事件不会主动降级。人工指定回滚是例外。
- 合集数据和官网版本发布仍由各自流程管理，本工作流不访问它们。

验证：`npm run test:release-data`。
