# Windows scheduled update error decoding

Issue: https://github.com/qufei1993/skills-hub/issues/181

Task Scheduler command errors were decoded as UTF-8 even when Windows returned local-code-page bytes. On Simplified Chinese Windows, this replaced readable errors such as “错误: 拒绝访问。” with replacement characters.

Creation, deletion, status queries, and immediate runs now share one decoder. Valid UTF-8 is preserved; other output is converted with the Windows system OEM code page through `MultiByteToWideChar`. This also supports non-Chinese Windows installations without hard-coding GBK. Conversion failures retain the existing lossy fallback.

Regression tests cover CP936 Chinese, CP949 Korean, CP850 French, UTF-8, ASCII, empty output, and an invalid code page. These tests require Windows; macOS checks do not execute them. This change improves error reporting and does not resolve the underlying scheduler failure, such as missing permissions.

Validation on macOS: `npm run check` and `npm run version:check` passed; the decoder and regression tests passed an isolated Windows-target type check. `npm run tauri:dev` compiled and launched an independent development process using a separate identifier and port. The PR Windows CI job runs the three decoder regressions against the native Windows API. A localized Windows scheduler smoke test remains pending.
