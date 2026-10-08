# Website collection installation

Website links use `skills-hub://install?manifest=<encoded JSON>`. Manifest v1 contains a title, GitHub sources pinned to 40-character commit hashes, and selected Skill names/directories. This contract supports both single-repository collections and curated combinations from several authors.

Opening a link only validates and displays the request. The user confirms installation and may select tools for global distribution; no tools are selected by default. Existing same-source Skills remain unchanged, conflicts are reported, and failed distribution can resume without reinstalling the Skill.

Protocol registration ships in the desktop installer. Users need the version containing this feature; an older installed app cannot receive these requests. macOS development runs alone do not register a URL scheme: test OS-level launch with a bundled app. Windows/Linux use the single-instance plugin to forward links to the running window. Scheduled background update processes bypass the single-instance lock.

The parser rejects unsupported versions, unsafe paths, credentials, extra parameters, duplicate entries/names, unpinned sources and oversized requests. Receiving a link performs no network or credential access. Installation uses existing services, operation locks and proxy-aware Git commands. No database migration or automatic installation is added.

On macOS, valid collection links show, restore, and focus the main window, including when the app is already running or minimized. Invalid links do not activate the window.

Opening a valid collection request dismisses the idle manual Add Skill view, preserving its form values, so the collection confirmation is visible. Active operations and other confirmations still finish before the collection dialog opens.

Candidate directories are normalized across Windows and Unix. Temporarily deferred confirmations retain installation progress and retry records.
