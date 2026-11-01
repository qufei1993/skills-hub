# v1.0.0 Windows 子目录技能更新修复（[#182](https://github.com/qufei1993/skills-hub/issues/182)）

[English](windows-subpath-updates.md) · [PR #189](https://github.com/qufei1993/skills-hub/pull/189)

## 问题与用户影响

Windows 上从仓库子目录安装的 Skill，可能在更新时报 `path not found in repo`。v1.0.0 恢复了这类安装记录的更新能力，无需重新安装或迁移数据库。

## 路径与缓存处理

已保存的反斜杠路径被 Git 稀疏检出当作转义模式，导致 Git 可能返回成功，却未检出目标目录。

仓库扫描和安装现在统一使用正斜杠来源路径。更新兼容已有的反斜杠记录，并在更新成功后保存统一格式。首次稀疏检出和已有缓存更新都会转换路径分隔符；缓存即使尚未过期，只要缺少目标目录，也会重新检出。

## 回归覆盖

回归测试使用临时本地仓库和数据库，覆盖首次检出、重复更新、安装、旧记录兼容及空稀疏缓存恢复。
