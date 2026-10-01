import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { copyFileSync, mkdirSync, mkdtempSync, readFileSync, rmSync, statSync, symlinkSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { createRequire } from 'node:module'
import * as manifests from './cli-manifest.mjs'

const { it } = process.env.VITEST ? await import('vitest') : await import('node:test')
const { load } = createRequire(import.meta.url)('js-yaml')
const release = load(readFileSync(new URL('../.github/workflows/release.yml', import.meta.url), 'utf8'))
const ci = load(readFileSync(new URL('../.github/workflows/ci.yml', import.meta.url), 'utf8'))
const commit = 'a'.repeat(40)
const expected = { version: '0.11.0', sourceCommit: commit, target: 'aarch64-apple-darwin', profile: 'release' }
const valid = { ...expected, assetName: 'skillshub-cli-0.11.0-darwin-arm64', size: 3, sha256: 'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad' }

it('reused CLI requires unchanged bytes and current release identity', () => {
  assert.equal(typeof manifests.verifyCliBinary, 'function')
  const root = mkdtempSync(path.join(tmpdir(), 'reuse-cli-'))
  try {
    const binary = path.join(root, 'cli')
    writeFileSync(binary, 'abc')
    assert.deepEqual(manifests.verifyCliBinary(valid, binary, expected), valid)
    for (const patch of [{ sourceCommit: 'b'.repeat(40) }, { target: 'x86_64-apple-darwin' }, { profile: 'debug' }, { version: '0.10.1' }]) {
      assert.throws(() => manifests.verifyCliBinary({ ...valid, ...patch }, binary, expected), /CLI_MANIFEST_MISMATCH/)
    }
    for (const bytes of ['xyz', 'ab', 'abcd']) {
      writeFileSync(binary, bytes)
      assert.throws(() => manifests.verifyCliBinary(valid, binary, expected), /CLI_INTEGRITY_FAILED/)
    }
    writeFileSync(binary, 'abc')
    if (process.platform !== 'win32') {
      const link = path.join(root, 'link')
      symlinkSync(binary, link)
      assert.throws(() => manifests.verifyCliBinary(valid, link, expected), /CLI_INTEGRITY_FAILED/)
    }
  } finally { rmSync(root, { recursive: true, force: true }) }
})

it('release handoff verifies all five platform artifacts before restoring executable permission', () => {
  const step = release.jobs['cli-build'].steps.find(step => step.name === 'Verify reused CLI build')
  assert.ok(step, 'CLI signing must verify the reused build')
  const script = step.run.match(/node --input-type=module <<'NODE'\n([\s\S]*?)\nNODE/)[1]
  for (const { target } of release.jobs['cli-build'].strategy.matrix.include) {
    for (const tampered of [false, true]) {
      const root = mkdtempSync(path.join(tmpdir(), 'handoff-cli-'))
      try {
        mkdirSync(path.join(root, 'scripts'))
        copyFileSync(new URL('./cli-manifest.mjs', import.meta.url), path.join(root, 'scripts/cli-manifest.mjs'))
        mkdirSync(path.join(root, 'src-tauri/binaries'), { recursive: true })
        writeFileSync(path.join(root, 'package.json'), '{"version":"0.11.0"}')
        const binary = path.join(root, `src-tauri/binaries/skillshub-cli-${target}${target.includes('windows') ? '.exe' : ''}`)
        writeFileSync(binary, 'abc', { mode: 0o600 })
        const manifest = manifests.createCliManifest({ ...expected, target, binaryPath: binary })
        writeFileSync(path.join(root, `src-tauri/binaries/skillshub-cli-${target}.json`), JSON.stringify(manifest))
        if (tampered) writeFileSync(binary, 'xyz')
        const result = spawnSync(process.execPath, ['--input-type=module'], {
          cwd: root, encoding: 'utf8', input: script.replaceAll('${{ matrix.target }}', target), env: { ...process.env, GITHUB_SHA: commit },
        })
        assert.equal(result.status, tampered ? 1 : 0, result.stderr)
        if (process.platform !== 'win32') {
          assert.equal(statSync(binary).mode & 0o777, !tampered && !target.includes('windows') ? 0o755 : 0o600)
        }
      } finally { rmSync(root, { recursive: true, force: true }) }
    }
  }
})

it('native jobs share target directories and caches while signing consumes verified artifacts', () => {
  for (const job of [release.jobs.verify, ci.jobs['cli-native']]) {
    assert.equal(job.env.CARGO_BUILD_TARGET, '${{ matrix.target }}')
    assert.equal(job.steps.find(step => step.uses === 'Swatinem/rust-cache@v2').with['shared-key'], 'native-${{ matrix.target }}')
  }
  for (const job of [release.jobs.verify, release.jobs['desktop-build'], ci.jobs['cli-native']]) {
    assert.equal(job.steps.find(step => step.uses === 'Swatinem/rust-cache@v2').with.key, 'release')
  }
  assert.equal(ci.jobs.rust.steps.find(step => step.uses === 'Swatinem/rust-cache@v2').with.key, 'debug')
  assert.equal(ci.jobs.rust.env.CARGO_BUILD_TARGET, 'x86_64-unknown-linux-gnu')
  assert.equal(ci.jobs.rust.steps.find(step => step.uses === 'Swatinem/rust-cache@v2').with['shared-key'], 'native-x86_64-unknown-linux-gnu')
  assert.equal(release.jobs['desktop-build'].env.CARGO_BUILD_TARGET, '${{ matrix.target }}')
  assert.equal(release.jobs['desktop-build'].steps.find(step => step.uses === 'Swatinem/rust-cache@v2').with['shared-key'], 'native-${{ matrix.target }}')
  const uploaded = release.jobs.verify.steps.find(step => step.uses === 'actions/upload-artifact@v4')
  const signing = release.jobs['cli-build'].steps
  const downloaded = signing.find(step => step.uses === 'actions/download-artifact@v4')
  assert.equal(downloaded.with.name, uploaded.with.name)
  assert.ok(signing.findIndex(step => step.name === 'Verify reused CLI build') < signing.findIndex(step => step.name === 'Sign and verify macOS CLI'))
  assert.ok(!signing.some(step => /cli:prepare|cargo build|npm run build|npm ci/.test(step.run ?? '')))
  const desktop = release.jobs['desktop-build'].steps.find(step => step.uses === 'actions/download-artifact@v4')
  const manifest = signing.find(step => step.name === 'Upload desktop CLI manifest')
  assert.equal(desktop.with.name, manifest.with.name)
})


it('Linux signing jobs install runtime libraries before executing the reused CLI', () => {
  const steps = release.jobs['cli-build'].steps
  const runtime = steps.findIndex(step => step.name === 'Install Linux CLI runtime dependencies')
  assert.ok(runtime >= 0)
  assert.equal(steps[runtime].if, "runner.os == 'Linux'")
  assert.ok(runtime < steps.findIndex(step => step.name === 'Native bundled CLI smoke'))
  assert.match(steps[runtime].run, /libwebkit2gtk-4\.1-0/)
  assert.match(steps[runtime].run, /libdbus-1-3/)
  assert.match(steps[runtime].run, /zlib1g/)
})
