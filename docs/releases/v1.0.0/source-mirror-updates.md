# Source-mirrored Skill updates

Skill updates now treat the selected source as authoritative for the central library copy. Files removed or renamed in the source no longer cause an `UPDATE_HELD_BACK` error. Explicit updates also replace local edits and remove extra files in that central copy.

Single updates, batch updates, and same-source Git incremental installation apply source deletions. Unmodified deployed copies receive those deletions through the existing distribution mechanism. Existing tool-target conflict checks, concurrent-change detection, and replacement rollback remain in place.

An update source must still contain `SKILL.md`; an empty or wrong source directory is rejected before replacing the central copy. Update checks continue reporting removal counts for compatibility, with `held_back` set to false.

Regression coverage includes source deletion, deployed-copy refresh, Git incremental installation, central local edits, batch updates, and missing `SKILL.md`. Tests use temporary Skill directories.
