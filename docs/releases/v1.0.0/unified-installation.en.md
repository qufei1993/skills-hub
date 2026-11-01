# Unified installation confirmation

[简体中文](unified-installation.zh.md) · [PR #187](https://github.com/qufei1993/skills-hub/pull/187)

## Selection and settings

Git and local sources are detected before confirmation, including single Skills. Search results reuse that confirmation; collections open their manifest directly. All three sources share “Select Skills → Installation settings.” Next opens settings without installing; Back preserves selections, searches, tags, tools, and scope.

The shared selection list provides search, eligible select-all, expandable descriptions, inline paths, and clear disabled rows. Settings use square tool checkboxes, a bounded tag area, and scope controls. Shared-directory changes are confirmed inline. Footer actions remain visible while long content scrolls.

## Behavior and fixes

Cancelled scans ignore late results instead of reopening confirmation, and closing source entry restores focus. Conflicting collection items are unchecked, disabled, and excluded from counts and submitted installation requests. Project directories and recent paths align with other controls, including narrow windows.

Git updates retain source/conflict and local-edit protections. Local invalid entries cannot be installed. Git/local search installs visible selected items; collection search preserves the full selection. Shared tool directories are deduplicated, project deployment requires a directory, and unsupported tools are unavailable for project scope.

## Regression coverage

PR tests cover single-Skill confirmation, Next without installation, retained state on Back, inline shared-tool confirmation, focus containment and restoration, cancelled scans, eligible select-all, actual IPC submission, and failure retries. Tests use temporary sources and mocked IPC; desktop validation covered repository and collection previews and narrow-window layouts.
