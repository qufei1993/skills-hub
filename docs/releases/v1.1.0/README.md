# Skills Hub v1.1.0

[Chinese](README.zh.md)

Unreleased. This index summarizes the current feature work; package versions remain 1.0.1. Current behavior, compatibility rules and verification evidence are maintained in the [implementation guide](implementation.md). The fixed design remains a historical baseline.

## Features and improvements

| Topic | User-visible changes | Details and pull requests |
| --- | --- | --- |
| Project management | Filter and manage installed Skills by project while keeping preset configuration in Skill Presets and preserving unrelated deployments. | [Library and projects](implementation.md#library-and-project-management) |
| Skill presets and history | Apply saved Skill/tool combinations from one dialog, inspect conflicts, retry unfinished work and undo eligible additions; retain 100 finished records plus runs needing recovery. | [Skill presets](implementation.md#skill-presets) |
| Updates and recovery | Check sources explicitly, update all or selected pending Skills, review differences when needed, restore pre-update content and repair sources or missing deployments. | [Updates and recovery](implementation.md#updates-and-recovery) |
| Interface organization | Keep setup tips separate from Skill results and use compact summaries, expandable details, keyboard support and three-language copy throughout the affected flows. | [Current behavior](implementation.md#library-and-project-management) |

## Bug fixes

| Problem | Issue | Pull request | Details |
| --- | --- | --- | --- |
| Conflicts, retries and partial failures could obscure the affected target or completed progress. | — | — | Preserve selections and outcomes, show actionable conflict locations, and retain guarded undo and recoverable backups; see [integrity rules](implementation.md#data-integrity-and-compatibility). |
| Git checks could stall on helper processes or let one broken source block healthy results. | — | — | Terminate helper processes on cancellation or timeout, isolate failures and exclude internal cache metadata; see [update checks](implementation.md#updates-and-recovery). |

The consolidated change list remains under [Unreleased](../../../CHANGELOG.md#unreleased). See the [verification record](implementation.md#verification-record) for completed checks and remaining coverage.
