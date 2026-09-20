import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import {
  chmodSync,
  existsSync,
  lstatSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from 'node:fs'
import { createRequire } from 'node:module'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const { describe, it } = process.env.VITEST ? await import('vitest') : await import('node:test')
const {
  packageCli,
  parsePackageCliArgs,
} = await import('./package-cli.mjs')

const require = createRequire(import.meta.url)
const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const VERSION = JSON.parse(readFileSync(path.join(ROOT, 'package.json'), 'utf8')).version
const ROOT_LICENSE = readFileSync(path.join(ROOT, 'LICENSE'), 'utf8')
const TARGETS = [
  { target: 'darwin-arm64', triple: 'aarch64-apple-darwin', os: 'darwin', cpu: 'arm64', extension: '' },
  { target: 'darwin-x64', triple: 'x86_64-apple-darwin', os: 'darwin', cpu: 'x64', extension: '' },
  { target: 'win32-x64', triple: 'x86_64-pc-windows-msvc', os: 'win32', cpu: 'x64', extension: '.exe' },
  { target: 'linux-x64', triple: 'x86_64-unknown-linux-gnu', os: 'linux', cpu: 'x64', extension: '' },
  { target: 'linux-arm64', triple: 'aarch64-unknown-linux-gnu', os: 'linux', cpu: 'arm64', extension: '' },
]
const {
  launch,
  resolveBinaryPackage,
  resolveBinaryPath,
} = require('../packages/skillshub-cli/lib/platform.cjs')

function json(relativePath) {
  return JSON.parse(readFileSync(path.join(ROOT, relativePath), 'utf8'))
}

function createFixtureBinary(directory, target, version = VERSION, metadata = {}) {
  const definition = TARGETS.find(item => item.target === target)
  assert.ok(definition)
  const source = path.join(directory, `skillshub-cli-${definition.triple}${definition.extension}`)
  const contents = `#!/usr/bin/env node\nif (process.argv.slice(2).join(' ') !== 'version --json') process.exit(2)\nprocess.stdout.write(${JSON.stringify(`${JSON.stringify({ ok: true, command: 'version', data: { version } })}\n`)})\n`
  writeFileSync(source, contents)
  chmodSync(source, 0o755)
  const sha256 = createHash('sha256').update(contents).digest('hex')
  const metadataPath = path.join(directory, `skillshub-cli-${definition.triple}.json`)
  writeFileSync(metadataPath, `${JSON.stringify({
    version,
    target: definition.triple,
    profile: 'release',
    sha256,
    ...metadata,
  })}\n`)
  return { source, metadataPath, sha256 }
}

function createVersionFixture(directory, rootVersion = VERSION) {
  mkdirSync(path.join(directory, 'src-tauri'), { recursive: true })
  writeFileSync(path.join(directory, 'package.json'), `${JSON.stringify({
    name: 'skills-hub',
    private: true,
    version: rootVersion,
  }, null, 2)}\n`)
  writeFileSync(path.join(directory, 'src-tauri/tauri.conf.json'), `${JSON.stringify({ version: VERSION }, null, 2)}\n`)
  writeFileSync(path.join(directory, 'src-tauri/Cargo.toml'), `[package]\nname = "app"\nversion = "${VERSION}"\n\n[dependencies]\n`)
  for (const directoryName of ['skillshub-cli', ...TARGETS.map(({ target }) => `cli-${target}`)]) {
    const destination = path.join(directory, 'packages', directoryName)
    mkdirSync(destination, { recursive: true })
    writeFileSync(
      path.join(destination, 'package.json'),
      readFileSync(path.join(ROOT, 'packages', directoryName, 'package.json')),
    )
  }
}

describe('skillshub-cli npm launcher', () => {
  it('maps exactly the five supported platform and architecture pairs', () => {
    for (const [platform, arch, expected] of [
      ['darwin', 'arm64', '@skillshub-app/cli-darwin-arm64/skillshub-cli'],
      ['darwin', 'x64', '@skillshub-app/cli-darwin-x64/skillshub-cli'],
      ['win32', 'x64', '@skillshub-app/cli-win32-x64/skillshub-cli'],
      ['linux', 'x64', '@skillshub-app/cli-linux-x64/skillshub-cli'],
      ['linux', 'arm64', '@skillshub-app/cli-linux-arm64/skillshub-cli'],
    ]) {
      assert.equal(resolveBinaryPackage(platform, arch), expected)
    }

    for (const [platform, arch] of [
      ['freebsd', 'x64'],
      ['win32', 'arm64'],
      ['linux', 'ia32'],
      ['darwin/../../bad', 'arm64'],
    ]) {
      assert.throws(() => resolveBinaryPackage(platform, arch), /unsupported platform/i)
    }
  })

  it('resolves only the fixed package export, including the Windows executable', () => {
    const calls = []
    const binary = resolveBinaryPath('win32', 'x64', {
      resolvePath(specifier, options) {
        calls.push({ specifier, options })
        return 'C:\\npm\\node_modules\\@skillshub-app\\cli-win32-x64\\skillshub-cli.exe'
      },
      lstatPath() {
        return { isFile: () => true, isSymbolicLink: () => false }
      },
      realpathPath(value) {
        return value
      },
    })

    assert.equal(binary, 'C:\\npm\\node_modules\\@skillshub-app\\cli-win32-x64\\skillshub-cli.exe')
    assert.equal(calls.length, 1)
    assert.equal(calls[0].specifier, '@skillshub-app/cli-win32-x64/skillshub-cli')
    assert.deepEqual(calls[0].options.paths.length, 1)
  })

  it('reports a missing optional dependency without searching cwd or PATH', () => {
    const error = new Error('Cannot find module')
    error.code = 'MODULE_NOT_FOUND'
    assert.throws(
      () => resolveBinaryPath('linux', 'arm64', {
        resolvePath() {
          throw error
        },
      }),
      /optional platform package.*@skillshub-app\/cli-linux-arm64.*reinstall skillshub-cli/i,
    )
  })

  it('rejects non-regular, symlinked, or redirected binary resolutions', () => {
    const base = {
      resolvePath: () => '/safe/node_modules/@skillshub-app/cli-linux-x64/skillshub-cli',
    }

    assert.throws(
      () => resolveBinaryPath('linux', 'x64', {
        ...base,
        lstatPath: () => ({ isFile: () => false, isSymbolicLink: () => false }),
      }),
      /regular file/i,
    )
    assert.throws(
      () => resolveBinaryPath('linux', 'x64', {
        ...base,
        lstatPath: () => ({ isFile: () => true, isSymbolicLink: () => true }),
      }),
      /symbolic link/i,
    )
    assert.throws(
      () => resolveBinaryPath('linux', 'x64', {
        ...base,
        lstatPath: () => ({ isFile: () => true, isSymbolicLink: () => false }),
        realpathPath: () => '/redirected/skillshub-cli',
      }),
      /redirected/i,
    )
    assert.throws(
      () => resolveBinaryPath('linux', 'x64', {
        resolvePath: () => '/tmp/attacker/skillshub-cli',
        lstatPath: () => ({ isFile: () => true, isSymbolicLink: () => false }),
        realpathPath: value => value,
      }),
      /package boundary/i,
    )
  })

  it('passes arguments unchanged with inherited streams and shell disabled', () => {
    const calls = []
    const status = launch({
      platform: 'darwin',
      arch: 'arm64',
      args: ['skills', 'list', '--json', 'literal;$(touch nope)'],
      resolveBinary: () => '/verified/skillshub-cli',
      spawn(binary, args, options) {
        calls.push({ binary, args, options })
        return { status: 7, signal: null }
      },
    })

    assert.equal(status, 7)
    assert.deepEqual(calls, [{
      binary: '/verified/skillshub-cli',
      args: ['skills', 'list', '--json', 'literal;$(touch nope)'],
      options: { shell: false, stdio: 'inherit' },
    }])
  })

  it('forwards a child signal to the launcher process', () => {
    const kills = []
    const status = launch({
      platform: 'darwin',
      arch: 'arm64',
      args: [],
      resolveBinary: () => '/verified/skillshub-cli',
      spawn: () => ({ status: null, signal: 'SIGTERM' }),
      kill(pid, signal) {
        kills.push({ pid, signal })
      },
    })

    assert.equal(status, 1)
    assert.deepEqual(kills, [{ pid: process.pid, signal: 'SIGTERM' }])
  })

  it('writes a clear error and exits nonzero when resolution or spawning fails', () => {
    for (const options of [
      { resolveBinary: () => { throw new Error('platform package missing') } },
      { resolveBinary: () => '/verified/skillshub-cli', spawn: () => ({ error: new Error('cannot execute') }) },
    ]) {
      let stderr = ''
      const status = launch({
        platform: 'linux',
        arch: 'x64',
        args: [],
        ...options,
        writeError(value) {
          stderr += value
        },
      })
      assert.notEqual(status, 0)
      assert.match(stderr, /skillshub-cli:/i)
    }
  })
})

describe('skillshub-cli package manifests', () => {
  it('keeps the root private and exposes workspaces without making the desktop depend on the CLI package', () => {
    const root = json('package.json')
    assert.equal(root.private, true)
    assert.deepEqual(root.workspaces, ['packages/skillshub-cli'])
    assert.equal(root.dependencies?.['skillshub-cli'], undefined)
    assert.equal(root.devDependencies?.['skillshub-cli'], undefined)
  })

  it('declares the public launcher and exact optional platform dependencies', () => {
    const manifest = json('packages/skillshub-cli/package.json')
    assert.equal(manifest.name, 'skillshub-cli')
    assert.equal(manifest.version, VERSION)
    assert.equal(manifest.license, 'MIT')
    assert.deepEqual(manifest.bin, { 'skillshub-cli': 'bin/skillshub-cli.cjs' })
    assert.deepEqual(manifest.engines, { node: '>=20' })
    assert.deepEqual(manifest.files, ['bin', 'lib', 'README.md', 'LICENSE'])
    assert.deepEqual(manifest.optionalDependencies, Object.fromEntries(
      TARGETS.map(({ target }) => [`@skillshub-app/cli-${target}`, VERSION]),
    ))
    assert.equal(manifest.scripts, undefined)
    assert.equal(readFileSync(path.join(ROOT, 'packages/skillshub-cli/LICENSE'), 'utf8'), ROOT_LICENSE)
  })

  it('pins platform, architecture, fixed binary export, and minimal files for every native package', () => {
    for (const { target, os, cpu, extension } of TARGETS) {
      const directory = `packages/cli-${target}`
      const manifest = json(`${directory}/package.json`)
      assert.equal(manifest.name, `@skillshub-app/cli-${target}`)
      assert.equal(manifest.version, VERSION)
      assert.equal(manifest.license, 'MIT')
      assert.deepEqual(manifest.os, [os])
      assert.deepEqual(manifest.cpu, [cpu])
      assert.deepEqual(manifest.exports, { './skillshub-cli': `./skillshub-cli${extension}` })
      assert.deepEqual(manifest.files, [`skillshub-cli${extension}`, 'README.md', 'LICENSE'])
      assert.equal(manifest.scripts, undefined)
      assert.equal(readFileSync(path.join(ROOT, directory, 'LICENSE'), 'utf8'), ROOT_LICENSE)
      assert.ok(readFileSync(path.join(ROOT, directory, 'README.md'), 'utf8').length > 0)
    }
  })

  it('does not force foreign native packages into desktop dependency installation', () => {
    const temporary = mkdtempSync(path.join(tmpdir(), 'skillshub-cli-workspaces-'))
    try {
      const root = json('package.json')
      writeFileSync(path.join(temporary, 'package.json'), `${JSON.stringify({
        name: root.name,
        private: root.private,
        version: root.version,
        workspaces: root.workspaces,
      }, null, 2)}\n`)
      for (const directoryName of ['skillshub-cli', ...TARGETS.map(({ target }) => `cli-${target}`)]) {
        const destination = path.join(temporary, 'packages', directoryName)
        mkdirSync(destination, { recursive: true })
        writeFileSync(
          path.join(destination, 'package.json'),
          readFileSync(path.join(ROOT, 'packages', directoryName, 'package.json')),
        )
      }

      const installed = spawnSync('npm', [
        'install', '--package-lock-only', '--ignore-scripts', '--offline', '--no-audit', '--no-fund',
      ], {
        cwd: temporary,
        encoding: 'utf8',
        env: { ...process.env, npm_config_offline: 'true' },
        shell: false,
      })
      assert.equal(installed.status, 0, installed.stderr)
    } finally {
      rmSync(temporary, { recursive: true, force: true })
    }
  })
})

describe('deterministic CLI package staging', () => {
  it('requires each explicit argument exactly once and only accepts supported targets', () => {
    assert.deepEqual(parsePackageCliArgs([
      '--version', VERSION,
      '--target', 'linux-arm64',
      '--source', '/tmp/skillshub-cli-aarch64-unknown-linux-gnu',
      '--output', '/tmp/packages',
    ]), {
      version: VERSION,
      target: 'linux-arm64',
      source: '/tmp/skillshub-cli-aarch64-unknown-linux-gnu',
      output: '/tmp/packages',
    })

    for (const args of [
      [],
      ['--version', VERSION, '--target', 'linux-x64', '--source', '/tmp/binary'],
      ['--version', VERSION, '--version', VERSION, '--target', 'linux-x64', '--source', '/tmp/binary', '--output', '/tmp/out'],
      ['--version', VERSION, '--target', 'freebsd-x64', '--source', '/tmp/binary', '--output', '/tmp/out'],
      ['--version', VERSION, '--target', 'linux-x64;touch bad', '--source', '/tmp/binary', '--output', '/tmp/out'],
      ['--version', VERSION, '--target', 'linux-x64', '--source', '/tmp/binary', '--output', '/tmp/out', '--publish'],
    ]) {
      assert.throws(() => parsePackageCliArgs(args), /usage|duplicate|unsupported|unknown/i)
    }
  })

  it('validates release metadata, version JSON, and exact staging before npm pack', () => {
    const temporary = mkdtempSync(path.join(tmpdir(), 'skillshub-cli-stage-'))
    try {
      const output = path.join(temporary, 'output')
      const { source, sha256 } = createFixtureBinary(temporary, 'darwin-arm64')
      const calls = []
      const result = packageCli({
        root: ROOT,
        version: VERSION,
        target: 'darwin-arm64',
        source,
        output,
        run(command, args, options) {
          calls.push({ command, args, options })
          if (command === source) {
            return {
              status: 0,
              signal: null,
              stdout: `${JSON.stringify({ ok: true, command: 'version', data: { version: VERSION } })}\n`,
              stderr: '',
            }
          }
          assert.equal(command, 'npm')
          assert.equal(args[0], 'pack')
          assert.equal(args.includes('--ignore-scripts'), true)
          assert.equal(args.includes('--json'), true)
          assert.equal(options.shell, false)
          assert.equal(options.env.npm_config_offline, 'true')
          const staging = args[1]
          const manifest = JSON.parse(readFileSync(path.join(staging, 'package.json'), 'utf8'))
          assert.equal(manifest.name, '@skillshub-app/cli-darwin-arm64')
          assert.equal(manifest.version, VERSION)
          assert.equal(readFileSync(path.join(staging, 'skillshub-cli'), 'utf8'), readFileSync(source, 'utf8'))
          assert.equal(lstatSync(path.join(staging, 'skillshub-cli')).isSymbolicLink(), false)
          assert.equal(lstatSync(path.join(staging, 'skillshub-cli')).mode & 0o777, 0o755)
          mkdirSync(output, { recursive: true })
          const filename = `skillshub-app-cli-darwin-arm64-${VERSION}.tgz`
          writeFileSync(path.join(output, filename), 'tarball')
          return { status: 0, signal: null, stdout: JSON.stringify([{ filename }]), stderr: '' }
        },
      })

      assert.equal(calls.length, 2)
      assert.deepEqual(calls[0].args, ['version', '--json'])
      assert.deepEqual(calls[0].options.stdio, ['ignore', 'pipe', 'pipe'])
      assert.equal(calls[0].options.shell, false)
      assert.equal(result.binarySha256, sha256)
      assert.equal(result.target, 'aarch64-apple-darwin')
      assert.equal(result.profile, 'release')
      assert.equal(existsSync(result.tarball), true)
      assert.equal(existsSync(path.join(output, '.skillshub-cli-staging-darwin-arm64')), false)
    } finally {
      rmSync(temporary, { recursive: true, force: true })
    }
  })

  it('rejects symlink sources, profile/target/version drift, and hash mismatch before packing', () => {
    const temporary = mkdtempSync(path.join(tmpdir(), 'skillshub-cli-invalid-'))
    try {
      const output = path.join(temporary, 'output')
      const fixtures = [
        { metadata: { profile: 'debug' }, message: /release/i },
        { metadata: { target: 'x86_64-apple-darwin' }, message: /target/i },
        { metadata: { version: '9.9.9' }, message: /version/i },
        { metadata: { sha256: '0'.repeat(64) }, message: /hash/i },
      ]
      for (const [index, fixture] of fixtures.entries()) {
        const directory = path.join(temporary, `fixture-${index}`)
        mkdirSync(directory)
        const { source } = createFixtureBinary(directory, 'darwin-arm64', VERSION, fixture.metadata)
        assert.throws(() => packageCli({
          root: ROOT,
          version: VERSION,
          target: 'darwin-arm64',
          source,
          output,
          run() {
            throw new Error('must not spawn')
          },
        }), fixture.message)
      }

      const validDirectory = path.join(temporary, 'symlink-source')
      mkdirSync(validDirectory)
      const { source } = createFixtureBinary(validDirectory, 'darwin-arm64')
      const link = path.join(validDirectory, 'linked-binary')
      symlinkSync(source, link)
      assert.throws(() => packageCli({
        root: ROOT,
        version: VERSION,
        target: 'darwin-arm64',
        source: link,
        output,
      }), /symlink|symbolic/i)
    } finally {
      rmSync(temporary, { recursive: true, force: true })
    }
  })

  it('rejects a binary whose JSON stream does not report the exact requested version', () => {
    const temporary = mkdtempSync(path.join(tmpdir(), 'skillshub-cli-version-'))
    try {
      const { source } = createFixtureBinary(temporary, 'darwin-arm64')
      let calls = 0
      assert.throws(() => packageCli({
        root: ROOT,
        version: VERSION,
        target: 'darwin-arm64',
        source,
        output: path.join(temporary, 'output'),
        run() {
          calls += 1
          return {
            status: 0,
            signal: null,
            stdout: `${JSON.stringify({ ok: true, command: 'version', data: { version: '9.9.9' } })}\n`,
            stderr: '',
          }
        },
      }), /version/i)
      assert.equal(calls, 1)
    } finally {
      rmSync(temporary, { recursive: true, force: true })
    }
  })

  it('produces byte-identical tarballs from the same verified input', { skip: process.platform === 'win32' }, () => {
    const temporary = mkdtempSync(path.join(tmpdir(), 'skillshub-cli-deterministic-'))
    try {
      const definition = TARGETS.find(item => item.os === process.platform && item.cpu === process.arch)
      assert.ok(definition, `host ${process.platform}-${process.arch} must be supported`)
      const source = process.env.SKILLSHUB_CLI_SMOKE_SOURCE
        ? path.resolve(process.env.SKILLSHUB_CLI_SMOKE_SOURCE)
        : createFixtureBinary(temporary, definition.target).source
      const output = path.join(temporary, 'output')
      const first = packageCli({ root: ROOT, version: VERSION, target: definition.target, source, output })
      const firstHash = createHash('sha256').update(readFileSync(first.tarball)).digest('hex')
      const second = packageCli({ root: ROOT, version: VERSION, target: definition.target, source, output })
      const secondHash = createHash('sha256').update(readFileSync(second.tarball)).digest('hex')
      assert.equal(secondHash, firstHash)
    } finally {
      rmSync(temporary, { recursive: true, force: true })
    }
  })

  it('installs explicit local main and host tarballs offline and executes version JSON', { skip: process.platform === 'win32' }, () => {
    const temporary = mkdtempSync(path.join(tmpdir(), 'skillshub-cli-smoke-'))
    try {
      const definition = TARGETS.find(item => item.os === process.platform && item.cpu === process.arch)
      assert.ok(definition, `host ${process.platform}-${process.arch} must be supported`)
      const source = process.env.SKILLSHUB_CLI_SMOKE_SOURCE
        ? path.resolve(process.env.SKILLSHUB_CLI_SMOKE_SOURCE)
        : createFixtureBinary(temporary, definition.target).source
      const tarballs = path.join(temporary, 'tarballs')
      const platformPackage = packageCli({ root: ROOT, version: VERSION, target: definition.target, source, output: tarballs })
      const packedMain = spawnSync('npm', [
        'pack', path.join(ROOT, 'packages/skillshub-cli'),
        '--json', '--ignore-scripts', '--pack-destination', tarballs,
      ], {
        encoding: 'utf8',
        env: { ...process.env, npm_config_offline: 'true' },
        shell: false,
      })
      assert.equal(packedMain.status, 0, packedMain.stderr)
      const [{ filename }] = JSON.parse(packedMain.stdout)
      const mainPackage = path.join(tarballs, filename)

      const installRoot = path.join(temporary, 'install')
      mkdirSync(installRoot)
      writeFileSync(path.join(installRoot, 'package.json'), '{"private":true}\n')
      const installed = spawnSync('npm', [
        'install', '--ignore-scripts', '--offline', '--no-audit', '--no-fund', '--no-package-lock', '--no-save',
        mainPackage, platformPackage.tarball,
      ], {
        cwd: installRoot,
        encoding: 'utf8',
        env: {
          ...process.env,
          npm_config_offline: 'true',
          npm_config_registry: 'http://127.0.0.1:9',
        },
        shell: false,
      })
      assert.equal(installed.status, 0, installed.stderr)

      const invoked = spawnSync(path.join(installRoot, 'node_modules/.bin/skillshub-cli'), ['version', '--json'], {
        cwd: installRoot,
        encoding: 'utf8',
        shell: false,
      })
      assert.equal(invoked.status, 0, invoked.stderr)
      assert.equal(invoked.stderr, '')
      assert.deepEqual(JSON.parse(invoked.stdout), {
        ok: true,
        command: 'version',
        data: { version: VERSION },
      })
    } finally {
      rmSync(temporary, { recursive: true, force: true })
    }
  })
})

describe('CLI package version synchronization', () => {
  it('reports platform manifest and exact optional dependency drift', () => {
    const temporary = mkdtempSync(path.join(tmpdir(), 'skillshub-cli-version-check-'))
    try {
      createVersionFixture(temporary)
      const platformPath = path.join(temporary, 'packages/cli-linux-x64/package.json')
      const platform = JSON.parse(readFileSync(platformPath, 'utf8'))
      platform.version = '9.9.9'
      writeFileSync(platformPath, `${JSON.stringify(platform, null, 2)}\n`)
      const mainPath = path.join(temporary, 'packages/skillshub-cli/package.json')
      const main = JSON.parse(readFileSync(mainPath, 'utf8'))
      main.optionalDependencies['@skillshub-app/cli-darwin-arm64'] = '9.9.9'
      writeFileSync(mainPath, `${JSON.stringify(main, null, 2)}\n`)

      const checked = spawnSync(process.execPath, [path.join(ROOT, 'scripts/version.mjs'), 'check'], {
        cwd: temporary,
        encoding: 'utf8',
        shell: false,
      })
      assert.equal(checked.status, 1)
      assert.match(checked.stderr, /packages\/cli-linux-x64\/package\.json version=9\.9\.9/)
      assert.match(checked.stderr, /@skillshub-app\/cli-darwin-arm64=9\.9\.9/)
    } finally {
      rmSync(temporary, { recursive: true, force: true })
    }
  })

  it('syncs every CLI manifest and exact optional dependency from the root version', () => {
    const temporary = mkdtempSync(path.join(tmpdir(), 'skillshub-cli-version-sync-'))
    try {
      createVersionFixture(temporary, '1.2.3')
      const synced = spawnSync(process.execPath, [path.join(ROOT, 'scripts/version.mjs'), 'sync'], {
        cwd: temporary,
        encoding: 'utf8',
        shell: false,
      })
      assert.equal(synced.status, 0, synced.stderr)
      assert.equal(jsonFrom(temporary, 'src-tauri/tauri.conf.json').version, '1.2.3')
      assert.match(readFileSync(path.join(temporary, 'src-tauri/Cargo.toml'), 'utf8'), /version = "1\.2\.3"/)
      const main = jsonFrom(temporary, 'packages/skillshub-cli/package.json')
      assert.equal(main.version, '1.2.3')
      assert.deepEqual(main.optionalDependencies, Object.fromEntries(
        TARGETS.map(({ target }) => [`@skillshub-app/cli-${target}`, '1.2.3']),
      ))
      for (const { target } of TARGETS) {
        assert.equal(jsonFrom(temporary, `packages/cli-${target}/package.json`).version, '1.2.3')
      }
    } finally {
      rmSync(temporary, { recursive: true, force: true })
    }
  })
})

function jsonFrom(root, relativePath) {
  return JSON.parse(readFileSync(path.join(root, relativePath), 'utf8'))
}
