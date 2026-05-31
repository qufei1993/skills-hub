# Bug 修复计划

来源：https://github.com/AlwaysCodeHQ/skills-syncer/issues

---

## Bug 1：Git 安装时 skill 名称为 "skills" 导致路径重复（#28）

**Issue**: https://github.com/AlwaysCodeHQ/skills-syncer/issues/28
**严重程度**: P0
**状态**: ✅ 已修复（commit 69ab806）

### 问题描述

通过 Git URL 安装 skill 时，如果 URL 指向名为 `skills` 的子目录（如 `https://github.com/xxx/repo/tree/main/skills`），`install_git_skill` 会从 subpath 推导 name 为 `"skills"`，导致同步到工具时路径变成 `~/.claude/skills/skills/`。

### 根因

`installer.rs:94-104` 的 name 推导逻辑只看 subpath/URL，不读 SKILL.md 的 name 字段：

```rust
let name = name.unwrap_or_else(|| {
    if let Some(subpath) = &parsed.subpath {
        subpath.rsplit('/').next()  // ← subpath 是 "skills" 时，name 就是 "skills"
```

而本地导入走 `install_local_skill_from_selection`，会先从 SKILL.md 读 name，所以不受影响。

### 复现步骤

1. 在 SkillSyncer 中选择「Git 仓库」
2. 输入 `https://github.com/anthropics/skills/tree/main/skills`
3. 不填显示名称，直接安装
4. 同步到 Claude Code 后，路径变成 `~/.claude/skills/skills/`

### 修复方案

修改 `installer.rs` 的 `install_git_skill`，在 name 推导后、写入 central_path 之前，尝试从已下载内容的 SKILL.md 读取 name 覆盖。同时增加保底校验：如果 name 仍为 `"skills"`，报错要求用户手动指定名称。

### 涉及文件

- `src-tauri/src/core/installer.rs` — `install_git_skill` 函数 name 推导逻辑

---

## Bug 2：GitHub API 限流错误未提示重置时间

**Issue**: 使用中发现（关联 #28 复现过程）
**严重程度**: P2
**状态**: ✅ 已修复

### 问题描述

当 GitHub API 返回 403（速率限制）时，错误提示只显示"请稍后再试"，未告诉用户具体的重置时间。GitHub 响应头中包含 `x-ratelimit-reset` 字段（Unix 时间戳），应提取并展示。

### 根因

`github_download.rs:70` 调用 `.error_for_status()` 直接将 403 转为通用错误，丢弃了响应头信息。`installer.rs:148-149` 捕获 403 时也只返回固定文案。

### 修复方案

在 `github_download.rs` 中，不使用 `.error_for_status()`，而是手动检查 status code。当遇到 403 时，从响应头提取 `x-ratelimit-reset` 和 `x-ratelimit-remaining`，将重置时间格式化为本地时间后包含在错误消息中。

示例错误消息：`"GitHub API 访问被拒绝（触发了频率限制）。将于 18:44 重置，请届时再试。"`

### 涉及文件

- `src-tauri/src/core/github_download.rs` — HTTP 请求错误处理
- `src-tauri/src/core/installer.rs` — 403 错误消息构造

---

## 改进：设置页增加 GitHub Token 配置

**严重程度**: P1
**状态**: ✅ 已实现（commit d2c1cc0）

### 问题描述

当前所有 GitHub API 请求均为未认证（60 次/小时/IP），安装 skill 时递归下载目录会快速耗尽配额。用户无法配置 Token 来提升限额。

### 方案

1. **设置页 UI**：增加一个可选的 GitHub Token 输入框（密码类型，支持显示/隐藏），存入 `settings` 表（key: `github_token`）
2. **后端传递**：`github_download.rs` 和 `github_search.rs` 发请求时，从 SkillStore 读取 token，如果存在则加上 `Authorization: Bearer <token>` 请求头
3. **限额提升**：认证后 GitHub API 限额从 60 → 5000 次/小时（按账号计算）
4. **安全**：Token 仅存本地 SQLite，不上传不同步。设置页提示用户生成 fine-grained PAT，只需 `public_repo` 读取权限

### 涉及文件

- `src-tauri/src/core/github_download.rs` — 请求时携带 token
- `src-tauri/src/core/github_search.rs` — 请求时携带 token
- `src-tauri/src/core/skill_store.rs` — 读取 `github_token` 设置
- `src/App.tsx` — 设置弹窗增加 GitHub Token 输入
- `src/i18n/resources.ts` — 新增中英文翻译

---

## Bug 3：Windows 拒绝访问 OS error 5（#20）

**Issue**: https://github.com/AlwaysCodeHQ/skills-syncer/issues/20
**严重程度**: P0
**状态**: ✅ 已修复（commit 93d9aca）

### 问题描述

Windows 用户点击 AI-IDE 同步选项时报 OS error 5（权限不足），即使对应工具未安装。

### 可能原因

1. Windows 上创建 symlink 需要管理员权限或开发者模式，`sync_engine.rs` 的 fallback（symlink → junction → copy）可能在某些环境下全部失败
2. 尝试同步到未安装工具的目录时，`is_tool_installed` 检查可能误判（检测目录存在但无写入权限）

### 涉及文件

- `src-tauri/src/core/sync_engine.rs` — symlink/junction/copy fallback 逻辑
- `src-tauri/src/core/tool_adapters/mod.rs` — `is_tool_installed` 检测逻辑
- `src-tauri/src/commands/mod.rs` — `sync_skill_to_tool` 错误处理

---

## Bug 4：Skill 扫描逻辑对部分目录结构失效（#18 + #8）

**Issue**: https://github.com/AlwaysCodeHQ/skills-syncer/issues/18 / https://github.com/AlwaysCodeHQ/skills-syncer/issues/8
**严重程度**: P1
**状态**: ✅ 已修复

### 问题描述

- #18：某些 git 仓库目录结构无法被正确识别为 skill
- #8：skill 显示在仓库中但实际不存在；发现的 skill 无法导入

### 根因

**#18**：`list_git_skills` 和 `install_git_skill` 的多技能检测只扫描 `skills/` 目录下的子目录。当仓库把 skill 直接放在根目录的子文件夹（如 `repo/my-skill/SKILL.md`，不套 `skills/` 父目录）时，扫描结果为空，用户看到"该仓库中没有 Skills"。

示例仓库：`axtonliu/axton-obsidian-visual-skills`，结构为 `repo/excalidraw-diagram/SKILL.md`、`repo/mermaid-visualizer/SKILL.md` 等，无 `skills/` 目录。

**#8**：`scan_tool_dir`（onboarding 扫描）把工具 skills 目录下的**每个子目录**都当成 skill（不检查 SKILL.md），导致不含 SKILL.md 的目录也被"发现"，但导入时又因缺少 SKILL.md 失败。

### 修复方案

1. **#18**：在 `list_git_skills` 中增加扫描仓库根目录直接子目录中的 SKILL.md；在 `install_git_skill` 的多技能检测中同样覆盖根目录子目录
2. **#8**：`scan_tool_dir` 保持现有行为（不强制要求 SKILL.md），但前端导入时已有校验。或者在 `scan_tool_dir` 中区分"有 SKILL.md"和"无 SKILL.md"的目录

### 涉及文件

- `src-tauri/src/core/installer.rs` — `list_git_skills` 和 `install_git_skill` 的扫描范围
- `src-tauri/src/core/tool_adapters/mod.rs` — `scan_tool_dir` 扫描逻辑

---

## Bug 5：Skill 名称冲突无法安装（#12）

**Issue**: https://github.com/AlwaysCodeHQ/skills-syncer/issues/12
**严重程度**: P1
**状态**: ✅ 已关闭（PR #30 合并，UI 已有「显示名称」输入框可手动指定别名，冲突时提示用户重命名）

### 涉及文件

- `src-tauri/src/core/installer.rs` — 安装时 name 冲突处理

---

## Bug 6：新增 skill 未被自动扫描（#19）

**Issue**: https://github.com/AlwaysCodeHQ/skills-syncer/issues/19
**严重程度**: P2
**状态**: ✅ 已关闭（非 bug，预期行为）

### 调查结论

`~/.skillsyncer/` 是 SkillSyncer 的内部存储，外部工具不应直接写入。实际上 OpenCode 创建的 skill 会落在 `~/.config/opencode/skills/` 下（普通目录），不会进入 `~/.skillsyncer/`。用户误以为写入了 skillsyncer 目录。

如需将外部工具中新建的 skill 纳入 SkillSyncer 管理，可通过"导入已有 Skill"功能手动操作。

---

## Bug 7：不支持 .claude/skills/ 目录格式的仓库（#27）

**Issue**: https://github.com/AlwaysCodeHQ/skills-syncer/issues/27
**严重程度**: P1
**状态**: ✅ 已修复（PR #31 合并）

### 问题描述

使用 `.claude-plugin/plugin.json` + `.claude/skills/` 目录结构的仓库（如 `nextlevelbuilder/ui-ux-pro-max-skill`）无法被 SkillSyncer 识别和安装，因为扫描逻辑只查找 `SKILL.md` 文件。

### 根因

`list_git_skills`、`count_skills_in_repo`、`list_local_skills` 只扫描 `skills/`、`skills/.curated/` 等目录，不扫描 `.claude/skills/`。且只认 `SKILL.md` 为有效标记，`.claude/skills/` 下的目录即使内容完整也被忽略。

### 修复方案

1. 新增 `.claude/skills/` 为扫描路径（与 `skills/` 并列）
2. `.claude/skills/` 下的子目录即使没有 `SKILL.md` 也视为有效 skill
3. 无 `SKILL.md` 时用文件夹名做 name，从 `.claude-plugin/plugin.json` 读取 description

### 涉及文件

- `src-tauri/src/core/installer.rs` — `SKILL_SCAN_BASES` 常量、`is_skill_dir`、`is_claude_skill_dir`、`read_plugin_description`、`extract_skill_info` 辅助函数

---

## Bug 8：界面上没有 OpenClaw 的同步（#29）

**Issue**: https://github.com/AlwaysCodeHQ/skills-syncer/issues/29
**严重程度**: P2
**状态**: ✅ 已关闭（PR #26 已修复代码，文档已补充更新）

### 问题描述

用户界面上找不到 OpenClaw 的同步选项。

### 根因

OpenClaw 更名后将配置目录从 `.moltbot/` 迁移到 `.openclaw/`，代码中的 `tool_adapters` 仍指向旧路径。

### 修复内容

1. PR #26（社区贡献）修复了代码：OpenClaw 路径 `.moltbot/skills` → `.openclaw/skills`，原 `.moltbot` 拆分为独立的 MoltBot 工具
2. 补充更新了 `README.md` 和 `docs/README.zh.md` 中的工具列表（OpenClaw 路径 + MoltBot 新增行）
3. 补充了 `src/i18n/resources.ts` 中缺少的 `moltbot: 'MoltBot'` 翻译

### 涉及文件

- `src-tauri/src/core/tool_adapters/mod.rs` — OpenClaw 路径更新 + MoltBot 新增（PR #26）
- `README.md` — 工具列表更新
- `docs/README.zh.md` — 工具列表更新
- `src/i18n/resources.ts` — MoltBot 翻译新增
