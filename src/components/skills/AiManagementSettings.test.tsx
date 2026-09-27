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

const t = ((key: string) => key) as TFunction
const fixture = (): AgentAccessStatusDto => ({
  officialState: 'missing', conflict: null, skillId: null, skillEnabled: true,
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
  const state = { ...fixture(), officialState: 'healthy' as const, installed: true, skillEnabled: false, skillId: 'id' }
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
  await waitFor(() => expect(invoke).toHaveBeenCalledWith('enable_ai_management'))
})
