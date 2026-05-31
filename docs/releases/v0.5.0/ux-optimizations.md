# UX 优化记录

收录不需要单独文档的小型 UX 改进。

---

## 关闭按钮改为隐藏窗口（macOS）

**变更：** 点击红色 X 按钮不再退出应用，而是隐藏窗口。

**原因：** macOS 上许多主流应用（Slack、Discord 等）均采用此交互模式——应用在后台持续运行，下次打开时响应更快。需要真正退出时使用 `Cmd+Q` 或菜单栏退出。

**实现方式：**
- 拦截 `CloseRequested` 窗口事件，阻止默认关闭行为，改为隐藏窗口。
- 点击 Dock 图标时触发 `RunEvent::Reopen`，重新显示窗口并聚焦。

**涉及文件：** `src-tauri/src/lib.rs`

---

## Skill 描述与 Markdown 预览优化

**变更：** 修复 `SKILL.md` frontmatter 在列表和详情页中的展示问题。

**原因：** 部分 Skill 使用 YAML 折叠块语法（例如 `description: >-`）。旧解析逻辑没有识别 `>-`、`>+`、`|-`、`|+`，导致列表卡片错误显示 `>-`，详情页元信息也可能出现描述缺失或排版异常。

**实现方式：**
- 后端 `SKILL.md` 解析支持 YAML block scalar 的 chomping indicator：`>`、`>-`、`>+`、`|`、`|-`、`|+`。
- 启动时重新从 `SKILL.md` 比对并回填描述，纠正已入库的旧错误值。
- 详情页 frontmatter 改为响应式 key/value 元信息区，避免短字段被表格挤压成竖排。
- Markdown 预览内容区居中展示，并保留最大可读宽度。

**涉及文件：**
- `src-tauri/src/core/installer.rs`
- `src-tauri/src/core/skill_store.rs`
- `src-tauri/src/core/tests/installer.rs`
- `src-tauri/src/core/tests/skill_store.rs`
- `src/components/skills/SkillDetailView.tsx`
- `src/App.css`

---

## 取消同步后的 Agent 重新启用入口

**变更：** 修复在“我的 skills”页面取消部分 agent 同步后，已取消同步的 agent 没有重新启用入口的问题。

**原因：** v0.4.3 中未同步 agent 只会在卡片展开状态下渲染。触发场景是：某个 skill 同步到多个 agent 后，用户取消 Qoder、GitHub Copilot 等部分 agent；如果该 skill 剩余已同步 agent 数量不超过 5 个，卡片不会显示 `+N more` 展开按钮，导致被取消同步的 agent 无法以灰色按钮显示，看起来像“消失了，无法重新添加”。关闭重开应用也不会恢复，因为这是渲染条件问题，不是临时状态卡住。

**实现方式：**
- 当卡片不需要折叠时，直接显示未同步 agent 的灰色按钮。
- 保留折叠场景下通过 `+N more` 展开查看完整 agent 列表的行为。

**涉及文件：** `src/components/skills/SkillCard.tsx`

---

## Bug：导入同名且内容一致的 Skill 时同步冲突

**关联 Issue：** https://github.com/AlwaysCodeHQ/skills-syncer/issues/51

**状态：** 已在本地验证修复，待 v0.5.0 发版后关闭 issue。

**变更：** 修复从一个工具导入 Skill 后，同名且内容一致的其它工具目录仍被判定为同步冲突的问题；同时修复导入弹窗在部分同步失败后不关闭、未选择任何 Skill 时仍可点击“导入并同步”的问题。

**原因：** 旧同步逻辑只判断目标目录是否存在。即使 Codex、Cursor 等工具下的同名 Skill 内容完全一致，也会返回 `TARGET_EXISTS`，导致该工具永远无法被 Hub 接管。导入流程还会在部分失败时只显示错误 toast，不关闭 `ImportModal`。

**实现方式：**
- `sync_skill_to_tool` 增加 `overwriteIfSameContent` 参数。
- 后端在目标目录已存在时比较 Hub 中央仓库 Skill 与目标目录的内容 hash；内容一致时允许安全接管，内容不一致时继续阻止覆盖。
- 所有前端同步入口都传入 `overwriteIfSameContent: true`，确保导入、创建后同步、单个工具同步、批量同步和范围切换行为一致。
- 导入流程改为逐项处理，单个 Skill 导入或同步失败不会中断后续项。
- 导入结束后统一关闭弹窗；有错误时只显示错误提示，全部成功时才显示成功提示。
- 未选择任何 Skill 时禁用“导入并同步”，并在提交入口增加空选择校验。

**涉及文件：**
- `src-tauri/src/commands/mod.rs`
- `src/App.tsx`
- `src/components/skills/modals/ImportModal.tsx`

---

## 新增：Hermes Agent 全局同步支持

**关联 Issue：** https://github.com/AlwaysCodeHQ/skills-syncer/issues/54

**状态：** 已在本地验证，纳入 v0.5.0。

**变更：** 新增 Hermes Agent 工具适配，支持将 Skill 全局同步到 `~/.hermes/skills`。

**原因：** Issue #54 请求支持 Hermes Agent。调研 Hermes Agent 官方文档后，仅确认默认 `HERMES_HOME` 为 `~/.hermes`，Skills 位于 `HERMES_HOME/skills`。没有找到明确的项目级 skills 目录规范，因此本次只声明并实现全局同步支持，避免为项目级同步捏造路径。

**实现方式：**
- 新增 `hermes_agent` 工具 key，展示名为 `Hermes Agent`。
- 全局 skills 目录配置为 `.hermes/skills`，安装检测目录为 `.hermes`。
- `supports_project_scope` 对 Hermes Agent 返回 `false`。
- 后端在项目级同步入口拒绝不支持项目级同步的工具，返回 `PROJECT_SCOPE_UNSUPPORTED|<tool>`。
- 前端在项目级批量同步时跳过 Hermes Agent；用户在项目级 Skill 上单独点击 Hermes Agent 时显示“不支持项目级同步”提示。
- README 和 CHANGELOG 同步标注 Hermes Agent 仅支持全局同步，并列入 v0.5.0。

**涉及文件：**
- `src-tauri/src/core/tool_adapters/mod.rs`
- `src-tauri/src/commands/mod.rs`
- `src-tauri/src/core/tests/tool_adapters.rs`
- `src/App.tsx`
- `src/i18n/resources.ts`
- `README.md`
- `docs/README.zh.md`
- `CHANGELOG.md`
- `docs/CHANGELOG.zh.md`
