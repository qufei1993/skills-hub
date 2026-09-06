# Consistent Skill issue status

- Persist source-update failures separately from device and tool synchronization results. A successful source update or unchanged source check clears the source issue; device synchronization cannot clear it.
- Refresh My Skills and open details when background update progress changes, without requiring the Updates page to be open.
- Show pending tool distribution as partial completion and distinguish historical failures from their current recovery state.
- Conservatively mark legacy missing-source failures as requiring a fresh check. Store only categorized diagnostic codes in the new source-check records.

## 中文

- 来源更新异常独立保存，只有来源更新或无变化检查成功才会清除，设备同步不会误清除。
- 后台更新进度变化时刷新首页和已打开的详情，无需先进入更新页。
- 工具分发待处理明确标为部分完成，历史失败区分当前是否恢复；旧来源缺失错误保守提示重新检查。
