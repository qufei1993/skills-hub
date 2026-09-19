# Verified desktop CLI bridge / 桌面 CLI 桥接

Desktop builds include the same-version native `skillshub-cli`. Normal startup verifies the bundled binary against embedded build metadata, invalidates old stamps, copies to a sibling temporary file, and atomically replaces the bridge binary before publishing SHA-256 and version stamps. Failures do not prevent desktop startup and retain a recoverable damaged status. Status inspection only reads bridge files; it never executes the CLI or accesses credentials.

Production publishes to `~/.skills-hub/bin`; debug builds publish to `~/.skills-hub-dev/bin`. The binary is `skillshub-cli` (`skillshub-cli.exe` on Windows), while stamps are always `skillshub-cli.version` and `skillshub-cli.sha256`.

Sidecar metadata includes its actual debug/release profile, verified from Cargo's compiler artifact. Desktop builds reject either direction of profile mismatch, including conflicting `debug_assertions`; custom Cargo profiles are unsupported. Bridge publication rejects symlinks or Windows reparse points in any existing destination ancestor before creating directories, and pins validated directories for subsequent file operations.

桌面构建内置同版本 CLI，正常启动时验证内置校验值并原子发布。失败不会阻止桌面启动；开发版与正式版目录隔离，状态查询不执行 CLI、不读取凭据，也不复制数据库或配置。

For local Rust checks, first prepare the host sidecar, for example:

```sh
npm run cli:prepare -- --target darwin-arm64 --debug
npm run check
```

Supported explicit targets: `darwin-arm64`, `darwin-x64`, `win32-x64`, `linux-x64`, `linux-arm64`, or their corresponding Rust triples. `tauri:dev` and `tauri:build` prepare the sidecar automatically. Cross-compilation toolchains must already be installed; the script does not download them. Universal macOS builds are not supported by this preparation step.

macOS signing and notarization must include the sidecar; that release-pipeline work is tracked separately. If signing changes the sidecar bytes, regenerate its SHA-256 metadata before compiling the desktop executable.
