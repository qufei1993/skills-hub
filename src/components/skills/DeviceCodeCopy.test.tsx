// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, expect, it, vi } from 'vitest'
import type { TFunction } from 'i18next'
import DeviceCodeCopy from './DeviceCodeCopy'

const t = ((key: string) => key) as TFunction
afterEach(() => { cleanup(); vi.useRealTimers(); vi.restoreAllMocks() })

it('confirms only completed copies and resets the feedback', async () => {
  vi.useFakeTimers()
  let finish!: () => void
  Object.defineProperty(navigator, 'clipboard', { configurable: true, value: {
    writeText: (value: string) => { expect(value).toBe('ABCD'); return new Promise<void>(resolve => { finish = resolve }) },
  } })
  render(<DeviceCodeCopy code="ABCD" t={t} />)
  fireEvent.click(screen.getByRole('button'))
  expect(screen.queryByText('copied')).toBeNull()
  await act(async () => finish())
  expect(screen.getByText('copied')).toBeTruthy()
  act(() => vi.advanceTimersByTime(2500))
  expect(screen.getByText('deviceSync.copyCode')).toBeTruthy()
})

it('shows a safe failure and allows retry', async () => {
  let fail = true
  Object.defineProperty(navigator, 'clipboard', { configurable: true, value: {
    writeText: async () => { if (fail) throw new Error('secret') },
  } })
  render(<DeviceCodeCopy code="ABCD" t={t} />)
  await act(async () => fireEvent.click(screen.getByRole('button')))
  expect(screen.getByRole('alert').textContent).toBe('copyFailed')
  expect(screen.queryByText('secret')).toBeNull()
  fail = false
  await act(async () => fireEvent.click(screen.getByRole('button')))
  expect(screen.getByText('copied')).toBeTruthy()
  expect(screen.queryByRole('alert')).toBeNull()
})
