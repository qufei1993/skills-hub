# Collection installation workspace

The desktop confirmation keeps a searchable, independently scrolling Skill list beside installation settings, with a persistent heading and action bar. Narrow windows place settings before the list. Existing light/dark tokens and keyboard focus behavior are preserved.

Installation reuses repository discovery, single-Skill installation, tagging and deployment commands, plus tool icons, scope selection and project job generation. Users can select existing tags or create one for newly installed items only. Tags are saved before deployment so partially successful batches remain discoverable. Existing tags are retained. Same-source library content is preserved while still allowing deployment to selected tools.

Each pinned repository is scanned once per attempt. Retries refresh discovery and retain installed IDs, resuming tagging and deployment. Project deployment cannot start without a project directory.

The official Agent management Skill documents website manifest handling, pinned SHA installation, existing single-Skill CLI commands and partial-failure recovery. No collection CLI command is introduced.

Same-source items use the backend-verified Skill ID, preventing renamed Skills from resolving to an unrelated item with the same name. Scope changes normalize shared-tool selections.

Validation: `npm run check` passed, including regression coverage for tagging, retries, scan reuse, project scope and source/name collisions. The desktop development build was rebuilt and launched. Fixture-based visual checks covered the engineering collection in light/dark themes, EN/ZH/KO and a narrow window. A manual website handoff exposed rejection of pinned repository-root references; all 31 items failed before download. This validation bug now has regression coverage and a fix. After the fix, the real desktop app installed all 31 Skills from Skills for Real Engineers and deployed them to Claude Code and Codex. Cleanup tag `合集安装测试-20261008` contains 31 items; the library grew from 61 to 92. Deselecting one item showed 30 / 31, and select-all restored 31 / 31 with the matching action count.

Follow-up fix: installation accepts repository-root `/tree/<ref>` references while still requiring a file path for `/blob/<ref>/<path>`. Regression coverage includes pinned SHA and invalid sources. Pending rows use text status without a radio-like empty circle.

Collection Skills are selected by default, with individual checkboxes and a select-all control supporting mixed state. The install action shows the selected count and is disabled for an empty selection. Search preserves selection; installation locks it, and progress and retries include only selected items.

After all selected items succeed, refresh the library, close the dialog and show the successful count. Failures keep the dialog open; a successful retry also closes it automatically.

A second real desktop installation completed all 31 items, closed the dialog automatically and displayed the success notification. Newly installed items retained the dedicated test tag.

A read-only local preview classifies existing sources and name conflicts using the same Git installation classifier, without downloads or credential access. Installation waits for the check; failures can be retried. Preview status is separate from execution results, and installation revalidates sources before preserving existing content and tags or deploying to selected tools.
