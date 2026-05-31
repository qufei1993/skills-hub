# 需求：支持 QoderWork 目录（Issue #34）

## Context

来源：https://github.com/AlwaysCodeHQ/skills-syncer/issues/34

QoderWork 是 Qoder 推出的桌面 AI 代理产品，与 Qoder IDE 独立，使用 `~/.qoderwork/skills/` 目录存放技能。当前代码库已支持 Qoder（`.qoder/skills`），但尚未支持 QoderWork。

## 实施方案

参照现有 Qoder 适配器模式，在 3 处添加 QoderWork 支持：

### 1. `src-tauri/src/core/tool_adapters/mod.rs`

- **ToolId 枚举**：在 `Qoder` 之后添加 `QoderWork`
- **as_key() 方法**：添加 `ToolId::QoderWork => "qoderwork"`
- **default_tool_adapters()**：添加 ToolAdapter 实例：
  ```rust
  ToolAdapter {
      id: ToolId::QoderWork,
      display_name: "QoderWork",
      relative_skills_dir: ".qoderwork/skills",
      relative_detect_dir: ".qoderwork",
  },
  ```

### 2. `src/i18n/resources.ts`

- 英文 tools 对象：添加 `qoderwork: 'QoderWork'`
- 中文 tools 对象：添加 `qoderwork: 'QoderWork'`

## 验证

- `npm run check` 确保 lint、build、Rust clippy/test 全部通过
