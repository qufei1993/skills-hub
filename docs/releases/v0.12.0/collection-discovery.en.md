# Collection discovery and search

Add Skills opens the website’s published collection catalog. An empty query shows collections; typing shows matching collections alongside individual skills.sh results. Git and local import remain available in the page header and open the shared installation flow. The Top 300 JSON, generator, scheduled workflow, backend reader and obsolete labels are removed.

Collection data comes from https://skills-hub-collections.qzfweb.workers.dev through the application proxy. The desktop checks the small current.json publication pointer and reuses validated cached data when the version is unchanged. Details load on demand from the same snapshot as the index. Invalid responses or offline requests fall back to validated cached data. Korean UI uses English collection content because the website does not publish Korean content.

Search matches only collection titles, descriptions, categories, tags and authors, including English translations. Internal Skill content is excluded from collection search. The published index contains 345 collections in approximately 430 KB; complete Skill details and installation sources remain available on demand. No dynamic search API is required.

Discovery initially renders 12 collection cards. Scrolling near the list end appends 12 more; changing the query resets the batch and returning from details preserves the loaded batch. A manual load button is available only when IntersectionObserver is unavailable.

Collection details reuse category colors and icons, show topic labels and consolidate identical repository labels. Responsive headers keep installation accessible. Installation reuses local-state checks, Skill selection, tags, tool distribution and retries.

Verification covered desktop scrolling from 12 to 24 to 36 of 345 collections, query filtering, engineering collection details and its 31-Skill installation entry. Automated tests cover batch limits, stale observer callbacks, query resets, snapshot pinning, cache reuse and metadata-only search. Discovery verification did not repeat installations.

Native collection installation validates its manifest directly through IPC, supporting up to 2,000 Skills while retaining source, revision, duplicate and path validation. External deep-link limits remain unchanged. Regression coverage includes a 1,297-Skill collection and unsafe or excessive manifests.
