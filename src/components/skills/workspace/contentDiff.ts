export type ContentDiffLineEnding = 'lf' | 'crlf' | 'cr' | 'none'

export type ContentDiffLine = {
  text: string
  lineNumber: number
  lineEnding: ContentDiffLineEnding
  kind: 'equal' | 'removed' | 'added'
}

export type ContentDiffRow = {
  before: ContentDiffLine | null
  after: ContentDiffLine | null
  changed: boolean
}

export type ContentDiff = {
  rows: ContentDiffRow[]
  added: number
  removed: number
  isApproximate: boolean
}

export type ContentDiffSegment = { text: string; changed: boolean }
export type InlineContentDiff = { before: ContentDiffSegment[]; after: ContentDiffSegment[] }

type SourceLine = Omit<ContentDiffLine, 'kind'>
type AlignedRow<T> = { before: T | null; after: T | null; changed: boolean }

// Bound both the LCS work and its table to at most one million cells (4 MB).
// Larger changed sections use a marked, linear-time comparison instead.
const MAX_COMPARISON_CELLS = 1_000_000

function splitLines(content: string): SourceLine[] {
  const lines: SourceLine[] = []
  const endings = /\r\n|\r|\n/g
  let start = 0
  for (const match of content.matchAll(endings)) {
    lines.push({
      text: content.slice(start, match.index),
      lineNumber: lines.length + 1,
      lineEnding: match[0] === '\r\n' ? 'crlf' : match[0] === '\r' ? 'cr' : 'lf',
    })
    start = match.index + match[0].length
  }
  // A terminator belongs to its preceding line; it does not create a phantom
  // empty line. An actual blank line still has its own terminator and number.
  if (start < content.length) {
    lines.push({ text: content.slice(start), lineNumber: lines.length + 1, lineEnding: 'none' })
  }
  return lines
}

function equalLines(before: SourceLine, after: SourceLine): boolean {
  return before.text === after.text && before.lineEnding === after.lineEnding
}

function alignItems<T>(oldLines: T[], newLines: T[], equalLines: (before: T, after: T) => boolean) {
  const result: { rows: AlignedRow<T>[]; isApproximate: boolean } = { rows: [], isApproximate: false }
  const append = (before: T | null, after: T | null) => {
    result.rows.push({ before, after, changed: !(before !== null && after !== null && equalLines(before, after)) })
  }

  let prefix = 0
  while (prefix < oldLines.length && prefix < newLines.length && equalLines(oldLines[prefix], newLines[prefix])) {
    append(oldLines[prefix], newLines[prefix])
    prefix += 1
  }
  let oldEnd = oldLines.length
  let newEnd = newLines.length
  while (oldEnd > prefix && newEnd > prefix && equalLines(oldLines[oldEnd - 1], newLines[newEnd - 1])) {
    oldEnd -= 1
    newEnd -= 1
  }

  const oldCount = oldEnd - prefix
  const newCount = newEnd - prefix
  const cells = (oldCount + 1) * (newCount + 1)
  if (!oldCount || !newCount || cells > MAX_COMPARISON_CELLS) {
    result.isApproximate = oldCount > 0 && newCount > 0 && cells > MAX_COMPARISON_CELLS
    for (let index = 0; index < Math.max(oldCount, newCount); index += 1) {
      append(index < oldCount ? oldLines[prefix + index] : null, index < newCount ? newLines[prefix + index] : null)
    }
  } else {
    const width = newCount + 1
    const lengths = new Uint32Array(cells)
    for (let oldIndex = oldCount - 1; oldIndex >= 0; oldIndex -= 1) {
      for (let newIndex = newCount - 1; newIndex >= 0; newIndex -= 1) {
        const cell = oldIndex * width + newIndex
        lengths[cell] = equalLines(oldLines[prefix + oldIndex], newLines[prefix + newIndex])
          ? lengths[cell + width + 1] + 1
          : Math.max(lengths[cell + width], lengths[cell + 1])
      }
    }

    let oldIndex = 0
    let newIndex = 0
    let removed: T[] = []
    let added: T[] = []
    const flushChanges = () => {
      for (let index = 0; index < Math.max(removed.length, added.length); index += 1) {
        append(removed[index] ?? null, added[index] ?? null)
      }
      removed = []
      added = []
    }
    while (oldIndex < oldCount || newIndex < newCount) {
      if (oldIndex < oldCount && newIndex < newCount && equalLines(oldLines[prefix + oldIndex], newLines[prefix + newIndex])) {
        flushChanges()
        append(oldLines[prefix + oldIndex], newLines[prefix + newIndex])
        oldIndex += 1
        newIndex += 1
      } else if (oldIndex < oldCount && (newIndex === newCount || lengths[(oldIndex + 1) * width + newIndex] >= lengths[oldIndex * width + newIndex + 1])) {
        removed.push(oldLines[prefix + oldIndex])
        oldIndex += 1
      } else {
        added.push(newLines[prefix + newIndex])
        newIndex += 1
      }
    }
    flushChanges()
  }

  for (; oldEnd < oldLines.length && newEnd < newLines.length; oldEnd += 1, newEnd += 1) {
    append(oldLines[oldEnd], newLines[newEnd])
  }
  return result
}

export function buildContentDiff(before: string, after: string): ContentDiff {
  const aligned = alignItems(splitLines(before), splitLines(after), equalLines)
  let added = 0
  let removed = 0
  const rows: ContentDiffRow[] = aligned.rows.map(row => {
    if (row.changed && row.before) removed += 1
    if (row.changed && row.after) added += 1
    return {
      before: row.before ? { ...row.before, kind: row.changed ? 'removed' : 'equal' } : null,
      after: row.after ? { ...row.after, kind: row.changed ? 'added' : 'equal' } : null,
      changed: row.changed,
    }
  })
  return { rows, added, removed, isApproximate: aligned.isApproximate }
}

function splitTokens(content: string): string[] {
  return content.match(/[\p{L}\p{M}]+|\p{N}+|\s+|[^\p{L}\p{M}\p{N}\s]/gu) ?? []
}

export function buildInlineDiff(before: string, after: string): InlineContentDiff {
  const aligned = alignItems(splitTokens(before), splitTokens(after), (left, right) => left === right)
  const result: InlineContentDiff = { before: [], after: [] }
  for (const row of aligned.rows) {
    for (const side of ['before', 'after'] as const) {
      const text = row[side]
      if (text === null) continue
      const previous = result[side].at(-1)
      if (previous?.changed === row.changed) previous.text += text
      else result[side].push({ text, changed: row.changed })
    }
  }
  return result
}
