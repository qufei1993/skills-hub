# 讨论原型与历史检查归档

本目录保存产品探索过程，便于回看和复现。它不是应用源码入口，不参与产品构建或发布；页面中的数据、文件操作和结果均为模拟。当前产品行为见[实施说明](../../docs/releases/v1.1.0/implementation.zh.md)。

| 材料 | 状态与用途 |
| --- | --- |
| [skills-hub-roadmap.html](skills-hub-roadmap.html) | 早期方向探索，保留历史，不作为当前需求。 |
| [skills-hub-prototype.html](skills-hub-prototype.html) | 技能预设、项目与更新的早期交互原型，是下列历史检查脚本的默认输入；与后来的固定稿不同。 |
| [review/](review/) | 上述原型的交互、工作区、导航检查脚本，以及 2026-10-23 的交互和浏览器布局报告。历史结果不代表当前产品验收通过。 |
| [next/](next/README.md) | MCP 管理讨论原型，当前不实现、未排期；目录名不承诺下一版交付。 |

固定设计基线另存于 [v1.1.0 原型](../../docs/releases/v1.1.0/prototypes/skills-hub.html)，由基线清单校验；归档和重跑检查不改写它。既有原型不会为匹配当前产品而重写，后续讨论先确认范围。

## 查看与重跑

两份 HTML 可直接在浏览器中打开。历史脚本依赖项目中的 jsdom，安装项目依赖后，在仓库根目录运行：

```sh
node work/prototypes/review/review-independent.cjs
node work/prototypes/review/check-workspace.cjs
node work/prototypes/review/check-final-navigation.cjs
```

脚本通过 JSDOM 检查模拟交互，不验证真实桌面、文件写入或客户端集成。默认只在终端输出结果，不覆盖已归档报告；如需导出新一轮交互结果，可显式指定输出文件：

```sh
SKILLS_REVIEW_REPORT=/tmp/skills-hub-prototype-review.json node work/prototypes/review/review-independent.cjs
```

`SKILLS_REVIEW_HTML` 可以指定其他输入，但检查针对旧原型的交互约定，不能直接用来判断当前应用。历史 `interaction-results.json` 的输入路径已改成仓库相对路径，原日期和结果保留；`browser-layout-results.json` 是当时的浏览器检查快照，没有配套的一键重跑脚本。JSDOM 重跑不能更新或替代该浏览器证据。
