# Windows 定时更新错误提示乱码

对应 Issue：https://github.com/qufei1993/skills-hub/issues/181

任务计划程序返回本地代码页编码的错误信息时，原先按 UTF-8 解码会产生替换字符。例如中文 Windows 的“错误: 拒绝访问。”会显示为乱码。

创建、删除、查询状态和立即运行任务统一使用同一个解码入口：保留有效 UTF-8，其余输出通过 Windows `MultiByteToWideChar` 按系统 OEM 代码页转换，兼容中文及其他语言系统。转换失败时保留原有容错处理。

回归测试覆盖 CP936 中文、CP949 韩文、CP850 法文、UTF-8、ASCII、空输出和无效代码页。测试依赖 Windows API，macOS 检查不会执行这些 Windows 专用测试。本次修复让错误原因可读，不改变权限不足等底层调度失败的处理逻辑。

macOS 验证：`npm run check`、`npm run version:check` 通过；解码函数及回归测试通过独立的 Windows 目标类型检查。`npm run tauri:dev` 使用独立标识和端口完成编译及进程启动。PR 的 Windows CI 任务会通过原生 Windows API 执行三个解码回归测试；本地语言 Windows 系统的完整调度验证仍待人工完成。
