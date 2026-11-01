# Collection installation workspace

[简体中文](collection-install-workspace.zh.md) · [PR #187](https://github.com/qufei1993/skills-hub/pull/187)

## Selection, tagging, and deployment

Collection confirmation uses the shared two-step flow. Skills are selected by default; users can deselect items or use select-all. Search preserves selection, an empty selection disables installation, and installation locks selection. Progress and retries include only selected items.

Choose existing tags or create a tag for new installs, then select tools and global or project deployment. Tags are saved before distribution and existing tags remain unchanged. Project deployment requires a directory. Existing same-source content is preserved but can still be deployed using its backend-verified Skill ID.

## Conflicts, pinned sources, and retries

A read-only local preview identifies existing sources and name conflicts without downloads or credential access. Installation waits for preview and revalidates sources during execution. Conflicting items cannot be selected or installed.

Valid repository-root `/tree/<ref>` URLs are accepted, fixing rejection before download for pinned collection sources. `/blob/<ref>/<path>` still requires a file path. Each pinned repository is scanned once per attempt. Retries refresh discovery, retain installed IDs, and resume tagging or distribution without reinstalling successful items.

All selected items succeeding refreshes the library, closes confirmation, and shows a success notification. Failures retain the dialog for retry; successful retries also close it. The official management Skill documents pinned installation and partial-failure recovery through existing single-Skill CLI commands.

## Regression coverage

PR tests cover pinned and invalid sources, source/name conflicts, preview, tagging, scan reuse, selected-only installation, retries, completion, and project requirements. Recorded desktop validation installed all 31 Skills from Skills for Real Engineers with a dedicated tag and deployment to Claude Code and Codex, and checked light/dark themes, EN/ZH/KO, and narrow windows.
