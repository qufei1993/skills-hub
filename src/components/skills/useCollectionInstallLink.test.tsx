// @vitest-environment jsdom
import { act, cleanup, renderHook, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { getCurrent, onOpenUrl } from '@tauri-apps/plugin-deep-link'
import { message } from '@tauri-apps/plugin-dialog'
import { toast } from 'sonner'
import { useCollectionInstallLink } from './useCollectionInstallLink'
import i18n from '../../i18n'

vi.mock('@tauri-apps/plugin-deep-link', () => ({ getCurrent: vi.fn(), onOpenUrl: vi.fn() }))
vi.mock('@tauri-apps/plugin-dialog', () => ({ message: vi.fn() }))
const parsed = { v: 1, title: 'Collection', sources: [{ repo: 'author/repo', ref: 'a'.repeat(40) }], skills: [{ name: 'design', path: 'skills/design', source: 0 }] }
let deliver: (urls: string[]) => void
beforeEach(async () => {
  await i18n.changeLanguage('zh')
  vi.mocked(message).mockResolvedValue('Ok')
  vi.mocked(getCurrent).mockResolvedValue(null)
  vi.mocked(onOpenUrl).mockImplementation(async callback => { deliver = callback; return () => {} })
})
afterEach(() => { cleanup(); vi.clearAllMocks(); vi.restoreAllMocks() })
describe('collection link reception', () => {
  it.each([
    ['COLLECTION_LINK_TOO_LARGE', '安装清单太大', '分批安装'],
    ['COLLECTION_LINK_TOO_MANY_SKILLS', '一次选择的 Skills 太多', '2,000'],
    ['COLLECTION_LINK_TOO_MANY_SOURCES', '一次选择的来源仓库太多', '100'],
    ['COLLECTION_LINK_UNSUPPORTED_VERSION', '当前版本无法读取这份安装清单', '更新 Skills Hub'],
    ['COLLECTION_LINK_EMPTY', '还没有选择要安装的 Skills', '至少勾选一个 Skill'],
    ['COLLECTION_LINK_DUPLICATE', '安装清单中有重名或重复的 Skills', '取消重名或重复的项目'],
    ['COLLECTION_LINK_UNSAFE', '这个合集的安装信息有误', '反馈合集名称'],
    ['COLLECTION_LINK_INVALID', '无法读取安装清单', '重新选择 Skills'],
  ])('explains %s and its recovery action until the user closes the dialog', async (code, title, recovery) => {
    const calls: string[] = []
    const { result } = renderHook(() => useCollectionInstallLink(true, async command => {
      calls.push(command)
      throw code
    }))
    await act(async () => deliver(['skills-hub://install?manifest=bad']))
    await waitFor(() => expect(message).toHaveBeenCalledWith(expect.stringContaining(recovery), {
      title, kind: 'error', buttons: { ok: '关闭' },
    }))
    expect(result.current.manifest).toBeNull()
    expect(calls).toEqual(['parse_collection_link'])
  })
  it('does not expose unexpected backend error details in the user-facing explanation', async () => {
    renderHook(() => useCollectionInstallLink(true, async () => { throw new Error('private-token-example') }))
    await act(async () => deliver(['skills-hub://install?manifest=bad']))
    await waitFor(() => expect(message).toHaveBeenCalledWith(expect.stringContaining('重启 Skills Hub'), {
      title: '暂时无法打开安装确认', kind: 'error', buttons: { ok: '关闭' },
    }))
    expect(JSON.stringify(vi.mocked(message).mock.calls)).not.toContain('private-token-example')
  })
  it.each([
    ['en', 'The installation list is too large', 'batches', 'Close'],
    ['ko', '설치 목록이 너무 큽니다', '나누어', '닫기'],
  ])('localizes the explanation and recovery action in %s', async (language, title, recovery, close) => {
    await i18n.changeLanguage(language)
    renderHook(() => useCollectionInstallLink(true, async () => { throw new Error('COLLECTION_LINK_TOO_LARGE') }))
    await act(async () => deliver(['skills-hub://install?manifest=large']))
    await waitFor(() => expect(message).toHaveBeenCalledWith(expect.stringContaining(recovery), {
      title, kind: 'error', buttons: { ok: close },
    }))
  })
  it('explains a link receiver failure with a restart action', async () => {
    vi.mocked(getCurrent).mockRejectedValueOnce(new Error('receiver failed'))
    renderHook(() => useCollectionInstallLink(true, async <T,>() => parsed as T))
    await waitFor(() => expect(message).toHaveBeenCalledWith(expect.stringContaining('重启 Skills Hub'), {
      title: '无法接收官网的安装请求', kind: 'error', buttons: { ok: '关闭' },
    }))
  })
  it('retains the reason and recovery action if the native error dialog cannot open', async () => {
    vi.mocked(message).mockRejectedValueOnce(new Error('dialog unavailable'))
    const notification = vi.spyOn(toast, 'error')
    renderHook(() => useCollectionInstallLink(true, async () => { throw 'COLLECTION_LINK_TOO_LARGE' }))
    await act(async () => deliver(['skills-hub://install?manifest=large']))
    await waitFor(() => expect(notification).toHaveBeenCalledWith('安装清单太大', {
      description: expect.stringContaining('分批安装'), duration: Infinity, closeButton: true,
    }))
  })
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
