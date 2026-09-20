'use strict'

const { spawnSync } = require('node:child_process')
const { lstatSync, realpathSync } = require('node:fs')
const path = require('node:path')

const PACKAGE_ROOT = path.resolve(__dirname, '..')

const PLATFORM_PACKAGES = Object.freeze({
  'darwin-arm64': '@skillshub-app/cli-darwin-arm64/skillshub-cli',
  'darwin-x64': '@skillshub-app/cli-darwin-x64/skillshub-cli',
  'win32-x64': '@skillshub-app/cli-win32-x64/skillshub-cli',
  'linux-x64': '@skillshub-app/cli-linux-x64/skillshub-cli',
  'linux-arm64': '@skillshub-app/cli-linux-arm64/skillshub-cli',
})

function resolveBinaryPackage(platform, arch) {
  const key = `${platform}-${arch}`
  const packageExport = PLATFORM_PACKAGES[key]
  if (!packageExport) {
    throw new Error(`Unsupported platform or architecture: ${platform}-${arch}`)
  }
  return packageExport
}

function missingPlatformPackage(packageExport, platform, arch, cause) {
  const message = `Missing optional platform package ${packageExport}; reinstall skillshub-cli for ${platform}-${arch}.`
  const error = new Error(message)
  error.cause = cause
  return error
}

function requireDirectory(filePath, description, lstatPath) {
  const metadata = lstatPath(filePath)
  if (metadata.isSymbolicLink()) throw new Error(`${description} must not be a symbolic link: ${filePath}`)
  if (!metadata.isDirectory()) throw new Error(`${description} must be a directory: ${filePath}`)
}

function locateBinaryCandidate(packageExport, platform, arch, {
  searchPaths,
  lstatPath,
  realpathPath,
}) {
  const pathApi = platform === 'win32' ? path.win32 : path.posix
  const packageName = packageExport.slice(0, packageExport.lastIndexOf('/'))
  const [scope, unscopedName] = packageName.split('/')
  const expectedBinaryName = platform === 'win32' ? 'skillshub-cli.exe' : 'skillshub-cli'
  const roots = searchPaths(packageName)
  if (!Array.isArray(roots)) throw missingPlatformPackage(packageExport, platform, arch)

  for (const nodeModulesDirectory of roots) {
    const scopeDirectory = pathApi.join(nodeModulesDirectory, scope)
    const packageDirectory = pathApi.join(scopeDirectory, unscopedName)
    let packageMetadata
    try {
      packageMetadata = lstatPath(packageDirectory)
    } catch (error) {
      if (error?.code === 'ENOENT') continue
      throw error
    }
    requireDirectory(nodeModulesDirectory, 'Node modules directory', lstatPath)
    requireDirectory(scopeDirectory, 'Scoped package parent', lstatPath)
    if (packageMetadata.isSymbolicLink()) throw new Error(`Platform package must not be a symbolic link: ${packageDirectory}`)
    if (!packageMetadata.isDirectory()) throw new Error(`Platform package must be a directory: ${packageDirectory}`)

    const binary = pathApi.join(packageDirectory, expectedBinaryName)
    let binaryMetadata
    try {
      binaryMetadata = lstatPath(binary)
    } catch (error) {
      throw missingPlatformPackage(packageExport, platform, arch, error)
    }
    if (binaryMetadata.isSymbolicLink()) throw new Error(`CLI binary must not be a symbolic link: ${binary}`)
    if (!binaryMetadata.isFile()) throw new Error(`CLI binary must be a regular file: ${binary}`)

    const canonicalPackage = realpathPath(packageDirectory)
    const canonicalBinary = realpathPath(binary)
    if (pathApi.dirname(canonicalBinary) !== canonicalPackage) {
      throw new Error(`CLI binary escaped its fixed package root: ${binary}`)
    }
    return { binary, canonicalBinary, canonicalPackage }
  }
  throw missingPlatformPackage(packageExport, platform, arch)
}

function resolveBinaryPath(platform, arch, {
  resolvePath = require.resolve,
  searchPaths = require.resolve.paths,
  lstatPath = lstatSync,
  realpathPath = realpathSync,
} = {}) {
  const packageExport = resolveBinaryPackage(platform, arch)
  const candidate = locateBinaryCandidate(packageExport, platform, arch, {
    searchPaths,
    lstatPath,
    realpathPath,
  })
  let resolvedBinary
  try {
    resolvedBinary = resolvePath(packageExport, { paths: [PACKAGE_ROOT] })
  } catch (error) {
    throw missingPlatformPackage(packageExport, platform, arch, error)
  }
  const pathApi = platform === 'win32' ? path.win32 : path.posix
  const canonicalResolved = realpathPath(resolvedBinary)
  if (canonicalResolved !== candidate.canonicalBinary
    || pathApi.dirname(canonicalResolved) !== candidate.canonicalPackage) {
    throw new Error(`Resolved skillshub-cli binary escaped its fixed package boundary: ${resolvedBinary}`)
  }
  return candidate.binary
}

function launch({
  platform = process.platform,
  arch = process.arch,
  args = process.argv.slice(2),
  resolveBinary = resolveBinaryPath,
  spawn = spawnSync,
  kill = process.kill.bind(process),
  writeError = process.stderr.write.bind(process.stderr),
} = {}) {
  let result
  try {
    const binary = resolveBinary(platform, arch)
    result = spawn(binary, args, { shell: false, stdio: 'inherit' })
  } catch (error) {
    writeError(`skillshub-cli: ${error.message}\n`)
    return 1
  }

  if (result.error) {
    writeError(`skillshub-cli: ${result.error.message}\n`)
    return 1
  }
  if (result.signal) {
    kill(process.pid, result.signal)
    return 1
  }
  return Number.isInteger(result.status) ? result.status : 1
}

module.exports = {
  launch,
  resolveBinaryPackage,
  resolveBinaryPath,
}
