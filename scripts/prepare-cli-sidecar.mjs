import { spawnSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { copyFileSync, mkdirSync, readFileSync, renameSync, rmSync, writeFileSync } from 'node:fs'
import path from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'

const targets = {
  'darwin-arm64': 'aarch64-apple-darwin',
  'darwin-x64': 'x86_64-apple-darwin',
  'win32-x64': 'x86_64-pc-windows-msvc',
  'linux-x64': 'x86_64-unknown-linux-gnu',
  'linux-arm64': 'aarch64-unknown-linux-gnu',
}

export function resolveSidecarTarget(target) {
  if (typeof target !== 'string' || !target) throw new Error('An explicit CLI sidecar target is required.')
  const triple = Object.hasOwn(targets, target) ? targets[target] : target
  if (!Object.values(targets).includes(triple)) throw new Error('Unsupported CLI sidecar target.')
  return triple
}

export function desktopSidecarOptions(args, platform = process.platform, arch = process.arch) {
  let target
  for (let index = 0; index < args.length; index += 1) {
    if (args[index] === '--target' || args[index] === '-t' || args[index].startsWith('--target=')) {
      if (target !== undefined) throw new Error('Only one --target is supported.')
      target = args[index] === '--target' || args[index] === '-t' ? args[++index] : args[index].slice('--target='.length)
      target = resolveSidecarTarget(target)
    }
  }
  const debug = args.includes('--dev') ? !args.includes('--release') : args.includes('--debug') || args.includes('-d')
  return { target: resolveSidecarTarget(target ?? `${platform}-${arch}`), debug }
}

export function prepareCliSidecar({ root, target, debug = false, run = spawnSync, env = process.env }) {
  const triple = resolveSidecarTarget(target)
  const tauriRoot = path.join(root, 'src-tauri')
  const binaries = path.join(tauriRoot, 'binaries')
  const base = `skillshub-cli-${triple}`
  const extension = triple.includes('windows') ? '.exe' : ''
  const metadataPath = path.join(binaries, `${base}.json`)
  mkdirSync(binaries, { recursive: true })
  rmSync(metadataPath, { force: true })
  const version = JSON.parse(readFileSync(path.join(root, 'package.json'), 'utf8')).version
  const desktopVersion = JSON.parse(readFileSync(path.join(tauriRoot, 'tauri.conf.json'), 'utf8')).version
  if (typeof version !== 'string' || !/^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/.test(version) || version !== desktopVersion) {
    throw new Error('CLI and desktop package versions must match.')
  }
  const args = ['build', '--locked', '--bin', 'skillshub-cli', '--target', triple]
  if (!debug) args.push('--release')
  const result = run('cargo', args, {
    cwd: tauriRoot,
    env: { ...env, SKILLS_HUB_PREPARE_CLI_SIDECAR: '1', CARGO_TARGET_DIR: path.join(tauriRoot, 'target') },
    stdio: 'inherit',
    shell: false,
  })
  if (result.error || result.status !== 0) throw new Error('CLI sidecar build failed.')
  const source = path.join(tauriRoot, 'target', triple, debug ? 'debug' : 'release', `skillshub-cli${extension}`)
  const binaryPath = path.join(binaries, `${base}${extension}`)
  const tempBinary = `${binaryPath}.${process.pid}.tmp`
  const tempMetadata = `${metadataPath}.${process.pid}.tmp`
  try {
    copyFileSync(source, tempBinary)
    const sha256 = createHash('sha256').update(readFileSync(tempBinary)).digest('hex')
    renameSync(tempBinary, binaryPath)
    writeFileSync(tempMetadata, `${JSON.stringify({ version, target: triple, sha256 })}\n`, { mode: 0o600 })
    renameSync(tempMetadata, metadataPath)
    return { binaryPath, metadataPath }
  } finally {
    rmSync(tempBinary, { force: true })
    rmSync(tempMetadata, { force: true })
  }
}

function main(args) {
  const targetIndex = args.indexOf('--target')
  if (targetIndex !== 0 || !args[1] || args.some((arg, index) => index > 1 && arg !== '--debug')) {
    throw new Error('Usage: prepare-cli-sidecar.mjs --target <platform-arch|rust-triple> [--debug]')
  }
  prepareCliSidecar({ root: path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..'), target: args[1], debug: args.includes('--debug') })
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  try { main(process.argv.slice(2)) } catch (error) { console.error(error.message); process.exitCode = 1 }
}
