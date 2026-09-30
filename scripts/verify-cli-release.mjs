import { createHash } from 'node:crypto'
import { readdirSync, readFileSync } from 'node:fs'
import path from 'node:path'
import { pathToFileURL } from 'node:url'
import { cliPlatforms, validateCliManifest } from './cli-manifest.mjs'

export function readReleaseManifests(directory, { version, sourceCommit } = {}) {
  const manifests = readdirSync(directory).filter(name => /^cli-manifest-.*\.json$/.test(name)).map(name => JSON.parse(readFileSync(path.join(directory, name), 'utf8')))
  validateReleaseManifests(manifests)
  if (manifests.some(m => (version && m.version !== version) || (sourceCommit && m.sourceCommit !== sourceCommit))) throw new Error('CLI_RELEASE_IDENTITY_MISMATCH')
  return manifests
}
function validateReleaseManifests(manifests) {
  if (manifests.length !== 5 || new Set(manifests.map(m => m.target)).size !== 5 || new Set(manifests.map(m => m.sourceCommit)).size !== 1 || new Set(manifests.map(m => m.version)).size !== 1) throw new Error('CLI_RELEASE_INCOMPLETE')
  for (const manifest of manifests) validateCliManifest(manifest, { ...manifest, profile: 'release' })
  if (manifests.some(m => !Object.hasOwn(cliPlatforms, m.target))) throw new Error('CLI_RELEASE_INCOMPLETE')
}
async function downloadBytes(url, maximum) {
  const signal = AbortSignal.timeout(120000)
  for (let redirects = 0; redirects <= 5; redirects += 1) {
    const parsed = new URL(url)
    if (parsed.protocol !== 'https:' || parsed.username || parsed.password) throw new Error('CLI_RELEASE_REDIRECT_REJECTED')
    const response = await fetch(url, { redirect: 'manual', signal })
    if ([301,302,303,307,308].includes(response.status)) {
      await response.body?.cancel()
      const location = response.headers.get('location')
      if (!location) throw new Error('CLI_RELEASE_DOWNLOAD_FAILED')
      url = new URL(location,url).href
      continue
    }
    if (!response.ok || !response.body) { await response.body?.cancel(); throw new Error('CLI_RELEASE_UNAVAILABLE') }
    const chunks=[]; let size=0
    for await (const bytes of response.body) {
      size += bytes.length
      if (size > maximum) throw new Error('CLI_RELEASE_SIZE_MISMATCH')
      chunks.push(bytes)
    }
    return Buffer.concat(chunks)
  }
  throw new Error('CLI_RELEASE_REDIRECT_REJECTED')
}
export async function verifyCliRelease({ manifests, download = downloadBytes }) {
  validateReleaseManifests(manifests)
  for (const manifest of manifests) {
    const base = `https://github.com/qufei1993/skills-hub-cli/releases/download/v${manifest.version}/`
    const published = JSON.parse(Buffer.from(await download(`${base}cli-manifest-${manifest.target}.json`,4096)).toString('utf8'))
    validateCliManifest(published, manifest)
    if (published.sha256 !== manifest.sha256 || published.size !== manifest.size || published.assetName !== manifest.assetName) throw new Error('CLI_RELEASE_IMMUTABLE_CONFLICT')
    const bytes = await download(base + manifest.assetName, manifest.size)
    if (bytes.length !== manifest.size || createHash('sha256').update(bytes).digest('hex') !== manifest.sha256) throw new Error('CLI_RELEASE_INTEGRITY_FAILED')
  }
}
if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  try {
    await verifyCliRelease({manifests:readReleaseManifests(process.argv[2], {version:process.env.GITHUB_REF_NAME?.replace(/^v/,''),sourceCommit:process.env.GITHUB_SHA})})
    console.log('All five CLI resources are publicly available and verified.')
  } catch { console.error('CLI_RELEASE_VERIFICATION_FAILED'); process.exitCode=1 }
}
