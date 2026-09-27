# Standalone CLI installation

The Bash and PowerShell installers reuse the versioned native CLI assets and SHA-256 files from the latest stable GitHub release. They do not publish npm packages, require Node.js, or change the desktop bridge directories. Users can repeat the same command to upgrade.

See the [English installation guide](../../../README.md#standalone-cli-installation) or [中文安装说明](../../README.zh.md#独立安装-cli) for commands, destinations, and limitations. The scripts must be merged into `main`, and v0.11.0 or later must be published with its CLI assets before those commands work.

Downloads and checksums are verified before replacing an existing executable. Installation stages replacement on the destination filesystem. Bash updates Bash/Zsh profiles idempotently; PowerShell updates user and session PATH. Unsupported systems or architectures fail explicitly. Independent installation does not install the official management Skill or provide desktop-only settings and sync features.

Validation uses isolated download fixtures and temporary user directories, including repeat installation, platform selection, checksum failure, failed download, and existing-file preservation. The Windows installer tests also run in the existing lightweight Windows CI job, without a full desktop build.
