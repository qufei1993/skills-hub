import { buildPublication } from './release-publication.mjs';

export const PAGES_HEADERS = `/*
  Access-Control-Allow-Origin: *
  X-Content-Type-Options: nosniff

/current.json
  Cache-Control: no-store

/archive.json
  Cache-Control: no-store

/snapshots/*
  Cache-Control: public, max-age=31536000, immutable
`;
const HASH = /^[a-f0-9]{64}$/;

async function read(base, path, fetchImpl, optional = false) {
  const response = await fetchImpl(new URL(path, base), { cache: 'no-store', signal: AbortSignal.timeout(30000) });
  if (optional && response.status === 404) return null;
  if (!response.ok) throw Error('Cannot read existing release data');
  return response.text();
}

export async function buildPagesDirectory(snapshot, { baseUrl, fetchImpl = fetch, bootstrap = false }) {
  const base = new URL(baseUrl);
  if (base.protocol !== 'https:' || base.username || base.password || base.search || base.hash || base.pathname !== '/') throw Error('Invalid public release source');
  const manifestText = await read(base, 'archive.json', fetchImpl, bootstrap);
  if (manifestText === null && await read(base, 'current.json', fetchImpl, true) !== null) throw Error('Existing release pointer cannot be replaced by bootstrap');
  let hashes = [];
  if (manifestText !== null) {
    const archive = JSON.parse(manifestText);
    if (archive.schemaVersion !== 1 || !Array.isArray(archive.snapshots) || archive.snapshots.length > 1000 || archive.snapshots.some(hash => !HASH.test(hash)) || new Set(archive.snapshots).size !== archive.snapshots.length) throw Error('Invalid release archive');
    hashes = archive.snapshots;
    const pointer = JSON.parse(await read(base, 'current.json', fetchImpl));
    if (pointer.schemaVersion !== 1 || !hashes.some(hash => pointer.snapshotPath === `snapshots/${hash}/`)) throw Error('Published pointer missing from archive');
  }
  const files = new Map();
  for (const hash of hashes) {
    const prefix = `snapshots/${hash}/`;
    const index = JSON.parse(await read(base, prefix + 'index.json', fetchImpl));
    const notes = { zh: {}, en: {} };
    if (!Array.isArray(index.releases)) throw Error('Invalid archived catalog');
    const tasks = [];
    for (const release of index.releases) {
      if (!/^v\d+\.\d+\.\d+$/.test(release.version)) throw Error('Invalid archived version');
      for (const lang of ['zh', 'en']) {
        if (release.notes?.[lang]) tasks.push({ lang, version: release.version });
      }
    }
    // Bound concurrent reads so release publication remains small and predictable.
    for (let offset = 0; offset < tasks.length; offset += 8) {
      await Promise.all(tasks.slice(offset, offset + 8).map(async ({ lang, version }) => {
        notes[lang][version] = await read(base, prefix + `changelog/${lang}/${version}.md`, fetchImpl);
      }));
    }
    const restored = buildPublication({ index, notes });
    const complete = JSON.parse(await read(base, prefix + 'complete.json', fetchImpl));
    if (restored.pointer.snapshotPath !== prefix || complete.latest !== index.latest) throw Error('Corrupt archived snapshot');
    for (const file of restored.files) files.set(prefix + file.key, file.body);
  }
  const publication = buildPublication(snapshot);
  const hash = publication.pointer.snapshotPath.split('/')[1];
  for (const file of publication.files) files.set(publication.pointer.snapshotPath + file.key, file.body);
  files.set('archive.json', JSON.stringify({ schemaVersion: 1, snapshots: [...new Set([...hashes, hash])] }));
  files.set('current.json', JSON.stringify(publication.pointer));
  files.set('_headers', PAGES_HEADERS);
  files.set('404.html', '<!doctype html><title>Not found</title>Not found');
  if (files.size > 19000 || [...files.values()].some(body => Buffer.byteLength(body) > 25 * 1024 * 1024)) throw Error('Release data exceeds Pages capacity');
  return { files, pointer: publication.pointer };
}
