import { readFileSync } from 'node:fs'
import path from 'node:path'
import { pathToFileURL } from 'node:url'

export function verifyLinuxCli(bytes) {
  const invalid = () => { throw new Error('CLI_ELF_INVALID') }
  const range = (offset, size) => {
    if (!Number.isSafeInteger(offset) || !Number.isSafeInteger(size) || offset < 0 || size < 0 || offset + size > bytes.length) invalid()
  }
  range(0, 64)
  if (bytes.subarray(0, 6).toString('hex') !== '7f454c460201' || ![62, 183].includes(bytes.readUInt16LE(18))) invalid()
  const integer = offset => { range(offset, 8); const value = Number(bytes.readBigUInt64LE(offset)); if (!Number.isSafeInteger(value)) invalid(); return value }
  const start = integer(40)
  const width = bytes.readUInt16LE(58)
  const count = bytes.readUInt16LE(60)
  if (width < 64 || !count) invalid()
  range(start, width * count)
  const sections = Array.from({ length: count }, (_, index) => {
    const offset = start + width * index
    return { type: bytes.readUInt32LE(offset + 4), offset: integer(offset + 24), size: integer(offset + 32), link: bytes.readUInt32LE(offset + 40) }
  })
  const dynamic = sections.find(section => section.type === 6)
  if (!dynamic || dynamic.size % 16) invalid()
  const strings = sections[dynamic.link]
  if (!strings || strings.type !== 3) invalid()
  range(dynamic.offset, dynamic.size)
  range(strings.offset, strings.size)
  const dependencies = []
  for (let offset = dynamic.offset; offset < dynamic.offset + dynamic.size; offset += 16) {
    const tag = integer(offset)
    if (tag === 0) break
    if (tag !== 1) continue
    const index = integer(offset + 8)
    if (index >= strings.size) invalid()
    const begin = strings.offset + index
    const end = bytes.indexOf(0, begin)
    if (end < begin || end >= strings.offset + strings.size) invalid()
    dependencies.push(bytes.toString('utf8', begin, end))
  }
  const gui = dependencies.filter(name => /^lib(?:webkit|javascriptcore|gtk|gdk|soup|pango|cairo|atk|glib|gobject|gio|gmodule|gthread|harfbuzz)/.test(name))
  if (gui.length) throw new Error(`CLI_GUI_RUNTIME_DEPENDENCY: ${gui.join(', ')}`)
  return dependencies
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  verifyLinuxCli(readFileSync(process.argv[2]))
  console.log('Linux CLI has no GUI runtime dependencies.')
}
