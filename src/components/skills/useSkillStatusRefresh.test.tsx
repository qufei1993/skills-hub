// @vitest-environment jsdom
import { act, cleanup, renderHook } from '@testing-library/react'
import { afterEach, expect, it, vi } from 'vitest'
import type { AutoUpdateConfigDto } from './types'
import { useSkillStatusRefresh } from './useSkillStatusRefresh'

afterEach(() => { cleanup(); vi.useRealTimers() })
it('refreshes on initial finished result, background progress and window focus', async () => {
  vi.useFakeTimers()
  let config = { last_run_at: 100, last_status: 'ok', progress: { total: 0, succeeded: [], failed: [], pending: [], running: null } } as unknown as AutoUpdateConfigDto
  const read = async () => config
  let refreshes = 0
  const refresh = async () => { refreshes++ }
  const receive = () => {}
  const { unmount } = renderHook(() => useSkillStatusRefresh(true, read, receive, refresh))
  await act(async () => {})
  expect(refreshes).toBe(1)
  await act(async () => { await vi.advanceTimersByTimeAsync(5000) })
  expect(refreshes).toBe(1)
  config = { ...config, last_run_at: 200, last_status: 'error' }
  await act(async () => { await vi.advanceTimersByTimeAsync(5000) })
  expect(refreshes).toBe(2)
  await act(async () => { window.dispatchEvent(new Event('focus')) })
  expect(refreshes).toBe(3)
  unmount()
  await act(async () => { await vi.advanceTimersByTimeAsync(5000); window.dispatchEvent(new Event('focus')) })
  expect(refreshes).toBe(3)
})
