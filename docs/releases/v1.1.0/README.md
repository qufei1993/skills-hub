# Skills Hub v1.1.0

[Chinese](README.zh.md)

Unreleased. This index summarizes the current feature work; package versions remain 1.0.1. Current behavior, compatibility rules and verification evidence are maintained in the [implementation guide](implementation.md). The fixed design remains a historical baseline.

## Features and improvements

| Topic | User-visible changes | Details and pull requests |
| --- | --- | --- |
| Project management | Filter and manage installed Skills by project while keeping preset configuration in Skill Presets and preserving unrelated deployments. | [Library and projects](implementation.md#library-and-project-management) |
| Skill presets and history | Edit and apply presets from the list, create copies from More actions, and use compact selection and result views; preserve conflict checks, history, retries and undo. | [Skill presets](implementation.md#skill-presets) |
| Updates and recovery | Check sources explicitly, update all or selected pending Skills, review differences when needed, restore pre-update content and repair sources or missing deployments. | [Updates and recovery](implementation.md#updates-and-recovery) |
| Interface organization | Keep current update issues in one list, use a compact run summary with log export, and support keyboard actions and three-language copy throughout the affected flows. | [Current behavior](implementation.md#library-and-project-management) |

## Bug fixes

| Problem | Issue | Pull request | Details |
| --- | --- | --- | --- |
| Conflicts, retries and partial failures could obscure the affected target or completed progress. | — | — | Preserve selections and outcomes, show actionable conflict locations, and retain guarded undo and recoverable backups; see [integrity rules](implementation.md#data-integrity-and-compatibility). |
| Library actions opened preset setup or extra confirmations; shared-tool prompts only registered one tool. | — | — | Restore direct tool toggles and updates, keep project management scoped, and apply explicitly confirmed shared tools together; see [library behavior](implementation.md#library-and-project-management). |
| Git checks could stall on helper processes or let one broken source block healthy results. | — | — | Terminate helper processes on cancellation or timeout, isolate failures and exclude internal cache metadata; see [update checks](implementation.md#updates-and-recovery). |

The consolidated change list remains under [Unreleased](../../../CHANGELOG.md#unreleased). See the [verification record](implementation.md#verification-record) for completed checks and remaining coverage.
