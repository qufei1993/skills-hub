# 官网桌面发布数据

正式 GitHub Release 发布后，工作流读取 GitHub 最新稳定版本、安装包链接与中英文日志，发布到独立 Cloudflare Pages 项目。官网运行时读取，无需重建官网。合集发布不受影响。

## 一次性配置

1. 已创建并初始化 Direct Upload Pages 项目 `skills-hub-release-data`，生产分支 `main`，绑定 `releases.aiskillshub.link`。使用 Pages 免费计划；不启用 Functions、R2 或数据库。
2. 在桌面仓库 Actions Secrets 添加 `CLOUDFLARE_API_TOKEN`，权限为对应账户的 **Cloudflare Pages / Edit**。不把密钥放进代码、文档或聊天。
3. 工作流合入默认分支后，手动运行 `Publish desktop release data`，仅首次勾选 `bootstrap`。确认 `current.json` 和日志可读取，再上线官网读取改动。

[Cloudflare 项目](https://dash.cloudflare.com/e2238e7536f05ab6dbd4eb993fcfed80/workers-and-pages) · [桌面仓库 Secrets](https://github.com/qufei1993/skills-hub/settings/secrets/actions) · [发布工作流](https://github.com/qufei1993/skills-hub/actions/workflows/publish-release-data.yml)

## 数据与发布

- `current.json` 指向 `snapshots/<SHA256>/index.json`，日志在该快照的 `changelog/zh/`、`changelog/en/`。`archive.json` 记录保留的快照。
- 每次拉取并校验历史快照，生成完整目录，再一次部署。历史读取失败或内容不完整时停止，不覆盖线上数据。首次初始化只允许明确的 404，不忽略网络错误。
- `current.json`、归档目录不缓存；快照长期缓存。公开文件允许跨域读取，缺失文件返回 404。
- 安装包仍在 GitHub。工作流不编译桌面应用，不发布官网或合集。重跑旧事件仍读取 GitHub 当前 latest。
- 回滚在此独立 Pages 项目的生产部署列表中选择之前成功的部署。回滚后不要删除旧部署，方便恢复。
- 达到 19,000 文件或单文件超过 25 MiB 时停止发布，提前规划历史快照清理。正常新增版本无需修改官网。

验证：`npm run test:release-data`。本项目无需开通 R2 或付费订阅。
