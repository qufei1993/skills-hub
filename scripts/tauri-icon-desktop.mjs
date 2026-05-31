import { spawnSync } from 'node:child_process'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'

const projectRoot = path.resolve(import.meta.dirname, '..')
const outDir = path.join(projectRoot, 'src-tauri', 'icons')

const cliArgs = process.argv.slice(2)
const sourceArg = cliArgs[0]

const defaultSourceCandidates = [
  path.join(outDir, 'icon-source.png'),
  path.join(projectRoot, 'public', 'logo.png'),
]

const source =
  sourceArg ??
  defaultSourceCandidates.find((candidate) => fs.existsSync(candidate))

if (!source) {
  console.error(
    '未找到图标源文件，请传入路径，例如：node scripts/tauri-icon-desktop.mjs public/logo.png',
  )
  process.exit(1)
}

if (!fs.existsSync(outDir)) fs.mkdirSync(outDir, { recursive: true })

const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'skills-syncer-tauri-icons-'))
try {
  const res = spawnSync(
    'npx',
    ['--yes', 'tauri', 'icon', source, '-o', tempDir],
    { stdio: 'inherit', cwd: projectRoot },
  )
  if (res.status !== 0) process.exit(res.status ?? 1)

  const keepFiles = new Set([
    '32x32.png',
    '64x64.png',
    '128x128.png',
    '128x128@2x.png',
    'icon.png',
    'icon.icns',
    'icon.ico',
  ])

  for (const filename of keepFiles) {
    const src = path.join(tempDir, filename)
    const dst = path.join(outDir, filename)
    if (!fs.existsSync(src)) {
      console.error(`缺少生成文件：${src}`)
      process.exit(1)
    }
    fs.copyFileSync(src, dst)
  }

  for (const entry of fs.readdirSync(outDir, { withFileTypes: true })) {
    if (entry.isDirectory()) {
      fs.rmSync(path.join(outDir, entry.name), { recursive: true, force: true })
      continue
    }
    if (entry.name === 'icon-source.png') continue
    if (!keepFiles.has(entry.name)) fs.rmSync(path.join(outDir, entry.name))
  }
} finally {
  fs.rmSync(tempDir, { recursive: true, force: true })
}

