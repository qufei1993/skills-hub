// @vitest-environment jsdom
import { cleanup, render, screen, fireEvent } from '@testing-library/react'
import { afterEach, beforeEach, expect, it } from 'vitest'
import i18n from '../../../i18n'
import ContentFileDiff from './ContentFileDiff'
import type { ContentPlan } from './ContentReview'

type Change = ContentPlan['changes'][number]
const base: Change = { path: 'SKILL.md', kind: 'modified', before: '', after: '' }
beforeEach(async () => { await i18n.changeLanguage('en') })
afterEach(cleanup)

it('aligns inserted lines without marking the unchanged tail as modified', () => {
  const { container } = render(<ContentFileDiff change={{ ...base, before: 'first\nlast\n', after: 'first\ninserted\nlast\n' }} t={i18n.t} />)
  expect(screen.getByText('− 0 removed')).toBeTruthy()
  expect(screen.getByText('+ 1 added')).toBeTruthy()
  expect(container.querySelectorAll('.content-diff-line-added')).toHaveLength(1)
  expect(container.querySelector('.content-diff-line-added code')?.textContent).toBe('inserted')
  const last = container.querySelectorAll('.content-diff-row')[2]
  expect(last.querySelector('.content-diff-line-before .content-diff-line-number')?.textContent).toBe('2')
  expect(last.querySelector('.content-diff-line-after .content-diff-line-number')?.textContent).toBe('3')
})

it('folds unchanged context and allows inspecting the original lines', () => {
  const context = Array.from({ length: 30 }, (_, index) => `context ${index}`).join('\n')
  const { container } = render(<ContentFileDiff change={{ ...base, before: `${context}\nold`, after: `${context}\nnew` }} t={i18n.t} />)
  const details = container.querySelector('details')!
  expect(details.open).toBe(false)
  expect(details.querySelector('summary')?.textContent).toBe('27 unchanged lines · expand / collapse')
  fireEvent.click(details.querySelector('summary')!)
  expect(details.open).toBe(true)
  expect(details.textContent).toContain('context 0')
  expect(container.querySelector('.content-diff-line-removed code')?.textContent).toContain('old')
})

it.each(['added', 'removed'])('handles an %s empty file without calling it binary or missing text', kind => {
  render(<ContentFileDiff change={{ ...base, kind, before: kind === 'added' ? null : '', after: kind === 'removed' ? null : '', beforeMetadata: kind === 'added' ? null : { kind: 'file' }, afterMetadata: kind === 'removed' ? null : { kind: 'file' } }} t={i18n.t} />)
  expect(screen.getByText('Empty file')).toBeTruthy()
  expect(screen.queryByText(/Binary or large file/)).toBeNull()
  expect(screen.queryByText('Text unchanged; file properties changed.')).toBeNull()
  expect(screen.queryByText('Text unchanged.')).toBeNull()
})

it('does not infer zero changes when a present file has no available text', () => {
  render(<ContentFileDiff change={{ ...base, before: null, after: null, beforeMetadata: { kind: 'file' }, afterMetadata: { kind: 'file' } }} t={i18n.t} />)
  expect(screen.getAllByText('Binary or large file — text preview unavailable.')).toHaveLength(2)
  expect(screen.queryByText('− 0 removed')).toBeNull()
  expect(screen.queryByText('Text unchanged.')).toBeNull()
})

it('distinguishes permissions-only changes and emphasizes both values', () => {
  const { container } = render(<ContentFileDiff change={{ ...base, before: 'same', after: 'same', beforeMetadata: { kind: 'file', mode: 0o644 }, afterMetadata: { kind: 'file', mode: 0o755 } }} t={i18n.t} />)
  expect(screen.getByText('Text unchanged; file properties changed.')).toBeTruthy()
  expect(container.querySelector('.content-diff-metadata-before')?.textContent).toContain('0644')
  expect(container.querySelector('.content-diff-metadata-after')?.textContent).toContain('0755')
  expect(container.querySelector('.content-diff-line-added')).toBeNull()
})

it('preserves the known text side of a file-to-directory replacement', () => {
  render(<ContentFileDiff change={{ ...base, before: 'old file', after: null, beforeMetadata: { kind: 'file' }, afterMetadata: { kind: 'directory' } }} t={i18n.t} />)
  expect(screen.getByText('Directory entry')).toBeTruthy()
  expect(screen.getByText('old file')).toBeTruthy()
  expect(screen.queryByText(/Binary or large file/)).toBeNull()
})

it('makes a final-newline-only change visible without inventing an added blank line', () => {
  const { container } = render(<ContentFileDiff change={{ ...base, before: 'same', after: 'same\n' }} t={i18n.t} />)
  expect(screen.getByText('No final newline')).toBeTruthy()
  expect(screen.getByText('LF')).toBeTruthy()
  expect(container.querySelectorAll('.content-diff-row')).toHaveLength(1)
  expect(screen.getByText('+ 1 added')).toBeTruthy()
})

it.each(['en', 'zh', 'ko'])('localizes comparison labels and change summaries in %s', async language => {
  await i18n.changeLanguage(language)
  render(<ContentFileDiff change={{ ...base, before: 'old', after: 'new' }} t={i18n.t} />)
  expect(screen.getByRole('region', { name: `SKILL.md · ${i18n.t('workspace110.diff.comparison')}` })).toBeTruthy()
  expect(screen.getByText(i18n.t('workspace110.diff.addedLines', { count: 1 }))).toBeTruthy()
  expect(screen.getByText(i18n.t('workspace110.diff.removedLines', { count: 1 }))).toBeTruthy()
})
