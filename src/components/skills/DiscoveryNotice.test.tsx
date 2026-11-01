// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, expect, it, vi } from 'vitest'
import type { TFunction } from 'i18next'
import DiscoveryNotice from './DiscoveryNotice'

const t = ((key: string) => key) as TFunction
const props = () => ({ count: 3, loading: false, onReviewImport: vi.fn(), onOpenScanSettings: vi.fn(), t })

afterEach(() => {
  cleanup()
  vi.restoreAllMocks()
  localStorage.clear()
})

it('does not show an import notice when no Skills have been detected', () => {
  render(<DiscoveryNotice {...props()} count={0} />)
  expect(screen.queryByRole('complementary')).toBeNull()
  expect(screen.queryByRole('button')).toBeNull()
})

it('keeps dismissal when remounted and does not invoke import or scan settings on close', () => {
  const callbacks = props()
  const notice = render(<DiscoveryNotice {...callbacks} />)
  expect(screen.getByRole('complementary', { name: 'discoveredTitle' })).toBeTruthy()
  fireEvent.click(screen.getByRole('button', { name: 'discovery.dismissNotice' }))
  expect(screen.queryByRole('complementary')).toBeNull()
  notice.unmount()
  render(<DiscoveryNotice {...callbacks} />)
  expect(screen.queryByRole('complementary')).toBeNull()
  expect(callbacks.onReviewImport).not.toHaveBeenCalled()
  expect(callbacks.onOpenScanSettings).not.toHaveBeenCalled()
})

it('still renders and closes for the current page if local storage is unavailable', () => {
  vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => { throw new Error('Storage unavailable') })
  vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => { throw new Error('Storage unavailable') })
  const callbacks = props()
  render(<DiscoveryNotice {...callbacks} />)
  fireEvent.click(screen.getByRole('button', { name: 'reviewImport' }))
  fireEvent.click(screen.getByRole('button', { name: 'discoveryScan.action' }))
  expect(callbacks.onReviewImport).toHaveBeenCalledOnce()
  expect(callbacks.onOpenScanSettings).toHaveBeenCalledOnce()
  fireEvent.click(screen.getByRole('button', { name: 'discovery.dismissNotice' }))
  expect(screen.queryByRole('complementary')).toBeNull()
})

it('disables import and scan changes during loading but still allows dismissing the notice', () => {
  const callbacks = props()
  render(<DiscoveryNotice {...callbacks} loading />)
  const importButton = screen.getByRole('button', { name: 'reviewImport' }) as HTMLButtonElement
  const scanButton = screen.getByRole('button', { name: 'discoveryScan.action' }) as HTMLButtonElement
  expect(importButton.disabled).toBe(true)
  expect(scanButton.disabled).toBe(true)
  fireEvent.click(importButton)
  fireEvent.click(scanButton)
  expect(callbacks.onReviewImport).not.toHaveBeenCalled()
  expect(callbacks.onOpenScanSettings).not.toHaveBeenCalled()
  fireEvent.click(screen.getByRole('button', { name: 'discovery.dismissNotice' }))
  expect(screen.queryByRole('complementary')).toBeNull()
})
