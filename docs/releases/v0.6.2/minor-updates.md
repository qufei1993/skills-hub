# v0.6.2 小需求与体验优化记录

本文件保留 v0.6.2 发布周期的历史记录，不再追加后续版本的功能或修复。新变更遵循[变更与发布规则](../../../CONTRIBUTING.md#change-records-and-releases)。

## 2026-06-19

### 新增 WorkBuddy 支持

- 新增 WorkBuddy 工具适配器，支持将全局 Skill 同步到 `~/.workbuddy/skills/`（PR [#73](https://github.com/qufei1993/skills-hub/pull/73)）。
- 当用户目录下存在 `~/.workbuddy/` 时，Skills Hub 会将 WorkBuddy 识别为已安装工具。
- WorkBuddy 当前仅支持全局 Skill 同步；由于项目级 Skill 目录尚未确认，暂不提供项目级同步。
- 英文和中文工具支持列表已同步补充 WorkBuddy。
- 增加 Rust 测试，覆盖工具标识查询和项目级同步能力判断。
- 修复验证：`npm run check`。

### 新增 CodeWhale 支持

- 新增 CodeWhale 工具适配器，响应功能请求 Issue [#70](https://github.com/qufei1993/skills-hub/issues/70)。
- 支持将全局 Skill 同步到 `~/.codewhale/skills/`，并通过 `~/.codewhale/` 检测 CodeWhale 是否已安装。
- 支持将项目级 Skill 同步到项目下的 `.codewhale/skills/`，确保 CodeWhale 同步可独立启停，不与其他工具共享 `.agents/skills/`。
- 英文和中文工具支持列表已同步补充 CodeWhale。
- 增加 Rust 测试，覆盖工具标识、全局目录、检测目录和项目目录。
- 实现验证：`npm run check`。

### 自动发现 Claude Code 插件中的 Skills

- 支持读取 Claude Code 的已安装插件清单，自动发现用户级插件中包含的 Skills（Issue [#69](https://github.com/qufei1993/skills-hub/issues/69)）。
- 支持插件根目录、标准 `skills/` 目录，以及 `.claude-plugin/plugin.json` 声明的自定义 Skill 路径。
- 项目级插件暂不纳入自动发现，避免将只对特定项目生效的 Skill 错误导入为全局托管项。
- 发现结果会显示插件名称和版本，并继续由用户审核选择后导入。
- 按真实路径去重，并拒绝插件根目录之外的声明路径，避免重复条目和越界扫描。
- 实现验证：`npm run check`。
