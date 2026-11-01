import { chmodSync, copyFileSync, mkdirSync, readdirSync, statSync } from 'node:fs'
import path from 'node:path'
import { pathToFileURL } from 'node:url'

export function prepareLinuxAssets({ bundle, output, tag, target }) {
  const arch = { 'x86_64-unknown-linux-gnu': 'x64', 'aarch64-unknown-linux-gnu': 'arm64' }[target]
  if (!arch || !/^v\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/.test(tag)) throw new Error('LINUX_RELEASE_IDENTITY_INVALID')
  const select = (folder, extension) => {
    const directory = path.join(bundle, folder)
    const files = readdirSync(directory, { withFileTypes: true }).filter(file => file.isFile() && file.name.endsWith(extension))
    if (files.length !== 1) throw new Error('LINUX_INSTALLER_MISSING_OR_AMBIGUOUS')
    return path.join(directory, files[0].name)
  }
  const deb = select('deb', '.deb')
  const image = select('appimage', '.AppImage')
  const files = [[deb, '.deb'], [`${deb}.sig`, '.deb.sig'], [image, '.AppImage'], [`${image}.sig`, '.AppImage.sig']]
  for (const [file] of files) if (!statSync(file).isFile() || !statSync(file).size) throw new Error('LINUX_INSTALLER_OR_SIGNATURE_EMPTY')
  mkdirSync(output, { recursive: true })
  for (const [file, extension] of files) {
    const destination = path.join(output, `Skills-Hub-${tag}-Linux-${arch}${extension}`)
    copyFileSync(file, destination)
    if (extension === '.AppImage') chmodSync(destination, 0o755)
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  prepareLinuxAssets({ bundle: process.argv[2], output: process.argv[3], tag: process.argv[4], target: process.argv[5] })
}
