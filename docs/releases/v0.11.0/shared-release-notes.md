# Shared draft release notes

The final v0.11.0 asset audit found that the release body still contained the early CLI placeholder, although all 32 assets had uploaded successfully. The release action recovered the earlier canonical draft after creating and removing a duplicate, retaining that draft's old body.

The final CLI verification now explicitly writes the generated bilingual release notes to the inspected draft ID and reads them back. It refuses public releases, keeps `draft=true`, and checks the exact resulting body. Download tables remain generated from actual desktop assets; updater notes remain separate.

Regression tests fail before the fix and pass afterward, covering canonical draft discovery, replacing placeholder notes, refusing public release mutation and workflow wiring. This metadata-only correction does not change the already verified desktop/CLI binaries from commit `05bc94ab5edb249dcdd871fb4341d4828b737121`.
