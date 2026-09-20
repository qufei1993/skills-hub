// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, expect, it, vi } from 'vitest'
import { createInstance, type TFunction } from 'i18next'
import { resources } from '../../i18n/resources'
import AgentAccessPage from './AgentAccessPage'
import Header from './Header'
import type { AgentAccessStatusDto } from './types'

const t = ((key: string, values?: Record<string, unknown>) => values?.agent ? `${key}:${values.agent}` : key) as TFunction
const fixture = (): AgentAccessStatusDto => ({
  bridge: { status: 'damaged', reason: 'SOURCE_MISSING', path: '/test/bin/skillshub-cli', version: null },
  bundledVersion: '0.10.1', installedVersion: null, installed: false, centralReason: null,
  agents: [
    { key: 'codex', label: 'Codex', detected: true, enabled: true, deployed: false, needsRepair: false, reason: null, path: '/test/.codex/skills' },
    { key: 'cursor', label: 'Cursor', detected: true, enabled: true, deployed: false, needsRepair: false, reason: null, path: '/test/.cursor/skills' },
    { key: 'claude-code', label: 'Claude Code', detected: false, enabled: false, deployed: false, needsRepair: false, reason: null, path: '/test/.claude/skills' },
  ],
})
afterEach(() => { cleanup(); vi.restoreAllMocks() })

it.each(['en', 'zh', 'ko'])('renders status, damage details and removal confirmation in %s', async language => {
  const i18n = createInstance()
  await i18n.init({ resources, lng: language, fallbackLng: false })
  const state = fixture()
  state.installed = true
  state.agents[0].deployed = true
  state.agents[0].needsRepair = true
  state.agents[0].reason = 'TARGET_MODIFIED'
  const invoke = vi.fn(async () => state)
  render(<AgentAccessPage isTauri invokeTauri={invoke} t={i18n.t.bind(i18n)} />)
  const row = await screen.findByRole('row', { name: /Codex/ })
  fireEvent.click(within(row).getByRole('button', { name: i18n.t('agentAccess.remove') }))
  expect(screen.getByRole('dialog', { name: i18n.t('agentAccess.removeTitle') })).toBeTruthy()
  expect(document.body.textContent).not.toMatch(/agentAccess\.|manageTabs\./)
  expect(document.body.textContent).not.toContain('{{agent}}')
})

it('lists detected Agents and retains unavailable Agents with managed deployments', async () => {
  const state = fixture()
  state.agents[0].detected = false
  state.agents[0].deployed = true
  state.agents[1].enabled = false
  render(<AgentAccessPage isTauri invokeTauri={async () => state} t={t} />)
  const unavailable = await screen.findByRole('row', { name: /Codex/ })
  expect(within(unavailable).getByText('agentAccess.notDetected')).toBeTruthy()
  expect(within(unavailable).getByRole<HTMLButtonElement>('button', { name: 'agentAccess.repair' }).disabled).toBe(true)
  expect(within(await screen.findByRole('row', { name: /Cursor/ })).getByText('agentAccess.disabled')).toBeTruthy()
  expect(screen.queryByRole('row', { name: /Claude Code/ })).toBeNull()
})

it('reads only on entry and explicit refresh, and explains a damaged bridge', async () => {
  const invoke = vi.fn<(command: string) => Promise<AgentAccessStatusDto>>().mockResolvedValue(fixture())
  const { rerender } = render(<AgentAccessPage isTauri invokeTauri={invoke} t={t} />)
  await screen.findByText('agentAccess.bridgeState.damaged')
  expect(screen.getByText('agentAccess.reason.SOURCE_MISSING')).toBeTruthy()
  expect(screen.getByText('/test/bin/skillshub-cli')).toBeTruthy()
  expect(screen.getByText('0.10.1')).toBeTruthy()
  rerender(<AgentAccessPage isTauri invokeTauri={invoke} t={t} />)
  expect(invoke).toHaveBeenCalledTimes(1)
  fireEvent.click(screen.getByRole('button', { name: 'agentAccess.refresh' }))
  await waitFor(() => expect(invoke).toHaveBeenCalledTimes(2))
  expect(invoke.mock.calls.every(call => call[0] === 'get_agent_access_status')).toBe(true)
})

it('refresh exposes modified copy and link damage reasons returned by the service', async () => {
  const state = fixture()
  state.installed = true
  state.agents[0].deployed = true
  const invoke = vi.fn(async () => structuredClone(state))
  render(<AgentAccessPage isTauri invokeTauri={invoke} t={t} />)
  await screen.findByText('agentAccess.deployed')
  state.agents[0].needsRepair = true
  state.agents[0].reason = 'TARGET_MODIFIED'
  fireEvent.click(screen.getByRole('button', { name: 'agentAccess.refresh' }))
  await screen.findByText('agentAccess.healthReason.TARGET_MODIFIED')
  expect(screen.getByText('agentAccess.needsRepair')).toBeTruthy()
  state.agents[0].reason = 'TARGET_OWNERSHIP'
  fireEvent.click(screen.getByRole('button', { name: 'agentAccess.refresh' }))
  await screen.findByText('agentAccess.healthReason.TARGET_OWNERSHIP')
  state.agents[0].reason = 'TARGET_MISSING'
  fireEvent.click(screen.getByRole('button', { name: 'agentAccess.refresh' }))
  await screen.findByText('agentAccess.healthReason.TARGET_MISSING')
  state.agents[0].needsRepair = false
  state.agents[0].reason = null
  fireEvent.click(screen.getByRole('button', { name: 'agentAccess.refresh' }))
  await waitFor(() => expect(screen.queryByText('agentAccess.needsRepair')).toBeNull())
})

it('installs only the selected Agent, prevents duplicate operations, and uses the returned state', async () => {
  let finish!: (value: AgentAccessStatusDto) => void
  const invoke = vi.fn(async (command: string) => command === 'get_agent_access_status' ? fixture() : new Promise<AgentAccessStatusDto>(resolve => { finish = resolve }))
  render(<AgentAccessPage isTauri invokeTauri={invoke} t={t} />)
  const row = await screen.findByRole('row', { name: /Codex/ })
  const button = within(row).getByRole('button', { name: 'agentAccess.install' })
  fireEvent.click(button)
  fireEvent.click(button)
  expect(invoke).toHaveBeenCalledWith('set_agent_access', { agent: 'codex', action: 'install' })
  expect(invoke).toHaveBeenCalledTimes(2)
  expect(within(row).getByText('agentAccess.installing')).toBeTruthy()
  expect(screen.getAllByRole<HTMLButtonElement>('button').filter(b => b.textContent === 'agentAccess.install').every(b => b.disabled)).toBe(true)
  const updated = fixture()
  updated.installed = true
  updated.agents[0].deployed = true
  await act(async () => finish(updated))
  expect(within(row).getByText('agentAccess.deployed')).toBeTruthy()
  expect(screen.getByText('agentAccess.success.install:Codex')).toBeTruthy()
  expect(invoke).toHaveBeenCalledTimes(2)
})

it('offers safe repair and keeps actionable errors visible', async () => {
  const state = fixture()
  state.installed = true
  state.agents[0].deployed = true
  state.agents[0].needsRepair = true
  const invoke = vi.fn(async (command: string) => {
    if (command === 'get_agent_access_status') return state
    throw new Error('TARGET_MODIFIED|private detail')
  })
  render(<AgentAccessPage isTauri invokeTauri={invoke} t={t} />)
  const row = await screen.findByRole('row', { name: /Codex/ })
  expect(within(row).getByText('agentAccess.needsRepair')).toBeTruthy()
  fireEvent.click(within(row).getByRole('button', { name: 'agentAccess.repair' }))
  await screen.findByRole('alert')
  expect(invoke).toHaveBeenCalledWith('set_agent_access', { agent: 'codex', action: 'repair' })
  expect(screen.getByRole('alert').textContent).toContain('agentAccess.errors.targetModified')
  expect(screen.queryByText(/private detail/)).toBeNull()
  expect(within(row).getByRole<HTMLButtonElement>('button', { name: 'agentAccess.repair' }).disabled).toBe(false)
})

it('requires removal confirmation, traps focus, supports Escape and restores focus', async () => {
  const state = fixture()
  state.installed = true
  state.agents[0].deployed = true
  const invoke = vi.fn(async () => state)
  render(<AgentAccessPage isTauri invokeTauri={invoke} t={t} />)
  const remove = within(await screen.findByRole('row', { name: /Codex/ })).getByRole('button', { name: 'agentAccess.remove' })
  remove.focus()
  fireEvent.click(remove)
  expect(invoke).toHaveBeenCalledTimes(1)
  const dialog = screen.getByRole('dialog', { name: 'agentAccess.removeTitle' })
  expect(within(dialog).getByText('agentAccess.removeBody:Codex')).toBeTruthy()
  expect(document.activeElement).toBe(within(dialog).getByRole('button', { name: 'cancel' }))
  fireEvent.keyDown(document, { key: 'Tab', shiftKey: true })
  expect(document.activeElement).toBe(within(dialog).getByRole('button', { name: 'agentAccess.confirmRemove' }))
  fireEvent.keyDown(document, { key: 'Escape' })
  expect(screen.queryByRole('dialog')).toBeNull()
  expect(document.activeElement).toBe(remove)
  fireEvent.click(remove)
  fireEvent.click(screen.getByRole('button', { name: 'agentAccess.confirmRemove' }))
  await waitFor(() => expect(invoke).toHaveBeenCalledWith('set_agent_access', { agent: 'codex', action: 'remove', confirmed: true }))
})

it('copies only the fixed npm command and provides manual copy recovery', async () => {
  const writeText = vi.fn().mockRejectedValueOnce(new Error('denied')).mockResolvedValue(undefined)
  Object.defineProperty(navigator, 'clipboard', { configurable: true, value: { writeText } })
  const invoke = vi.fn(async () => fixture())
  render(<AgentAccessPage isTauri invokeTauri={invoke} t={t} />)
  await screen.findByText('Codex')
  fireEvent.click(screen.getByRole('button', { name: 'agentAccess.copyCommand' }))
  expect(await screen.findByText('agentAccess.copyFailed')).toBeTruthy()
  expect(screen.getByText('npm install -g skillshub-cli')).toBeTruthy()
  fireEvent.click(screen.getByRole('button', { name: 'agentAccess.copyCommand' }))
  await screen.findByText('agentAccess.copied')
  expect(writeText).toHaveBeenCalledWith('npm install -g skillshub-cli')
  expect(invoke).toHaveBeenCalledTimes(1)
})

it('does not invoke native commands in browser preview and recovers from read errors', async () => {
  const invoke = vi.fn().mockRejectedValueOnce(new Error('unavailable')).mockResolvedValue(fixture())
  const view = render(<AgentAccessPage isTauri={false} invokeTauri={invoke} t={t} />)
  expect(invoke).not.toHaveBeenCalled()
  expect(screen.getByText('agentAccess.desktopOnly')).toBeTruthy()
  view.rerender(<AgentAccessPage isTauri invokeTauri={invoke} t={t} />)
  await screen.findByText('agentAccess.errors.read')
  fireEvent.click(screen.getByRole('button', { name: 'agentAccess.refresh' }))
  await screen.findByText('Codex')
  expect(screen.queryByRole('alert')).toBeNull()
})

it('exposes a real Agent management navigation control when the sidebar is collapsed without badges', () => {
  const onChange = vi.fn()
  render(<Header activeView="manage" managementTab="agents" skillCount={0} tagCount={0} toolCount={0} updateCount={0} syncConflictCount={0} recycleBinCount={0} appVersion="" updateAvailableVersion={null} updateChecking={false} updateInstalling={false} updateDone={false} collapsed onToggleCollapsed={() => {}} onOpenSettings={() => {}} onOpenUpdate={() => {}} onRestart={() => {}} onViewChange={() => {}} onManagementTabChange={onChange} t={t} />)
  const button = screen.getByRole('button', { name: 'manageTabs.agents' })
  expect(button.title).toBe('manageTabs.agents')
  expect(button.className).toBe('active')
  expect(button.querySelector('em')).toBeNull()
  fireEvent.click(button)
  expect(onChange).toHaveBeenCalledWith('agents')
})
