// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import App from '../../App'
import type { AutoUpdateConfigDto, ManagedSkill } from './types'

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }))

vi.mock('@tauri-apps/plugin-updater', () => ({ check: async () => null }))
vi.mock('@tauri-apps/api/app', () => ({ getVersion: async () => '0.10.1' }))

const autoUpdateConfig: AutoUpdateConfigDto = {
  enabled: false,
  interval_hours: 24,
  schedule_type: 'interval',
  interval_value: 24,
  interval_unit: 'hours',
  daily_time: '03:00',
  local_skill_count: 0,
  protected_local_skill_count: 0,
  task_registered: false,
  task_status_detail: '',
  last_run_at: null,
  last_started_at: null,
  last_finished_at: null,
  last_status: null,
  last_error: null,
  last_checked: 0,
  last_unchanged: 0,
  last_updated: 0,
  last_failed: 0,
  progress: {
    total: 0,
    succeeded: [],
    failed: [],
    running: null,
    pending: [],
  },
}

beforeEach(() => {
  vi.useFakeTimers()
  vi.stubGlobal('matchMedia', () => ({
    matches: false,
    addEventListener: () => {},
    removeEventListener: () => {},
  }))
  Object.assign(window, { __TAURI_INTERNALS__: { invoke } })
  invoke.mockReset()
  invoke.mockImplementation(async (command: string) => {
    if (command === 'get_auto_update_config') return autoUpdateConfig
    if (command === 'get_auto_update_runtime') return autoUpdateConfig
    if (command === 'get_managed_skills') return []
    if (command === 'get_tags') return []
    if (command === 'get_tool_status') {
      return { tools: [], installed: [], newly_installed: [] }
    }
    if (command === 'get_onboarding_plan') {
      return { groups: [], total_skills_found: 0 }
    }
    if (command === 'get_recent_projects') return []
    if (command === 'get_recycle_bin_items') return []
    if (command === 'get_agent_access_status') return {
      officialState: 'missing', conflict: null, skillId: null, skillEnabled: false,
      bridge: { status: 'missing', reason: 'DIRECTORY_MISSING', path: '/test/bin/skillshub-cli', version: null },
      installed: false, bundledVersion: '0.10.1', installedVersion: null, centralReason: null, agents: [],
    }
    throw new Error(`Unavailable in test: ${command}`)
  })
})

afterEach(() => {
  cleanup()
  vi.useRealTimers()
  vi.unstubAllGlobals()
  delete (window as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__
})

it.each(['menu', 'focus', 'skills-page', 'tags-page'])('refreshes externally created Skills and tags on %s without clearing search', async trigger => {
  render(<App />)
  await act(async () => { await vi.advanceTimersByTimeAsync(0) })
  const input = screen.getByPlaceholderText('searchPlaceholder') as HTMLInputElement
  fireEvent.change(input, { target: { value: 'external' } })
  await act(async () => { await vi.advanceTimersByTimeAsync(500) })
  const original = invoke.getMockImplementation()!
  const tag = { id: 91, name: 'productivity', skill_count: 1, updated_at: 123 }
  const skill: ManagedSkill = {
    id: 'external-skill', name: 'external-skill', source_type: 'local', central_path: '/test/external-skill',
    created_at: 123, updated_at: 123, enabled: true, status: 'ok', tags: [{ id: 91, name: 'productivity' }], targets: [],
  }
  invoke.mockImplementation(async (command: string, ...args: unknown[]) => {
    if (command === 'get_tags') return [tag]
    if (command === 'get_managed_skills') return [skill]
    return original(command, ...args)
  })
  const readsBefore = invoke.mock.calls.filter(([command]) => command === 'get_tags').length
  await act(async () => {
    if (trigger === 'menu') fireEvent.click(screen.getByRole('button', { name: 'tags' }))
    if (trigger === 'focus') window.dispatchEvent(new Event('focus'))
    if (trigger === 'skills-page') fireEvent.click(screen.getByRole('button', { name: /navMySkills/ }))
    if (trigger === 'tags-page') fireEvent.click(screen.getByRole('button', { name: /manageTabs.tags/ }))
  })
  expect(invoke.mock.calls.filter(([command]) => command === 'get_tags').length).toBeGreaterThan(readsBefore)
  if (trigger === 'tags-page') {
    expect(screen.getByText('productivity')).toBeTruthy()
  } else {
    expect(screen.getByText('external-skill')).toBeTruthy()
    expect(input.value).toBe('external')
    if (trigger !== 'menu') await act(async () => { fireEvent.click(screen.getByRole('button', { name: 'tags' })) })
    const option = screen.getByRole('button', { name: 'productivity 1' }) as HTMLButtonElement
    expect(option.disabled).toBe(false)
    fireEvent.click(option)
    await act(async () => { window.dispatchEvent(new Event('focus')) })
    expect(screen.getByRole('button', { name: 'productivity 1' }).getAttribute('aria-pressed')).toBe('true')
    expect(input.value).toBe('external')
  }
})

it('ignores a stale tag response after a newer refresh completes', async () => {
  render(<App />)
  await act(async () => { await vi.advanceTimersByTimeAsync(0) })
  const original = invoke.getMockImplementation()!
  let finishOld!: (value: unknown) => void
  let reads = 0
  invoke.mockImplementation(async (command: string, ...args: unknown[]) => {
    if (command === 'get_tags') {
      if (++reads === 1) return new Promise(resolve => { finishOld = resolve })
      return [{ id: 92, name: 'new-tag', skill_count: 0, updated_at: 456 }]
    }
    return original(command, ...args)
  })
  await act(async () => { window.dispatchEvent(new Event('focus')) })
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: 'tags' })) })
  expect(screen.getByText('new-tag')).toBeTruthy()
  await act(async () => { finishOld([{ id: 91, name: 'old-tag', skill_count: 0, updated_at: 123 }]) })
  expect(screen.getByText('new-tag')).toBeTruthy()
  expect(screen.queryByText('old-tag')).toBeNull()
})

it('keeps recurring progress polling off the system task configuration command', async () => {
  render(<App />)

  await act(async () => {
    await vi.advanceTimersByTimeAsync(0)
  })

  expect(invoke.mock.calls.filter(([command]) => command === 'get_auto_update_config')).toHaveLength(1)
  expect(invoke.mock.calls.filter(([command]) => command === 'get_auto_update_runtime')).toHaveLength(1)

  await act(async () => {
    await vi.advanceTimersByTimeAsync(5000)
  })

  expect(invoke.mock.calls.filter(([command]) => command === 'get_auto_update_config')).toHaveLength(1)
  expect(invoke.mock.calls.filter(([command]) => command === 'get_auto_update_runtime')).toHaveLength(2)

  await act(async () => {
    window.dispatchEvent(new Event('focus'))
  })

  expect(invoke.mock.calls.filter(([command]) => command === 'get_auto_update_config')).toHaveLength(1)
  expect(invoke.mock.calls.filter(([command]) => command === 'get_auto_update_runtime')).toHaveLength(3)
})

it('reads AI management for discovery and settings without polling or accessing credentials', async () => {
  render(<App />)
  await act(async () => { await vi.advanceTimersByTimeAsync(0) })
  const initialReads = invoke.mock.calls.filter(([command]) => command === 'get_agent_access_status').length
  expect(initialReads).toBeGreaterThan(0)
  await act(async () => { await vi.advanceTimersByTimeAsync(60000) })
  expect(invoke.mock.calls.filter(([command]) => command === 'get_agent_access_status')).toHaveLength(initialReads)
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: 'aiManagement.notice.action' })) })
  expect(document.activeElement).toBe(screen.getByRole('region', { name: 'aiManagement.title' }))
  expect(screen.queryByRole('button', { name: 'manageTabs.agents' })).toBeNull()
  expect(invoke.mock.calls.filter(([command]) => command === 'get_agent_access_status')).toHaveLength(initialReads + 1)
  await act(async () => { await vi.advanceTimersByTimeAsync(60000); window.dispatchEvent(new Event('focus')) })
  expect(invoke.mock.calls.filter(([command]) => command === 'get_agent_access_status')).toHaveLength(initialReads + 1)
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: /manageTabs.tags/ })) })
  expect(screen.queryByRole('button', { name: 'agentAccess.refresh' })).toBeNull()
  await act(async () => { await vi.advanceTimersByTimeAsync(60000) })
  expect(invoke.mock.calls.filter(([command]) => command === 'get_agent_access_status')).toHaveLength(initialReads + 1)
  const originalInvoke = invoke.getMockImplementation()!
  let finishRefresh!: (value: unknown) => void
  invoke.mockImplementation((command: string, ...args: unknown[]) => command === 'get_agent_access_status'
    ? new Promise(resolve => { finishRefresh = resolve }) : originalInvoke(command, ...args))
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: 'settings' })) })
  expect(invoke.mock.calls.filter(([command]) => command === 'get_agent_access_status')).toHaveLength(initialReads + 2)
  expect(screen.queryByText('agentAccess.loading')).toBeNull()
  expect(screen.getByRole('button', { name: 'aiManagement.enable' })).toBeTruthy()
  expect(screen.queryByText('aiManagement.enabling')).toBeNull()
  await act(async () => { finishRefresh(await originalInvoke('get_agent_access_status')) })
  expect(invoke.mock.calls.some(([command]) => /credential|execute_cli|run_cli/.test(command))).toBe(false)
})


it('re-enables update after polling recovers an interrupted run with completed progress', async () => {
  const original = invoke.getMockImplementation()!
  let runtime: AutoUpdateConfigDto = {
    ...autoUpdateConfig,
    last_status: 'running',
    last_run_at: Date.now() - 24 * 60 * 60 * 1000,
    last_unchanged: 1,
    progress: {
      total: 2,
      succeeded: [{ skill_id: 'done', name: 'Done' }],
      failed: [],
      running: { skill_id: 'interrupted', name: 'Interrupted' },
      pending: [],
    },
  }
  invoke.mockImplementation(async (command: string, ...args: unknown[]) => {
    if (command === 'get_auto_update_config' || command === 'get_auto_update_runtime') return runtime
    return original(command, ...args)
  })
  render(<App />)
  await act(async () => { await vi.advanceTimersByTimeAsync(0) })
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: /manageTabs.updates/ })) })
  expect((screen.getByRole('button', { name: 'autoUpdateRunningButton' }) as HTMLButtonElement).disabled).toBe(true)
  runtime = {
    ...runtime,
    last_status: 'stopped',
    last_finished_at: Date.now(),
    progress: { ...runtime.progress!, running: null, pending: [{ skill_id: 'interrupted', name: 'Interrupted' }] },
  }
  await act(async () => { await vi.advanceTimersByTimeAsync(5000) })
  expect((screen.getByRole('button', { name: 'autoUpdateRunNow' }) as HTMLButtonElement).disabled).toBe(false)
  await act(async () => { await vi.advanceTimersByTimeAsync(5000) })
  expect((screen.getByRole('button', { name: 'autoUpdateRunNow' }) as HTMLButtonElement).disabled).toBe(false)
})
