# Compact device sync and shared device records

## English

The sync dashboard now uses three compact rows for the last result and actions, the local-to-repository relationship, and the automatic schedule. Historical devices live in the first activity tab. Device names and last sync times remain visible, with local alias editing; online and version estimates are removed. The connection guide opens beside its own link with setup steps and the current repository and branch, separately from sync mechanics help. Light/dark themes and English, Chinese, and Korean copy are supported.

The repository root contains one `devices.json`:

```json
{
  "version": 1,
  "devices": {
    "device-uuid": {
      "name": "Office Mac",
      "lastSyncedAt": 1788671380000
    }
  }
}
```

`lastSyncedAt` is a Unix timestamp in milliseconds, captured for the published sync, not an online heartbeat. A successful sync can create a metadata-only commit to refresh it. Runs without Skill changes remain absent from the application's content history. Device IDs persist locally, and local aliases, credentials, tool paths, and app settings are not exported.

Sync fetches the latest branch, preserves the other device entries, updates the current entry, and pushes without force. A non-fast-forward push retries the entire fetch/merge/publish operation once. An invalid, oversized, unsupported, or symlinked registry stops the operation instead of being overwritten. Unknown compatible JSON fields are preserved.

Repositories without the JSON are bootstrapped from existing commit trailers. With a registry present, only the newer unchanged-registry commit tail is inspected for older clients; normal registry commits stop that walk immediately. Commit trailers continue to be written for previous clients and baseline recovery. The existing local `device_sync_devices` table caches discovered records; listing devices reads the cache and does not open Git or read credentials. Manual checks immediately refresh the visible list. No database schema or project version change is needed.

Validation covers real two-device repositories, migration and subsequent legacy commits, stale push rejection and reconciliation, invalid metadata protection, symlink boundaries, preserved metadata and local aliases, and immediate UI refresh. Full project checks and desktop startup are required before delivery. Visual checks use the real component with synthetic device data.

## 中文

同步页面顶部改为三行紧凑布局，分别展示同步结果与操作、本机与仓库、自动同步设置。下方首个标签为“设备同步记录”，保留设备名、最近同步时间和本地别名编辑，移除在线与版本落后的推断，接入帮助在链接旁独立展示操作步骤、当前仓库和分支，与同步原理说明分开。

仓库根目录使用一个 `devices.json`，按设备 ID 保存名称和毫秒级 Unix 同步时间。同步会先拉取、保留其他设备条目，再更新本机记录并推送。即使 Skills 没有变化，更新设备时间也可能产生元数据提交；应用的内容同步历史仍忽略零变化运行。记录不代表设备当前在线。

没有 JSON 的旧仓库从提交记录迁入；新仓库仅检查最近一次 JSON 更新之后的旧客户端提交，并继续写提交标记以兼容旧客户端及同步基线恢复。本地 `device_sync_devices` 表仅作展示缓存，页面刷新不再打开 Git 或读取凭据。“检查变化”完成后立即刷新列表。

过期推送会重新拉取合并并重试一次。格式损坏、不支持的版本、过大文件和符号链接会中止操作，不覆盖原文件。别名、凭据、工具路径和应用设置保持本地存储，无需数据库迁移或版本升级。

## First-sync conflict deletion fix / 首次同步冲突误删修复

Resolving a conflict previously advanced the entire library baseline to the remote commit, even when unrelated remote Skills had never been applied locally. The next run could delete those unseen Skills. Resolution now persists a repository-scoped, per-Skill remote baseline, atomically with the conflict status, without advancing the last successful sync. Text merging uses the matching per-Skill commit. Choices survive restart and retries, and are ignored once the successful baseline advances, including interrupted cleanup. No schema change is required; the previous version can ignore the new feature-specific setting.

此前，解决冲突会把整个仓库标记为已同步，导致未下载到新设备的 Skill 在下次同步被误判为本机删除。现在仅为解决的单个 Skill 保存对应远端基线，并与冲突状态原子写入，不推进整体同步基线。状态支持重启与重试，成功同步后即使清理中断也不会再次生效。回归测试使用真实双设备 Git 仓库，覆盖缺少 31 个 Skill 时的三种冲突选择、重启和后续远端更新。已有误删需要从 Git 历史单独恢复；升级不会悄悄改写既有同步历史。

## Device-local source paths / 本地来源路径隔离

Local Skill exports omit source_ref, source_subpath and source_revision. Legacy manifests and Git baselines are normalized before comparison, so machine-specific paths do not become content conflicts. Git source metadata remains unchanged. Receivers bind imported local Skills to their own managed directory; existing local installations preserve their own binding, including genuinely missing sources.

A feature-specific local setting records source ownership. Legacy records without the setting use their creation time within a sync run or after a resolved Skill conflict to identify imports. Local installation explicitly records ownership, deletion removes it, and identity reconciliation migrates it. Only a missing-source error associated with replacing a foreign binding is cleared in the same database transaction; other source and tool issues remain intact. No shared schema version changes. Existing databases remain readable by earlier versions, but both devices must upgrade for the new path-free sync metadata semantics.

本地 Skill 只共享内容及可移植信息，不再携带来源路径、子路径和本地版本字段；旧清单与历史基线先规范化，再比较，避免路径差异制造冲突。Git 来源信息不变。接收端使用自己的托管目录；本机安装保留原始来源，即使来源暂时不存在也不会掩盖异常。

本地来源归属标记随安装、删除和身份合并维护。旧版无标记记录依据导入时间与同步／冲突处理记录识别。修复外来路径时，仅原子清除对应的“来源缺失”异常，其他来源和工具异常继续显示。回归覆盖真实双设备、旧清单、无标记旧数据、外来路径在接收端恰好存在、原机来源缺失、手动刷新，以及删除后本地重装。两台设备应同时升级后再同步；升级后再次同步即可修复已导入的错误绑定。

## Accurate sync history / 同步统计与明细

Counts are computed before applying the merge plan. A Skill present on both sides counts as an update, while new IDs count as additions. Internal `.skills-hub-cache.json` files are excluded from export and normalized out of legacy manifest hashes; cache-only or device-registry-only changes do not create content history rows. New runs atomically save names, change kinds and directions with their totals using feature-specific local settings; repository reset removes those details with the history. No database schema change is required. Existing history without details is labelled as legacy/unadjusted rather than guessed from incomplete evidence. Conflicted plans explicitly say they have not been applied.

统计在执行合并前计算，已有 Skill 的单侧变化计为更新，真正新增的 ID 才计新增。内部缓存文件不参与导出或旧快照哈希，缓存或设备登记变化不再制造内容历史。新记录原子保存每个 Skill 的名称、变化类型、方向和总数，可展开查看；切换仓库清理历史时一并清理明细。不改数据库结构。旧记录未保存明细，明确标注“旧版统计 · 未校正”，不凭猜测修改历史；冲突计划明确提示尚未执行。中英韩文案、深浅色和窄窗口均已检查，视觉预览使用真实组件与示例数据。


## Follow-up reliability fixes / 复核修复

Conflict adoption reads the immutable recorded remote commit, validates its file paths and hashes, and preserves executable modes. Checking for newer changes cannot silently change the chosen version. Applied remote changes and Keep Both copies are recorded as a separate resolved operation with per-Skill names and directions; this does not advance the library baseline or the last full-sync status. Unchanged content does not create an update entry.

Self-bound local sources represent downloaded managed copies. They no longer trigger original-source overlap protection during redistribution, deletion, or storage migration; actual independent source directories remain protected. Moving storage atomically updates both the managed path and any self-bound source reference. Legacy import inference requires a completed bounded run; abandoned running records cannot claim later installs.

History now displays all loaded records, with an option to load older records beyond the original 50-row limit. Requests use a stable ordering, replace the loaded prefix to avoid duplicates, and discard stale responses. These changes require no database schema migration and retain English, Chinese, and Korean translations.

冲突处理锁定当时记录的提交，校验快照路径和内容并保留脚本权限，检查更新不会偷偷改变已选择的版本。实际采用仓库内容和保留双方生成的副本单独写入“冲突已处理”明细，不推进全库基线，也不把整个仓库标记为同步成功；相同内容不会多计更新。

下载的托管副本可正常重复分发、删除及迁移；真正的本地原始目录仍受保护。迁移同时重定向托管路径和自绑定来源。旧版导入推断只使用有结束时间的记录，避免崩溃留下的同步状态影响后来安装的 Skill。历史列表支持继续加载更早记录，不再隐藏第 9 条及第 51 条以后的内容。

Validation: `npm run check` passed (160 frontend tests, 352 Rust tests, lint, build, formatting, and clippy). Regression failures were reproduced before their fixes for snapshot identity, resolution history, executable permissions, legacy status fallback, abandoned-run inference, self-bound source migration, existing symlink redistribution, and older history access. The working branch’s normal `tauri:dev` process rebuilt and started successfully.


## Recycle-bin metadata / 回收站元数据

Manual and remote deletions snapshot the saved description and tag names in local settings together with the recycle-bin entry, before deleting the Skill record. Restore atomically recreates the Skill and tags and consumes the entry, refreshes the Skill and tag lists, and preserves the backup on failure. An existing Skill ID is not overwritten. Legacy entries without snapshots can recover descriptions from SKILL.md; missing legacy tags are not guessed. No database schema or shared sync format changes are required.

手动删除及远端同步删除均在删除 Skill 记录前，保存描述和标签名称到本机回收站元数据。恢复时原子还原 Skill、标签关联并移除回收站记录，同时刷新标签计数；恢复失败保留备份，不覆盖已有同 ID 的 Skill。旧条目从 SKILL.md 补充描述，无法凭空补回未保存的旧标签。不修改数据库结构及跨设备同步格式。

Restore validation: `npm run check` passed with 160 frontend and 354 Rust tests. Tests cover restart, saved description precedence, tag recreation, invalid-tag rollback, legacy description fallback, and existing-directory collisions. The normal working-branch Tauri development app rebuilt and started.
