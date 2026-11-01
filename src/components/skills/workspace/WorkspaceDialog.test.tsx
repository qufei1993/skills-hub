// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, expect, it, vi } from 'vitest'
import WorkspaceDialog from './WorkspaceDialog'

afterEach(cleanup)

it('keeps keyboard focus inside the dialog and excludes collapsed tool choices from the tab loop', () => {
  render(<WorkspaceDialog title="Apply preset" closeLabel="Close" busy={false} onClose={vi.fn()} footer={null}>
    <details><summary>Adjust tools</summary><input aria-label="Hidden tool choice" type="checkbox" /></details>
  </WorkspaceDialog>)
  const dialog = screen.getByRole('dialog', { name: 'Apply preset' })
  const close = screen.getByRole('button', { name: 'Close' })
  const summary = screen.getByText('Adjust tools', { selector: 'summary' })
  expect(document.activeElement).toBe(dialog)
  fireEvent.keyDown(document, { key: 'Tab', shiftKey: true })
  expect(document.activeElement).toBe(summary)
  fireEvent.keyDown(document, { key: 'Tab' })
  expect(document.activeElement).toBe(close)

  fireEvent.click(summary)
  dialog.focus()
  fireEvent.keyDown(document, { key: 'Tab', shiftKey: true })
  expect(document.activeElement).toBe(screen.getByLabelText('Hidden tool choice'))
  fireEvent.keyDown(document, { key: 'Tab' })
  expect(document.activeElement).toBe(close)
})

it('blocks Escape during an operation and uses the latest close handler once the operation finishes', () => {
  const initialClose = vi.fn()
  const nextClose = vi.fn()
  const { rerender } = render(<WorkspaceDialog title="Apply preset" closeLabel="Close" busy onClose={initialClose} footer={null}>Applying</WorkspaceDialog>)
  fireEvent.keyDown(document, { key: 'Escape' })
  expect(initialClose).not.toHaveBeenCalled()
  expect((screen.getByRole('button', { name: 'Close' }) as HTMLButtonElement).disabled).toBe(true)
  rerender(<WorkspaceDialog title="Some Skills need attention" closeLabel="Close" busy={false} onClose={nextClose} footer={null}>Review choices</WorkspaceDialog>)
  fireEvent.keyDown(document, { key: 'Escape' })
  expect(nextClose).toHaveBeenCalledOnce()
  expect(initialClose).not.toHaveBeenCalled()
})

it('returns focus to the triggering control after the dialog closes', () => {
  const trigger = document.createElement('button')
  trigger.textContent = 'Apply Daily'
  document.body.appendChild(trigger)
  trigger.focus()
  const { unmount } = render(<WorkspaceDialog title="Apply preset" closeLabel="Close" busy={false} onClose={vi.fn()} footer={null}>Saved Skills</WorkspaceDialog>)
  expect(document.activeElement).toBe(screen.getByRole('dialog'))
  unmount()
  expect(document.activeElement).toBe(trigger)
  trigger.remove()
})


it('restores an explicit trigger even when making the background inert already blurred it', () => {
  const trigger = document.createElement('button')
  trigger.textContent = 'Apply Daily'
  document.body.appendChild(trigger)
  trigger.focus()
  const returnFocusRef = { current: trigger }
  // WebKit moves focus away as React commits inert on the background page.
  trigger.blur()
  expect(document.activeElement).toBe(document.body)
  const { unmount } = render(<WorkspaceDialog title="Apply preset" closeLabel="Close" busy={false} onClose={vi.fn()} returnFocusRef={returnFocusRef} footer={null}>Saved Skills</WorkspaceDialog>)
  expect(document.activeElement).toBe(screen.getByRole('dialog'))
  unmount()
  expect(document.activeElement).toBe(trigger)
  trigger.remove()
})
