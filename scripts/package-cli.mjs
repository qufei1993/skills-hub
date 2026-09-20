#!/usr/bin/env node
import { spawnSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import {
  chmodSync,
  copyFileSync,
  existsSync,
  lstatSync,
  mkdirSync,
  readFileSync,
  rmSync,
  utimesSync,
} from 'node:fs'
import path from 'node:path'
import process from 'node:process'
import { fileURLToPath, pathToFileURL } from 'node:url'
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

function prepareStaging({ root, version, target, source, output, copyFile }) {
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
  const resolvedOutput = path.resolve(output)
  mkdirSync(resolvedOutput, { recursive: true })
  const outputMetadata = lstatSync(resolvedOutput)
  if (outputMetadata.isSymbolicLink() || !outputMetadata.isDirectory()) {
    throw new Error(`Package output must be a regular directory: ${resolvedOutput}`)
  }
  const staging = path.join(resolvedOutput, `.skillshub-cli-staging-${target}`)
  if (existsSync(staging)) {
    if (lstatSync(staging).isSymbolicLink()) throw new Error(`Refusing symlinked staging directory: ${staging}`)
    rmSync(staging, { recursive: true, force: true })
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
    return { binarySha256, definition, resolvedOutput, stagedBinary, staging, triple }
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
}) {
  validateVersion(version)
  const prepared = prepareStaging({ root: path.resolve(root), version, target, source, output, copyFile })
  try {
    verifyBinaryVersion(prepared.stagedBinary, version, run)
    const result = runTrustedNpm([
      'pack', prepared.staging,
      '--json',
      '--ignore-scripts',
      '--pack-destination', prepared.resolvedOutput,
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
    if (filename !== path.basename(filename)) throw new Error('npm pack returned an unsafe filename')
    const tarball = path.join(prepared.resolvedOutput, filename)
    requireRegularFile(tarball, 'npm tarball')
    return {
      binarySha256: prepared.binarySha256,
      packageSha256: sha256(tarball),
      profile: 'release',
      target: prepared.triple,
      tarball,
      version,
    }
  } finally {
    rmSync(prepared.staging, { recursive: true, force: true })
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
