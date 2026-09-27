// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, expect, it, vi } from 'vitest'
import type { TFunction } from 'i18next'
import AiManagementNotice from './AiManagementNotice'
import type { AgentAccessStatusDto } from './types'

const t = ((key: string) => key) as TFunction
const missing = { installed: false, officialState: 'missing' } as AgentAccessStatusDto
const skills: [] = []
afterEach(() => { cleanup(); localStorage.clear(); vi.restoreAllMocks() })

it('waits for a successful status read, then navigates without installing anything', async () => {
  let resolve!: (status: AgentAccessStatusDto) => void
  const invoke = vi.fn(() => new Promise<AgentAccessStatusDto>(done => { resolve = done }))
  const open = vi.fn()
  render(<AiManagementNotice isTauri invokeTauri={invoke} skills={skills} onOpen={open} t={t} />)
  expect(screen.queryByRole('button')).toBeNull()
  await act(async () => resolve(missing))
  fireEvent.click(screen.getByRole('button', { name: 'aiManagement.notice.action' }))
  expect(open).toHaveBeenCalledOnce()
  expect(invoke.mock.calls).toEqual([['get_agent_access_status']])
})

it('remembers dismissal when the library page is reopened', async () => {
  const props = { isTauri: true, invokeTauri: vi.fn(async () => missing), skills, onOpen: vi.fn(), t }
  const view = render(<AiManagementNotice {...props} />)
  fireEvent.click(await screen.findByRole('button', { name: 'aiManagement.notice.dismiss' }))
  view.unmount()
  render(<AiManagementNotice {...props} />)
  expect(screen.queryByRole('button')).toBeNull()
  expect(props.invokeTauri).toHaveBeenCalledTimes(1)
})

it.each(['healthy', 'needs_repair', 'name_conflict'] as const)('does not advertise setup for %s installations', async officialState => {
  const invoke = vi.fn(async () => ({ ...missing, installed: true, officialState }))
  render(<AiManagementNotice isTauri invokeTauri={invoke} skills={skills} onOpen={() => {}} t={t} />)
  await waitFor(() => expect(invoke).toHaveBeenCalledOnce())
  expect(screen.queryByRole('button')).toBeNull()
})

it('hides a same-name unmanaged conflict even when the official Skill is not installed', async () => {
  const invoke = vi.fn(async () => ({ ...missing, officialState: 'name_conflict' }))
  render(<AiManagementNotice isTauri invokeTauri={invoke} skills={skills} onOpen={() => {}} t={t} />)
  await act(async () => {})
  expect(screen.queryByRole('button')).toBeNull()
})

it('hides on a failed read and rechecks on focus after external installation', async () => {
  const invoke = vi.fn().mockRejectedValueOnce(new Error('unavailable')).mockResolvedValueOnce(missing).mockResolvedValue({ ...missing, installed: true, officialState: 'healthy' })
  render(<AiManagementNotice isTauri invokeTauri={invoke} skills={skills} onOpen={() => {}} t={t} />)
  await act(async () => {})
  expect(screen.queryByRole('button')).toBeNull()
  fireEvent.focus(window)
  await screen.findByRole('button', { name: 'aiManagement.notice.action' })
  fireEvent.focus(window)
  await waitFor(() => expect(screen.queryByRole('button')).toBeNull())
})

it('ignores a late missing response after a newer read finds an installation', async () => {
  let resolveOld!: (status: AgentAccessStatusDto) => void
  const invoke = vi.fn().mockImplementationOnce(() => new Promise<AgentAccessStatusDto>(resolve => { resolveOld = resolve }))
    .mockResolvedValue({ ...missing, installed: true, officialState: 'healthy' })
  render(<AiManagementNotice isTauri invokeTauri={invoke} skills={skills} onOpen={() => {}} t={t} />)
  fireEvent.focus(window)
  await act(async () => {})
  await act(async () => resolveOld(missing))
  expect(screen.queryByRole('button')).toBeNull()
})

it('rechecks after library changes and does not read status in a browser preview', async () => {
  const invoke = vi.fn(async () => missing)
  const props = { invokeTauri: invoke, skills, onOpen: vi.fn(), t }
  const view = render(<AiManagementNotice {...props} isTauri={false} />)
  expect(invoke).not.toHaveBeenCalled()
  view.rerender(<AiManagementNotice {...props} isTauri />)
  await screen.findByRole('button', { name: 'aiManagement.notice.action' })
  view.rerender(<AiManagementNotice {...props} isTauri skills={[]} />)
  await waitFor(() => expect(invoke).toHaveBeenCalledTimes(2))
})
