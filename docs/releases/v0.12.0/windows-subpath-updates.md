# Windows subdirectory Skill updates (#182)

Skills installed from repository subdirectories could fail to update with `path not found in repo` on Windows. Saved backslashes were passed to Git sparse-checkout as pattern escapes, so Git reported success without checking out the selected directory.

Repository discovery and installation now use forward slashes for Git source paths. Updates accept existing backslash paths and save their portable form after a successful update. Sparse checkout normalizes separators for both new and existing caches; a fresh cache missing the selected directory is refreshed instead of reused.

No database migration or reinstallation is required. Regression tests use temporary local repositories and databases to cover initial checkout, repeated updates, installation, legacy records, and recovery from an empty sparse checkout.
