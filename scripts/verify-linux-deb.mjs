import { spawnSync } from 'node:child_process'
import path from 'node:path'
import { pathToFileURL } from 'node:url'

export function verifyDebMetadata(metadata, { version, architecture }) {
  if (!/^[a-z0-9][a-z0-9+.-]+$/.test(metadata.package ?? '') || metadata.version !== version || metadata.architecture !== architecture) {
    throw new Error('LINUX_DEB_METADATA_INVALID')
  }
  const dependencies = (metadata.depends ?? '').split(',').map(value => value.trim())
  const libc = dependencies.some(value => {
    const match = /^libc6(?::(?:any|amd64|arm64))?\s*\(>=\s*(\d+)\.(\d+)(?:[.-][0-9A-Za-z.+-]+)?\)$/.exec(value)
    return match && (Number(match[1]) > 2 || (Number(match[1]) === 2 && Number(match[2]) >= 39))
  })
  const webkit = dependencies.some(value => /^libwebkit2gtk-4\.1-0(?:\s*\([^|]+\))?$/.test(value))
  if (!libc || !webkit) throw new Error('LINUX_DEB_DEPENDENCIES_INVALID')
}

export function verifyDebPackage(filename, options) {
  const metadata = {}
  for (const field of ['Package', 'Version', 'Architecture', 'Depends']) {
    const result = spawnSync('dpkg-deb', ['--field', filename, field], { encoding: 'utf8', shell: false })
    if (result.error || result.status !== 0) throw new Error('LINUX_DEB_METADATA_INVALID')
    metadata[field.toLowerCase()] = result.stdout.trim()
  }
  verifyDebMetadata(metadata, options)
  return metadata
}
if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  try {
    verifyDebPackage(process.argv[2], { version: process.argv[3], architecture: process.argv[4] })
    console.log('Debian package metadata and minimum runtime dependencies verified.')
  } catch (error) { console.error(error.message); process.exitCode = 1 }
}
