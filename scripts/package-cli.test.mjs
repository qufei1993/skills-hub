import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import {
  chmodSync,
  copyFileSync,
  existsSync,
  lstatSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from 'node:fs'
import { createRequire } from 'node:module'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { gzipSync } from 'node:zlib'

const { describe, it } = process.env.VITEST ? await import('vitest') : await import('node:test')
const {
  packageCli,
  parsePackageCliArgs,
  readPackedBinarySha256,
  runTrustedNpm,
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
const TEST_NPM_EXECPATH = [
  process.env.npm_execpath,
  path.resolve(path.dirname(process.execPath), '../lib/node_modules/npm/bin/npm-cli.js'),
  path.resolve(path.dirname(process.execPath), 'node_modules/npm/bin/npm-cli.js'),
].find(candidate => {
  if (!candidate) return false
  try {
    const metadata = lstatSync(candidate)
    return metadata.isFile() && !metadata.isSymbolicLink()
  } catch {
    return false
  }
})
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

function writeTarString(header, offset, length, value) {
  const bytes = Buffer.from(value, 'utf8')
  assert.ok(bytes.length < length)
  bytes.copy(header, offset)
}

function writeTarOctal(header, offset, length, value) {
  const valueBytes = Buffer.from(value.toString(8).padStart(length - 1, '0') + '\0', 'ascii')
  assert.equal(valueBytes.length, length)
  valueBytes.copy(header, offset)
}

function createTarHeader({ name, contents = Buffer.alloc(0), declaredSize = contents.length, type = '0', linkName = '' }) {
  const header = Buffer.alloc(512)
  writeTarString(header, 0, 100, name)
  writeTarOctal(header, 100, 8, type === '5' ? 0o755 : 0o644)
  writeTarOctal(header, 108, 8, 0)
  writeTarOctal(header, 116, 8, 0)
  writeTarOctal(header, 124, 12, declaredSize)
  writeTarOctal(header, 136, 12, 0)
  header.fill(0x20, 148, 156)
  header.write(type, 156, 1, 'ascii')
  writeTarString(header, 157, 100, linkName)
  header.write('ustar\0', 257, 6, 'binary')
  header.write('00', 263, 2, 'ascii')
  const checksum = header.reduce((total, byte) => total + byte, 0)
  const checksumBytes = Buffer.from(`${checksum.toString(8).padStart(6, '0')}\0 `, 'ascii')
  checksumBytes.copy(header, 148)
  return header
}

function writeTarball(tarball, entries, { omitEnd = false } = {}) {
  const chunks = []
  for (const entry of entries) {
    const contents = Buffer.from(entry.contents ?? '')
    chunks.push(createTarHeader({ ...entry, contents }), contents)
    const remainder = contents.length % 512
    if (remainder) chunks.push(Buffer.alloc(512 - remainder))
  }
  if (!omitEnd) chunks.push(Buffer.alloc(1024))
  writeFileSync(tarball, gzipSync(Buffer.concat(chunks), { mtime: 0 }))
}

function writeInstalledPlatformPackage(packageDirectory, target, contents = 'native-binary') {
  const definition = TARGETS.find(item => item.target === target)
  assert.ok(definition)
  mkdirSync(packageDirectory, { recursive: true })
  const binary = path.join(packageDirectory, `skillshub-cli${definition.extension}`)
  writeFileSync(binary, contents)
  chmodSync(binary, 0o755)
  writeFileSync(path.join(packageDirectory, 'package.json'), `${JSON.stringify({
    name: `@skillshub-app/cli-${target}`,
    version: VERSION,
    exports: { './skillshub-cli': `./skillshub-cli${definition.extension}` },
  })}\n`)
  return binary
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
    const searchRoot = 'C:\\npm\\node_modules'
    const expectedBinary = 'C:\\npm\\node_modules\\@skillshub-app\\cli-win32-x64\\skillshub-cli.exe'
    const binary = resolveBinaryPath('win32', 'x64', {
      searchPaths: () => [searchRoot],
      resolvePath(specifier, options) {
        calls.push({ specifier, options })
        return expectedBinary
      },
      lstatPath(value) {
        return {
          isDirectory: () => value !== expectedBinary,
          isFile: () => value === expectedBinary,
          isSymbolicLink: () => false,
        }
      },
      realpathPath(value) {
        return value
      },
    })

    assert.equal(binary, expectedBinary)
    assert.equal(calls.length, 1)
    assert.equal(calls[0].specifier, '@skillshub-app/cli-win32-x64/skillshub-cli')
    assert.deepEqual(calls[0].options.paths.length, 1)
  })

  it('reports a missing optional dependency without searching cwd or PATH', () => {
    assert.throws(
      () => resolveBinaryPath('linux', 'arm64', {
        searchPaths: () => ['/missing/node_modules'],
        lstatPath() {
          const error = new Error('missing')
          error.code = 'ENOENT'
          throw error
        },
      }),
      /optional platform package.*@skillshub-app\/cli-linux-arm64.*reinstall skillshub-cli/i,
    )
  })

  it('rejects non-regular, symlinked, or redirected binary resolutions', () => {
    const expectedBinary = '/safe/node_modules/@skillshub-app/cli-linux-x64/skillshub-cli'
    const base = {
      searchPaths: () => ['/safe/node_modules'],
      resolvePath: () => expectedBinary,
      realpathPath: value => value,
    }
    const lstatWithBinary = binary => value => ({
      isDirectory: () => value !== expectedBinary,
      isFile: () => value === expectedBinary && binary === 'file',
      isSymbolicLink: () => value === expectedBinary && binary === 'symlink',
    })

    assert.throws(
      () => resolveBinaryPath('linux', 'x64', {
        ...base,
        lstatPath: lstatWithBinary('other'),
      }),
      /regular file/i,
    )
    assert.throws(
      () => resolveBinaryPath('linux', 'x64', {
        ...base,
        lstatPath: lstatWithBinary('symlink'),
      }),
      /symbolic link/i,
    )
    assert.throws(
      () => resolveBinaryPath('linux', 'x64', {
        ...base,
        resolvePath: () => '/tmp/attacker/skillshub-cli',
        lstatPath: lstatWithBinary('file'),
      }),
      /package boundary/i,
    )
  })

  it('rejects real node_modules package and binary symlink substitution before require.resolve canonicalizes them', { skip: process.platform === 'win32' }, () => {
    for (const attack of ['package', 'binary']) {
      const temporary = mkdtempSync(path.join(tmpdir(), `skillshub-cli-${attack}-link-`))
      try {
        const consumer = path.join(temporary, 'consumer')
        const searchRoot = path.join(consumer, 'node_modules')
        const candidatePackage = path.join(searchRoot, '@skillshub-app/cli-linux-x64')
        const externalPackage = path.join(temporary, 'external/node_modules/@skillshub-app/cli-linux-x64')
        const externalBinary = writeInstalledPlatformPackage(externalPackage, 'linux-x64', `${attack}-replacement`)
        mkdirSync(path.dirname(candidatePackage), { recursive: true })
        if (attack === 'package') {
          symlinkSync(externalPackage, candidatePackage, 'dir')
        } else {
          const candidateBinary = writeInstalledPlatformPackage(candidatePackage, 'linux-x64', 'expected')
          rmSync(candidateBinary)
          symlinkSync(externalBinary, candidateBinary)
        }
        const consumerRequire = createRequire(path.join(consumer, 'consumer.cjs'))
        assert.throws(() => resolveBinaryPath('linux', 'x64', {
          searchPaths: () => [searchRoot],
          resolvePath: specifier => consumerRequire.resolve(specifier),
        }), /symbolic link/i)
      } finally {
        rmSync(temporary, { recursive: true, force: true })
      }
    }
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

      assert.ok(TEST_NPM_EXECPATH, 'test environment must provide a trusted npm CLI entry')
      const installed = runTrustedNpm([
        'install', '--package-lock-only', '--ignore-scripts', '--offline', '--no-audit', '--no-fund',
      ], {
        npmExecPath: TEST_NPM_EXECPATH,
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
  it('reads the exact regular binary entry hash from a strict gzip ustar archive', () => {
    const temporary = mkdtempSync(path.join(tmpdir(), 'skillshub-cli-tar-hash-'))
    try {
      const binary = Buffer.from('release-binary-bytes')
      for (const [index, target] of ['package/skillshub-cli', 'package/skillshub-cli.exe'].entries()) {
        const tarball = path.join(temporary, `platform-${index}.tgz`)
        writeTarball(tarball, [
          { name: 'package/package.json', contents: '{}' },
          { name: target, contents: binary },
        ])
        assert.equal(
          readPackedBinarySha256(readFileSync(tarball), target),
          createHash('sha256').update(binary).digest('hex'),
        )
      }
    } finally {
      rmSync(temporary, { recursive: true, force: true })
    }
  })

  it('rejects ambiguous, linked, traversing, extended, or truncated tar entries', () => {
    const temporary = mkdtempSync(path.join(tmpdir(), 'skillshub-cli-tar-invalid-'))
    try {
      const target = 'package/skillshub-cli'
      const cases = [
        {
          name: 'duplicate',
          entries: [{ name: target, contents: 'one' }, { name: target, contents: 'two' }],
          message: /exactly one|duplicate/i,
        },
        {
          name: 'link',
          entries: [
            { name: 'package/linked-readme', type: '2', linkName: '/tmp/other' },
            { name: target, contents: 'good' },
          ],
          message: /link|regular/i,
        },
        {
          name: 'missing',
          entries: [{ name: 'package/not-the-cli', contents: 'wrong' }],
          message: /exactly one/i,
        },
        {
          name: 'traversal',
          entries: [{ name: 'package/../escape', contents: 'bad' }, { name: target, contents: 'good' }],
          message: /path|traversal/i,
        },
        {
          name: 'pax',
          entries: [{ name: 'pax', type: 'x', contents: '31 path=package/skillshub-cli\n' }, { name: target, contents: 'good' }],
          message: /extended|type|pax/i,
        },
        {
          name: 'longname',
          entries: [{ name: '././@LongLink', type: 'L', contents: `${target}\0` }, { name: target, contents: 'good' }],
          message: /extended|type|longname|path/i,
        },
        {
          name: 'truncated',
          entries: [{ name: target, contents: 'short', declaredSize: 2048 }],
          options: { omitEnd: true },
          message: /bounds|truncated|size/i,
        },
      ]
      for (const testCase of cases) {
        const tarball = path.join(temporary, `${testCase.name}.tgz`)
        writeTarball(tarball, testCase.entries, testCase.options)
        assert.throws(() => readPackedBinarySha256(readFileSync(tarball), target), testCase.message)
      }
    } finally {
      rmSync(temporary, { recursive: true, force: true })
    }
  })

  it('rejects every Windows extraction alias of the exact executable entry', () => {
    const temporary = mkdtempSync(path.join(tmpdir(), 'skillshub-cli-windows-alias-'))
    try {
      const target = 'package/skillshub-cli.exe'
      for (const [index, alias] of [
        'package/SKILLSHUB-CLI.EXE',
        'package/skillshub-cli.exe.',
        'package/skillshub-cli.exe ',
      ].entries()) {
        const tarball = path.join(temporary, `alias-${index}.tgz`)
        writeTarball(tarball, [
          { name: target, contents: 'release' },
          { name: alias, contents: 'substitute' },
        ])
        assert.throws(
          () => readPackedBinarySha256(readFileSync(tarball), target),
          /alias|ambiguous|duplicate/i,
        )
      }
    } finally {
      rmSync(temporary, { recursive: true, force: true })
    }
  })

  it('hashes the exact Unix binary while allowing distinct case and trailing-dot siblings', () => {
    const temporary = mkdtempSync(path.join(tmpdir(), 'skillshub-cli-unix-names-'))
    try {
      const tarball = path.join(temporary, 'unix.tgz')
      writeTarball(tarball, [
        { name: 'package/skillshub-cli', contents: 'release' },
        { name: 'package/SKILLSHUB-CLI', contents: 'other' },
        { name: 'package/skillshub-cli.', contents: 'other' },
        { name: 'package/skillshub-cli ', contents: 'other' },
      ])
      for (const platform of ['darwin', 'linux']) {
        assert.equal(readPackedBinarySha256(readFileSync(tarball), 'package/skillshub-cli', platform),
          createHash('sha256').update('release').digest('hex'))
      }
      assert.throws(() => readPackedBinarySha256(readFileSync(tarball), 'package/skillshub-cli', 'win32'), /alias/i)
      for (const name of ['../escape', '/absolute', 'package\\escape', 'package/skillshub-cli']) {
        writeTarball(tarball, [
          { name: 'package/skillshub-cli', contents: 'release' },
          { name, contents: 'other' },
        ])
        assert.throws(() => readPackedBinarySha256(readFileSync(tarball), 'package/skillshub-cli', 'linux'), /path|duplicate/i)
      }
    } finally {
      rmSync(temporary, { recursive: true, force: true })
    }
  })

  for (const failure of ['source', 'hash', 'npm-json', 'tar', 'packed-hash', 'write', 'fsync', 'rename']) {
    it(`leaves absent output ancestors absent after ${failure} failure and preserves existing content`, () => {
      const temporary = mkdtempSync(path.join(tmpdir(), 'skillshub-cli-output-rollback-'))
      try {
        for (const existing of [false, true, 'ancestor', 'concurrent']) {
          const caseDirectory = path.join(temporary, String(existing))
          mkdirSync(caseDirectory)
          const ancestor = path.join(caseDirectory, 'new-parent')
          const output = path.join(ancestor, 'new-child', 'output')
          const packDirectory = path.join(caseDirectory, 'private-pack')
          const filename = 'native.tgz'
          if (existing === 'ancestor') mkdirSync(ancestor)
          if (existing === true) {
            mkdirSync(output, { recursive: true })
            writeFileSync(path.join(output, filename), 'original')
          }
          const { source } = createFixtureBinary(caseDirectory, 'darwin-arm64', VERSION,
            failure === 'hash' ? { sha256: '0'.repeat(64) } : {})
          if (failure === 'source') rmSync(source)
          const npmExecPath = path.join(caseDirectory, 'npm-cli.js')
          writeFileSync(npmExecPath, '# fixture')
          const fail = () => {
            if (existing === 'concurrent') writeFileSync(path.join(output, 'concurrent.txt'), 'keep concurrent content')
            throw new Error(`injected ${failure} failure`)
          }
          assert.throws(() => packageCli({
            root: ROOT, version: VERSION, target: 'darwin-arm64', source, output, npmExecPath,
            createPackDirectory() {
              mkdirSync(packDirectory)
              return packDirectory
            },
            ...(failure === 'write' ? { writeFile(fd, contents) { writeFileSync(fd, contents.subarray(0, 8)); fail() } } : {}),
            ...(failure === 'fsync' ? { syncFile: fail } : {}),
            ...(failure === 'rename' ? { renameFile: fail } : {}),
            run(command, args) {
              if (command !== process.execPath) return {
                status: 0, stdout: JSON.stringify({ ok: true, command: 'version', data: { version: VERSION } }), stderr: '',
              }
              const destination = path.join(args[args.indexOf('--pack-destination') + 1], filename)
              writeTarball(destination, [{ name: 'package/skillshub-cli',
                contents: failure === 'packed-hash' ? 'substitute' : readFileSync(source) }])
              if (failure === 'tar') writeFileSync(destination, 'invalid gzip')
              return { status: 0, stdout: failure === 'npm-json' ? 'invalid JSON' : JSON.stringify([{ filename }]), stderr: '' }
            },
          }), /source.*exist|hash|valid JSON|gzip|injected/i)
          assert.equal(existsSync(packDirectory), false)
          if (existing === true) {
            assert.deepEqual(readdirSync(output), [filename])
            assert.equal(readFileSync(path.join(output, filename), 'utf8'), 'original')
          } else if (existing === 'ancestor') {
            assert.deepEqual(readdirSync(ancestor), [])
          } else if (existing === 'concurrent' && ['write', 'fsync', 'rename'].includes(failure)) {
            assert.deepEqual(readdirSync(output), ['concurrent.txt'])
            assert.equal(readFileSync(path.join(output, 'concurrent.txt'), 'utf8'), 'keep concurrent content')
          } else {
            assert.equal(existsSync(ancestor), false)
          }
        }
      } finally {
        rmSync(temporary, { recursive: true, force: true })
      }
    })
  }

  it('runs npm through the trusted CLI entry with Node on win32', () => {
    const temporary = mkdtempSync(path.join(tmpdir(), 'skillshub-cli-npm-entry-'))
    try {
      const npmExecPath = path.join(temporary, 'npm-cli.js')
      writeFileSync(npmExecPath, '# npm CLI fixture\n')
      const calls = []
      const result = runTrustedNpm(['pack', 'C:\\package'], {
        platform: 'win32',
        nodePath: 'C:\\Program Files\\nodejs\\node.exe',
        npmExecPath,
        run(command, args, options) {
          calls.push({ command, args, options })
          return { status: 0 }
        },
        encoding: 'utf8',
      })
      assert.deepEqual(result, { status: 0 })
      assert.equal(calls.length, 1)
      assert.equal(calls[0].command, 'C:\\Program Files\\nodejs\\node.exe')
      assert.deepEqual(calls[0].args, [npmExecPath, 'pack', 'C:\\package'])
      assert.equal(calls[0].options.shell, false)
      assert.equal(calls[0].options.encoding, 'utf8')
    } finally {
      rmSync(temporary, { recursive: true, force: true })
    }
  })

  it('fails closed when npm_execpath is missing or not a trusted regular file', () => {
    const temporary = mkdtempSync(path.join(tmpdir(), 'skillshub-cli-npm-untrusted-'))
    try {
      assert.throws(() => runTrustedNpm(['pack'], { npmExecPath: null }), /npm_execpath.*required/i)
      const target = path.join(temporary, 'npm-cli-real.js')
      const linked = path.join(temporary, 'npm-cli.js')
      writeFileSync(target, '# npm CLI fixture\n')
      symlinkSync(target, linked)
      assert.throws(() => runTrustedNpm(['pack'], { npmExecPath: linked }), /symbolic link/i)
      assert.throws(() => runTrustedNpm(['pack'], { npmExecPath: temporary }), /regular file/i)
    } finally {
      rmSync(temporary, { recursive: true, force: true })
    }
  })

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
      const packDirectory = path.join(temporary, 'private-pack')
      const { source, sha256 } = createFixtureBinary(temporary, 'darwin-arm64')
      const npmExecPath = path.join(temporary, 'npm-cli.js')
      writeFileSync(npmExecPath, '# npm CLI fixture\n')
      const calls = []
      const result = packageCli({
        root: ROOT,
        version: VERSION,
        target: 'darwin-arm64',
        source,
        output,
        npmExecPath,
        createPackDirectory() {
          mkdirSync(packDirectory, { mode: 0o700 })
          return packDirectory
        },
        run(command, args, options) {
          calls.push({ command, args, options })
          if (command !== process.execPath) {
            assert.equal(command, path.join(packDirectory, 'staging/skillshub-cli'))
            return {
              status: 0,
              signal: null,
              stdout: `${JSON.stringify({ ok: true, command: 'version', data: { version: VERSION } })}\n`,
              stderr: '',
            }
          }
          assert.equal(command, process.execPath)
          assert.equal(args[0], npmExecPath)
          assert.equal(args[1], 'pack')
          assert.equal(args.includes('--ignore-scripts'), true)
          assert.equal(args.includes('--json'), true)
          assert.equal(options.shell, false)
          assert.equal(options.env.npm_config_offline, 'true')
          const staging = args[2]
          const manifest = JSON.parse(readFileSync(path.join(staging, 'package.json'), 'utf8'))
          assert.equal(manifest.name, '@skillshub-app/cli-darwin-arm64')
          assert.equal(manifest.version, VERSION)
          assert.equal(readFileSync(path.join(staging, 'skillshub-cli'), 'utf8'), readFileSync(source, 'utf8'))
          assert.equal(lstatSync(path.join(staging, 'skillshub-cli')).isSymbolicLink(), false)
          assert.equal(lstatSync(path.join(staging, 'skillshub-cli')).mode & 0o777, 0o755)
          const packDestination = args[args.indexOf('--pack-destination') + 1]
          mkdirSync(packDestination, { recursive: true })
          const filename = `skillshub-app-cli-darwin-arm64-${VERSION}.tgz`
          writeTarball(path.join(packDestination, filename), [
            { name: 'package/package.json', contents: JSON.stringify(manifest) },
            { name: 'package/skillshub-cli', contents: readFileSync(source) },
          ])
          return { status: 0, signal: null, stdout: JSON.stringify([{ filename }]), stderr: '' }
        },
      })

      assert.equal(calls.length, 2)
      assert.notEqual(calls[0].command, source)
      assert.deepEqual(calls[0].args, ['version', '--json'])
      assert.deepEqual(calls[0].options.stdio, ['ignore', 'pipe', 'pipe'])
      assert.equal(calls[0].options.shell, false)
      assert.equal(result.binarySha256, sha256)
      assert.equal(result.target, 'aarch64-apple-darwin')
      assert.equal(result.profile, 'release')
      assert.equal(existsSync(result.tarball), true)
      assert.equal(existsSync(packDirectory), false)
    } finally {
      rmSync(temporary, { recursive: true, force: true })
    }
  })

  it('publishes the single verified tarball Buffer snapshot and hashes that same snapshot', () => {
    const temporary = mkdtempSync(path.join(tmpdir(), 'skillshub-cli-tar-snapshot-'))
    try {
      const output = path.join(temporary, 'output')
      const packDirectory = path.join(temporary, 'private-pack')
      const { source, sha256 } = createFixtureBinary(temporary, 'darwin-arm64')
      const npmExecPath = path.join(temporary, 'npm-cli.js')
      writeFileSync(npmExecPath, '# npm CLI fixture\n')
      const fixtureTarball = path.join(temporary, 'verified.tgz')
      writeTarball(fixtureTarball, [
        { name: 'package/package.json', contents: '{}' },
        { name: 'package/skillshub-cli', contents: readFileSync(source) },
      ])
      const verifiedSnapshot = readFileSync(fixtureTarball)
      const filename = `skillshub-app-cli-darwin-arm64-${VERSION}.tgz`
      let reads = 0
      const result = packageCli({
        root: ROOT,
        version: VERSION,
        target: 'darwin-arm64',
        source,
        output,
        npmExecPath,
        createPackDirectory() {
          mkdirSync(packDirectory, { mode: 0o700 })
          return packDirectory
        },
        readTarball(tarball) {
          reads += 1
          const snapshot = readFileSync(tarball)
          writeFileSync(tarball, 'replaced after snapshot')
          return snapshot
        },
        run(command, args) {
          if (command !== process.execPath) {
            return {
              status: 0,
              signal: null,
              stdout: `${JSON.stringify({ ok: true, command: 'version', data: { version: VERSION } })}\n`,
              stderr: '',
            }
          }
          const destination = args[args.indexOf('--pack-destination') + 1]
          mkdirSync(destination, { recursive: true })
          writeFileSync(path.join(destination, filename), verifiedSnapshot)
          return { status: 0, signal: null, stdout: JSON.stringify([{ filename }]), stderr: '' }
        },
      })

      assert.equal(reads, 1)
      assert.equal(result.binarySha256, sha256)
      assert.equal(result.packageSha256, createHash('sha256').update(verifiedSnapshot).digest('hex'))
      assert.deepEqual(readFileSync(result.tarball), verifiedSnapshot)
      assert.equal(existsSync(packDirectory), false)
    } finally {
      rmSync(temporary, { recursive: true, force: true })
    }
  })

  it('keeps output unchanged and removes the private pack directory for every npm result failure', () => {
    const temporary = mkdtempSync(path.join(tmpdir(), 'skillshub-cli-pack-failure-'))
    try {
      const failures = [
        {
          name: 'exit-error',
          result: filename => ({ status: 1, signal: null, stdout: JSON.stringify([{ filename }]), stderr: 'failed' }),
          message: /npm pack failed/i,
        },
        {
          name: 'invalid-json',
          result: () => ({ status: 0, signal: null, stdout: 'not-json', stderr: '' }),
          message: /valid JSON/i,
        },
        {
          name: 'unsafe-path',
          result: () => ({ status: 0, signal: null, stdout: JSON.stringify([{ filename: '../escape.tgz' }]), stderr: '' }),
          message: /unsafe filename/i,
        },
      ]
      for (const failure of failures) {
        const caseDirectory = path.join(temporary, failure.name)
        const output = path.join(caseDirectory, 'output')
        const packDirectory = path.join(caseDirectory, 'private-pack')
        const filename = `generated-${failure.name}.tgz`
        mkdirSync(output, { recursive: true })
        writeFileSync(path.join(output, 'keep.txt'), 'original')
        writeFileSync(path.join(output, filename), 'original tarball')
        const { source } = createFixtureBinary(caseDirectory, 'darwin-arm64')
        const npmExecPath = path.join(caseDirectory, 'npm-cli.js')
        writeFileSync(npmExecPath, '# npm CLI fixture\n')
        let packDirectoriesCreated = 0
        assert.throws(() => packageCli({
          root: ROOT,
          version: VERSION,
          target: 'darwin-arm64',
          source,
          output,
          npmExecPath,
          createPackDirectory() {
            packDirectoriesCreated += 1
            mkdirSync(packDirectory, { mode: 0o700 })
            return packDirectory
          },
          run(command, args) {
            if (command !== process.execPath) {
              return {
                status: 0,
                signal: null,
                stdout: `${JSON.stringify({ ok: true, command: 'version', data: { version: VERSION } })}\n`,
                stderr: '',
              }
            }
            const destination = args[args.indexOf('--pack-destination') + 1]
            mkdirSync(destination, { recursive: true })
            writeTarball(path.join(destination, filename), [
              { name: 'package/skillshub-cli', contents: readFileSync(source) },
            ])
            return failure.result(filename)
          },
        }), failure.message)
        assert.equal(packDirectoriesCreated, 1)
        assert.deepEqual(readdirSync(output).sort(), [filename, 'keep.txt'].sort())
        assert.equal(readFileSync(path.join(output, 'keep.txt'), 'utf8'), 'original')
        assert.equal(readFileSync(path.join(output, filename), 'utf8'), 'original tarball')
        assert.equal(existsSync(packDirectory), false)
      }
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

  it('rejects replacement bytes copied into staging before version execution or packing', () => {
    const temporary = mkdtempSync(path.join(tmpdir(), 'skillshub-cli-copy-race-'))
    try {
      const { source } = createFixtureBinary(temporary, 'darwin-arm64')
      let spawned = false
      assert.throws(() => packageCli({
        root: ROOT,
        version: VERSION,
        target: 'darwin-arm64',
        source,
        output: path.join(temporary, 'output'),
        copyFile(from, to) {
          if (from === source) {
            writeFileSync(to, '#!/bin/sh\necho replaced\n')
            chmodSync(to, 0o755)
          } else {
            copyFileSync(from, to)
          }
        },
        run() {
          spawned = true
          throw new Error('replacement bytes must not execute')
        },
      }), /staged.*hash|hash.*staged/i)
      assert.equal(spawned, false)
      assert.equal(existsSync(path.join(temporary, 'output/.skillshub-cli-staging-darwin-arm64')), false)
    } finally {
      rmSync(temporary, { recursive: true, force: true })
    }
  })

  it('rejects and removes a real tarball when staged bytes change after version verification', { skip: process.platform === 'win32' }, () => {
    const temporary = mkdtempSync(path.join(tmpdir(), 'skillshub-cli-pack-race-'))
    try {
      const output = path.join(temporary, 'output')
      const { source } = createFixtureBinary(temporary, 'darwin-arm64')
      assert.ok(TEST_NPM_EXECPATH, 'test environment must provide a trusted npm CLI entry')
      let versionVerified = false
      assert.throws(() => packageCli({
        root: ROOT,
        version: VERSION,
        target: 'darwin-arm64',
        source,
        output,
        npmExecPath: TEST_NPM_EXECPATH,
        run(command, args, options) {
          if (command === process.execPath && args[0] === TEST_NPM_EXECPATH) {
            assert.equal(versionVerified, true)
            const stagedBinary = path.join(args[2], 'skillshub-cli')
            writeFileSync(stagedBinary, `${readFileSync(stagedBinary, 'utf8')}\n// same version, different build bytes\n`)
            chmodSync(stagedBinary, 0o755)
          }
          const result = spawnSync(command, args, options)
          if (command !== process.execPath) {
            assert.equal(result.status, 0, result.stderr)
            versionVerified = true
          }
          return result
        },
      }), /tarball.*hash|hash.*tarball/i)
      assert.equal(versionVerified, true)
      assert.equal(existsSync(output), false)
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
      assert.ok(TEST_NPM_EXECPATH, 'test environment must provide a trusted npm CLI entry')
      const first = packageCli({ root: ROOT, version: VERSION, target: definition.target, source, output, npmExecPath: TEST_NPM_EXECPATH })
      const firstHash = createHash('sha256').update(readFileSync(first.tarball)).digest('hex')
      const second = packageCli({ root: ROOT, version: VERSION, target: definition.target, source, output, npmExecPath: TEST_NPM_EXECPATH })
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
      assert.ok(TEST_NPM_EXECPATH, 'test environment must provide a trusted npm CLI entry')
      const platformPackage = packageCli({
        root: ROOT,
        version: VERSION,
        target: definition.target,
        source,
        output: tarballs,
        npmExecPath: TEST_NPM_EXECPATH,
      })
      const packedMain = runTrustedNpm([
        'pack', path.join(ROOT, 'packages/skillshub-cli'),
        '--json', '--ignore-scripts', '--pack-destination', tarballs,
      ], {
        npmExecPath: TEST_NPM_EXECPATH,
        encoding: 'utf8',
        env: { ...process.env, npm_config_offline: 'true' },
      })
      assert.equal(packedMain.status, 0, packedMain.stderr)
      const [{ filename }] = JSON.parse(packedMain.stdout)
      const mainPackage = path.join(tarballs, filename)

      const installRoot = path.join(temporary, 'install')
      mkdirSync(installRoot)
      writeFileSync(path.join(installRoot, 'package.json'), '{"private":true}\n')
      const installed = runTrustedNpm([
        'install', '--ignore-scripts', '--offline', '--no-audit', '--no-fund', '--no-package-lock', '--no-save',
        mainPackage, platformPackage.tarball,
      ], {
        npmExecPath: TEST_NPM_EXECPATH,
        cwd: installRoot,
        encoding: 'utf8',
        env: {
          ...process.env,
          npm_config_offline: 'true',
          npm_config_registry: 'http://127.0.0.1:9',
        },
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
