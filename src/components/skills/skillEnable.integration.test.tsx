// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, within } from '@testing-library/react'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { toast } from 'sonner'
import App from '../../App'
import i18n from '../../i18n'
import type { AutoUpdateConfigDto, ManagedSkill, ToolInfoDto } from './types'

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/plugin-updater', () => ({ check: async () => null }))
vi.mock('@tauri-apps/api/app', () => ({ getVersion: async () => '1.0.0' }))
vi.mock('@tauri-apps/plugin-deep-link', () => ({ getCurrent: async () => null, onOpenUrl: async () => () => {} }))
vi.mock('sonner', () => ({ Toaster: () => null, toast: { error: vi.fn(), success: vi.fn() } }))

const autoUpdate: AutoUpdateConfigDto = {
  enabled: false, interval_hours: 24, schedule_type: 'interval', interval_value: 24,
  interval_unit: 'hours', daily_time: '03:00', local_skill_count: 0,
  protected_local_skill_count: 0, task_registered: false, task_status_detail: '',
  last_run_at: null, last_started_at: null, last_finished_at: null, last_status: null,
  last_error: null, last_checked: 0, last_unchanged: 0, last_updated: 0, last_failed: 0,
  progress: { total: 0, succeeded: [], failed: [], running: null, pending: [] },
}

const tools: ToolInfoDto[] = [
  ['antigravity', false, true], ['codex', true, true], ['claude_code', true, true],
  ['cursor', true, true], ['not_installed', true, false],
].map(([key, enabled, installed]) => ({
  key: key as string, label: key as string, enabled: enabled as boolean,
  installed: installed as boolean, is_custom: false, skills_dir: `/test/${key}/skills`,
  project_skills_dir: `.${key}/skills`, supports_project_scope: true, sync_mode: 'symlink',
}))

let skills: ManagedSkill[]
let failTool: string | undefined

function fixture(name: string, scope: 'global' | 'project' = 'global'): ManagedSkill {
  return {
    id: name, name, source_type: 'git', source_ref: 'https://github.com/example/repo',
    central_path: `/test/central/${name}`, created_at: 1, updated_at: 1,
    enabled: true, status: 'ok', tags: [],
    targets: ['antigravity', 'codex', 'claude_code', 'not_installed'].map(tool => ({
      tool, scope, project_path: scope === 'project' ? '/test/project' : null,
      target_path: `/test/${tool}/${name}`, mode: 'symlink', status: 'ok',
    })),
  }
}

beforeEach(async () => {
  await i18n.changeLanguage('zh')
  localStorage.clear()
  vi.useFakeTimers()
  vi.stubGlobal('matchMedia', () => ({ matches: false, addEventListener: () => {}, removeEventListener: () => {} }))
  Object.assign(window, { __TAURI_INTERNALS__: { invoke } })
  failTool = undefined
  skills = [fixture('one'), fixture('two')]
  invoke.mockReset()
  vi.mocked(toast.error).mockClear()
  invoke.mockImplementation(async (command: string, args?: Record<string, unknown>) => {
    if (command === 'get_managed_skills') return structuredClone(skills)
    if (command === 'get_auto_update_config' || command === 'get_auto_update_runtime') return autoUpdate
    if (command === 'get_tags' || command === 'get_recent_projects' || command === 'get_recycle_bin_items') return []
    if (command === 'get_onboarding_plan') return { groups: [], total_skills_found: 0 }
    if (command === 'get_tool_config') return { disabled_builtin_tools: ['antigravity'], custom_tools: [] }
    if (command === 'get_tool_status') return { tools, installed: tools.filter(tool => tool.installed).map(tool => tool.key), newly_installed: [] }
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
    const skill = skills.find(skill => skill.id === args?.skillId)
    if (command === 'set_skill_enabled' && skill) {
      skill.enabled = args?.enabled === true
      if (!skill.enabled) skill.targets.forEach(target => { target.status = 'disabled' })
      return
    }
    if (command === 'sync_skill_to_tool' && skill) {
      const tool = tools.find(tool => tool.key === args?.tool)
      if (!tool?.enabled || (args?.scope !== 'project' && !tool.installed) || tool.key === failTool) {
        throw `TOOL_NOT_INSTALLED|${args?.tool}`
      }
      const target = skill.targets.find(target => target.tool === args?.tool && target.scope === args?.scope && (target.project_path ?? undefined) === args?.projectPath)
      if (!target) throw new Error('Unexpected deployment outside saved selections')
      target.status = 'ok'
      return
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

async function selectAll() {
  await clickButton(i18n.t('bulk.manage'))
  await clickButton(i18n.t('bulk.selectVisible'))
}

it.each(['global', 'project'] as const)('restores saved %s selections after bulk disable without restoring disabled tools or new tools', async scope => {
  skills = [fixture('one', scope), fixture('two', scope)]
  render(<App />)
  await act(async () => { await vi.advanceTimersByTimeAsync(0) })
  await selectAll()
  await clickButton(i18n.t('bulk.disable'), true)
  const saved = structuredClone(skills.map(skill => skill.targets))
  expect(skills.every(skill => !skill.enabled)).toBe(true)
  await selectAll()
  await clickButton(i18n.t('bulk.enable'), true)
  const expected = scope === 'project' ? ['codex', 'claude_code', 'not_installed'] : ['codex', 'claude_code']
  for (const skill of skills) {
    expect(skill.enabled).toBe(true)
    expect(skill.targets.map(target => target.tool)).toEqual(saved[0].map(target => target.tool))
    expect(skill.targets.filter(target => target.status === 'ok').map(target => target.tool)).toEqual(expected)
  }
  const calls = invoke.mock.calls.filter(([command]) => command === 'sync_skill_to_tool')
  expect(calls.map(([, args]) => args.tool)).toEqual([...expected, ...expected])
  if (scope === 'project') expect(calls.every(([, args]) => args.projectPath === '/test/project')).toBe(true)
  expect(screen.getAllByRole('button', { name: 'Codex · 已同步' })).toHaveLength(2)
  expect(toast.error).not.toHaveBeenCalled()
})

it.each(['single', 'bulk'] as const)('continues saved tool restoration and refreshes visible status when a tool fails during %s enable', async mode => {
  skills = [fixture('one')]
  skills[0].enabled = false
  skills[0].targets = skills[0].targets.filter(target => target.tool !== 'antigravity')
  skills[0].targets.forEach(target => { target.status = 'disabled' })
  failTool = 'codex'
  render(<App />)
  await act(async () => { await vi.advanceTimersByTimeAsync(0) })
  if (mode === 'bulk') { await selectAll(); await clickButton(i18n.t('bulk.enable'), true) }
  else await clickButton(i18n.t('enableSkill'))
  expect(skills[0].targets.find(target => target.tool === 'claude_code')?.status).toBe('ok')
  expect(screen.getByRole('button', { name: `${i18n.t('tools.claude_code')} · 已同步` }).getAttribute('aria-pressed')).toBe('true')
  expect(within(document.querySelector('.skill-card') as HTMLElement).getByRole('button', { name: i18n.t('disableSkill') })).toBeTruthy()
  expect(JSON.stringify(vi.mocked(toast.error).mock.calls)).not.toContain('TOOL_NOT_INSTALLED|')
  expect(JSON.stringify(vi.mocked(toast.error).mock.calls)).toContain(i18n.t('errors.toolNotInstalled'))
})

it('refreshes the disabled switch and saved targets when target cleanup reports a failure', async () => {
  skills = [fixture('one')]
  const original = invoke.getMockImplementation()!
  invoke.mockImplementation(async (command: string, args?: Record<string, unknown>) => {
    const result = await original(command, args)
    if (command === 'set_skill_enabled') throw 'TARGET_MODIFIED|/test/codex/one'
    return result
  })
  render(<App />)
  await act(async () => { await vi.advanceTimersByTimeAsync(0) })
  await clickButton(i18n.t('disableSkill'))
  expect(screen.getByRole('button', { name: i18n.t('enableSkill') })).toBeTruthy()
  expect(skills[0].targets).toHaveLength(4)
  expect(JSON.stringify(vi.mocked(toast.error).mock.calls)).toContain(i18n.t('errors.targetModified', { path: '/test/codex/one' }))
})

it.each([
  ['zh', '同步目录已变更'],
  ['en', 'different sync directory'],
  ['ko', '동기화 디렉터리'],
])('explains changed tool paths in %s without claiming that a directory exists', async (language, explanation) => {
  await i18n.changeLanguage(language)
  skills = [fixture('one')]
  skills[0].enabled = false
  skills[0].targets.forEach(target => { target.status = 'disabled' })
  const original = invoke.getMockImplementation()!
  invoke.mockImplementation(async (command: string, args?: Record<string, unknown>) => {
    if (command === 'sync_skill_to_tool' && args?.tool === 'codex') throw 'TOOL_PATH_CHANGED|codex|/test/new/one'
    return original(command, args)
  })
  render(<App />)
  await act(async () => { await vi.advanceTimersByTimeAsync(0) })
  await clickButton(i18n.t('enableSkill'))
  const notices = JSON.stringify(vi.mocked(toast.error).mock.calls)
  expect(notices).toContain(explanation)
  expect(notices).toContain('/test/new/one')
  expect(notices).not.toContain('TOOL_PATH_CHANGED|')
})
