# Bugfix：首次安装后打开时"检测到新工具"弹窗样式异常

## 问题描述

安装后第一次打开 SkillSyncer，"New tools detected"（检测到新工具）弹窗的样式与其他弹窗不一致：标题文字紧贴弹窗边框顶部，操作按钮缺少内边距和分隔线。

## 根因分析

`NewToolsModal` 的 DOM 结构与项目其他弹窗不一致：

```tsx
// 修复前（错误结构）
<div className="modal">
  <div className="modal-title">...</div>   // 无 padding，贴顶
  <div className="modal-body">...</div>
  <div className="modal-actions">...</div> // 只有 margin-top，无 padding
</div>
```

- `.modal-title` 本身只有 `font-size` / `font-weight` / `color`，依赖 `.modal-header` 提供 `padding: 16px 20px` 和 `border-bottom`
- `.modal-actions` 只有 `margin-top: 16px`，没有水平和底部内边距，按钮会贴近弹窗边缘
- 缺少 `role="dialog"` 和 `aria-modal="true"` 无障碍属性

## 修复方案

将结构统一为与其他弹窗一致的 `.modal-header` + `.modal-footer` 模式：

```tsx
// 修复后（正确结构）
<div className="modal" role="dialog" aria-modal="true">
  <div className="modal-header">
    <div className="modal-title">...</div>  // padding: 16px 20px + border-bottom
  </div>
  <div className="modal-body">...</div>
  <div className="modal-footer">...</div>   // padding: 16px 20px + border-top
</div>
```

## 修改文件

- `src/components/skills/modals/NewToolsModal.tsx`：补充 `.modal-header` 包裹标题，将 `.modal-actions` 改为 `.modal-footer`，新增 `role`/`aria-modal` 属性
