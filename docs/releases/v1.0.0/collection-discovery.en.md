# Collection discovery and search

[简体中文](collection-discovery.zh.md) · [PR #187](https://github.com/qufei1993/skills-hub/pull/187)

## Browsing and search

Add Skills opens the website's published collection catalog. An empty query shows collections; typing shows matching collections alongside individual skills.sh results. Git and local import remain in the page header. The previous Top 300 feed, generator, scheduled workflow, and legacy four-collection endpoint are retired.

Search matches collection titles, descriptions, categories, tags, and authors, including English translations. It does not search internal Skill content. The list renders 12 cards initially and appends 12 more near the bottom. A new query resets the batch; returning from details preserves loaded batches. A manual load button is available when IntersectionObserver is unsupported.

Details use category colors, icons, and topic labels. Identical repository labels are consolidated, while mixed-source items retain their individual sources.

## Snapshot and cache behavior

Data is read from `skills-hub-collections.qzfweb.workers.dev` through the application proxy. The desktop checks the small `current.json` pointer and reuses validated caches when the version has not changed. Details load on demand from the index's immutable snapshot. Offline requests and invalid responses fall back to validated cached data. Korean UI uses English collection content.

## Large collection installation and regression coverage

Native installation validates manifests directly through IPC, supporting up to 2,000 Skills while preserving source, pinned revision, duplicate, and path checks. Website links share these manifest limits and retain the 30,000-byte encoded link limit; see [website collection installation](website-collection-install.md).

PR validation covered scrolling from 12 to 24 to 36 cards, search, details, snapshot pinning, cache reuse, stale observer callbacks, and query resets. Native manifest regressions include a 1,297-Skill collection, unsafe paths, and excessive manifests.
