// @vitest-environment jsdom
import { act, cleanup, renderHook, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { getCurrent, onOpenUrl } from '@tauri-apps/plugin-deep-link'
import { useCollectionInstallLink } from './useCollectionInstallLink'
import i18n from '../../i18n'

vi.mock('@tauri-apps/plugin-deep-link', () => ({ getCurrent: vi.fn(), onOpenUrl: vi.fn() }))
const parsed = { v: 1, title: 'Collection', sources: [{ repo: 'author/repo', ref: 'a'.repeat(40) }], skills: [{ name: 'design', path: 'skills/design', source: 0 }] }
let deliver: (urls: string[]) => void
beforeEach(() => {
  vi.mocked(getCurrent).mockResolvedValue(null)
  vi.mocked(onOpenUrl).mockImplementation(async callback => { deliver = callback; return () => {} })
})
afterEach(() => { cleanup(); vi.clearAllMocks() })
describe('collection link reception', () => {
  it('validates a cold-start link without making installation calls, and does not replay it on language change', async () => {
    vi.mocked(getCurrent).mockResolvedValue(['skills-hub://install?manifest=test'])
    const calls: string[] = []
    const invoke = async <T,>(command: string) => { calls.push(command); return parsed as T }
    const { result } = renderHook(() => useCollectionInstallLink(true, invoke))
    await waitFor(() => expect(result.current.manifest?.title).toBe('Collection'))
    expect(calls).toEqual(['parse_collection_link'])
    act(() => result.current.dismiss())
    await act(() => i18n.changeLanguage('en'))
    expect(result.current.manifest).toBeNull()
    expect(calls).toEqual(['parse_collection_link'])
  })
  it('keeps the displayed collection when a second link arrives, and accepts a new request after closing', async () => {
    const calls: string[] = []
    const invoke = async <T,>(command: string) => { calls.push(command); return parsed as T }
    const { result } = renderHook(() => useCollectionInstallLink(true, invoke))
    await waitFor(() => expect(deliver).toBeDefined())
    await act(async () => deliver(['skills-hub://install?manifest=first']))
    await act(async () => deliver(['skills-hub://install?manifest=second']))
    expect(calls).toEqual(['parse_collection_link'])
    act(() => result.current.dismiss())
    await act(async () => deliver(['skills-hub://install?manifest=second']))
    expect(calls).toEqual(['parse_collection_link', 'parse_collection_link'])
  })
  it('hands off from an existing add-Skills view only after validating the website request', async () => {
    const accepted = vi.fn()
    const invoke = vi.fn().mockRejectedValueOnce(new Error('COLLECTION_LINK_INVALID')).mockResolvedValue(parsed)
    const { result } = renderHook(() => useCollectionInstallLink(true, invoke, accepted))
    await waitFor(() => expect(deliver).toBeDefined())
    await act(async () => deliver(['skills-hub://install?manifest=bad']))
    expect(accepted).not.toHaveBeenCalled()
    await act(async () => deliver(['skills-hub://install?manifest=valid']))
    expect(accepted).toHaveBeenCalledTimes(1)
    expect(result.current.manifest).toEqual(parsed)
  })
  it('ignores invalid requests without opening the install dialog', async () => {
    const { result } = renderHook(() => useCollectionInstallLink(true, async () => { throw new Error('COLLECTION_LINK_INVALID') }))
    await waitFor(() => expect(deliver).toBeDefined())
    await act(async () => deliver(['skills-hub://install?manifest=bad']))
    expect(result.current.manifest).toBeNull()
  })
})
