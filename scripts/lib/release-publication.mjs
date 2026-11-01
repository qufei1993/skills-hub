import { createHash } from 'node:crypto';

export function buildPublication({ index, notes }) {
  if (index.schemaVersion !== 1 || index.repository !== 'qufei1993/skills-hub') throw Error('Invalid release catalog');
  const latest = index.releases.find(r => r.version === index.latest);
  if (!latest?.installers.length) throw Error('Latest release has no installers');
  const files = [{ key: 'index.json', body: JSON.stringify(index), contentType: 'application/json; charset=utf-8' }];
  const versions = new Set();
  for (const release of index.releases) {
    if (!/^v\d+\.\d+\.\d+$/.test(release.version) || versions.has(release.version)) throw Error('Invalid release version');
    versions.add(release.version);
    for (const lang of ['zh', 'en']) {
      if (!release.notes[lang]) continue;
      const key = `changelog/${lang}/${release.version}.md`;
      if (release.notes[lang].path !== `/${key}` || !notes[lang]?.[release.version]?.trim()) throw Error('Missing release notes');
      files.push({ key, body: notes[lang][release.version], contentType: 'text/markdown; charset=utf-8' });
    }
  }
  const hash = createHash('sha256').update(JSON.stringify(files)).digest('hex');
  files.push({ key: "complete.json", body: JSON.stringify({ latest: index.latest }), contentType: "application/json; charset=utf-8" });
  return { pointer: { schemaVersion: 1, snapshotPath: `snapshots/${hash}/`, latest: index.latest }, files };
}
