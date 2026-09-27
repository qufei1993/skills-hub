import assert from 'node:assert/strict'
import { execFileSync, spawnSync } from 'node:child_process'
import { cpSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { it } from 'vitest'

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')

it('version set synchronizes both lockfiles and check rejects stale lockfile versions', () => {
  const fixture = mkdtempSync(path.join(tmpdir(), 'skillshub-version-'))
  try {
    for (const file of ['package.json', 'package-lock.json', 'src-tauri/Cargo.toml', 'src-tauri/Cargo.lock', 'src-tauri/tauri.conf.json', 'scripts/version.mjs']) {
      mkdirSync(path.dirname(path.join(fixture, file)), { recursive: true })
      cpSync(path.join(root, file), path.join(fixture, file))
    }
    execFileSync(process.execPath, ['scripts/version.mjs', 'set', '99.0.0'], { cwd: fixture })
    const lock = JSON.parse(readFileSync(path.join(fixture, 'package-lock.json')))
    assert.equal(lock.version, '99.0.0')
    assert.equal(lock.packages[''].version, '99.0.0')
    assert.match(readFileSync(path.join(fixture, 'src-tauri/Cargo.lock'), 'utf8'), /name = "app"\nversion = "99\.0\.0"/)
    execFileSync(process.execPath, ['scripts/version.mjs', 'check'], { cwd: fixture })
    for (const file of ['package-lock.json', 'src-tauri/Cargo.lock']) {
      const filename = path.join(fixture, file)
      const original = readFileSync(filename, 'utf8')
      writeFileSync(filename, original.replace('99.0.0', '98.0.0'))
      const result = spawnSync(process.execPath, ['scripts/version.mjs', 'check'], { cwd: fixture, encoding: 'utf8' })
      assert.equal(result.status, 1)
      assert.ok(result.stderr.includes(file))
      writeFileSync(filename, original)
    }
  } finally {
    rmSync(fixture, { recursive: true, force: true })
  }
})
