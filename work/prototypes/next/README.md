# MCP 管理讨论原型（当前不实现）

状态：2026-10-30 确认为未来功能讨论，未排期，不属于本次版本；`next` 目录名不代表下一版承诺。正式讨论入口见 [MCP 管理](../../../docs/future/mcp-management.md)。本目录纳入版本管理，保存设计来源和可复现原型，不接入应用运行、构建或发布。

原型基于 `v1.1.0-design-r1`，初稿日期 2026-10-24。数据与文件操作均为内存模拟，不修改本机工具配置、不运行 MCP 命令、不进行真实授权；刷新后重置。不要输入真实密钥。

## 文件与职责

| 文件 | 用途 |
| --- | --- |
| [DESIGN.md](DESIGN.md) / [REVIEW.md](REVIEW.md) | 候选设计与历史原型评审，不是研发承诺。 |
| [mcp.js](mcp.js) / [mcp.css](mcp.css) | 模拟交互逻辑和样式，后续修改从这里开始。 |
| [build.py](build.py) | 校验固定基线后生成独立页面，不改写基线。 |
| [index.html](index.html) | 已生成、可直接打开的讨论页面，随源码一并保存。 |
| [review-mcp.cjs](review-mcp.cjs) | 读取生成页面，检查模拟交互。 |

本目录不要求持续跟随当前 UI 更新。重新讨论时先确认需求与最小范围，再决定是否更新原型或建立开发基线。固定参考稿中的既有功能也不代表当前产品布局。

## 预览与复现

从仓库根目录运行，需要 Python 3；交互检查使用项目现有的 Node.js 与 jsdom 依赖，可先运行 `npm ci`。

```sh
python3 work/prototypes/next/build.py
node work/prototypes/next/review-mcp.cjs
```

随后可直接用浏览器打开 `work/prototypes/next/index.html`。也可启动只监听本机的静态服务：

```sh
python3 -m http.server 4178 --bind 127.0.0.1 --directory work/prototypes
```

访问 `http://127.0.0.1:4178/next/`；已有服务占用端口时换一个端口，不停止其他服务。`build.py --serve` 是旧本地预览的兼容选项：仅在 `/tmp/skills-hub-local-preview/next/` 已存在时复制页面，不负责启动服务器，普通预览无需使用它。

修改源码后重新生成页面并运行交互检查，提交时同时保存源码与生成页。原型检查只能验证模拟行为；真实客户端适配、凭据、文件写入和跨平台运行需要另行设计和验收。
