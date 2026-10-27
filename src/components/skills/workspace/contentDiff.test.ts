import { describe, expect, it } from 'vitest'
import { buildContentDiff, buildInlineDiff, type ContentDiff, type ContentDiffLineEnding } from './contentDiff'

const endings: Record<ContentDiffLineEnding, string> = { lf: '\n', crlf: '\r\n', cr: '\r', none: '' }
function reconstruct(diff: ContentDiff, side: 'before' | 'after'): string {
  return diff.rows.map(row => row[side] ? row[side].text + endings[row[side].lineEnding] : '').join('')
}

describe('buildContentDiff', () => {
  it('identifies the actual changed line in the update preview instead of comparing two plain documents', () => {
    const header = '---\nname: Verify Beta\n---\n# Verify Beta\n\n'
    const diff = buildContentDiff(`${header}Original content version 1.`, `${header}Updated content version 2.`)

    expect(diff.rows.filter(row => row.changed)).toEqual([{
      before: { text: 'Original content version 1.', lineNumber: 6, lineEnding: 'none', kind: 'removed' },
      after: { text: 'Updated content version 2.', lineNumber: 6, lineEnding: 'none', kind: 'added' },
      changed: true,
    }])
    expect(diff).toMatchObject({ added: 1, removed: 1, isApproximate: false })
    expect(diff.rows.slice(0, 5).every(row => !row.changed)).toBe(true)
  })

  it('aligns insertions and deletions without marking the following unchanged lines', () => {
    const diff = buildContentDiff('title\nold\nkeep\nend\n', 'title\nkeep\nnew\nend\n')
    expect(diff.rows.map(row => [row.before?.text ?? null, row.after?.text ?? null, row.changed])).toEqual([
      ['title', 'title', false],
      ['old', null, true],
      ['keep', 'keep', false],
      [null, 'new', true],
      ['end', 'end', false],
    ])
    expect(diff.rows[2]).toMatchObject({ before: { lineNumber: 3 }, after: { lineNumber: 2 } })
    expect(diff).toMatchObject({ added: 1, removed: 1 })
  })

  it('pairs consecutive replacement lines and leaves a gap for an extra added line', () => {
    const diff = buildContentDiff('start\nold one\nold two\nend\n', 'start\nnew one\nnew two\nnew three\nend\n')
    expect(diff.rows.slice(1, -1).map(row => [row.before?.text ?? null, row.after?.text ?? null])).toEqual([
      ['old one', 'new one'], ['old two', 'new two'], [null, 'new three'],
    ])
    expect(diff).toMatchObject({ added: 3, removed: 2, isApproximate: false })
  })

  it.each([
    ['', '', 0, 0, 0],
    ['', 'a\n\nb\n', 3, 3, 0],
    ['a\n\nb\n', '', 3, 0, 3],
    ['\n', '', 1, 0, 1],
    ['same\n\n', 'same\n\n', 2, 0, 0],
  ])('preserves empty files and actual blank lines (%j -> %j)', (before, after, rowCount, added, removed) => {
    const diff = buildContentDiff(before, after)
    expect(diff.rows).toHaveLength(rowCount)
    expect(diff).toMatchObject({ added, removed, isApproximate: false })
    expect(reconstruct(diff, 'before')).toBe(before)
    expect(reconstruct(diff, 'after')).toBe(after)
  })

  it.each([
    ['line', 'line\n', 'none', 'lf'],
    ['line\n', 'line', 'lf', 'none'],
    ['line\r\n', 'line\n', 'crlf', 'lf'],
    ['line\r', 'line\r\n', 'cr', 'crlf'],
  ])('exposes line ending changes even when the visible text is identical', (before, after, oldEnding, newEnding) => {
    const diff = buildContentDiff(before, after)
    expect(diff.rows).toHaveLength(1)
    expect(diff.rows[0]).toMatchObject({
      before: { text: 'line', lineEnding: oldEnding, kind: 'removed' },
      after: { text: 'line', lineEnding: newEnding, kind: 'added' },
      changed: true,
    })
    expect(diff).toMatchObject({ added: 1, removed: 1 })
  })

  it('does not trim indentation, trailing whitespace, Unicode, or mixed line endings', () => {
    const before = '  中文🙂 \r\n\rvalue\nlast'
    const after = ' 中文🙂\r\n\rvalue\nlast\n'
    const diff = buildContentDiff(before, after)
    expect(diff.rows.filter(row => row.changed)).toHaveLength(2)
    expect(reconstruct(diff, 'before')).toBe(before)
    expect(reconstruct(diff, 'after')).toBe(after)
  })

  it('preserves repeated lines in order when the same text appears more than once', () => {
    const before = 'a\nb\na\nb\na\n'
    const after = 'a\na\nb\na\nb\n'
    const diff = buildContentDiff(before, after)
    expect(diff).toMatchObject({ added: 1, removed: 1, isApproximate: false })
    expect(reconstruct(diff, 'before')).toBe(before)
    expect(reconstruct(diff, 'after')).toBe(after)
    for (const side of ['before', 'after'] as const) {
      expect(diff.rows.flatMap(row => row[side] ? [row[side].lineNumber] : [])).toEqual([1, 2, 3, 4, 5])
    }
  })

  it('keeps a small change exact in a large document by removing matching prefix and suffix first', () => {
    const prefix = 'unchanged prefix\n'.repeat(2000)
    const suffix = 'unchanged suffix\n'.repeat(2000)
    const diff = buildContentDiff(`${prefix}before\n${suffix}`, `${prefix}after\n${suffix}`)
    expect(diff).toMatchObject({ added: 1, removed: 1, isApproximate: false })
    expect(diff.rows.filter(row => row.changed)).toHaveLength(1)
  })

  it('uses a bounded fallback for a large changed region without losing text or the exact outer context', () => {
    const before = `prefix\n${'old\n'.repeat(1200)}suffix\n`
    const after = `prefix\n${'new\n'.repeat(1201)}suffix\n`
    const diff = buildContentDiff(before, after)
    expect(diff).toMatchObject({ added: 1201, removed: 1200, isApproximate: true })
    expect(diff.rows[0].changed).toBe(false)
    expect(diff.rows.at(-1)?.changed).toBe(false)
    expect(reconstruct(diff, 'before')).toBe(before)
    expect(reconstruct(diff, 'after')).toBe(after)
  })
})

describe('buildInlineDiff', () => {
  it('highlights the separate changed words and numbers without highlighting the unchanged text between them', () => {
    expect(buildInlineDiff('Original content version 1.', 'Updated content version 2.')).toEqual({
      before: [
        { text: 'Original', changed: true },
        { text: ' content version ', changed: false },
        { text: '1', changed: true },
        { text: '.', changed: false },
      ],
      after: [
        { text: 'Updated', changed: true },
        { text: ' content version ', changed: false },
        { text: '2', changed: true },
        { text: '.', changed: false },
      ],
    })
  })

  it.each([
    ['', '', [], []],
    ['', 'new text!', [], [{ text: 'new text!', changed: true }]],
    ['old text!', '', [{ text: 'old text!', changed: true }], []],
    ['same  123.', 'same  123.', [{ text: 'same  123.', changed: false }], [{ text: 'same  123.', changed: false }]],
  ])('handles an empty or unchanged side (%j -> %j)', (before, after, expectedBefore, expectedAfter) => {
    expect(buildInlineDiff(before, after)).toEqual({ before: expectedBefore, after: expectedAfter })
  })

  it('aligns insertions and deletions within a sentence', () => {
    const diff = buildInlineDiff('keep old words end', 'keep words new end')
    expect(diff.before.filter(segment => segment.changed).map(segment => segment.text).join('')).toBe('old ')
    expect(diff.after.filter(segment => segment.changed).map(segment => segment.text).join('')).toBe(' new')
  })

  it('highlights changed punctuation and whitespace without changing either source', () => {
    expect(buildInlineDiff('value = 1,\tkeep', 'value  = 1; keep')).toEqual({
      before: [
        { text: 'value', changed: false }, { text: ' ', changed: true },
        { text: '= 1', changed: false }, { text: ',\t', changed: true }, { text: 'keep', changed: false },
      ],
      after: [
        { text: 'value', changed: false }, { text: '  ', changed: true },
        { text: '= 1', changed: false }, { text: '; ', changed: true }, { text: 'keep', changed: false },
      ],
    })
  })

  it('keeps Unicode letters and combining marks together and never splits surrogate pairs', () => {
    const diff = buildInlineDiff('版本 １ 🙂 cafe\u0301', '版本 ２ 🚀 cafe\u0301')
    expect(diff.before).toEqual([
      { text: '版本 ', changed: false }, { text: '１', changed: true },
      { text: ' ', changed: false }, { text: '🙂', changed: true }, { text: ' cafe\u0301', changed: false },
    ])
    expect(diff.after).toEqual([
      { text: '版本 ', changed: false }, { text: '２', changed: true },
      { text: ' ', changed: false }, { text: '🚀', changed: true }, { text: ' cafe\u0301', changed: false },
    ])
  })

  it('preserves large changed content when token alignment exceeds the comparison budget', () => {
    const before = `prefix ${'old+'.repeat(1500)}suffix`
    const after = `prefix ${'new-'.repeat(1501)}suffix`
    const diff = buildInlineDiff(before, after)
    expect(diff.before.map(segment => segment.text).join('')).toBe(before)
    expect(diff.after.map(segment => segment.text).join('')).toBe(after)
    expect(diff.before[0]).toEqual({ text: 'prefix ', changed: false })
    expect(diff.after.at(-1)).toEqual({ text: 'suffix', changed: false })
    for (const side of ['before', 'after'] as const) {
      expect(diff[side].every((segment, index, segments) => index === 0 || segment.changed !== segments[index - 1].changed)).toBe(true)
    }
  })
})
