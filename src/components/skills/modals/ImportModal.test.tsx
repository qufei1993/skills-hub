// @vitest-environment jsdom
import { useState } from 'react'
import { cleanup, fireEvent, render, screen, within } from '@testing-library/react'
import { afterEach, expect, it, vi } from 'vitest'
import type { TFunction } from 'i18next'
import type { OnboardingPlan } from '../types'
import ImportModal from './ImportModal'

const t = ((key: string) => key) as TFunction
const plan: OnboardingPlan = {
  total_tools_scanned: 2,
  total_skills_found: 2,
  groups: [
    { name: 'alpha', has_conflict: false, variants: [{ name: 'alpha', tool: 'cursor', path: '/tools/cursor/alpha', is_link: false }] },
    { name: 'beta', has_conflict: true, variants: [
      { name: 'beta', tool: 'codex', path: '/tools/codex/beta', is_link: false, plugin_name: 'design-pack' },
      { name: 'beta', tool: 'cursor', path: '/tools/cursor/beta', is_link: false },
    ] },
  ],
}
const props = () => ({
  open: true, loading: false, plan, selected: { alpha: true, beta: false },
  variantChoice: { beta: '/tools/codex/beta' }, onRequestClose: vi.fn(),
  onToggleGroup: vi.fn(), onSelectVariant: vi.fn(), onImport: vi.fn(), t,
})
afterEach(cleanup)

it('focuses search on open and keeps Tab and Shift+Tab within the import dialog', () => {
  render(<ImportModal {...props()} />)
  const dialog = screen.getByRole('dialog', { name: 'importTitle' })
  expect(dialog.getAttribute('aria-modal')).toBe('true')
  expect(document.activeElement).toBe(within(dialog).getByRole('textbox', { name: 'searchPlaceholder' }))
  const closeButtons = within(dialog).getAllByRole('button', { name: 'close' })
  const first = closeButtons[0]
  const last = closeButtons[closeButtons.length - 1]
  last.focus()
  fireEvent.keyDown(last, { key: 'Tab' })
  expect(document.activeElement).toBe(first)
  fireEvent.keyDown(first, { key: 'Tab', shiftKey: true })
  expect(document.activeElement).toBe(last)
})

it('closes with Escape and restores focus to the trigger when the dialog unmounts', () => {
  const onClose = vi.fn()
  const callbacks = props()
  function Page() {
    const [open, setOpen] = useState(false)
    return <>
      <button type="button" onClick={() => setOpen(true)}>Open import</button>
      {open && <ImportModal {...callbacks} onRequestClose={() => { onClose(); setOpen(false) }} />}
    </>
  }
  render(<Page />)
  const trigger = screen.getByRole('button', { name: 'Open import' })
  trigger.focus()
  fireEvent.click(trigger)
  fireEvent.keyDown(screen.getByRole('textbox', { name: 'searchPlaceholder' }), { key: 'Escape' })
  expect(onClose).toHaveBeenCalledOnce()
  expect(screen.queryByRole('dialog')).toBeNull()
  expect(document.activeElement).toBe(trigger)
})

it('prevents Escape, closing, and selection changes while importing', () => {
  const callbacks = props()
  render(<ImportModal {...callbacks} loading />)
  const dialog = screen.getByRole('dialog', { name: 'importTitle' })
  fireEvent.keyDown(dialog, { key: 'Escape' })
  fireEvent.click(dialog.parentElement!)
  for (const button of within(dialog).getAllByRole('button')) {
    expect((button as HTMLButtonElement).disabled).toBe(true)
    ;(button as HTMLButtonElement).click()
  }
  for (const input of within(dialog).getAllByRole('checkbox').concat(within(dialog).getAllByRole('radio'))) {
    expect((input as HTMLInputElement).disabled).toBe(true)
    ;(input as HTMLInputElement).click()
  }
  expect((within(dialog).getByRole('textbox', { name: 'searchPlaceholder' }) as HTMLInputElement).disabled).toBe(true)
  expect(callbacks.onRequestClose).not.toHaveBeenCalled()
  expect(callbacks.onToggleGroup).not.toHaveBeenCalled()
  expect(callbacks.onSelectVariant).not.toHaveBeenCalled()
  expect(callbacks.onImport).not.toHaveBeenCalled()
  fireEvent.keyDown(dialog, { key: 'Tab' })
  expect(document.activeElement).toBe(dialog)
})

it('filters detected Skills by keyword and limits select all to the visible matches', () => {
  const callbacks = props()
  render(<ImportModal {...callbacks} />)
  const search = screen.getByRole('textbox', { name: 'searchPlaceholder' })
  fireEvent.change(search, { target: { value: 'design-pack' } })
  expect(screen.queryByRole('checkbox', { name: 'alpha' })).toBeNull()
  expect(screen.getByRole('checkbox', { name: 'beta' })).toBeTruthy()
  fireEvent.click(screen.getByRole('checkbox', { name: 'selectAll' }))
  expect(callbacks.onToggleGroup.mock.calls).toEqual([['beta', true]])
  fireEvent.change(search, { target: { value: 'no-matches' } })
  expect(screen.getByText('importSearchEmpty')).toBeTruthy()
  expect((screen.getByRole('checkbox', { name: 'selectAll' }) as HTMLInputElement).disabled).toBe(true)
  fireEvent.change(search, { target: { value: '' } })
  expect(screen.getByRole('checkbox', { name: 'alpha' })).toBeTruthy()
  expect(screen.getByRole('checkbox', { name: 'beta' })).toBeTruthy()
})

it('keeps the total import selection visible when search hides selected Skills', () => {
  const countTranslation = ((key: string, values?: { selected: number; total: number }) =>
    key === 'selectedCount' ? `${values?.selected} / ${values?.total} selected` : key) as TFunction
  render(<ImportModal {...props()} selected={{ alpha: true, beta: true }} t={countTranslation} />)
  fireEvent.change(screen.getByRole('textbox', { name: 'searchPlaceholder' }), { target: { value: 'alpha' } })
  expect(screen.queryByRole('checkbox', { name: 'beta' })).toBeNull()
  expect(screen.getByText('2 / 2 selected')).toBeTruthy()
  expect((screen.getByRole('button', { name: 'importAndSync' }) as HTMLButtonElement).disabled).toBe(false)
})
