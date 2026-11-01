# Windows subdirectory Skill updates in v1.0.0 ([#182](https://github.com/qufei1993/skills-hub/issues/182))

[简体中文](windows-subpath-updates.zh.md) · [PR #189](https://github.com/qufei1993/skills-hub/pull/189)

## Problem and user impact

Skills installed from repository subdirectories could fail to update on Windows with `path not found in repo`. v1.0.0 restores updates for these installations without requiring reinstallation or a database migration.

## Path and cache handling

Saved backslashes were passed to Git sparse-checkout as pattern escapes, so Git could report success without checking out the selected directory.

Repository discovery and installation now use forward slashes for Git source paths. Updates accept existing backslash paths and save their portable form after a successful update. Both initial sparse checkout and existing-cache updates normalize separators. A cache missing the selected directory is refreshed even when it has not expired.

## Regression coverage

Regression tests use temporary local repositories and databases to cover initial checkout, repeated updates, installation, legacy records, and recovery from an empty sparse checkout.
