// @vitest-environment jsdom
import { StrictMode } from 'react'
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import i18n from '../../../i18n'
import type { SourceConflictOutcome, SourceConflictPreview } from '../types'
import SourceConflictDialog from './SourceConflictDialog'

const sourcePath = '/tool/demo'
function preview(id = 'preview-one'): SourceConflictPreview {
  return { id, skillId: 'demo', name: 'Demo', sourcePath, centralPath: '/central/demo',
    changes: [{ path: 'SKILL.md', kind: 'modified', before: 'central version\n', after: 'tool version\n', beforeMetadata: { kind: 'file' }, afterMetadata: { kind: 'file' } }],
    central: { targets: [sourcePath], blocked: null }, tool: { targets: [sourcePath, '/other/demo'], blocked: null },
  }
}
function outcome(): SourceConflictOutcome { return { id: 'preview-one', skillId: 'demo', name: 'Demo', choice: 'central', targets: [sourcePath] } }
function deferred<T>() { let resolve!: (value: T) => void; const promise = new Promise<T>(done => { resolve = done }); return { promise, resolve } }
const w = (key: string) => i18n.t(`sourceConflict.${key}`)
beforeEach(async () => { await i18n.changeLanguage('en') })
afterEach(cleanup)

it('shares StrictMode preview work, labels both versions and requires an explicit choice', async () => {
  const pending = deferred<SourceConflictPreview>()
  const invoke = vi.fn().mockReturnValue(pending.promise)
  render(<StrictMode><SourceConflictDialog sourcePath={sourcePath} invokeTauri={invoke} onChanged={vi.fn(async () => {})} onClose={vi.fn()} t={i18n.t} /></StrictMode>)
  expect(invoke).toHaveBeenCalledTimes(1)
  expect(screen.queryByRole('button', { name: /back up/i })).toBeNull()
  expect((screen.getByRole('button', { name: w('resolve') }) as HTMLButtonElement).disabled).toBe(true)
  await act(async () => pending.resolve(preview()))
  expect(screen.getByRole('region').textContent).toContain('central version')
  expect(screen.getByRole('region').textContent).toContain('tool version')
  expect(screen.getByText(w('central'), { selector: 'strong' })).toBeTruthy()
  expect(screen.getByText(w('tool'), { selector: 'strong' })).toBeTruthy()
  expect(screen.getAllByRole('radio').every(radio => !(radio as HTMLInputElement).checked)).toBe(true)
  fireEvent.click(screen.getByRole('radio', { name: new RegExp(w('keepTool')) }))
  expect(screen.getByText('/other/demo')).toBeTruthy()
  expect((screen.getByRole('button', { name: w('resolve') }) as HTMLButtonElement).disabled).toBe(false)
})

it('applies the chosen version once, blocks closing while saving and shows a simple completion', async () => {
  const pending = deferred<SourceConflictOutcome>()
  const changed = vi.fn(async () => {})
  const close = vi.fn()
  const invoke = vi.fn().mockImplementation((command: string) => command === 'source_conflict_preview' ? Promise.resolve(preview()) : command === 'source_conflict_execute' ? pending.promise : Promise.resolve())
  render(<SourceConflictDialog sourcePath={sourcePath} invokeTauri={invoke} onChanged={changed} onClose={close} t={i18n.t} />)
  await screen.findByText('Demo', { selector: 'h3' })
  fireEvent.click(screen.getByRole('radio', { name: new RegExp(w('keepCentral')) }))
  const apply = screen.getByRole('button', { name: w('resolve') })
  fireEvent.click(apply); fireEvent.click(apply)
  fireEvent.keyDown(document, { key: 'Escape' })
  expect(close).not.toHaveBeenCalled()
  expect(screen.getAllByRole('radio').every(radio => radio.matches(':disabled'))).toBe(true)
  expect(invoke.mock.calls.filter(([command]) => command === 'source_conflict_execute')).toEqual([['source_conflict_execute', { id: 'preview-one', choice: 'central' }]])
  await act(async () => pending.resolve(outcome()))
  expect(changed).toHaveBeenCalledTimes(1)
  expect(screen.getByText(i18n.t('sourceConflict.success', { name: 'Demo' }))).toBeTruthy()
  expect(screen.queryByText(/backup/i)).toBeNull()
  expect(invoke.mock.calls.some(([command]) => command === 'source_conflict_open_backup')).toBe(false)
  fireEvent.click(screen.getByRole('button', { name: w('done') }))
  expect(close).toHaveBeenCalledTimes(1)
})

it('retains the version choice but requires a new preview when content becomes stale', async () => {
  let reads = 0
  const invoke = vi.fn().mockImplementation((command: string) => command === 'source_conflict_preview' ? Promise.resolve(preview(++reads === 1 ? 'old' : 'new')) : Promise.reject(JSON.stringify({ code: 'PLAN_STALE', details: {} })))
  render(<SourceConflictDialog sourcePath={sourcePath} invokeTauri={invoke} onChanged={vi.fn(async () => {})} onClose={vi.fn()} t={i18n.t} />)
  await screen.findByText('Demo', { selector: 'h3' })
  fireEvent.click(screen.getByRole('radio', { name: new RegExp(w('keepTool')) }))
  fireEvent.click(screen.getByRole('button', { name: w('resolve') }))
  await screen.findByText(w('stale'))
  expect((screen.getByRole('button', { name: w('resolve') }) as HTMLButtonElement).disabled).toBe(true)
  fireEvent.click(screen.getByRole('button', { name: w('recheck') }))
  await waitFor(() => expect((screen.getByRole('button', { name: w('resolve') }) as HTMLButtonElement).disabled).toBe(false))
  expect((screen.getByRole('radio', { name: new RegExp(w('keepTool')) }) as HTMLInputElement).checked).toBe(true)
  fireEvent.click(screen.getByRole('button', { name: w('resolve') }))
  await waitFor(() => expect(invoke).toHaveBeenLastCalledWith('source_conflict_execute', { id: 'new', choice: 'tool' }))
})

it('reconciles an unknown response with the same operation id and choice', async () => {
  let executions = 0
  const invoke = vi.fn().mockImplementation((command: string) => command === 'source_conflict_preview' ? Promise.resolve(preview()) : ++executions === 1 ? Promise.reject(new Error('connection lost')) : executions === 2 ? Promise.reject(JSON.stringify({ code: 'OPERATION_BUSY' })) : Promise.resolve(outcome()))
  render(<SourceConflictDialog sourcePath={sourcePath} invokeTauri={invoke} onChanged={vi.fn(async () => {})} onClose={vi.fn()} t={i18n.t} />)
  await screen.findByText('Demo', { selector: 'h3' })
  fireEvent.click(screen.getByRole('radio', { name: new RegExp(w('keepCentral')) }))
  fireEvent.click(screen.getByRole('button', { name: w('resolve') }))
  await screen.findByText(w('unknown'))
  expect(screen.getAllByRole('radio').every(radio => radio.matches(':disabled'))).toBe(true)
  fireEvent.click(screen.getByRole('button', { name: w('verifyResult') }))
  await screen.findByText(w('unknown'))
  fireEvent.click(screen.getByRole('button', { name: w('verifyResult') }))
  await screen.findByText(i18n.t('sourceConflict.success', { name: 'Demo' }))
  expect(invoke.mock.calls.filter(([command]) => command === 'source_conflict_execute').map(([, args]) => args)).toEqual(Array.from({ length: 3 }, () => ({ id: 'preview-one', choice: 'central' })))
  expect(invoke.mock.calls.filter(([command]) => command === 'source_conflict_preview')).toHaveLength(1)
})

it('reports a committed result separately from a library refresh failure', async () => {
  const changed = vi.fn().mockRejectedValueOnce(new Error('refresh failed')).mockResolvedValue(undefined)
  const invoke = vi.fn().mockImplementation((command: string) => Promise.resolve(command === 'source_conflict_preview' ? preview() : outcome()))
  render(<SourceConflictDialog sourcePath={sourcePath} invokeTauri={invoke} onChanged={changed} onClose={vi.fn()} t={i18n.t} />)
  await screen.findByText('Demo', { selector: 'h3' })
  fireEvent.click(screen.getByRole('radio', { name: new RegExp(w('keepCentral')) }))
  fireEvent.click(screen.getByRole('button', { name: w('resolve') }))
  await screen.findByText(w('refreshFailed'))
  expect(screen.getByText(i18n.t('sourceConflict.success', { name: 'Demo' }))).toBeTruthy()
  fireEvent.click(screen.getByRole('button', { name: w('refresh') }))
  await waitFor(() => expect(screen.queryByText(w('refreshFailed'))).toBeNull())
  expect(invoke.mock.calls.filter(([command]) => command === 'source_conflict_execute')).toHaveLength(1)
})

it('disables an unsafe choice while keeping the safe alternative available', async () => {
  const plan = preview()
  plan.tool = { targets: [], blocked: { code: 'TARGET_CONFLICT', path: '/modified-copy/demo' } }
  render(<SourceConflictDialog sourcePath={sourcePath} invokeTauri={vi.fn().mockResolvedValue(plan)} onChanged={vi.fn(async () => {})} onClose={vi.fn()} t={i18n.t} />)
  await screen.findByText('/modified-copy/demo')
  expect((screen.getByRole('radio', { name: new RegExp(w('keepTool')) }) as HTMLInputElement).disabled).toBe(true)
  expect((screen.getByRole('radio', { name: new RegExp(w('keepCentral')) }) as HTMLInputElement).disabled).toBe(false)
})
