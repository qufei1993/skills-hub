// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, within } from '@testing-library/react'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import App from '../../App'
import i18n from '../../i18n'
import type { AutoUpdateConfigDto } from './types'

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/plugin-updater', () => ({ check: async () => null }))
vi.mock('@tauri-apps/api/app', () => ({ getVersion: async () => '1.0.0' }))
vi.mock('@tauri-apps/plugin-deep-link', () => ({ getCurrent: async () => null, onOpenUrl: async () => () => {} }))
vi.mock('sonner', () => ({ Toaster: () => null, toast: { error: vi.fn(), success: vi.fn(), warning: vi.fn(), info: vi.fn() } }))

const autoUpdate: AutoUpdateConfigDto = {
  enabled: false, interval_hours: 24, schedule_type: 'interval', interval_value: 24,
  interval_unit: 'hours', daily_time: '03:00', local_skill_count: 0,
  protected_local_skill_count: 0, task_registered: false, task_status_detail: '',
  last_run_at: null, last_started_at: null, last_finished_at: null, last_status: null,
  last_error: null, last_checked: 0, last_unchanged: 0, last_updated: 0, last_failed: 0,
  progress: { total: 0, succeeded: [], failed: [], running: null, pending: [] },
}

let failure = ''

beforeEach(async () => {
  await i18n.changeLanguage('zh')
  localStorage.clear()
  vi.useFakeTimers()
  vi.stubGlobal('matchMedia', () => ({ matches: false, addEventListener: () => {}, removeEventListener: () => {} }))
  Object.assign(window, { __TAURI_INTERNALS__: { invoke } })
  invoke.mockReset()
  invoke.mockImplementation(async (command: string) => {
    if (command === 'get_managed_skills') return []
    if (command === 'list_local_skills_cmd') return [{ name: 'collision-demo', subpath: '.', valid: true }]
    if (command === 'install_local_selection') {
      if (failure) throw failure
      return { skill_id: 'new-id', name: 'collision-demo', central_path: '/test/central/collision-demo' }
    }
    if (command === 'get_auto_update_config' || command === 'get_auto_update_runtime') return autoUpdate
    if (command === 'get_tags') return []
    if (command === 'get_recent_projects' || command === 'get_recycle_bin_items') return []
    if (command === 'get_onboarding_plan') return { groups: [], total_skills_found: 0 }
    if (command === 'get_tool_config') return { disabled_builtin_tools: [], custom_tools: [] }
    if (command === 'get_tool_status') return { tools: [], installed: [], newly_installed: [] }
    if (command === 'get_discovery_scan_settings') return { sources: [], disabled_source_keys: [] }
    if (command === 'get_central_repo_path') return '/test/central'
    if (command === 'get_git_cache_cleanup_days') return 7
    if (command === 'get_git_cache_ttl_secs') return 3600
    if (command === 'get_github_token_status') return { has_token: false }
    if (command === 'get_github_proxy_config') return { enabled: false, port: 7890, url: '', auto_detected: false }
    if (command === 'get_agent_access_status') return {
      officialState: 'missing', conflict: null, skillId: null, skillEnabled: false,
      bridge: { status: 'missing', reason: 'DIRECTORY_MISSING', path: '/test/bin/skillshub-cli', version: null },
      installed: false, bundledVersion: '1.0.0', installedVersion: null, centralReason: null, agents: [],
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

async function click(name: string) {
  await act(async () => { fireEvent.click(screen.getByRole('button', { name })); await vi.advanceTimersByTimeAsync(0) })
}

for (const language of ['en', 'zh', 'ko']) {
  for (const managed of [false, true]) {
    for (const path of ['/test/central/collision-demo', String.raw`C:\Users\tester\.skillshub\collision-demo`]) {
      it(`keeps ${managed ? 'managed' : 'unmanaged'} collision guidance visible in ${language}: ${path}`, async () => {
        await i18n.changeLanguage(language)
        failure = `${managed ? 'CENTRAL_SKILL_MANAGED' : 'CENTRAL_SKILL_UNMANAGED'}|${path}`
        render(<App />)
        await act(async () => { await vi.advanceTimersByTimeAsync(0) })
        await click(i18n.t('addSkills'))
        await click(i18n.t('discovery.local'))
        fireEvent.change(screen.getByPlaceholderText(i18n.t('localPathPlaceholder')), { target: { value: '/test/incoming' } })
        await click(i18n.t('installFlow.detect'))
        await click(i18n.t('installFlow.next'))
        await click(i18n.t('installSelected'))
        const expected = i18n.t(managed ? 'errors.centralSkillManaged' : 'errors.centralSkillUnmanaged', { path })
        expect(expected).not.toContain('errors.')
        expect(expected).toContain(path)
        const dialog = screen.getByRole('dialog')
        expect(within(dialog).getByRole('alert').textContent).toContain(expected)
        await act(async () => { await vi.advanceTimersByTimeAsync(10_000) })
        expect(within(dialog).getByRole('alert').textContent).toContain(expected)
        expect(invoke.mock.calls.filter(([command]) => command === 'install_local_selection').map(([, args]) => args))
          .toEqual([{ basePath: '/test/incoming', subpath: '.', name: undefined }])
        failure = ''
        await click(i18n.t('installSelected'))
        expect(screen.queryByRole('dialog')).toBeNull()
      })
    }
  }
}

it('reports a known managed name before invoking installation', async () => {
  const original = invoke.getMockImplementation()!
  invoke.mockImplementation(async (command: string, ...args: unknown[]) => {
    if (command === 'get_managed_skills') return [{
      id: 'existing-id', name: 'collision-demo', central_path: '/test/central/collision-demo',
      source_type: 'local', enabled: false, status: 'ok', tags: [], targets: [], created_at: 1, updated_at: 1,
    }]
    return original(command, ...args)
  })
  render(<App />)
  await act(async () => { await vi.advanceTimersByTimeAsync(0) })
  await click(i18n.t('addSkills'))
  await click(i18n.t('discovery.local'))
  fireEvent.change(screen.getByPlaceholderText(i18n.t('localPathPlaceholder')), { target: { value: '/test/incoming' } })
  await click(i18n.t('installFlow.detect'))
  await click(i18n.t('installFlow.next'))
  await click(i18n.t('installSelected'))
  expect(within(screen.getByRole('dialog')).getByRole('alert').textContent)
    .toBe(i18n.t('errors.centralSkillManaged', { path: '/test/central/collision-demo' }))
  expect(invoke.mock.calls.some(([command]) => command === 'install_local_selection')).toBe(false)
})

it.each(['/test/central/collision-demo', String.raw`C:\Users\tester\.skillshub\collision-demo`])('keeps the Skill name when formatting legacy collisions: %s', async path => {
  failure = `skill already exists in central repo: "${path}"`
  render(<App />)
  await act(async () => { await vi.advanceTimersByTimeAsync(0) })
  await click(i18n.t('addSkills'))
  await click(i18n.t('discovery.local'))
  fireEvent.change(screen.getByPlaceholderText(i18n.t('localPathPlaceholder')), { target: { value: '/test/incoming' } })
  await click(i18n.t('installFlow.detect'))
  await click(i18n.t('installFlow.next'))
  await click(i18n.t('installSelected'))
  expect(within(screen.getByRole('dialog')).getByRole('alert').textContent)
    .toBe(i18n.t('errors.skillExistsInHubNamed', { name: 'collision-demo' }))
})
