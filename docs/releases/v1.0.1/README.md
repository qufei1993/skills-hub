# Skills Hub v1.0.1

[Chinese](README.zh.md)

Released on October 16, 2026. Download installers and read the bilingual release notes in the [Skills Hub v1.0.1 release](https://github.com/qufei1993/skills-hub/releases/tag/v1.0.1).

## Features and improvements

| Topic | User-visible changes | Details and pull requests |
| --- | --- | --- |
| Status and tool filters | Filter Skills by enabled or disabled state and synced tools; counts follow other active filters and Clear filters resets them together. | [PR #197](https://github.com/qufei1993/skills-hub/pull/197) |
| Tool icon hints | Hover or focus a tool icon to see its name and sync state immediately; icons no longer overlap. | [PR #198](https://github.com/qufei1993/skills-hub/pull/198) |

## Bug fixes

| Problem | Issue | Pull request | Details |
| --- | --- | --- | --- |
| Open tooltips could show stale sync states after a tool was toggled. | — | [#202](https://github.com/qufei1993/skills-hub/pull/202) | Refresh the hint with the current state; hide it when the Skill is disabled or the tool is removed. |
| Successful or unchanged batch updates left a result panel requiring manual dismissal. | — | [#203](https://github.com/qufei1993/skills-hub/pull/203) | Show a brief completion notification; retain results requiring attention. |
| Collection installs now use the latest default-branch commit consistently across a batch and retries, and new installs support future updates. | — | [#199](https://github.com/qufei1993/skills-hub/pull/199) | Existing same-source Skills, website links and collection data remain compatible. |
| Windows junction deployment could fail after preview, and CLI local paths could lose literal characters. | [#196](https://github.com/qufei1993/skills-hub/issues/196) | [#201](https://github.com/qufei1993/skills-hub/pull/201) | Fix canonical-path handling and include safe deployment failure diagnostics. |
| Unrelated Git LFS assets could block subdirectory Skill discovery and installation. | [#195](https://github.com/qufei1993/skills-hub/issues/195) | [#200](https://github.com/qufei1993/skills-hub/pull/200) | Limit checkout to the requested subdirectory and keep discovery errors visible. |

See the [changelog](../../../CHANGELOG.md#101) for the full list of changes.
