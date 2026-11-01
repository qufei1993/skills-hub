# 发布构建复用

[English](build-performance.md) · [发布总览](README.zh.md)

[上一次候选版本构建](https://github.com/qufei1993/skills-hub/actions/runs/36714248531)在验证阶段已经编译并检查了正式 CLI，后面的 `cli-build` 仍重新编译。Mac Intel 的这次重复编译耗时 21.3 分钟，桌面编译又耗时 14.4 分钟。这些是优化前的观测数据，不代表本次优化后的实测提升。

原生发布验证、手动原生 CI 和普通 Rust CI 现在通过 `CARGO_BUILD_TARGET` 统一使用本平台的构建目录，避免 CLI 准备、Clippy 和测试在两个输出目录中重复编译依赖。发布验证、手动原生 CI 与桌面打包使用相同的本平台正式构建缓存标识；main 上的手动原生 CI 可为后续标签构建准备正式缓存。普通 Rust CI 使用独立调试缓存，避免不可变的调试缓存阻止保存正式构建依赖。缓存仍区分编译器和依赖版本，未命中时执行完整编译。参考 [rust-cache 配置](https://github.com/Swatinem/rust-cache#example-usage)。

验证阶段上传本次运行中的正式 CLI 及清单。`cli-build` 下载本平台产物，在恢复执行权限前检查提交、版本、平台、构建模式、大小和 SHA-256，随后执行原有签名、公证和原生检查，不再编译 CLI，也无需安装前端和原生编译依赖。Linux 签名任务仍安装 WebKit、D-Bus 和 zlib 运行库，再执行原生检查；旧 ARM 二进制仍链接 GTK/WebKit，复用编译产物不能省掉这些运行时前提。签名后按最终文件重新生成清单。产物传输会丢失执行权限，因此必须在验证后恢复；参考 [upload-artifact 权限说明](https://github.com/actions/upload-artifact#permission-loss)。

桌面打包只下载对应平台的最终清单，不再下载五个平台的 CLI 二进制集合。发布仍使用完整五平台 CLI 资源，签名、桌面打包和上传仍等待所有原生验证通过，完整版本保持草稿。本次修改不会触发标签构建、替换现有草稿资源、改变安装包内容或削弱发布检查。

缓存内容不可覆盖，参考 [GitHub 缓存规则](https://docs.github.com/en/actions/reference/workflows-and-actions/dependency-caching)。

## 验证

回归测试在本机执行复用文件校验和五种平台参数的工作流交接脚本，覆盖文件篡改及执行权限恢复。工作流测试检查平台范围、构建目录与缓存配置、签名顺序、对应清单选择和草稿规则。`CARGO_BUILD_TARGET=aarch64-apple-darwin npm run check` 全部通过：300 项前端测试、636 项 Rust 单元测试、2 项兼容性测试、19 项 CLI 集成测试，以及网络边界、代码检查、构建、格式和 Clippy 检查。独立交接和工作流测试通过 10 项，版本和差异空白检查通过。本次验证没有触发新的五平台发布或签名任务。实际缓存命中和总构建耗时需要在下一次获准的原生或发布构建中测量，暂不承诺提速比例或固定时长。
