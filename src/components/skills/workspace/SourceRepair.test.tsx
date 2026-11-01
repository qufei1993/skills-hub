// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import i18n from '../../../i18n'
import type { ManagedSkill } from '../types'
import SourceRepair from './SourceRepair'

vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }))
const skill: ManagedSkill = { id: 'alpha', name: 'Alpha', source_type: 'local', central_path: '/central/alpha', created_at: 1, updated_at: 1, enabled: true, status: 'ok', tags: [], targets: [] }
beforeEach(async () => { await i18n.changeLanguage('en') })
afterEach(cleanup)

it('keeps source fields labelled and prevents edits while validating their plan', async () => {
  let resolve!: (value: unknown) => void
  const invoke = vi.fn()
  invoke.mockImplementation(() => new Promise(done => { resolve = done }))
  render(<SourceRepair skill={skill} invokeTauri={invoke} onChanged={vi.fn(async () => {})} onClose={vi.fn()} t={i18n.t} />)
  const reference = screen.getByLabelText('Source location (no embedded credentials)')
  const kind = screen.getByLabelText('Source type')
  expect(document.activeElement).toBe(kind)
  fireEvent.change(reference, { target: { value: '/new-source' } })
  fireEvent.click(screen.getByRole('button', { name: 'Review plan' }))
  expect(reference.matches(':disabled')).toBe(true)
  expect(kind.matches(':disabled')).toBe(true)
  expect(screen.getByRole('button', { name: 'Choose source folder' }).matches(':disabled')).toBe(true)
  expect(screen.getByRole('status')).toBeTruthy()
  await act(async () => { resolve({ id: 'source-plan', sourceRef: '/new-source', targets: ['/tool/alpha'] }) })
  expect(screen.getByText('/new-source')).toBeTruthy()
  expect(screen.getByText('/tool/alpha')).toBeTruthy()
  expect(document.activeElement).toBe(screen.getByRole('heading', { name: 'Validated source' }))
  expect((screen.getByRole('button', { name: 'Confirm' }) as HTMLButtonElement).disabled).toBe(false)
  fireEvent.click(screen.getByRole('button', { name: 'Back' }))
  expect((screen.getByLabelText('Source location (no embedded credentials)') as HTMLInputElement).value).toBe('/new-source')
  expect(document.activeElement).toBe(screen.getByLabelText('Source location (no embedded credentials)'))
})

it('keeps an invalid source editable with its error next to the fields', async () => {
  const invoke = vi.fn().mockRejectedValue(new Error('INVALID_SOURCE'))
  render(<SourceRepair skill={skill} invokeTauri={invoke} onChanged={vi.fn(async () => {})} onClose={vi.fn()} t={i18n.t} />)
  fireEvent.change(screen.getByLabelText('Source type'), { target: { value: 'git' } })
  fireEvent.change(screen.getByLabelText('Source location (no embedded credentials)'), { target: { value: 'https://example.com/skills.git' } })
  fireEvent.change(screen.getByLabelText('Skill subdirectory (optional)'), { target: { value: 'skills/alpha' } })
  fireEvent.click(screen.getByRole('button', { name: 'Review plan' }))
  await screen.findByRole('alert')
  expect((screen.getByLabelText('Skill subdirectory (optional)') as HTMLInputElement).value).toBe('skills/alpha')
  expect(screen.getByLabelText('Source location (no embedded credentials)').matches(':disabled')).toBe(false)
})
