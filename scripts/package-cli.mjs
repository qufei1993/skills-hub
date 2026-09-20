#!/usr/bin/env node
import { spawnSync } from 'node:child_process'
import { createHash, randomBytes } from 'node:crypto'
import {
  chmodSync,
  copyFileSync,
  existsSync,
  lstatSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  renameSync,
  rmSync,
  utimesSync,
  writeFileSync,
} from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'
import process from 'node:process'
import { fileURLToPath, pathToFileURL } from 'node:url'
import { gunzipSync } from 'node:zlib'
import { resolveSidecarTarget } from './prepare-cli-sidecar.mjs'

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const NORMALIZED_TIME = new Date('1985-10-26T08:15:00.000Z')
const TARGETS = Object.freeze({
  'darwin-arm64': { directory: 'cli-darwin-arm64', extension: '' },
  'darwin-x64': { directory: 'cli-darwin-x64', extension: '' },
  'win32-x64': { directory: 'cli-win32-x64', extension: '.exe' },
  'linux-x64': { directory: 'cli-linux-x64', extension: '' },
  'linux-arm64': { directory: 'cli-linux-arm64', extension: '' },
})

function readJson(filePath, description) {
  try {
    return JSON.parse(readFileSync(filePath, 'utf8'))
  } catch (error) {
    throw new Error(`Invalid ${description}: ${filePath}`, { cause: error })
  }
}

function sha256(filePath) {
  return createHash('sha256').update(readFileSync(filePath)).digest('hex')
}

function tarString(header, offset, length, description) {
  const field = header.subarray(offset, offset + length)
  const terminator = field.indexOf(0)
  const value = terminator === -1 ? field : field.subarray(0, terminator)
  const padding = terminator === -1 ? Buffer.alloc(0) : field.subarray(terminator)
  if (padding.some(byte => byte !== 0)) throw new Error(`Invalid tar ${description} padding`)
  if (value.some(byte => byte < 0x20 || byte > 0x7e)) throw new Error(`Invalid tar ${description}`)
  return value.toString('ascii')
}

function tarOctal(header, offset, length, description) {
  const field = header.subarray(offset, offset + length)
  if (field[0] & 0x80) throw new Error(`Unsupported binary tar ${description}`)
  const text = field.toString('ascii')
  const match = /^ *([0-7]+)?[\0 ]*$/.exec(text)
  if (!match) throw new Error(`Invalid tar ${description}`)
  const value = match[1] ? Number.parseInt(match[1], 8) : 0
  if (!Number.isSafeInteger(value)) throw new Error(`Tar ${description} exceeds safe bounds`)
  return value
}

function isZeroTarBlock(archive, offset) {
  if (offset + 512 > archive.length) return false
  return archive.subarray(offset, offset + 512).every(byte => byte === 0)
}

function windowsTarPathKey(entryName) {
  if (!entryName
    || entryName.startsWith('/')
    || entryName.includes('\\')
    || /^[A-Za-z]:/.test(entryName)) {
    throw new Error(`Unsafe tar entry path: ${entryName}`)
  }
  const canonical = []
  for (const rawComponent of entryName.normalize('NFC').split('/')) {
    const component = rawComponent.replace(/[. ]+$/u, '')
    const folded = component.replace(/[A-Z]/g, character => character.toLowerCase())
    const deviceBase = folded.split('.')[0]
    if (!component
      || component === '.'
      || component === '..'
      || component.includes(':')
      || /^(?:con|prn|aux|nul|com[1-9]|lpt[1-9])$/.test(deviceBase)) {
      throw new Error(`Unsafe Windows tar entry path: ${entryName}`)
    }
    canonical.push(folded)
  }
  return canonical.join('/')
}

export function readPackedBinarySha256(tarballSnapshot, expectedEntry) {
  if (!Buffer.isBuffer(tarballSnapshot)) throw new Error('Packed tarball snapshot must be a Buffer')
  const expectedWindowsKey = windowsTarPathKey(expectedEntry)
  let archive
  try {
    archive = gunzipSync(tarballSnapshot)
  } catch (error) {
    throw new Error('Invalid gzip tarball snapshot', { cause: error })
  }
  if (archive.length === 0 || archive.length % 512 !== 0) {
    throw new Error('Tarball has invalid block bounds')
  }

  let binarySha256
  let binaryAliases = 0
  let offset = 0
  let reachedEnd = false
  while (offset < archive.length) {
    if (isZeroTarBlock(archive, offset)) {
      if (!isZeroTarBlock(archive, offset + 512)) throw new Error('Tarball has a truncated end marker')
      if (archive.subarray(offset + 1024).some(byte => byte !== 0)) {
        throw new Error('Tarball has non-zero data after its end marker')
      }
      reachedEnd = true
      break
    }
    if (offset + 512 > archive.length) throw new Error('Tar header exceeds archive bounds')
    const header = archive.subarray(offset, offset + 512)
    const magic = header.subarray(257, 263)
    const version = header.subarray(263, 265)
    if (!(magic.equals(Buffer.from('ustar\0', 'binary')) || magic.equals(Buffer.from('ustar ', 'ascii')))
      || !version.equals(Buffer.from('00', 'ascii'))) {
      throw new Error('Tarball entry is not strict ustar')
    }

    const storedChecksum = tarOctal(header, 148, 8, 'checksum')
    let actualChecksum = 0
    for (let index = 0; index < header.length; index += 1) {
      actualChecksum += index >= 148 && index < 156 ? 0x20 : header[index]
    }
    if (storedChecksum !== actualChecksum) throw new Error('Tar header checksum mismatch')

    const name = tarString(header, 0, 100, 'name')
    const prefix = tarString(header, 345, 155, 'prefix')
    const entryName = prefix ? `${prefix}/${name}` : name
    const windowsKey = windowsTarPathKey(entryName)
    const typeByte = header[156]
    const type = typeByte === 0 ? '0' : String.fromCharCode(typeByte)
    if (type !== '0' && type !== '5') {
      throw new Error(`Unsupported tar entry type ${JSON.stringify(type)}; links and extended headers are forbidden`)
    }
    const linkName = tarString(header, 157, 100, 'link name')
    if (linkName) throw new Error(`Tar links are forbidden: ${entryName}`)

    const size = tarOctal(header, 124, 12, 'entry size')
    if (type === '5' && size !== 0) throw new Error(`Tar directory has non-zero size: ${entryName}`)
    const contentsStart = offset + 512
    const contentsEnd = contentsStart + size
    const nextOffset = contentsStart + Math.ceil(size / 512) * 512
    if (!Number.isSafeInteger(contentsEnd) || contentsEnd > archive.length || nextOffset > archive.length) {
      throw new Error(`Tar entry size exceeds archive bounds: ${entryName}`)
    }
    if (windowsKey === expectedWindowsKey) {
      binaryAliases += 1
      if (entryName !== expectedEntry) {
        throw new Error(`Tarball contains an ambiguous Windows CLI binary alias: ${entryName}`)
      }
      if (type !== '0') throw new Error(`Packed CLI binary must be a regular file: ${entryName}`)
      if (binarySha256 !== undefined) throw new Error(`Tarball contains a duplicate CLI binary entry: ${entryName}`)
      binarySha256 = createHash('sha256').update(archive.subarray(contentsStart, contentsEnd)).digest('hex')
    }
    offset = nextOffset
  }
  if (!reachedEnd) throw new Error('Tarball is missing its end marker')
  if (binaryAliases !== 1 || binarySha256 === undefined) {
    throw new Error(`Tarball must contain exactly one CLI binary entry: ${expectedEntry}`)
  }
  return binarySha256
}

function requireRegularFile(filePath, description) {
  let metadata
  try {
    metadata = lstatSync(filePath)
  } catch (error) {
    throw new Error(`${description} does not exist: ${filePath}`, { cause: error })
  }
  if (metadata.isSymbolicLink()) throw new Error(`${description} must not be a symbolic link: ${filePath}`)
  if (!metadata.isFile()) throw new Error(`${description} must be a regular file: ${filePath}`)
}

function requireRegularDirectory(directory, description) {
  const metadata = lstatSync(directory)
  if (metadata.isSymbolicLink() || !metadata.isDirectory()) {
    throw new Error(`${description} must be a regular directory: ${directory}`)
  }
}

function prepareOutputDirectory(output) {
  const resolvedOutput = path.resolve(output)
  mkdirSync(resolvedOutput, { recursive: true })
  requireRegularDirectory(resolvedOutput, 'Package output')
  return resolvedOutput
}

function createPrivatePackDirectory() {
  return mkdtempSync(path.join(tmpdir(), 'skillshub-cli-pack-'))
}

function writeVerifiedTarballAtomically(tarballSnapshot, destination) {
  const temporary = path.join(
    path.dirname(destination),
    `.${path.basename(destination)}.${process.pid}.${randomBytes(8).toString('hex')}.tmp`,
  )
  try {
    writeFileSync(temporary, tarballSnapshot, { flag: 'wx', mode: 0o600 })
    chmodSync(temporary, 0o644)
    renameSync(temporary, destination)
  } finally {
    rmSync(temporary, { force: true })
  }
}

export function runTrustedNpm(args, {
  run = spawnSync,
  npmExecPath = process.env.npm_execpath,
  nodePath = process.execPath,
  platform = process.platform,
  lstatPath = lstatSync,
  ...options
} = {}) {
  if (typeof npmExecPath !== 'string' || !npmExecPath || !path.isAbsolute(npmExecPath)) {
    throw new Error('An absolute npm_execpath is required; refusing to search PATH')
  }
  if (platform === 'win32' && !nodePath.toLowerCase().endsWith('.exe')) {
    throw new Error('The Windows npm invocation requires a Node executable')
  }
  const metadata = lstatPath(npmExecPath)
  if (metadata.isSymbolicLink()) throw new Error(`Trusted npm CLI entry must not be a symbolic link: ${npmExecPath}`)
  if (!metadata.isFile()) throw new Error(`Trusted npm CLI entry must be a regular file: ${npmExecPath}`)
  return run(nodePath, [npmExecPath, ...args], { ...options, shell: false })
}

function validateVersion(version) {
  if (typeof version !== 'string' || !/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/.test(version)) {
    throw new Error(`Invalid package version: ${version}`)
  }
}

function targetDefinition(target) {
  if (!Object.hasOwn(TARGETS, target)) throw new Error(`Unsupported CLI package target: ${target}`)
  return TARGETS[target]
}

export function parsePackageCliArgs(args) {
  const values = {}
  const allowed = new Set(['--version', '--target', '--source', '--output'])
  for (let index = 0; index < args.length; index += 2) {
    const flag = args[index]
    const value = args[index + 1]
    if (!allowed.has(flag)) throw new Error(`Unknown argument: ${flag ?? ''}`)
    if (!value || value.startsWith('--')) throw new Error(`Usage requires a value for ${flag}`)
    const key = flag.slice(2)
    if (Object.hasOwn(values, key)) throw new Error(`Duplicate argument: ${flag}`)
    values[key] = value
  }
  if (Object.keys(values).length !== allowed.size) {
    throw new Error('Usage: package-cli.mjs --version <x.y.z> --target <platform-arch> --source <binary> --output <directory>')
  }
  validateVersion(values.version)
  targetDefinition(values.target)
  return values
}

function verifyRepositoryVersions(root, version, packageDirectory) {
  const rootVersion = readJson(path.join(root, 'package.json'), 'root package manifest').version
  const desktopVersion = readJson(path.join(root, 'src-tauri/tauri.conf.json'), 'Tauri manifest').version
  const packageVersion = readJson(path.join(root, 'packages', packageDirectory, 'package.json'), 'platform package manifest').version
  for (const [name, actual] of [
    ['root package', rootVersion],
    ['desktop package', desktopVersion],
    ['platform package', packageVersion],
  ]) {
    if (actual !== version) throw new Error(`${name} version ${actual} does not match requested version ${version}`)
  }
}

function verifyBinaryVersion(source, version, run) {
  const result = run(source, ['version', '--json'], {
    encoding: 'utf8',
    shell: false,
    stdio: ['ignore', 'pipe', 'pipe'],
  })
  if (result.error || result.status !== 0 || result.signal) {
    throw new Error('CLI version verification failed')
  }
  if ((result.stderr ?? '') !== '') throw new Error('CLI version verification wrote unexpected stderr')
  let payload
  try {
    payload = JSON.parse((result.stdout ?? '').trim())
  } catch (error) {
    throw new Error('CLI version verification did not return valid JSON', { cause: error })
  }
  if (payload?.ok !== true || payload?.command !== 'version' || payload?.data?.version !== version) {
    throw new Error(`CLI binary version does not match requested version ${version}`)
  }
}

function prepareStaging({ root, version, target, source, staging, copyFile }) {
  const definition = targetDefinition(target)
  const triple = resolveSidecarTarget(target)
  const resolvedSource = path.resolve(source)
  requireRegularFile(resolvedSource, 'CLI source')
  const expectedName = `skillshub-cli-${triple}${definition.extension}`
  if (path.basename(resolvedSource) !== expectedName) {
    throw new Error(`CLI source filename must be ${expectedName}`)
  }
  const metadataPath = path.join(path.dirname(resolvedSource), `skillshub-cli-${triple}.json`)
  requireRegularFile(metadataPath, 'CLI metadata')
  const metadata = readJson(metadataPath, 'CLI metadata')
  if (metadata.version !== version) throw new Error('CLI metadata version mismatch')
  if (metadata.target !== triple) throw new Error('CLI metadata target mismatch')
  if (metadata.profile !== 'release') throw new Error('Only a release CLI artifact may be packaged')

  verifyRepositoryVersions(root, version, definition.directory)
  if (existsSync(staging)) {
    throw new Error(`Refusing pre-existing staging path: ${staging}`)
  }
  mkdirSync(staging, { mode: 0o755 })
  try {
    const template = path.join(root, 'packages', definition.directory)
    for (const filename of ['package.json', 'README.md', 'LICENSE']) {
      const destination = path.join(staging, filename)
      copyFile(path.join(template, filename), destination)
      chmodSync(destination, 0o644)
      utimesSync(destination, NORMALIZED_TIME, NORMALIZED_TIME)
    }
    const stagedBinary = path.join(staging, `skillshub-cli${definition.extension}`)
    copyFile(resolvedSource, stagedBinary)
    chmodSync(stagedBinary, 0o755)
    utimesSync(stagedBinary, NORMALIZED_TIME, NORMALIZED_TIME)
    requireRegularFile(stagedBinary, 'Staged CLI binary')
    const binarySha256 = sha256(stagedBinary)
    if (metadata.sha256 !== binarySha256) throw new Error('Staged CLI binary hash does not match release metadata')
    return {
      definition,
      expectedBinarySha256: metadata.sha256,
      stagedBinary,
      staging,
      triple,
    }
  } catch (error) {
    rmSync(staging, { recursive: true, force: true })
    throw error
  }
}

export function packageCli({
  root = ROOT,
  version,
  target,
  source,
  output,
  run = spawnSync,
  npmExecPath = process.env.npm_execpath,
  nodePath = process.execPath,
  npmPlatform = process.platform,
  copyFile = copyFileSync,
  createPackDirectory = createPrivatePackDirectory,
  readTarball = readFileSync,
}) {
  validateVersion(version)
  const resolvedOutput = prepareOutputDirectory(output)
  const packDirectory = path.resolve(createPackDirectory())
  try {
    requireRegularDirectory(packDirectory, 'Private pack directory')
    const staging = path.join(packDirectory, 'staging')
    const packOutput = path.join(packDirectory, 'packed')
    mkdirSync(packOutput, { mode: 0o700 })
    requireRegularDirectory(packOutput, 'Private npm pack output')
    const prepared = prepareStaging({
      root: path.resolve(root),
      version,
      target,
      source,
      staging,
      copyFile,
    })
    verifyBinaryVersion(prepared.stagedBinary, version, run)
    const result = runTrustedNpm([
      'pack', prepared.staging,
      '--json',
      '--ignore-scripts',
      '--pack-destination', packOutput,
    ], {
      run,
      npmExecPath,
      nodePath,
      platform: npmPlatform,
      encoding: 'utf8',
      env: {
        ...process.env,
        npm_config_audit: 'false',
        npm_config_fund: 'false',
        npm_config_ignore_scripts: 'true',
        npm_config_offline: 'true',
      },
      shell: false,
      stdio: ['ignore', 'pipe', 'pipe'],
    })
    if (result.error || result.status !== 0 || result.signal) throw new Error('npm pack failed')
    let entries
    try {
      entries = JSON.parse((result.stdout ?? '').trim())
    } catch (error) {
      throw new Error('npm pack did not return valid JSON', { cause: error })
    }
    if (!Array.isArray(entries) || entries.length !== 1 || typeof entries[0].filename !== 'string') {
      throw new Error('npm pack returned an unexpected result')
    }
    const filename = entries[0].filename
    if (!filename
      || filename !== path.posix.basename(filename)
      || filename !== path.win32.basename(filename)) {
      throw new Error('npm pack returned an unsafe filename')
    }
    const privateTarball = path.join(packOutput, filename)
    requireRegularFile(privateTarball, 'npm tarball')
    const tarballSnapshot = readTarball(privateTarball)
    const packedBinaryName = `package/skillshub-cli${prepared.definition.extension}`
    const packedBinarySha256 = readPackedBinarySha256(tarballSnapshot, packedBinaryName)
    if (packedBinarySha256 !== prepared.expectedBinarySha256) {
      throw new Error('Packed tarball binary hash does not match release metadata')
    }
    const packageSha256 = createHash('sha256').update(tarballSnapshot).digest('hex')
    const tarball = path.join(resolvedOutput, filename)
    writeVerifiedTarballAtomically(tarballSnapshot, tarball)
    return {
      binarySha256: packedBinarySha256,
      packageSha256,
      profile: 'release',
      target: prepared.triple,
      tarball,
      version,
    }
  } finally {
    rmSync(packDirectory, { recursive: true, force: true })
  }
}

function main() {
  const options = parsePackageCliArgs(process.argv.slice(2))
  process.stdout.write(`${JSON.stringify(packageCli(options))}\n`)
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  try {
    main()
  } catch (error) {
    process.stderr.write(`package-cli: ${error.message}\n`)
    process.exitCode = 1
  }
}
