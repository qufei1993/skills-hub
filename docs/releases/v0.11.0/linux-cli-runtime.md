# Linux CLI runtime dependencies

[中文](linux-cli-runtime.zh.md)

The draft audit found that the ARM64 CLI directly linked GTK, WebKit, JavaScriptCore and related libraries, although the x64 CLI did not. AppImage's application environment does not transfer to a separate terminal or AI Agent. An ARM64 machine without system WebKit could therefore open the desktop while its installed CLI failed before entering the program. The source was `61b11dbdce0d5dfc59fb1ee1d9e4ab6d4fbe0fa8`; the actual ARM64 draft binary hash was `fdd3d55f908beb23f788c48804b5713a1b79c37a943174b909b4e4088465dd7a`.

ARM64 native builds use Clang/LLD, retaining required desktop dependencies while allowing unused CLI libraries to be discarded. x64 already uses LLD by default in [Rust 1.90 and later](https://blog.rust-lang.org/2025/09/01/rust-lld-on-1.90.0-stable/). This change is restricted to ARM64 Linux; other platforms retain their existing linker configuration and cache inputs.

`verify-linux-cli.mjs` reads actual ELF dynamic dependencies and rejects GUI libraries or malformed files. `verify-linux-cli.sh` then runs the exact binary from a read-only mount in a native Ubuntu 24.04 container with only libdbus and zlib added, checking version and doctor commands. Linux packaging PR CI, native CLI CI, release verification and the final CLI signing job all run this gate. Signing runners no longer install WebKit to mask a missing dependency.

The desktop still needs GTK/WebKit and is checked by the existing Debian metadata, bundle-content and AppImage window smoke tests. This does not promise compatibility with older glibc versions or remove the CLI's base system dependencies. The release remains a draft until explicitly approved.

Regression evidence: all three new dependency tests failed before the checker was implemented, then passed. The checker rejects the actual ARM64 draft binary and accepts the actual x64 one. Native Linux CI must additionally prove that the newly linked artifacts pass dependency inspection, the headless runtime test and desktop packaging before this change can merge.

Local validation passed: full `npm run check` (311 frontend tests, 639 Rust tests, 2 compatibility tests and 19 CLI tests), all 21 release-script regressions, and desktop development startup.

These x64 and ARM64 CLI binaries require glibc 2.39 or later on Ubuntu 24.04 or a compatible GNU/glibc distribution. Ubuntu 22.04, Debian 12 and Alpine/musl are outside this build baseline.
