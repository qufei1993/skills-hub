import { createHash } from 'node:crypto'
import { readFileSync, writeFileSync, renameSync, rmSync, lstatSync } from 'node:fs'
import { spawnSync } from 'node:child_process'
import path from 'node:path'
import { pathToFileURL } from 'node:url'

export const cliPlatforms = {
  'aarch64-apple-darwin': 'darwin-arm64',
  'x86_64-apple-darwin': 'darwin-x64',
  'x86_64-pc-windows-msvc': 'windows-x64',
  'aarch64-unknown-linux-gnu': 'linux-arm64',
  'x86_64-unknown-linux-gnu': 'linux-x64',
}
export function sourceCommitFor(root) {
  const result = spawnSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8', shell: false })
  if (result.status !== 0) throw new Error('CLI_MANIFEST_SOURCE_UNAVAILABLE')
  return result.stdout.trim()
}
export function validateCliManifest(manifest, expected) {
  if (!manifest || !/^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/.test(manifest.version) ||
      !/^[a-f0-9]{40}$/.test(manifest.sourceCommit) || !Object.hasOwn(cliPlatforms, manifest.target) ||
      !['debug', 'release'].includes(manifest.profile) || !Number.isSafeInteger(manifest.size) || manifest.size <= 0 ||
      !/^[a-f0-9]{64}$/.test(manifest.sha256)) throw new Error('CLI_MANIFEST_INVALID')
  const assetName = `skillshub-cli-${manifest.version}-${cliPlatforms[manifest.target]}${manifest.target.includes('windows') ? '.exe' : ''}`
  if (manifest.assetName !== assetName || ['version', 'sourceCommit', 'target', 'profile'].some(key => manifest[key] !== expected[key])) throw new Error('CLI_MANIFEST_MISMATCH')
  return manifest
}
export function createCliManifest({ binaryPath, version, sourceCommit, target, profile }) {
  const binary = readFileSync(binaryPath)
  const manifest = { version, sourceCommit, target, profile,
    assetName: `skillshub-cli-${version}-${cliPlatforms[target]}${target.includes('windows') ? '.exe' : ''}`,
    size: binary.length, sha256: createHash('sha256').update(binary).digest('hex') }
  return validateCliManifest(manifest, { version, sourceCommit, target, profile })
}
export function verifyCliBinary(manifest, binaryPath, expected) {
  validateCliManifest(manifest, expected)
  if (!lstatSync(binaryPath).isFile()) throw new Error('CLI_INTEGRITY_FAILED')
  const actual = createCliManifest({ binaryPath, ...expected })
  if (actual.size !== manifest.size || actual.sha256 !== manifest.sha256) throw new Error('CLI_INTEGRITY_FAILED')
  return manifest
}
export function writeCliManifest(filename, manifest) {
  const temporary = `${filename}.${process.pid}.tmp`
  try { writeFileSync(temporary, `${JSON.stringify(manifest)}\n`, { mode: 0o600 }); renameSync(temporary, filename) }
  finally { rmSync(temporary, { force: true }) }
}
if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  try {
    const [binaryPath, filename, version, sourceCommit, target, profile] = process.argv.slice(2)
    if (!profile) throw new Error('Usage: cli-manifest.mjs <binary> <manifest> <version> <commit> <target> <profile>')
    writeCliManifest(filename, createCliManifest({ binaryPath, version, sourceCommit, target, profile }))
  } catch (error) { console.error(error.message); process.exitCode = 1 }
}
