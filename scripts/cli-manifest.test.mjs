import assert from 'node:assert/strict'
import { mkdtempSync, writeFileSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'
const { describe, it } = process.env.VITEST ? await import('vitest') : await import('node:test')
const api = await import('./cli-manifest.mjs').catch(() => ({}))
const commit = '24b25e48153fcdda96bbb24e45d020911bc850fb'
const expected = { version: '0.11.0', sourceCommit: commit, target: 'aarch64-apple-darwin', profile: 'release' }
describe('CLI release manifest', () => {
  it('records final bytes and a fixed versioned platform asset', () => {
    assert.equal(typeof api.createCliManifest, 'function')
    const root = mkdtempSync(path.join(tmpdir(), 'cli-manifest-'))
    try {
      const binaryPath = path.join(root, 'cli'); writeFileSync(binaryPath, 'abc')
      assert.deepEqual(api.createCliManifest({ binaryPath, ...expected }), {
        ...expected, assetName: 'skillshub-cli-0.11.0-darwin-arm64', size: 3,
        sha256: 'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad',
      })
    } finally { rmSync(root, { recursive: true, force: true }) }
  })
  it('rejects altered identity, unsafe asset names and invalid byte expectations', () => {
    assert.equal(typeof api.validateCliManifest, 'function')
    const valid = { ...expected, assetName: 'skillshub-cli-0.11.0-darwin-arm64', size: 3, sha256: 'a'.repeat(64) }
    assert.deepEqual(api.validateCliManifest(valid, expected), valid)
    for (const patch of [{version:'0.10.1'}, {target:'x86_64-apple-darwin'}, {profile:'debug'}, {sourceCommit:'b'.repeat(40)}, {size:-1}, {size:3.5}, {sha256:'z'.repeat(64)}, {assetName:'../cli'}, {size:0}]) {
      assert.throws(() => api.validateCliManifest({...valid,...patch}, expected))
    }
  })
})
