# Re-enabling Skills and restoring saved tools

[简体中文](skill-enable-recovery.zh.md)

Disabling a Skill removes its active tool deployments and retains its saved tool records. Previously, re-enabling used the detected-tool list without checking whether a tool was enabled in Tools. A disabled but detected tool could fail with `TOOL_NOT_INSTALLED`, stop restoration, and leave the Skill enabled with its other saved tools still unsynced. Batch errors also displayed the internal error code because the operation title was prepended before translation.

Individual and bulk enable operations now restore only saved selections eligible under the current tool settings. Disabled or removed tools and undetected global tools are skipped without deleting their saved records. Project targets retain their saved project paths and still follow project-scope support checks. Newly detected tools are not added automatically.

A failed target no longer prevents other saved tools or Skills from restoring. Failure notices identify the Skill and tool and translate the reason separately. Individual operations refresh their state after both success and failure, including partial cleanup failures during disable.

Historical tool paths also recover correctly. For example, older Cline records used `.agents/skills`, while the current tool uses `.cline/skills`. If a target is disabled and the old entry has been removed, re-enabling may recreate it at the current configured tool path and update the saved record. Existing old entries and active historical records still block the change; current-path conflict, ownership, source-overlap, and content checks remain enforced. A path change is reported separately from an existing-directory conflict, with English, Simplified Chinese, and Korean explanations.

Regression tests cover global and project disable/enable round trips, retained selections, skipped tools, independent target failures in individual and bulk operations, readable errors, state refresh after cleanup errors, historical-path recovery, and refusal to replace existing or active historical targets. No database migration or version bump is required.
