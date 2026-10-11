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
    if (command === 'get_website_collections') return { schemaVersion: 1, collections: [] }
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
  fireEvent.click(screen.getByRole('button', { name: 'discovery.git' }))
  await act(async () => { await vi.advanceTimersByTimeAsync(0) })
  fireEvent.click(screen.getAllByRole('button', { name: 'discovery.git' }).at(-1)!)
  fireEvent.change(screen.getByPlaceholderText('gitUrlPlaceholder'), { target: { value: 'https://github.com/example/repo' } })
  fireEvent.click(screen.getByRole('button', { name: 'installFlow.detect' }))
  await act(async () => { await vi.advanceTimersByTimeAsync(0) })
  expect(screen.getByText('gitInstall.update')).toBeTruthy()
  expect((screen.getByRole('checkbox', { name: 'conflict' }) as HTMLInputElement).disabled).toBe(true)
  fireEvent.click(screen.getByRole('button', { name: 'installFlow.next' }))
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

it.each(['git', 'local'] as const)('requires confirmation for a single %s skill and keeps settings when going back', async kind => {
  const initial = invoke.getMockImplementation()!
  invoke.mockImplementation(async (command: string, args?: Record<string, unknown>) => {
    if (command === 'get_tags') return [{ id: 9, name: 'Regression', skill_count: 0 }]
    if (command === 'list_git_skills_cmd' || command === 'list_local_skills_cmd') return [
      { name: 'single', subpath: 'skills/single', status: 'install', valid: true },
    ]
    return initial(command, args)
  })
  render(<App />)
  await act(async () => { await vi.advanceTimersByTimeAsync(0) })
  fireEvent.click(screen.getByRole('button', { name: 'addSkills' }))
  await act(async () => { await vi.advanceTimersByTimeAsync(0) })
  fireEvent.click(screen.getByRole('button', { name: `discovery.${kind}` }))
  const placeholder = kind === 'git' ? 'gitUrlPlaceholder' : 'localPathPlaceholder'
  const source = kind === 'git' ? 'https://github.com/example/repo' : '/tmp/local-fixture'
  fireEvent.change(screen.getByPlaceholderText(placeholder), { target: { value: source } })
  fireEvent.click(screen.getByRole('button', { name: 'installFlow.detect' }))
  await act(async () => { await vi.advanceTimersByTimeAsync(0) })
  expect(screen.getAllByRole('dialog')).toHaveLength(1)
  expect(screen.getByText('installFlow.confirmTitle')).toBeTruthy()
  expect(invoke.mock.calls.filter(([command]) => command.startsWith('install_'))).toHaveLength(0)
  fireEvent.click(screen.getByRole('button', { name: 'installFlow.next' }))
  fireEvent.click(screen.getByRole('button', { name: '#Regression' }))
  fireEvent.click(screen.getByRole('button', { name: 'installFlow.back' }))
  fireEvent.click(screen.getByRole('checkbox', { name: 'selectAll' }))
  fireEvent.click(screen.getByRole('button', { name: 'installFlow.back' }))
  expect((screen.getByPlaceholderText(placeholder) as HTMLInputElement).value).toBe(source)
  fireEvent.click(screen.getByRole('button', { name: 'installFlow.detect' }))
  await act(async () => { await vi.advanceTimersByTimeAsync(0) })
  expect((screen.getByRole('checkbox', { name: 'selectAll' }) as HTMLInputElement).checked).toBe(false)
  fireEvent.click(screen.getByRole('checkbox', { name: 'selectAll' }))
  fireEvent.click(screen.getByRole('button', { name: 'installFlow.next' }))
  expect(screen.getByRole('button', { name: '#Regression' }).className).toContain('selected')
})

it('does not open confirmation when a cancelled source scan resolves late', async () => {
  let finish: (value: unknown) => void = () => {}
  const initial = invoke.getMockImplementation()!
  invoke.mockImplementation(async (command: string, args?: Record<string, unknown>) => {
    if (command === 'list_git_skills_cmd') return new Promise(resolve => { finish = resolve })
    if (command === 'cancel_current_operation') return undefined
    return initial(command, args)
  })
  render(<App />)
  await act(async () => { await vi.advanceTimersByTimeAsync(0) })
  fireEvent.click(screen.getByRole('button', { name: 'addSkills' }))
  await act(async () => { await vi.advanceTimersByTimeAsync(0) })
  fireEvent.click(screen.getByRole('button', { name: 'discovery.git' }))
  fireEvent.change(screen.getByPlaceholderText('gitUrlPlaceholder'), { target: { value: 'https://github.com/example/repo' } })
  fireEvent.click(screen.getByRole('button', { name: 'installFlow.detect' }))
  await act(async () => { await vi.advanceTimersByTimeAsync(0) })
  expect(screen.getAllByRole('dialog')).toHaveLength(1)
  fireEvent.click(screen.getByRole('button', { name: 'cancel' }))
  await act(async () => { finish([{ name: 'single', subpath: '.', status: 'install' }]); await vi.advanceTimersByTimeAsync(0) })
  expect(screen.queryByText('installFlow.confirmTitle')).toBeNull()
  expect(screen.getByPlaceholderText('gitUrlPlaceholder')).toBeTruthy()
})

it('confirms shared tool changes inline without opening another dialog', async () => {
  const initial = invoke.getMockImplementation()!
  invoke.mockImplementation(async (command: string, args?: Record<string, unknown>) => {
    if (command === 'get_tool_status') return {
      installed: ['codex', 'amp'], newly_installed: [],
      tools: ['codex', 'amp'].map(key => ({ key, label: key, enabled: true, installed: true, is_custom: false, skills_dir: '/test/shared', project_skills_dir: '.agents/skills', supports_project_scope: true, sync_mode: 'symlink' })),
    }
    if (command === 'list_git_skills_cmd') return [{ name: 'single', subpath: 'skills/single', status: 'install' }]
    return initial(command, args)
  })
  render(<App />)
  await act(async () => { await vi.advanceTimersByTimeAsync(0) })
  fireEvent.click(screen.getByRole('button', { name: 'addSkills' }))
  await act(async () => { await vi.advanceTimersByTimeAsync(0) })
  fireEvent.click(screen.getByRole('button', { name: 'discovery.git' }))
  fireEvent.change(screen.getByPlaceholderText('gitUrlPlaceholder'), { target: { value: 'https://github.com/example/repo' } })
  fireEvent.click(screen.getByRole('button', { name: 'installFlow.detect' }))
  await act(async () => { await vi.advanceTimersByTimeAsync(0) })
  fireEvent.click(screen.getByRole('button', { name: 'installFlow.next' }))
  const checkbox = screen.getByRole('checkbox', { name: 'codex' }) as HTMLInputElement
  const previous = checkbox.checked
  fireEvent.click(checkbox)
  expect(screen.getAllByRole('dialog')).toHaveLength(1)
  expect(screen.getByRole('alert').textContent).toContain('sharedDirConfirm')
  expect((screen.getByRole('button', { name: 'installSelected' }) as HTMLButtonElement).disabled).toBe(true)
  fireEvent.click(screen.getByRole('button', { name: 'confirm' }))
  expect(screen.queryByRole('alert')).toBeNull()
  expect(checkbox.checked).toBe(!previous)
  expect((screen.getByRole('checkbox', { name: 'amp' }) as HTMLInputElement).checked).toBe(!previous)
  expect((screen.getByRole('button', { name: 'installSelected' }) as HTMLButtonElement).disabled).toBe(false)
})

it.each(['git', 'local'] as const)('keeps a failed single %s install available for retry', async kind => {
  const initial = invoke.getMockImplementation()!
  let attempts = 0
  invoke.mockImplementation(async (command: string, args?: Record<string, unknown>) => {
    if (command === 'list_git_skills_cmd' || command === 'list_local_skills_cmd') return [{ name: 'single', subpath: 'single', status: 'install', valid: true }]
    if (command === `install_${kind}_selection`) {
      if (attempts++ === 0) throw new Error('temporary installation failure')
      return { skill_id: 'new', name: 'single', central_path: '/tmp/single', action: 'installed' }
    }
    return initial(command, args)
  })
  render(<App />)
  await act(async () => { await vi.advanceTimersByTimeAsync(0) })
  fireEvent.click(screen.getByRole('button', { name: 'addSkills' }))
  await act(async () => { await vi.advanceTimersByTimeAsync(0) })
  fireEvent.click(screen.getByRole('button', { name: `discovery.${kind}` }))
  fireEvent.change(screen.getByPlaceholderText(kind === 'git' ? 'gitUrlPlaceholder' : 'localPathPlaceholder'), { target: { value: kind === 'git' ? 'https://github.com/example/repo' : '/tmp/local-fixture' } })
  fireEvent.click(screen.getByRole('button', { name: 'installFlow.detect' }))
  await act(async () => { await vi.advanceTimersByTimeAsync(0) })
  if (screen.queryByRole('button', { name: 'installFlow.next' })) fireEvent.click(screen.getByRole('button', { name: 'installFlow.next' }))
  fireEvent.click(screen.getByRole('button', { name: 'installSelected' }))
  await act(async () => { await vi.advanceTimersByTimeAsync(0) })
  expect(screen.getByRole('dialog', { name: 'installFlow.confirmTitle' })).toBeTruthy()
  expect(screen.getByText(/temporary installation failure/)).toBeTruthy()
  if (screen.queryByRole('button', { name: 'installFlow.next' })) fireEvent.click(screen.getByRole('button', { name: 'installFlow.next' }))
  fireEvent.click(screen.getByRole('button', { name: 'installSelected' }))
  await act(async () => { await vi.advanceTimersByTimeAsync(0) })
  expect(screen.queryByRole('dialog', { name: 'installFlow.confirmTitle' })).toBeNull()
  expect(attempts).toBe(2)
})
