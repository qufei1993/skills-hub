import { mkdir, writeFile, mkdtemp } from 'node:fs/promises';
import { join } from 'node:path';
import { collectReleaseSnapshot } from './collect-release-data.mjs';
import { buildPagesDirectory } from './lib/release-pages.mjs';

async function main() {
  const baseUrl = process.env.RELEASES_BASE_URL || 'https://releases.aiskillshub.link/';
  const parsed = new URL(baseUrl);
  if (parsed.protocol !== 'https:' || parsed.username || parsed.password) throw Error('Invalid public source');
  // GitHub authorization is used only by the collector, never for public data reads.
  const githubFetch = (url, init) => fetch(url, { ...init, headers: { ...init?.headers, ...(process.env.GH_TOKEN ? { Authorization: `Bearer ${process.env.GH_TOKEN}` } : {}) } });
  const snapshot = await collectReleaseSnapshot({ fetchImpl: githubFetch, sourceRef: 'latest' });
  const publication = await buildPagesDirectory(snapshot, { baseUrl, bootstrap: process.env.RELEASES_BOOTSTRAP === 'true' });
  const output = await mkdtemp(join(process.cwd(), '.release-pages-'));
  for (const [path, body] of publication.files) {
    const destination = join(output, path);
    await mkdir(join(destination, '..'), { recursive: true });
    await writeFile(destination, body);
  }
  if (process.env.GITHUB_OUTPUT) await writeFile(process.env.GITHUB_OUTPUT, `directory=${output}\n`, { flag: 'a' });
  console.log(`Prepared release data: ${publication.pointer.latest} (${publication.files.size} files)`);
}
main().catch(() => { console.error('Release data preparation failed; no deployment was performed. Check configuration and retry.'); process.exitCode = 1; });
