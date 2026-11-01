// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, expect, it, vi } from 'vitest'
import { useState } from 'react'
import ConfirmActionModal from './ConfirmActionModal'

afterEach(cleanup)

it('includes native disclosure controls in the dialog focus loop', () => {
  render(<ConfirmActionModal open loading={false} title="Review files" cancelLabel="Cancel" confirmLabel="Confirm" onRequestClose={vi.fn()} onConfirm={vi.fn()} body={<details open><summary>Changed files</summary><button type="button">Inspect file</button></details>} />)
  const disclosure = screen.getByText('Changed files')
  const confirm = screen.getByRole('button', { name: 'Confirm' })
  expect(document.activeElement).toBe(disclosure)
  fireEvent.keyDown(document, { key: 'Tab', shiftKey: true })
  expect(document.activeElement).toBe(confirm)
  fireEvent.keyDown(document, { key: 'Tab' })
  expect(document.activeElement).toBe(disclosure)
})

it('keeps focus inside while content loads and returns it to the keyboard trigger on close', () => {
  function LoadingReview({ loading }: { loading: boolean }) {
    const [open, setOpen] = useState(false)
    return <>
      <button type="button" onClick={() => setOpen(true)}>Review changes</button>
      <ConfirmActionModal open={open} loading={loading} title="Review files" cancelLabel="Cancel" confirmLabel="Confirm" onRequestClose={() => setOpen(false)} onConfirm={vi.fn()} body={loading ? <p role="status">Loading files</p> : <details open><summary>Changed files</summary><button type="button">Inspect file</button></details>} />
    </>
  }
  const { rerender } = render(<LoadingReview loading />)
  const trigger = screen.getByRole('button', { name: 'Review changes' })
  trigger.focus()
  fireEvent.click(trigger)
  const dialog = screen.getByRole('dialog')
  expect(document.activeElement).toBe(dialog)
  fireEvent.keyDown(document, { key: 'Tab' })
  expect(document.activeElement).toBe(dialog)
  fireEvent.keyDown(document, { key: 'Escape' })
  fireEvent.click(dialog.parentElement!)
  expect(screen.getByRole('dialog')).toBe(dialog)

  rerender(<LoadingReview loading={false} />)
  expect(document.activeElement).toBe(dialog)
  fireEvent.keyDown(document, { key: 'Tab' })
  expect(document.activeElement).toBe(screen.getByText('Changed files'))
  fireEvent.keyDown(document, { key: 'Escape' })
  expect(screen.queryByRole('dialog')).toBeNull()
  expect(document.activeElement).toBe(trigger)
})
