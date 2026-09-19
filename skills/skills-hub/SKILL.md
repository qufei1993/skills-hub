---
name: skills-hub
description: Use this skill whenever an Agent needs to query, find, install, deploy, update, tag, adopt, diagnose, or safely remove Skills Hub content, including natural-language requests to manage Skills for Codex, Claude Code, Cursor, or another Agent. Always use the Skills Hub CLI workflow instead of editing Skill directories or the database.
---

# Skills Hub

Translate the user's requested Skill operation into the trusted `skillshub-cli` commands below. Manage the local library and explicitly selected Agents. Keep device sync, automation, credentials, and application settings in the desktop app.

## Resolve a trusted CLI before every workflow

1. Inspect `~/.skills-hub/bin` without executing a binary. On Windows expand the user's home directory and use `skillshub-cli.exe`; otherwise use `skillshub-cli`. Use the isolated `~/.skills-hub-dev/bin` only when the user explicitly requests the development environment; never mix development and production data.
2. If that bridge directory exists (including a broken link or inaccessible path), require a regular executable binary and both regular stamp files `skillshub-cli.version` and `skillshub-cli.sha256` in that directory. Read only these public stamps. Validate a nonempty version and a 64-digit hexadecimal SHA-256 value, then compute the binary's SHA-256 with the platform's hashing utility and require an exact match. Reject symlinked stamps/binaries and unresolvable paths. Do not execute an unverified binary.
3. After hash validation, bind `$CLI` to that absolute binary path and run `$CLI --json version`. Require exit 0, `ok: true`, and `data.version` equal to the version stamp. Missing files, invalid stamps, mismatched hash/version, execution failure, and malformed output mean **bridge damaged**: fail closed, stop, and ask the user to open the desktop app once to republish the bridge. Never fall back to PATH when the bridge location exists but fails validation. Do not repair or edit stamps yourself.
4. Only when the desktop bridge directory is completely absent, resolve `skillshub-cli` from PATH for a CLI-only environment. Bind `$CLI` to the resolved absolute path and run `$CLI --json version`. If unavailable, explain the desktop installation, global npm package `skillshub-cli`, or standalone release binary options. Do not silently install software.

`$CLI` below denotes that verified executable, not a literal command or shell fragment. Pass arguments as distinct values; quote paths correctly and never evaluate user input as shell code. Read-only bridge inspection and hashing are the only filesystem work this workflow needs.

## Interpret machine results

Use `--json` for **every** CLI invocation. Read successful JSON from stdout and failed JSON from stderr; inspect the exit status, `ok`, stable `code`, and structured `details`. Treat `message` as localized explanatory text, never as a parsing contract. Do not report success without a successful result and a CLI readback.

Exit codes: 0 success (including previews); 2 invalid arguments; 3 missing or ambiguous item; 4 safety/path/content conflict; 5 operation busy; 6 incompatible database; 7 network/authentication/source failure; 10 internal failure. Preserve unknown error codes and details rather than guessing. `--json` never implies confirmation.

## Query and choose

Run read-only queries directly:

```text
$CLI --json skills list
$CLI --json skills show <name-or-id>
$CLI --json skills search <query> --limit 20
$CLI --json skills status <name-or-id>
$CLI --json skills check <name-or-id>
$CLI --json skills check --all
$CLI --json agents list
$CLI --json doctor
$CLI --json version
```

Use list filters `--tag`, `--source`, `--agent`, `--untagged`, or `--status` when needed. Prefer returned full Skill IDs. On `AMBIGUOUS_SKILL`, show candidates and resolve the user's choice; never choose an arbitrary match.

## Install and deploy

Library installation and Agent deployment are separate states. Install a single Skill when the user explicitly requests it:

```text
$CLI --json skills install <local-path-or-git-ref>
$CLI --json skills install <repo-ref> --subpath <skill-path>
```

Use explicit local paths (`./`, `../`, absolute, or `~/`), Git references, or supported `owner/repo` shorthand. On `MULTI_SKILLS`, show `details.candidates` and use the selected subpath; never automatically install every candidate. Read back with `skills show` using the returned ID.

An install enters only the central library. Deploy only to Agents the user explicitly named; never infer all installed Agents. If the user requested installation without naming an Agent, finish with the library result and explain that deployment is separate.

```text
$CLI --json skills deploy <skill-id> --agent codex
$CLI --json skills deploy <skill-id> --agent codex --agent cursor --dry-run
$CLI --json skills deploy <skill-id> --agent codex --project <absolute-project-path>
$CLI --json skills undeploy <skill-id> --agent codex
```

Use global scope unless the user requested a project. Shared physical directories can affect additional Agents; inspect the preview and report every affected Agent. Do not silently broaden the user's scope. For batch undeploy, first run the exact request with `--dry-run`, show all targets, obtain confirmation, and then run it without `--dry-run`. Deployment commands do not accept `--yes`; do not invent flags. Read back `skills status` after deployment or undeployment. Undeploy retains the library copy.

## Update

For one requested Skill, run `skills check <skill-id>`, then `skills update <skill-id>`, and read back `skills show` and `skills status`. For a batch, always run `skills check --all`, show the affected Skills and held-back changes, and obtain confirmation before `skills update --all`. The check is the supported batch preview; update does not accept `--dry-run` or `--yes`.

Stop on `UPDATE_HELD_BACK`: files would be removed or unsafe changes detected. Keep the old version and guide the user to review in the desktop app. Never add `--force` or manually remove extra files to make an update pass.

## Adopt existing Skills

```text
$CLI --json skills adopt <agent-skills-dir> --dry-run
$CLI --json skills adopt <agent-skills-dir> --yes
```

Show the preview's candidates and exclusions, obtain confirmation of the exact scope, and only then execute with `--yes`. Managed sources/targets are excluded automatically. Adopt records unknown external content as local; never invent a Git origin. Verify the returned IDs using `skills show`.

## Tags

Execute explicitly requested single-Skill tag changes and tag renames, then read back:

```text
$CLI --json skills tag add <skill-id> <tag>...
$CLI --json skills tag remove <skill-id> <tag>...
$CLI --json skills tag set <skill-id> <tag>...
$CLI --json skills tag list <skill-id>
$CLI --json skills tag list
$CLI --json skills tag rename <old> <new>
$CLI --json skills tag delete <tag> --dry-run
$CLI --json skills tag delete <tag> --yes
```

For tag deletion, show the affected Skill count and obtain confirmation before `--yes`. Respect case-insensitive tag matching; do not create duplicate spellings to bypass constraints.

## Safe removal and this Skill's setup

```text
$CLI --json skills remove <skill-id> --dry-run
$CLI --json skills remove <skill-id> --yes
$CLI --json setup --agent codex
$CLI --json setup --agent codex --remove --dry-run
$CLI --json setup --agent codex --remove --yes
```

For removal, show the central directory, all deployed targets, and recycle-bin record from the preview. Obtain confirmation before `--yes`; reuse prior authorization only when it clearly covers that exact reviewed scope. Skills go into the Skills Hub recycle bin; restoration and permanent deletion belong to the desktop app. Verify removal using `skills list` or the expected `SKILL_NOT_FOUND` response.

Setup installs the bundled official Skill and deploys only to requested Agents. Removing setup undeploys the official Skill from the named Agents while retaining its library copy. Preview setup removal, confirm, then use `--yes`. Never remove a same-name unmanaged directory or an edited bundled copy.

## Recover safely

- `TARGET_CONFLICT`: report the actual safe `details.path` and reason. Ask the user to review the conflicting content in the desktop app; do not delete, rename, overwrite, or adopt it automatically.
- `UPDATE_HELD_BACK`: report the affected Skill and removal count; leave content intact for desktop review.
- `OPERATION_BUSY`: report that another operation owns the lock; wait for completion or let the user retry. Never remove lock files, kill the desktop app, or run an unbounded retry loop.
- `CONFIRMATION_REQUIRED`: present `details.plan`, obtain confirmation, and repeat with the supported confirmation flag.
- `PLAN_STALE`: create and review a fresh preview; prior confirmation is not authorization for changed scope.
- `AUTH_REQUIRED`: direct the user to configure credentials in the desktop app. Never request a Token in chat, command arguments, files, URLs, or logs.
- `INCOMPATIBLE_DATABASE`: request a compatible/current CLI or desktop version; never edit the schema or downgrade the database.
- Failed multi-step writes: report rollback/recovery details and preserved backup paths. Do not automatically retry destructive work or remove recovery content.

## Boundaries

Never access SQLite directly, execute SQL, or use `cp`, `rm`, `ln`, scripts, or manual filesystem edits to change the central library or Agent directories. These bypass ownership, locks, rollback, and recycle-bin protection. Do not implement CLI business logic yourself.

Never read, print, store, or request credentials, OAuth codes, Authorization headers, private keys, or credential-bearing URLs. The CLI may use existing secure credentials only during a user-requested supported network operation.

Do not expose device sync, sync conflict resolution, auto tasks, scheduled updates, proxy/storage/tool configuration, account configuration, or permanent recycle-bin cleanup through this Skill. Direct those requests to the desktop app. Never invent `sync`, credential commands, force overwrite switches, or a default all-Agent deployment.
