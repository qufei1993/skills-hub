# 删除 My Skills 刷新按钮

## 背景

GitHub Issue #61 反馈中文界面下 My Skills 筛选栏中的 `刷新` 按钮样式错乱。

检查后确认该按钮只会手动重新读取当前 Skill 列表和标签，不触发重新扫描工具、Git 更新或重新同步。安装、删除、同步、编辑标签等操作后已有自动刷新。

## 更新内容

- 删除 My Skills 筛选栏中的 `Refresh` / `刷新` 按钮。
- 移除对应的前端回调、组件 props、图标引用和中英文 i18n 文案。
- 通过 PR #63 关联修复 GitHub Issue #61。

## 验证

- `npm run check`
