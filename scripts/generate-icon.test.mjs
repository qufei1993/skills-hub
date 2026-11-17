import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { copyFileSync, mkdirSync, mkdtempSync, readFileSync, rmSync, symlinkSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { inflateSync } from 'node:zlib'
const { it } = process.env.VITEST ? await import('vitest') : await import('node:test')

// Decode the RGBA PNG emitted by Tauri so assertions cover rendered pixels,
// independently of the SVG transform or generated file size.
function alphaBounds(png) {
  const width = png.readUInt32BE(16)
  const height = png.readUInt32BE(20)
  assert.equal(png[24], 8, '8-bit PNG')
  assert.equal(png[25], 6, 'RGBA PNG')
  assert.equal(png[28], 0, 'non-interlaced PNG')
  const chunks = []
  for (let offset = 8; offset < png.length;) {
    const length = png.readUInt32BE(offset)
    if (png.toString('ascii', offset + 4, offset + 8) === 'IDAT') {
      chunks.push(png.subarray(offset + 8, offset + 8 + length))
    }
    offset += length + 12
  }
  const raw = inflateSync(Buffer.concat(chunks))
  const stride = width * 4
  const pixels = Buffer.alloc(width * height * 4)
  let left = width, top = height, right = 0, bottom = 0
  for (let y = 0; y < height; y++) {
    const filter = raw[y * (stride + 1)]
    assert.ok(filter <= 4)
    for (let x = 0; x < stride; x++) {
      const index = y * stride + x
      const a = x >= 4 ? pixels[index - 4] : 0
      const b = y > 0 ? pixels[index - stride] : 0
      const c = y > 0 && x >= 4 ? pixels[index - stride - 4] : 0
      const p = a + b - c
      const distances = [Math.abs(p - a), Math.abs(p - b), Math.abs(p - c)]
      const paeth = distances[0] <= distances[1] && distances[0] <= distances[2] ? a : distances[1] <= distances[2] ? b : c
      const predictor = [0, a, b, Math.floor((a + b) / 2), paeth][filter]
      pixels[index] = (raw[y * (stride + 1) + 1 + x] + predictor) & 255
      if (x % 4 === 3 && pixels[index] > 127) {
        const column = Math.floor(x / 4)
        left = Math.min(left, column)
        top = Math.min(top, y)
        right = Math.max(right, column + 1)
        bottom = Math.max(bottom, y + 1)
      }
    }
  }
  return { width, height, left, top, right, bottom }
}

it('regenerates the master from original artwork without accumulating margins', () => {
  const root = mkdtempSync(path.join(tmpdir(), 'skills-hub-icon-test-'))
  try {
    for (const directory of ['scripts', 'src/assets', 'src-tauri/icons']) {
      mkdirSync(path.join(root, directory), { recursive: true })
    }
    copyFileSync(new URL('./generate-icon.mjs', import.meta.url), path.join(root, 'scripts/generate-icon.mjs'))
    copyFileSync(new URL('../src/assets/app-icon.svg', import.meta.url), path.join(root, 'src/assets/app-icon.svg'))
    symlinkSync(fileURLToPath(new URL('../node_modules', import.meta.url)), path.join(root, 'node_modules'), process.platform === 'win32' ? 'junction' : 'dir')
    const output = path.join(root, 'src-tauri/icons/icon-source.png')
    // A generated master must never become the next generation's source.
    writeFileSync(output, 'stale generated output')
    const run = (...args) => spawnSync(process.execPath, [path.join(root, 'scripts/generate-icon.mjs'), ...args], {
      cwd: tmpdir(), encoding: 'utf8', timeout: 20_000,
    })
    const first = run()
    assert.ifError(first.error)
    assert.equal(first.status, 0, first.stderr)
    const original = readFileSync(output)
    assert.deepEqual(alphaBounds(original), { width: 2048, height: 2048, left: 200, top: 200, right: 1848, bottom: 1848 })
    const second = run()
    assert.ifError(second.error)
    assert.equal(second.status, 0, second.stderr)
    assert.deepEqual(readFileSync(output), original, 'regenerating must not shrink the artwork')
    const custom = path.join(root, 'custom.png')
    assert.equal(run(custom).status, 0)
    assert.deepEqual(readFileSync(custom), original)
    const invalid = run(output, custom)
    assert.notEqual(invalid.status, 0, 'reject the old source/output calling convention')
    assert.match(invalid.stderr, /Usage:/)
    assert.deepEqual(readFileSync(output), original)
  } finally {
    rmSync(root, { recursive: true, force: true })
  }
}, process.env.VITEST ? 60_000 : undefined)
