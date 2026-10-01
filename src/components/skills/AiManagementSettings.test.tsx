// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { useState } from 'react'
import { afterEach, expect, it, vi } from 'vitest'
import { createInstance, type TFunction } from 'i18next'
import { resources } from '../../i18n/resources'
import AiManagementSettings from './AiManagementSettings'
import type { AgentAccessStatusDto } from './types'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

const eventMock = vi.hoisted(() => ({ listen: vi.fn().mockResolvedValue(() => {}) }))
vi.mock('@tauri-apps/api/event', () => eventMock)

const t = ((key: string) => key) as TFunction
const fixture = (): AgentAccessStatusDto => ({
  officialState: 'missing', conflict: null, skillId: null, skillEnabled: true, terminalReady: true, terminalPathConflict: false,
  bridge: { status: 'valid', reason: null, path: '/test/bin/skillshub-cli', version: '0.11.0' },
  bundledVersion: '0.11.0', installedVersion: null, installed: false, centralReason: null,
  agents: [{ key: 'codex', label: 'Codex', detected: true, enabled: true, deployed: false, needsRepair: false, reason: null, path: '/test/.codex/skills' }],
})
afterEach(() => { cleanup(); vi.restoreAllMocks() })

it('explains shared directory scope expansion without reporting enabled', async () => {
  const invoke = vi.fn(async (command: string) => {
    if (command === 'enable_ai_management') throw new Error('SHARED_DIRECTORY_SCOPE_EXPANSION')
    return fixture()
  })
  render(<AiManagementSettings isTauri invokeTauri={invoke} onChanged={() => {}} onOpenSkill={() => {}} t={t} />)
  fireEvent.click(await screen.findByRole('button', { name: 'aiManagement.enable' }))
  expect((await screen.findByRole('alert')).textContent).toContain('aiManagement.errors.sharedDirectory')
  expect(screen.queryByText('aiManagement.ready')).toBeNull()
})

it.each([false, true])('retains the last result on reentry and handles refresh failure=%s', async fails => {
  const ready = fixture()
  Object.assign(ready, { officialState: 'healthy', installed: true, skillId: 'id' })
  ready.agents[0].deployed = true
  let resolveRefresh!: (value: AgentAccessStatusDto) => void
  let rejectRefresh!: (reason: Error) => void
  const invoke = vi.fn().mockResolvedValueOnce(ready).mockImplementationOnce(() => new Promise((resolve, reject) => {
    resolveRefresh = resolve
    rejectRefresh = reject
  }))
  const open = vi.fn()
  function Host() {
    const [visible, setVisible] = useState(true)
    const [status, setStatus] = useState<AgentAccessStatusDto | null>(null)
    return <><button onClick={() => setVisible(value => !value)}>toggle</button>{visible && <AiManagementSettings isTauri invokeTauri={invoke} initialStatus={status} onStatusChanged={setStatus} onChanged={() => {}} onOpenSkill={open} t={t} />}</>
  }
  render(<Host />)
  expect(screen.getByText('agentAccess.loading')).toBeTruthy()
  await screen.findByText('aiManagement.ready')
  fireEvent.click(screen.getByText('toggle'))
  fireEvent.click(screen.getByText('toggle'))
  expect(screen.getByText('aiManagement.ready')).toBeTruthy()
  expect(screen.queryByText('agentAccess.loading')).toBeNull()
  const viewButton = screen.getByRole('button', { name: 'aiManagement.viewSkill' }) as HTMLButtonElement
  expect(viewButton.disabled).toBe(false)
  fireEvent.click(viewButton)
  expect(open).toHaveBeenCalledWith('id')
  await act(async () => {
    if (fails) rejectRefresh(new Error('offline'))
    else resolveRefresh(fixture())
  })
  if (fails) {
    expect(screen.getByText('aiManagement.ready')).toBeTruthy()
    expect(screen.getByRole('alert').textContent).toContain('aiManagement.errors.read')
  } else {
    expect(screen.queryByText('aiManagement.ready')).toBeNull()
    expect(screen.getByRole('button', { name: 'aiManagement.enable' })).toBeTruthy()
  }
})

it('keeps the action beside the heading without a divided empty body', async () => {
  render(<><style>{readFileSync(resolve(process.cwd(), 'src/App.css'), 'utf8')}</style><AiManagementSettings isTauri invokeTauri={async () => fixture()} onChanged={() => {}} onOpenSkill={() => {}} t={t} /></>)
  const button = await screen.findByRole('button', { name: 'aiManagement.enable' })
  const header = screen.getByRole('heading', { name: 'aiManagement.title' }).closest('.settings-card-head')!
  expect(header.contains(button)).toBe(true)
  expect(getComputedStyle(header).borderBottomWidth).toBe('0px')
  expect(getComputedStyle(header).paddingBottom).toBe('0px')
  const details = document.querySelector('details')!
  fireEvent.click(screen.getByText('aiManagement.details'))
  expect(details.open).toBe(true)
  expect(details.querySelector('button')).toBeNull()
})

it('keeps the feature description after enabling and offers only skill navigation', async () => {
  const state = fixture()
  Object.assign(state, { officialState: 'healthy', installed: true, skillId: 'id' })
  state.agents[0].deployed = true
  render(<AiManagementSettings isTauri invokeTauri={async () => state} onChanged={() => {}} onOpenSkill={() => {}} t={t} />)
  const ready = await screen.findByText('aiManagement.ready')
  expect(screen.getByRole('heading').parentElement?.contains(ready)).toBe(true)
  expect(screen.getByText('aiManagement.description')).toBeTruthy()
  expect(screen.queryByText('aiManagement.readyHint')).toBeNull()
  expect(screen.queryByRole('button', { name: 'aiManagement.copyExample' })).toBeNull()
  expect(screen.getByRole('button', { name: 'aiManagement.viewSkill' })).toBeTruthy()
})

it('enables once, refreshes the library, and opens the installed official skill', async () => {
  const ready = fixture()
  Object.assign(ready, { officialState: 'healthy', installed: true, skillId: 'official-id' })
  ready.agents[0].deployed = true
  const invoke = vi.fn(async (command: string) => command === 'enable_ai_management' ? ready : fixture())
  const changed = vi.fn()
  const open = vi.fn()
  render(<AiManagementSettings isTauri invokeTauri={invoke} onChanged={changed} onOpenSkill={open} t={t} />)
  const button = await screen.findByRole('button', { name: 'aiManagement.enable' })
  fireEvent.click(button)
  fireEvent.click(button)
  await screen.findByText('aiManagement.ready')
  expect(invoke.mock.calls.filter(([command]) => command === 'enable_ai_management')).toHaveLength(1)
  expect(changed).toHaveBeenCalledOnce()
  fireEvent.click(screen.getByRole('button', { name: 'aiManagement.viewSkill' }))
  expect(open).toHaveBeenCalledWith('official-id')
  expect(screen.queryByRole('table')).toBeNull()
})

it.each(['missing', 'damaged'] as const)('does not report ready with a %s CLI', async health => {
  const state = fixture()
  Object.assign(state, { officialState: 'healthy', installed: true, skillId: 'id' })
  state.agents[0].deployed = true
  state.bridge.status = health
  render(<AiManagementSettings isTauri invokeTauri={async () => state} onChanged={() => {}} onOpenSkill={() => {}} t={t} />)
  await screen.findByRole('button', { name: 'aiManagement.enable' })
  expect(screen.queryByText('aiManagement.ready')).toBeNull()
})

it('keeps user-disabled or unsynced skills in normal skill management', async () => {
  const state = fixture()
  Object.assign(state, { officialState: 'healthy', installed: true, skillId: 'id' })
  render(<AiManagementSettings isTauri invokeTauri={async () => state} onChanged={() => {}} onOpenSkill={() => {}} t={t} />)
  await screen.findByText('aiManagement.inactive')
  expect(screen.queryByRole('button', { name: 'aiManagement.enable' })).toBeNull()
})

it('does not show a disabled official skill as ready even if targets remain', async () => {
  const state = { ...fixture(), officialState: 'healthy' as const, installed: true, skillEnabled: false, skillId: 'id', terminalPathConflict: true }
  state.agents[0].deployed = true
  render(<AiManagementSettings isTauri invokeTauri={async () => state} onChanged={() => {}} onOpenSkill={() => {}} t={t} />)
  await screen.findByText('aiManagement.inactive')
  expect(screen.queryByText('aiManagement.ready')).toBeNull()
})

it('shows recoverable errors without claiming success', async () => {
  const invoke = vi.fn(async (command: string) => {
    if (command === 'enable_ai_management') throw new Error('CLI_UNAVAILABLE')
    return fixture()
  })
  render(<AiManagementSettings isTauri invokeTauri={invoke} onChanged={() => {}} onOpenSkill={() => {}} t={t} />)
  fireEvent.click(await screen.findByRole('button', { name: 'aiManagement.enable' }))
  await screen.findByRole('alert')
  expect(screen.getByRole('alert').textContent).toContain('aiManagement.errors.cli')
  expect(screen.queryByText('aiManagement.ready')).toBeNull()
})

it.each(['en', 'zh', 'ko'])('has translated primary copy and collapsed technical details in %s', async lng => {
  const i18n = createInstance()
  await i18n.init({ resources, lng, fallbackLng: false })
  render(<AiManagementSettings isTauri invokeTauri={async () => fixture()} onChanged={() => {}} onOpenSkill={() => {}} t={i18n.t.bind(i18n)} />)
  await waitFor(() => expect(document.querySelector('details')).not.toBeNull())
  expect(document.querySelector('details')?.open).toBe(false)
  expect(document.body.textContent).not.toMatch(/aiManagement\.|npm|Node\.js/)
})

it('focuses and scrolls to AI management when opened from the library notice', async () => {
  const scroll = vi.fn()
  const original = HTMLElement.prototype.scrollIntoView
  HTMLElement.prototype.scrollIntoView = scroll
  try {
    render(<AiManagementSettings focusOnMount isTauri invokeTauri={async () => fixture()} onChanged={() => {}} onOpenSkill={() => {}} t={t} />)
    const card = screen.getByRole('region', { name: 'aiManagement.title' })
    expect(document.activeElement).toBe(card)
    expect(scroll).toHaveBeenCalledWith({ block: 'center', behavior: 'instant' })
    await screen.findByRole('button', { name: 'aiManagement.enable' })
  } finally { HTMLElement.prototype.scrollIntoView = original }
})

it.each(['missing', 'damaged'] as const)('only installs the CLI after an explicit click when bridge is %s', async bridgeState => {
  const status = fixture()
  status.bridge.status = bridgeState
  status.bridge.reason = bridgeState === 'damaged' ? 'VERSION_MISMATCH' : 'DIRECTORY_MISSING'
  const invoke = vi.fn(async (command: string) => {
    expect(['get_agent_access_status', 'enable_ai_management']).toContain(command)
    return status
  })
  render(<AiManagementSettings isTauri invokeTauri={invoke} onChanged={() => {}} onOpenSkill={() => {}} t={t} />)
  const button = await screen.findByRole('button', { name: 'aiManagement.enable' })
  expect(invoke.mock.calls.every(args => args[0] === 'get_agent_access_status')).toBe(true)
  fireEvent.click(button)
  await waitFor(() => expect(invoke).toHaveBeenCalledWith('enable_ai_management', { operationId: expect.any(String) }))
})

it('marks a previously enabled older CLI as pending update', async () => {
  const status = fixture()
  Object.assign(status, { officialState: 'healthy', installed: true, installedVersion: '0.10.0' })
  status.bridge.status = 'damaged'
  status.bridge.reason = 'VERSION_MISMATCH'
  status.agents[0].deployed = true
  render(<AiManagementSettings isTauri invokeTauri={async () => status} onChanged={() => {}} onOpenSkill={() => {}} t={t} />)
  await screen.findByRole('button', { name: 'aiManagement.update' })
  expect(screen.getByText('aiManagement.updatePending')).toBeTruthy()
  expect(screen.queryByText('aiManagement.ready')).toBeNull()
})

it('offers retry when only the official Skill version is older', async () => {
  const status = fixture()
  Object.assign(status, { officialState: 'healthy', installed: true, installedVersion: '0.10.0' })
  status.agents[0].deployed = true
  render(<AiManagementSettings isTauri invokeTauri={async () => status} onChanged={() => {}} onOpenSkill={() => {}} t={t} />)
  await screen.findByRole('button', { name: 'aiManagement.update' })
  expect(screen.queryByText('aiManagement.ready')).toBeNull()
})

it('keeps first-time setup available when the current PATH has an older CLI', async () => {
  const status = { ...fixture(), terminalPathConflict: true }
  render(<AiManagementSettings isTauri invokeTauri={async () => status} onChanged={() => {}} onOpenSkill={() => {}} t={t} />)
  await screen.findByRole('button', { name: 'aiManagement.enable' })
})

it('warns when another CLI precedes the managed CLI in the current PATH', async () => {
  const status = fixture()
  Object.assign(status, { officialState: 'healthy', installed: true, installedVersion: status.bundledVersion, terminalPathConflict: true })
  status.agents[0].deployed = true
  render(<AiManagementSettings isTauri invokeTauri={async () => status} onChanged={() => {}} onOpenSkill={() => {}} t={t} />)
  await screen.findByText('aiManagement.pathConflict')
  expect(screen.queryByText('aiManagement.ready')).toBeNull()
})

it('offers setup for an already enabled Skill whose terminal command is not configured', async () => {
  const status = fixture()
  Object.assign(status, { officialState: 'healthy', installed: true, terminalReady: false })
  status.agents[0].deployed = true
  const invoke = vi.fn().mockResolvedValue(status)
  render(<AiManagementSettings isTauri invokeTauri={invoke} onChanged={() => {}} onOpenSkill={() => {}} t={t} />)
  expect(await screen.findByRole('button', { name: 'aiManagement.enable' })).toBeTruthy()
})

it('reports terminal setup failure and leaves the enable action available for retry', async () => {
  const status = { ...fixture(), terminalReady: false }
  const invoke = vi.fn(async (command: string) => {
    if (command === 'enable_ai_management') throw new Error('CLI_TERMINAL_UNAVAILABLE')
    return status
  })
  render(<AiManagementSettings isTauri invokeTauri={invoke} onChanged={() => {}} onOpenSkill={() => {}} t={t} />)
  fireEvent.click(await screen.findByRole('button', { name: 'aiManagement.enable' }))
  expect((await screen.findByRole('alert')).textContent).toContain('aiManagement.errors.terminal')
  expect((screen.getByRole('button', { name: 'aiManagement.enable' }) as HTMLButtonElement).disabled).toBe(false)
})

it('refreshes the installed state after a later setup step fails', async () => {
  const before = fixture()
  const after = { ...before, officialState: 'healthy' as const, installed: true, skillId: 'official', terminalReady: false }
  after.agents = [{ ...before.agents[0], deployed: true }]
  let reads = 0
  const invoke = vi.fn(async (command: string) => {
    if (command === 'enable_ai_management') throw new Error('CLI_TERMINAL_UNAVAILABLE')
    return ++reads === 1 ? before : after
  })
  const changed = vi.fn()
  render(<AiManagementSettings isTauri invokeTauri={invoke} onChanged={changed} onOpenSkill={() => {}} t={t} />)
  fireEvent.click(await screen.findByRole('button', { name: 'aiManagement.enable' }))
  await screen.findByRole('button', { name: 'aiManagement.viewSkill' })
  expect(screen.getByRole('alert').textContent).toContain('aiManagement.errors.terminal')
  expect(changed).toHaveBeenCalledOnce()
})

it('subscribes only on click and ignores unrelated download progress', async () => {
  let handler!: (event: { payload: { operationId: string; phase: string; downloadedBytes: number; totalBytes: number | null } }) => void
  const unsubscribe = vi.fn()
  eventMock.listen.mockImplementationOnce(async (_event: string, callback: typeof handler) => { handler = callback; return unsubscribe })
  let finish!: (value: AgentAccessStatusDto) => void
  const invoke = vi.fn<(command: string, args?: Record<string, unknown>) => Promise<AgentAccessStatusDto>>((command) => command === 'enable_ai_management' ? new Promise<AgentAccessStatusDto>(resolve => { finish = resolve }) : Promise.resolve(fixture()))
  const { unmount } = render(<AiManagementSettings isTauri invokeTauri={invoke} onChanged={() => {}} onOpenSkill={() => {}} t={t} />)
  const button = await screen.findByRole('button', { name: 'aiManagement.enable' })
  const previousCalls = eventMock.listen.mock.calls.length
  fireEvent.click(button)
  fireEvent.click(button)
  await waitFor(() => expect(invoke.mock.calls.filter(([name]) => name === 'enable_ai_management')).toHaveLength(1))
  expect(eventMock.listen.mock.calls.length).toBe(previousCalls + 1)
  const operationId = invoke.mock.calls.find(([name]) => name === 'enable_ai_management')![1]!.operationId as string
  expect(operationId).toMatch(/^[0-9a-f-]{36}$/)
  await act(async () => handler({ payload: { operationId: 'other', phase: 'downloading', downloadedBytes: 50, totalBytes: 100 } }))
  expect(screen.queryByText('aiManagement.progress.downloading')).toBeNull()
  await act(async () => handler({ payload: { operationId, phase: 'downloading', downloadedBytes: 50, totalBytes: 100 } }))
  expect(screen.getByRole('progressbar').getAttribute('value')).toBe('50')
  expect((screen.getByRole('button', { name: 'aiManagement.enabling' }) as HTMLButtonElement).disabled).toBe(true)
  await act(async () => handler({ payload: { operationId, phase: 'downloading', downloadedBytes: 50, totalBytes: null } }))
  expect(screen.queryByRole('progressbar')).toBeNull()
  expect(screen.getByRole('status').textContent).toContain('aiManagement.progress.bytes')
  unmount()
  expect(unsubscribe).toHaveBeenCalledOnce()
  await act(async () => { finish(fixture()) })
  expect(unsubscribe).toHaveBeenCalledOnce()
})

it.each(['CLI_DOWNLOAD_FAILED', 'CLI_DOWNLOAD_UNAVAILABLE', 'CLI_INTEGRITY_FAILED'])('offers retry after %s without reporting ready', async code => {
  const state = fixture()
  state.bridge = { ...state.bridge, status: 'damaged', reason: 'HASH_MISMATCH', version: null }
  const invoke = vi.fn(async (command: string) => {
    if (command === 'enable_ai_management') throw new Error(code)
    return state
  })
  render(<AiManagementSettings isTauri invokeTauri={invoke} onChanged={() => {}} onOpenSkill={() => {}} t={t} />)
  fireEvent.click(await screen.findByRole('button', { name: 'aiManagement.enable' }))
  expect((await screen.findByRole('alert')).textContent).toContain(code === 'CLI_INTEGRITY_FAILED' ? 'aiManagement.errors.integrity' : code === 'CLI_DOWNLOAD_UNAVAILABLE' ? 'aiManagement.errors.unavailable' : 'aiManagement.errors.download')
  expect(screen.queryByText('aiManagement.ready')).toBeNull()
  const details = document.querySelector('details')!
  const key = code === 'CLI_INTEGRITY_FAILED' ? 'integrity' : code === 'CLI_DOWNLOAD_UNAVAILABLE' ? 'unavailable' : 'download'
  expect(details.textContent).toContain(`aiManagement.errors.${key}`)
  if (key !== 'unavailable') expect(details.textContent).not.toContain('aiManagement.errors.unavailable')
  fireEvent.click(screen.getByRole('button', { name: 'aiManagement.enable' }))
  await waitFor(() => expect(invoke.mock.calls.filter(([name]) => name === 'enable_ai_management')).toHaveLength(2))
})


it.each(['en', 'zh', 'ko'])('connects unavailable CLI resources to the blocked installation in %s', async lng => {
  const i18n = createInstance()
  await i18n.init({ lng, resources, fallbackLng: false })
  const state = fixture()
  state.bridge = { ...state.bridge, status: 'damaged', reason: 'HASH_MISMATCH', version: null }
  const invoke = vi.fn(async (command: string) => {
    if (command === 'enable_ai_management') throw new Error('CLI_DOWNLOAD_UNAVAILABLE')
    return state
  })
  render(<AiManagementSettings isTauri invokeTauri={invoke} onChanged={() => {}} onOpenSkill={() => {}} t={i18n.t} />)
  fireEvent.click(await screen.findByRole('button', { name: i18n.t('aiManagement.enable') }))
  const alert = await screen.findByRole('alert')
  const explanation = i18n.t('aiManagement.errors.unavailable')
  expect(alert.textContent).toBe(explanation)
  const details = document.querySelector('details')!
  await waitFor(() => expect(details.textContent).toContain(i18n.t('aiManagement.cliInstallationFailure')))
  expect(details.textContent).toContain(explanation)
  expect(details.textContent).toContain(i18n.t('aiManagement.cliNeedsInstallation'))
  expect(details.textContent).not.toContain(i18n.t('agentAccess.reason.HASH_MISMATCH'))
  expect(explanation).not.toContain('aiManagement.errors.')
  if (lng === 'zh') {
    expect(explanation).toContain('草稿')
    expect(explanation).toContain('公开发布')
    expect(explanation).not.toContain('稍后重试')
  }
})

it('keeps terminal repair available when a PATH conflict is also present', async () => {
  const state = fixture()
  Object.assign(state, { officialState: 'healthy', installed: true, terminalReady: false, terminalPathConflict: true })
  state.agents[0].deployed = true
  render(<AiManagementSettings isTauri invokeTauri={async () => state} onChanged={() => {}} onOpenSkill={() => {}} t={t} />)
  expect(await screen.findByRole('button', { name: 'aiManagement.enable' })).toBeTruthy()
})

it('does not report setup failure when only the library refresh fails after successful enabling', async () => {
  const ready = fixture()
  Object.assign(ready, { officialState: 'healthy', installed: true, skillId: 'official' })
  ready.agents[0].deployed = true
  const invoke = vi.fn(async (command: string) => command === 'enable_ai_management' ? ready : fixture())
  const changed = vi.fn().mockRejectedValue(new Error('refresh failed'))
  render(<AiManagementSettings isTauri invokeTauri={invoke} onChanged={changed} onOpenSkill={() => {}} t={t} />)
  fireEvent.click(await screen.findByRole('button', { name: 'aiManagement.enable' }))
  expect(await screen.findByRole('alert')).toHaveProperty('textContent', 'aiManagement.errors.refresh')
  expect(screen.getByText('aiManagement.ready')).toBeTruthy()
  expect(invoke.mock.calls.filter(([name]) => name === 'enable_ai_management')).toHaveLength(1)
  expect(changed).toHaveBeenCalledOnce()
})

it('offers an official Skill content update even when the version and CLI are current', async () => {
  const state = fixture()
  Object.assign(state, { officialState: 'healthy', installed: true, skillId: 'id', installedVersion: state.bundledVersion, skillUpdateAvailable: true })
  state.agents[0].deployed = true
  const invoke = vi.fn(async () => state)
  render(<AiManagementSettings isTauri invokeTauri={invoke} onChanged={() => {}} onOpenSkill={() => {}} t={t} />)
  fireEvent.click(await screen.findByRole('button', { name: 'aiManagement.update' }))
  await waitFor(() => expect(invoke).toHaveBeenCalledWith('enable_ai_management', { operationId: expect.any(String) }))
  expect(screen.queryByText('aiManagement.ready')).toBeNull()
})
