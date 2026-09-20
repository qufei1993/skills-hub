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

function resolveBinaryPath(platform, arch, {
  resolvePath = require.resolve,
  lstatPath = lstatSync,
  realpathPath = realpathSync,
} = {}) {
  const packageExport = resolveBinaryPackage(platform, arch)
  let binary
  try {
    binary = resolvePath(packageExport, { paths: [PACKAGE_ROOT] })
  } catch (error) {
    const message = `Missing optional platform package ${packageExport}; reinstall skillshub-cli for ${platform}-${arch}.`
    const wrapped = new Error(message)
    wrapped.cause = error
    throw wrapped
  }

  const metadata = lstatPath(binary)
  if (metadata.isSymbolicLink()) {
    throw new Error(`Refusing symbolic link for skillshub-cli binary: ${binary}`)
  }
  if (!metadata.isFile()) {
    throw new Error(`Resolved skillshub-cli binary is not a regular file: ${binary}`)
  }
  const pathApi = platform === 'win32' ? path.win32 : path.posix
  const packageName = packageExport.slice(0, packageExport.lastIndexOf('/'))
  const [scope, unscopedName] = packageName.split('/')
  const packageDirectory = pathApi.dirname(binary)
  const scopeDirectory = pathApi.dirname(packageDirectory)
  const nodeModulesDirectory = pathApi.dirname(scopeDirectory)
  const expectedBinaryName = platform === 'win32' ? 'skillshub-cli.exe' : 'skillshub-cli'
  const insideFixedPackage = pathApi.isAbsolute(binary)
    && pathApi.basename(binary) === expectedBinaryName
    && pathApi.basename(packageDirectory) === unscopedName
    && pathApi.basename(scopeDirectory) === scope
    && pathApi.basename(nodeModulesDirectory) === 'node_modules'
  if (!insideFixedPackage) {
    throw new Error(`Resolved skillshub-cli binary escaped its fixed package boundary: ${binary}`)
  }
  if (pathApi.resolve(realpathPath(binary)) !== pathApi.resolve(binary)) {
    throw new Error(`Refusing redirected skillshub-cli binary: ${binary}`)
  }
  return binary
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
