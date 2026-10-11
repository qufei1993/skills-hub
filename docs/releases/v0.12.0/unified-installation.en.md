# Unified installation confirmation

Git and local sources are detected before installation, even when only one Skill is found. Search results reuse confirmation; collections open their manifest directly.

The confirmation dialog separates choosing Skills from configuring installation. The first step dedicates its content area to the list; the second shows tools, scope, and tag editing. Back preserves selections, searches, and settings. Next never installs anything. Collection installation progress and failures use the full list area. Shared-tool changes are confirmed inline. Actions stay in the footer; long content scrolls.

Back preserves the source and settings, plus selection when rescanning the same source. Late results from cancelled scans are ignored. Closing source entry restores keyboard focus.

Existing Git update/conflict protection, local validation, shared-directory deduplication, project requirements, collection preservation, new-install tagging, retry and success notification behavior remain in their existing services. Git/local search submits visible selections; collection search preserves the full selection.

Regression tests use temporary directories and mocked IPC. Native desktop checks cover actual repository detection, the engineering collection preview, and a temporary local source without changing the real library.

Validation: full check passed (370 frontend tests, 1 existing conditional skip; 660 Rust unit tests, 2 auto-update integration tests, 19 CLI integration tests). Desktop development startup and independent review passed. New regression cases cover confirmation, Back state, inline shared-tool confirmation, focus restoration, cancelled scans and single-item failure retries.

The always-visible tools and scope update passed 24 focused regression tests and desktop layout checks at 1180×760 and 912×640, including required project paths and retained tag summaries. The full check hit the existing CLI platform-mapping test’s default five-second timeout. With a 15-second test limit, all 370 frontend tests passed; build, Rust formatting, Clippy, and all Rust tests then passed.

Two-step regressions cover Next without installation, retained selection/search/settings on Back, visible-only Git/local submission, and keyboard focus containment after changing steps. All 373 frontend tests passed with a 15-second CLI test timeout. Desktop review covered the 31-Skill engineering collection and a temporary local source without installing into the real library.

Installation settings now use compact, equal-height tool choices with natural widths. Tags are available immediately and scroll within a 144px bounded area when numerous. Tools, tags, and scope share the same subtle accent selection treatment.

Tool choices use native square checkboxes to communicate multiple selection, retaining the compact equal-height layout.

Discovery visual refinement: collection identity uses category color tokens with light/dark bindings; search has a distinct soft surface. The source dialog uses compact segmented controls and a smaller form. Installation behavior is unchanged.

Project scope layout: align the expanded directory panel with the control column, left-align recent paths, and share responsive label widths with tools and tags.

Collection, Git and local sources now share SkillSelectionList. Preview conflicts cannot be selected or installed. Search select-all only affects visible eligible entries; existing per-source submission rules remain unchanged. Regression covers conflicting entries through select-all and IPC submission; full check passes.

Unavailable rows use a muted background and minus-marked disabled checkbox. Checkboxes align with the heading; paths appear inline with a full-path tooltip.
