# CLI adoption with excluded entries

[简体中文](cli-adopt-excluded-entries.zh.md) · Fixes [#186](https://github.com/qufei1993/skills-hub/issues/186) · [PR #191](https://github.com/qufei1993/skills-hub/pull/191)

## Problem and fix

`skills adopt --yes` could falsely return `PLAN_STALE` when a noncanonical source path contained excluded entries such as empty directories, invalid Skills, or unsafe links. Preview stored exclusion paths relative to the supplied root while confirmation rescanned its canonical form, producing unequal snapshots without a filesystem change.

Direct and container scans now use the canonical root for preview and confirmation. Excluded links retain their own locations, including dangling links. Real content changes and path escapes still reject unsafe adoption.

## Regression coverage

Service and CLI tests cover relative paths, parent-directory segments and aliases, and escaping or dangling links. The three regressions failed before the fix and passed afterward. Unix CLI bridge test syscall declarations were also corrected for the newer Rust compiler without changing synchronization behavior.
