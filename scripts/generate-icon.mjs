import { spawnSync } from 'node:child_process'
import { copyFileSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { createRequire } from 'node:module'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const projectRoot = fileURLToPath(new URL('../', import.meta.url))
const args = process.argv.slice(2)
if (args.length > 1) {
  console.error('Usage: node scripts/generate-icon.mjs [output.png]')
  process.exit(1)
}
const output = args[0] ?? path.join(projectRoot, 'src-tauri/icons/icon-source.png')
// Always render the original vector artwork, never the generated PNG master.
const source = readFileSync(path.join(projectRoot, 'src/assets/app-icon.svg'), 'utf8')
const originalViewBox = 'viewBox="0 0 1024 1024"'
if (!source.includes(originalViewBox)) {
  throw new Error('Expected the original 1024×1024 app-icon.svg design space.')
}
// Map the original 928px artwork to an 824px box with 100px margins.
const scale = 824 / 928
const canvas = 1024 / scale
const origin = (1024 - canvas) / 2
const svg = source.replace(originalViewBox, `viewBox="${origin} ${origin} ${canvas} ${canvas}"`)
const tempDir = mkdtempSync(path.join(tmpdir(), 'skills-hub-icon-master-'))
try {
  const input = path.join(tempDir, 'icon.svg')
  writeFileSync(input, svg)
  const cli = createRequire(import.meta.url).resolve('@tauri-apps/cli/tauri.js')
  const result = spawnSync(process.execPath, [cli, 'icon', input, '-o', tempDir, '--png', '2048'], { stdio: 'inherit' })
  if (result.error) throw result.error
  if (result.status !== 0) throw new Error(`Icon rendering failed (${result.status ?? result.signal}).`)
  copyFileSync(path.join(tempDir, '2048x2048.png'), output)
  console.log(`Wrote ${output} (2048×2048)`)
} finally {
  rmSync(tempDir, { recursive: true, force: true })
}
