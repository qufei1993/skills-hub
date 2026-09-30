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
    if (command === 'get_tool_config') return { disabled_builtin_tools: [], custom_tools: [] }
    if (command === 'get_featured_skills') return []
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


it.each([false, true, 'cancelled'] as const)('installs new skills alongside an existing same-source skill (update fails: %s)', async (updateFails) => {
  const existing: ManagedSkill = {
    id: 'existing-id', name: 'existing', source_type: 'git', source_ref: 'https://github.com/example/repo',
    central_path: '/test/existing', created_at: 1, updated_at: 1,
    enabled: false, status: 'ok', tags: [], targets: [],
  }
  const initial = invoke.getMockImplementation()!
  invoke.mockImplementation(async (command: string, args?: Record<string, unknown>) => {
    if (command === 'get_managed_skills') return [existing]
    if (command === 'list_git_skills_cmd') return [
      { name: 'existing', subpath: 'skills/existing', status: 'update' },
      { name: 'new', subpath: 'skills/new', status: 'install' },
      { name: 'conflict', subpath: 'skills/conflict', status: 'conflict' },
    ]
    if (command === 'install_git_selection') {
      if (args?.subpath === 'skills/existing') {
        if (updateFails === 'cancelled') throw new Error('CANCELLED|')
        if (updateFails) throw new Error('TARGET_MODIFIED|/test/existing')
        return { skill_id: 'existing-id', name: 'existing', central_path: '/test/existing', action: 'updated', pending_targets: [] }
      }
      if (args?.subpath === 'skills/new') return {
        skill_id: 'new-id', name: 'new', central_path: '/test/new', action: 'installed', pending_targets: [],
      }
      throw new Error('Unexpected selected candidate')
    }
    return initial(command, args)
  })
  render(<App />)
  await act(async () => { await vi.advanceTimersByTimeAsync(0) })
  fireEvent.click(screen.getByRole('button', { name: 'addSkills' }))
  await act(async () => { await vi.advanceTimersByTimeAsync(0) })
  fireEvent.click(screen.getByRole('button', { name: 'manualAdd' }))
  await act(async () => { await vi.advanceTimersByTimeAsync(0) })
  fireEvent.click(screen.getByRole('button', { name: 'gitTab' }))
  fireEvent.change(screen.getByPlaceholderText('gitUrlPlaceholder'), { target: { value: 'https://github.com/example/repo' } })
  fireEvent.click(screen.getByRole('button', { name: 'install' }))
  await act(async () => { await vi.advanceTimersByTimeAsync(0) })
  expect(screen.getByText('gitInstall.update')).toBeTruthy()
  expect((screen.getByRole('checkbox', { name: 'conflict' }) as HTMLInputElement).disabled).toBe(true)
  fireEvent.click(screen.getByRole('button', { name: 'gitInstall.submit' }))
  await act(async () => { await vi.advanceTimersByTimeAsync(0) })
  const calls = invoke.mock.calls.filter(([command]) => command === 'install_git_selection')
  expect(calls.map(([, args]) => args.subpath)).toEqual(updateFails === 'cancelled' ? ['skills/existing'] : ['skills/existing', 'skills/new'])
  expect(invoke.mock.calls.some(([command, args]) =>
    (command === 'set_skill_tags' || command === 'sync_skill_to_tool') && args?.skillId === 'existing-id',
  )).toBe(false)
  expect(screen.queryByText('gitPickTitle')).toBeNull()
  if (updateFails === true) expect(screen.getByText(/errors.targetModified/)).toBeTruthy()
  if (updateFails === 'cancelled') expect(screen.getByText('gitInstall.cancelled')).toBeTruthy()
})
