import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync, existsSync } from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'
const { describe, it } = process.env.VITEST ? await import('vitest') : await import('node:test')
import { desktopSidecarOptions, prepareCliSidecar, resolveSidecarTarget } from './prepare-cli-sidecar.mjs'

describe('CLI sidecar preparation', () => {
  it('keeps the desktop as Cargo default executable when the CLI is also present', () => {
    const result = spawnSync('cargo', ['metadata', '--no-deps', '--format-version', '1'], { cwd: new URL('../src-tauri', import.meta.url), encoding: 'utf8' })
    assert.equal(result.status, 0, result.stderr)
    const app = JSON.parse(result.stdout).packages.find(item => item.name === 'app')
    assert.equal(app.default_run, 'app')
    assert.deepEqual(app.targets.find(target => target.name === 'skillshub-cli')['required-features'], ['cli'])
    assert.deepEqual(app.features.cli, [])
  })
  it('maps only the five explicitly supported targets and rejects shell input', () => {
    for (const [target, triple] of [
      ['darwin-arm64', 'aarch64-apple-darwin'], ['darwin-x64', 'x86_64-apple-darwin'],
      ['win32-x64', 'x86_64-pc-windows-msvc'], ['linux-x64', 'x86_64-unknown-linux-gnu'],
      ['linux-arm64', 'aarch64-unknown-linux-gnu'],
    ]) {
      assert.equal(resolveSidecarTarget(target), triple)
      assert.equal(resolveSidecarTarget(triple), triple)
    }
    for (const target of [undefined, '', 'universal-apple-darwin', 'win32-arm64', 'linux-x64; echo bad', '$(echo bad)']) {
      assert.throws(() => resolveSidecarTarget(target), /Unsupported|explicit/)
    }
  })

  it('uses the desktop target and debug mode without double builds or guessed cross targets', () => {
    assert.deepEqual(desktopSidecarOptions([], 'darwin', 'arm64'), { target: 'aarch64-apple-darwin', debug: false })
    assert.deepEqual(desktopSidecarOptions(['--dev'], 'linux', 'x64'), { target: 'x86_64-unknown-linux-gnu', debug: true })
    assert.deepEqual(desktopSidecarOptions(['--dev', '--release'], 'linux', 'x64'), { target: 'x86_64-unknown-linux-gnu', debug: false })
    assert.deepEqual(desktopSidecarOptions(['-t', 'x86_64-apple-darwin', '-d'], 'darwin', 'arm64'), { target: 'x86_64-apple-darwin', debug: true })
    assert.deepEqual(desktopSidecarOptions(['--target=x86_64-pc-windows-msvc', '--debug'], 'darwin', 'arm64'), { target: 'x86_64-pc-windows-msvc', debug: true })
    assert.throws(() => desktopSidecarOptions(['--target'], 'darwin', 'arm64'))
    assert.throws(() => desktopSidecarOptions(['--target', 'linux-x64', '--target=linux-arm64'], 'linux', 'x64'))
    assert.throws(() => desktopSidecarOptions(['--profile', 'custom'], 'linux', 'x64'), /CLI_BRIDGE_UNSUPPORTED_PROFILE/)
  })

  it('builds exactly the requested binary and stages exact sidecar naming and trusted metadata', () => {
    const root = mkdtempSync(path.join(tmpdir(), 'sidecar with spaces-'))
    try {
      mkdirSync(path.join(root, 'src-tauri'))
      writeFileSync(path.join(root, 'package.json'), JSON.stringify({ version: '0.10.1' }))
      writeFileSync(path.join(root, 'src-tauri/tauri.conf.json'), JSON.stringify({ version: '0.10.1' }))
      const calls = []
      const result = prepareCliSidecar({ root, sourceCommit: 'a'.repeat(40), target: 'win32-x64', debug: true, run: (command, args, options) => {
        calls.push({ command, args, options })
        const output = path.join(root, 'src-tauri/target/x86_64-pc-windows-msvc/debug')
        mkdirSync(output, { recursive: true })
        writeFileSync(path.join(output, 'skillshub-cli.exe'), 'abc')
        return { status: 0, stdout: JSON.stringify({ reason: 'compiler-artifact', target: { name: 'skillshub-cli', kind: ['bin'] }, executable: path.join(output, 'skillshub-cli.exe'), profile: { debug_assertions: true } }) }
      } })
      assert.equal(calls.length, 1)
      assert.equal(calls[0].command, 'cargo')
      assert.deepEqual(calls[0].args, ['build', '--locked', '--features', 'cli', '--bin', 'skillshub-cli', '--target', 'x86_64-pc-windows-msvc', '--message-format=json-render-diagnostics'])
      assert.equal(calls[0].options.shell, false)
      assert.equal(calls[0].options.env.SKILLS_HUB_PREPARE_CLI_SIDECAR, '1')
      assert.equal(calls[0].options.env.CARGO_TARGET_DIR, path.join(root, 'src-tauri/target'))
      assert.equal(readFileSync(path.join(root, 'src-tauri/binaries/skillshub-cli-x86_64-pc-windows-msvc.exe'), 'utf8'), 'abc')
      assert.deepEqual(JSON.parse(readFileSync(result.metadataPath, 'utf8')), {
        version: '0.10.1', sourceCommit: 'a'.repeat(40), assetName: 'skillshub-cli-0.10.1-windows-x64.exe', size: 3, target: 'x86_64-pc-windows-msvc', profile: 'debug', sha256: createHash('sha256').update('abc').digest('hex'),
      })
      assert.equal(existsSync(path.join(root, 'src-tauri/binaries/skillshub-cli-x86_64-pc-windows-msvc.exe.version')), false)
    } finally { rmSync(root, { recursive: true, force: true }) }
  })

  it('stops on failed builds and never stages stale output or metadata', () => {
    const root = mkdtempSync(path.join(tmpdir(), 'sidecar-failure-'))
    try {
      mkdirSync(path.join(root, 'src-tauri/binaries'), { recursive: true })
      writeFileSync(path.join(root, 'package.json'), JSON.stringify({ version: '0.10.1' }))
      writeFileSync(path.join(root, 'src-tauri/tauri.conf.json'), JSON.stringify({ version: '0.10.1' }))
      const metadata = path.join(root, 'src-tauri/binaries/skillshub-cli-aarch64-apple-darwin.json')
      writeFileSync(metadata, '{"old":true}')
      assert.throws(() => prepareCliSidecar({ root, sourceCommit: 'a'.repeat(40), target: 'darwin-arm64', run: () => ({ status: 1 }) }), /build failed/)
      assert.equal(existsSync(metadata), false)
      assert.equal(existsSync(path.join(root, 'src-tauri/binaries/skillshub-cli-aarch64-apple-darwin')), false)
    } finally { rmSync(root, { recursive: true, force: true }) }
  })

  it('refuses staging when the actual compiler profile disagrees in either direction', () => {
    for (const debug of [true, false]) {
      const root = mkdtempSync(path.join(tmpdir(), 'sidecar-profile-'))
      try {
        mkdirSync(path.join(root, 'src-tauri'))
        writeFileSync(path.join(root, 'package.json'), JSON.stringify({ version: '0.10.1' }))
        writeFileSync(path.join(root, 'src-tauri/tauri.conf.json'), JSON.stringify({ version: '0.10.1' }))
        assert.throws(() => prepareCliSidecar({ root, sourceCommit: 'a'.repeat(40), target: 'darwin-arm64', debug, run: (_command, args) => {
          assert.equal(args.includes('--release'), !debug)
          const output = path.join(root, 'src-tauri/target/aarch64-apple-darwin', debug ? 'debug' : 'release', 'skillshub-cli')
          mkdirSync(path.dirname(output), { recursive: true })
          writeFileSync(output, 'abc')
          return { status: 0, stdout: JSON.stringify({ reason: 'compiler-artifact', target: { name: 'skillshub-cli', kind: ['bin'] }, executable: output, profile: { debug_assertions: !debug } }) }
        } }), /CLI_BRIDGE_PROFILE_MISMATCH/)
        assert.equal(existsSync(path.join(root, 'src-tauri/binaries/skillshub-cli-aarch64-apple-darwin')), false)
        assert.equal(existsSync(path.join(root, 'src-tauri/binaries/skillshub-cli-aarch64-apple-darwin.json')), false)
      } finally { rmSync(root, { recursive: true, force: true }) }
    }
  })
})
