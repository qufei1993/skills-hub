# 本地安装测试构建

常规正式打包仍需匹配的已发布 CLI 清单。缺少清单时说明原因，同时提示 `tauri:dev` 和当前平台的本地测试打包命令，不自动切换构建模式。

新增 `npm run tauri:build:local`，并提供 macOS DMG、Windows MSI/NSIS、Linux DEB/AppImage 快捷入口。只编译所选目标的一份 CLI，将其原始字节嵌入经过 release 优化的桌面程序。启用 AI 管理时校验并安装这份文件，无需依赖源码目录或线上 CLI 发布。dev 与本地 release 的中间文件独立。

应用名称为 **Skills Hub Local Test**，标识为 `com.qufei1993.skillshub.local-test`，桌面可执行文件为 `skills-hub-local-test`，禁用正式版更新地址和 updater 产物。独立文件名避免 Linux 安装包冲突。桌面与 CLI 均使用开发凭据和 `.skills-hub-dev/bin`；数据库、技能库、配置、缓存、回收站和写锁仍与正式版共用，启用 AI 管理会影响真实 Agent 目录和终端 PATH。构建层拒绝缺少独立身份及更新配置的 local-test feature 构建；CLI 准备阶段保留避免循环依赖所需的例外。

## 验证

- 脚本回归测试先失败、修复后通过，覆盖平台提示、Tauri 参数传递、模式和清单冲突、feature 绕过、目标及 profile 选择和独立中间文件。
- 最终 `npm run check` 通过：287 项前端测试、627 项 Rust 单元测试、2 项兼容测试、19 项 CLI 集成测试，以及网络边界、lint、前端构建、格式检查和 Clippy。
- 携带本地测试配置及清单执行 release、all-features Rust 单元测试，627 项全部通过；包含在临时目录安装并执行真实内置 CLI、开发 bridge 选择、校验失败保留旧文件、凭据隔离和共享数据身份检查。
- macOS ARM 本地 DMG 构建成功，共 16,819,278 字节。只读挂载实际 DMG，将应用复制到源码目录之外，核验应用标识、桌面文件名和内置 CLI 字节，并确认窗口从 `tauri://localhost` 正常加载。dev 启动也已确认；验证应用及开发进程已停止。
- 一项既有文件锁测试出现临时失败，单独复测及最终全量检查均通过，未修改无关锁实现。
- 一次独立只读审查发现文件名冲突、feature 绕过和 release 测试命名空间断言问题，均已修复并重新验证。
- 版本与 diff 空白检查通过，保持 0.11.0。

本机未验证 Windows/Linux 原生安装、macOS 签名及公证、正式 CLI 下载流程。手动启动验证未点击真实的一键启用，安装测试全部使用临时目录。macOS universal 和 Windows ARM 仍不在当前 CLI 支持目标中。
