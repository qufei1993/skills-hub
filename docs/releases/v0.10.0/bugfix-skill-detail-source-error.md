# v0.10.0 源文件缺失时的 Skill 详情修复

本地原始来源目录不存在时，Skill 会正确显示来源异常，但异常提示条此前在纵向详情布局中占用了整个工作区高度，导致详情标题、文件列表和文件内容被挤出可视区域。

本次修复让异常提示只按自身内容占高。详情页继续读取中央仓库中的托管副本，因此原始来源缺失时仍可查看已有文件；来源异常状态和后续更新保护保持不变。

新增样式回归测试，直接加载应用样式并验证异常提示不会再次占满详情工作区。

## English

When an original local source directory is missing, the Skill correctly reports a source issue. The warning previously consumed the full height of the vertical detail layout, pushing the detail header, file list, and managed file content outside the visible workspace.

The warning now takes only its content height. The detail view continues to read the managed copy from the central repository, so existing files remain inspectable while the source issue and update safeguards stay intact.
