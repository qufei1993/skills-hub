# Standalone CLI installation

The recommended flow is Settings → AI management → Enable in one click in the desktop app. This downloads and verifies the matching version of the CLI, configures Bash/Zsh profiles or Windows user PATH, and installs the official Skill. Existing setups without terminal configuration retain the enable action. Desktop installers exclude CLI binaries. First-time setup requires network access; matching verified installations can be reused offline. CLI and official Skill updates require an explicit click in Settings; startup only reads their status. Startup and status reads never modify PATH. Open a new terminal after enabling; restart the terminal application if necessary. Terminal configuration failures are shown separately and can be retried.

Standalone installation is a non-recommended alternative for users who do not use the desktop app. The Bash and PowerShell installers reuse the versioned native CLI assets and SHA-256 files from the latest stable release in the public `qufei1993/skills-hub` repository. They do not publish npm packages, require Node.js, or change the desktop bridge directories. Users can repeat the same command to upgrade.

See the [English installation guide](../../../README.md#standalone-cli-installation) or [中文安装说明](../../README.zh.md#独立安装-cli) for commands, destinations, and limitations. The scripts must be merged into `main`, and a compatible CLI release must be publicly available in the resource repository before those commands work.

Downloads and checksums are verified before replacing an existing executable. Installation stages replacement on the destination filesystem. Bash updates Bash/Zsh profiles idempotently; PowerShell updates user and session PATH. Unsupported systems or architectures fail explicitly. Independent installation does not install the official management Skill or provide desktop-only settings and sync features.

Validation uses isolated download fixtures and temporary user directories, including repeat installation, platform selection, checksum failure, failed download, and existing-file preservation. The Windows installer tests also run in the existing lightweight Windows CI job, without a full desktop build.

If a CLI update fails, the previously verified CLI remains available and Settings shows a retryable pending update. When the desktop process finds another CLI earlier in its PATH, Settings asks users to verify the command location in a new terminal. A shell can modify PATH dynamically, so the desktop cannot prove every terminal’s command resolution without executing the user’s shell configuration.

Development builds publish the CLI sidecar prepared for that exact build. This avoids checksum mismatches when a different `skillshub-cli` executable happens to be located beside the running desktop binary. Packaged desktop builds embed only the matching release manifest and download its fixed version on demand.
