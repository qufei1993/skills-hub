// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, within } from '@testing-library/react'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { toast } from 'sonner'
import App from '../../App'
import i18n from '../../i18n'
import type { AutoUpdateConfigDto, ManagedSkill } from './types'

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

let skills: ManagedSkill[]

function fixture(name: string, overrides: Partial<ManagedSkill> = {}): ManagedSkill {
  return {
    id: name, name, source_type: 'git', source_ref: 'https://github.com/example/repo',
    central_path: `/test/central/${name}`, created_at: 1, updated_at: 1,
    enabled: true, status: 'ok', tags: [{ id: 1, name: 'Saved' }],
    targets: [{ tool: 'codex', scope: 'project', project_path: '/test/project',
      target_path: `/test/project/.agents/skills/${name}`, mode: 'symlink', status: 'ok' }],
    ...overrides,
  }
}

beforeEach(async () => {
  await i18n.changeLanguage('zh')
  localStorage.clear()
  vi.useFakeTimers()
  vi.stubGlobal('matchMedia', () => ({ matches: false, addEventListener: () => {}, removeEventListener: () => {} }))
  Object.assign(window, { __TAURI_INTERNALS__: { invoke } })
  skills = [fixture('modified')]
  vi.mocked(toast.error).mockClear()
  vi.mocked(toast.success).mockClear()
  invoke.mockReset()
  invoke.mockImplementation(async (command: string) => {
    if (command === 'get_managed_skills') return structuredClone(skills)
    if (command === 'delete_managed_skill') throw 'DELETE_TARGET_MODIFIED|/test/project/.agents/skills/modified'
    if (command === 'get_auto_update_config' || command === 'get_auto_update_runtime') return autoUpdate
    if (command === 'get_tags') return [{ id: 1, name: 'Saved', skill_count: skills.length }]
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

async function clickButton(name: string, bulk = false) {
  const query = bulk ? within(document.querySelector('.bulk-action-bar') as HTMLElement) : screen
  await act(async () => { fireEvent.click(query.getByRole('button', { name })); await vi.advanceTimersByTimeAsync(0) })

}

for (const language of ['en', 'zh', 'ko']) {
  for (const bulk of [false, true]) {
    it(`uses deletion wording for ${bulk ? 'bulk' : 'single'} deletion in ${language}`, async () => {
      await i18n.changeLanguage(language)
      render(<App />)
      await act(async () => { await vi.advanceTimersByTimeAsync(0) })
      if (bulk) {
        await clickButton(i18n.t('bulk.manage'))
        await clickButton(i18n.t('bulk.selectVisible'), true)
        await clickButton(i18n.t('bulk.delete'), true)
      } else {
        await clickButton(i18n.t('remove'))
      }
      const dialog = screen.getByRole('dialog')
      await act(async () => {
        fireEvent.click(within(dialog).getByRole('button', { name: i18n.t(bulk ? 'bulk.deleteConfirm' : 'delete.confirmButton', { count: 1 }) }))
        await vi.advanceTimersByTimeAsync(0)
      })
      const path = '/test/project/.agents/skills/modified'
      const expected = i18n.t('errors.deleteTargetModified', { path })
      expect(expected).not.toBe('errors.deleteTargetModified')
      expect(expected).toContain(path)
      if (bulk) {
        expect(toast.error).toHaveBeenCalledWith('modified', expect.objectContaining({ description: expected }))
      } else {
        expect(toast.error).toHaveBeenCalledWith(expected, expect.anything())
      }
      expect(JSON.stringify(vi.mocked(toast.error).mock.calls)).not.toContain(i18n.t('errors.targetModified', { path }))
      expect(invoke.mock.calls.filter(([command]) => command === 'delete_managed_skill').map(([, args]) => args)).toEqual([{ skillId: 'modified' }])
      expect(skills.map(skill => skill.id)).toEqual(['modified'])
      expect(toast.success).not.toHaveBeenCalled()
    })
  }
}
