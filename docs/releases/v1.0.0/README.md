# Skills Hub v1.0.0

[简体中文](README.zh.md)

Released on October 11, 2026. Download installers and read the bilingual release notes in the [Skills Hub v1.0.0 release](https://github.com/qufei1993/skills-hub/releases/tag/v1.0.0).

## Features and improvements

| Topic | User-visible changes | Details and pull requests |
| --- | --- | --- |
| Selected Skill batch updates | Update multiple selected Skills from their Git or local sources, preserving tool selections and deployment scope. | Progress, cancellation after the current Skill, persistent result details, and retained failures for retry; disabled and unbound Skills are skipped. |
| Collection discovery | Browse and search the website catalog, load cards while scrolling, and reuse versioned caches. | [Discovery](collection-discovery.en.md); [PR #187](https://github.com/qufei1993/skills-hub/pull/187) |
| Collection installation | Open website links, select Skills, configure tags and deployment, and retry failures. | [Website handoff](website-collection-install.md), [installation workspace](collection-install-workspace.en.md); [PR #185](https://github.com/qufei1993/skills-hub/pull/185), [PR #187](https://github.com/qufei1993/skills-hub/pull/187) |
| Unified installation | Git, local and collection sources share selection and settings steps. | [Installation flow](unified-installation.en.md); [PR #187](https://github.com/qufei1993/skills-hub/pull/187) |
| Website release data | Update download links and bilingual release notes without rebuilding the website. | [Publishing](website-release-data.md); [PR #184](https://github.com/qufei1993/skills-hub/pull/184) |

## Bug fixes

| Problem | Issue | Pull request | Details |
| --- | --- | --- | --- |
| Re-enabling disabled Skills stops at an unavailable tool and leaves saved tools unsynced | — | — | [Saved tool recovery](skill-enable-recovery.md) |
| Scheduled/manual update workers create windows or a macOS Dock icon | [#152](https://github.com/qufei1993/skills-hub/issues/152) | [#193](https://github.com/qufei1993/skills-hub/pull/193) | [Headless updates](headless-updates.md) |
| Skill updates blocked by source file deletions | — | — | [Source-mirrored updates](source-mirror-updates.md) |
| Garbled localized Windows scheduler errors | [#181](https://github.com/qufei1993/skills-hub/issues/181) | [#188](https://github.com/qufei1993/skills-hub/pull/188) | [Error decoding](windows-scheduler-output.md) |
| Windows subdirectory updates fail with `path not found in repo` | [#182](https://github.com/qufei1993/skills-hub/issues/182) | [#189](https://github.com/qufei1993/skills-hub/pull/189) | [Path compatibility](windows-subpath-updates.md) |
| Interrupted Skill updates remain running and block retries | [#183](https://github.com/qufei1993/skills-hub/issues/183) | [#192](https://github.com/qufei1993/skills-hub/pull/192) | [Recovery](auto-update-recovery.md) |
| CLI adoption falsely reports `PLAN_STALE` for excluded entries | [#186](https://github.com/qufei1993/skills-hub/issues/186) | [#191](https://github.com/qufei1993/skills-hub/pull/191) | [Canonical source paths](cli-adopt-excluded-entries.md) |

Collection fixes also cover large manifests, pinned repository-root URLs, conflict selection, completion notifications, cancelled scans, retained confirmation state, website window activation, Linux protocol registration, Windows candidate paths, and project-directory alignment. See the **Fixed** section in [the changelog](../../../CHANGELOG.md) and the linked topic documents.
