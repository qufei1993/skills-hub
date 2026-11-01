# Website collection installation in v1.0.0

[简体中文](website-collection-install.zh.md) · [PR #185](https://github.com/qufei1993/skills-hub/pull/185)

## Installation flow

Open a collection on the website to review its Skills and sources in Skills Hub. A collection can contain Skills from one GitHub repository or combine several authors' repositories, each pinned to a specific commit.

Opening a link only validates and displays the request. Installation starts after confirmation. Skills are added to the shared library, with optional global distribution to selected tools; no tools are selected by default. Existing same-source Skills remain unchanged, conflicts are reported, and failed items can be retried without reinstalling successful items. Failed tool distribution can resume without reinstalling the Skill.

An idle manual Add Skill view closes to show the collection confirmation while preserving its form values. Active operations and other confirmations finish before the collection dialog opens. Temporarily deferred confirmation retains installation and retry progress, and repository candidate paths match across Windows and Unix directory separators.

## Desktop launch and platform support

Protocol registration ships in the desktop installer; users of older versions need to update to v1.0.0 or later.

- **macOS**: Valid links show, restore, and focus the main window, including when the app is already running or minimized. Invalid links do not activate it. A development process alone does not register the URL scheme; use a bundled app to verify OS-level launch.
- **Windows and Linux**: The single-instance plugin forwards links to the running window. Scheduled background update processes bypass the single-instance lock. Debian installers include protocol-registration utilities; registration failures do not prevent desktop startup, and package startup verification uses an isolated desktop bus session.

## Link contract and installation safeguards

Website links use `skills-hub://install?manifest=<encoded JSON>`. Manifest v1 contains a collection title, GitHub sources pinned to 40-character commit hashes, and selected Skill names and directories.

Website and native manifests share limits of 2,000 Skills and 100 sources. External links must also fit within 30,000 encoded bytes. This replaces the outdated website-only 100-Skill limit; Aaron's 120-Skill marketing collection opens its complete confirmation list. Regression coverage uses the published collection manifest with its pinned revision and directory paths.

Failed website requests show a persistent error explanation with a specific reason and recovery action. For oversized links, select fewer Skills and install in batches, or open the collection from Add Skills. Unsupported manifest versions suggest updating the app; empty selections, duplicate entries, incorrect installation information, unreadable lists, and receiver failures have separate instructions. Native-dialog failures fall back to a persistent, dismissible notification. Unknown internal errors are replaced with safe recovery instructions.

The parser rejects unsupported versions, unsafe paths, credentials, extra parameters, duplicate entries or names, unpinned sources, and oversized requests. Receiving a link performs no network or credential access. Confirmed installation uses existing services, operation locks, and proxy-aware Git commands. No database migration is required.
