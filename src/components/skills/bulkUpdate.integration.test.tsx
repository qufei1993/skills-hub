// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, within } from '@testing-library/react'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { toast } from 'sonner'
import App from '../../App'
import i18n from '../../i18n'
import type { AutoUpdateConfigDto, ManagedSkill, UpdateResultDto } from './types'

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
let update: (skill: ManagedSkill) => Promise<UpdateResultDto>

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

function result(skill: ManagedSkill, changed = true): UpdateResultDto {
  return { skill_id: skill.id, name: skill.name, content_hash: 'new-hash',
    source_revision: 'new-revision', updated_targets: ['codex'], pending_targets: [], changed }
}

beforeEach(async () => {
  await i18n.changeLanguage('zh')
  localStorage.clear()
  vi.useFakeTimers()
  vi.stubGlobal('matchMedia', () => ({ matches: false, addEventListener: () => {}, removeEventListener: () => {} }))
  Object.assign(window, { __TAURI_INTERNALS__: { invoke } })
  skills = [fixture('one'), fixture('two'), fixture('unselected')]
  update = async skill => {
    skill.description = `Updated ${skill.name}`
    return result(skill, skill.id !== 'two')
  }
  vi.mocked(toast.success).mockClear()
  invoke.mockReset()
  invoke.mockImplementation(async (command: string, args?: Record<string, unknown>) => {
    if (command === 'get_managed_skills') return structuredClone(skills)
    if (command === 'workspace_content_preview') return { id: `preview-${args?.skillId}`, skillId: args?.skillId, name: String(args?.skillId), changes: [], targets: [], restore: false }
    if (command === 'workspace_content_execute') {
      const skill = skills.find(skill => `preview-${skill.id}` === args?.id)
      if (!skill) throw new Error('Unknown Skill')
      return update(skill)
    }
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
  if (name === i18n.t('bulk.update')) {
    expect(invoke.mock.calls.filter(([command]) => command === 'workspace_content_preview').length).toBeGreaterThan(0)
    await act(async () => { fireEvent.click(screen.getByRole('button', { name: i18n.t('workspace110.confirmUpdate') })); await vi.advanceTimersByTimeAsync(0) })
  }
}

async function openBulk(selectAll = true) {
  render(<App />)
  await act(async () => { await vi.advanceTimersByTimeAsync(0) })
  await clickButton(i18n.t('bulk.manage'))
  if (selectAll) await clickButton(i18n.t('bulk.selectVisible'), true)
}

const updatedIds = () => invoke.mock.calls.filter(([command]) => command === 'workspace_content_execute').map(([, args]) => String(args.id).replace('preview-', ''))
const report = () => within(screen.getByRole('region', { name: i18n.t('bulk.updateResultTitle') }))

it('requires a fresh target after clearing a concrete project filter', async () => {
  render(<App />)
  await act(async () => { await vi.advanceTimersByTimeAsync(0) })
  const scopes = document.querySelector('.skills-scope-tabs') as HTMLElement
  await act(async () => { fireEvent.click(within(scopes).getByRole('button', { name: new RegExp(i18n.t('scope.project')) })) })
  await clickButton(i18n.t('workspace110.project'))
  await act(async () => { fireEvent.click(screen.getByRole('option', { name: /\/test\/project/ })) })
  expect(screen.queryByRole('button', { name: '配置项目 Skills' })).toBeNull()
  expect(screen.queryByRole('button', { name: '保存当前配置' })).toBeNull()
  await clickButton(i18n.t('filters.clear'))
  expect(screen.queryByRole('button', { name: '配置项目 Skills' })).toBeNull()
  expect(screen.queryByRole('button', { name: '保存当前配置' })).toBeNull()
  expect(screen.queryByRole('button', { name: i18n.t('workspace110.addExisting') })).toBeNull()
  await clickButton(i18n.t('bulk.manage'))
  await clickButton(i18n.t('bulk.selectVisible'), true)
  expect(screen.queryByRole('button', { name: '保存为预设' })).toBeNull()
  await clickButton(i18n.t('bulk.sync'), true)
  expect((screen.getByRole('radio', { name: i18n.t('workspace110.global') }) as HTMLInputElement).checked).toBe(false)
  expect((screen.getByRole('radio', { name: i18n.t('workspace110.project') }) as HTMLInputElement).checked).toBe(false)
  expect((screen.getByRole('button', { name: i18n.t('workspace110.applyNow') }) as HTMLButtonElement).disabled).toBe(true)
  expect(invoke.mock.calls.some(([command]) => command === 'workspace_execute')).toBe(false)
})

it.each(['zh', 'en', 'ko'] as const)('updates only selected Skills and refreshes results in %s without changing deployment choices', async language => {
  await i18n.changeLanguage(language)
  const saved = structuredClone(skills.map(skill => ({ targets: skill.targets, tags: skill.tags, enabled: skill.enabled })))
  await openBulk(false)
  const bar = within(document.querySelector('.bulk-action-bar') as HTMLElement)
  const removedPresetLabel = { zh: '保存为预设', en: 'Save as preset', ko: '프리셋으로 저장' }[language]
  expect(bar.queryByRole('button', { name: removedPresetLabel })).toBeNull()
  expect(bar.getByRole('button', { name: i18n.t('bulk.sync') })).toBeTruthy()
  expect(bar.getByRole('button', { name: i18n.t('bulk.delete') })).toBeTruthy()
  expect((bar.getByRole('button', { name: i18n.t('bulk.update') }) as HTMLButtonElement).disabled).toBe(true)
  const boxes = screen.getAllByRole('checkbox', { name: i18n.t('bulk.toggleSkill') })
  await act(async () => { fireEvent.click(boxes[0]); fireEvent.click(boxes[1]) })
  await clickButton(i18n.t('bulk.update'), true)
  expect(updatedIds()).toEqual(['one', 'two'])
  expect(screen.getByText('Updated one')).toBeTruthy()
  expect(screen.getByText('Updated two')).toBeTruthy()
  expect(skills[2].description).toBeUndefined()
  expect(skills.map(skill => ({ targets: skill.targets, tags: skill.tags, enabled: skill.enabled }))).toEqual(saved)
  expect(screen.queryByRole('region', { name: i18n.t('bulk.updateResultTitle') })).toBeNull()
  expect(toast.success).toHaveBeenCalledWith(i18n.t('bulk.updateResult', { updated: 1, unchanged: 1, skipped: 0, failed: 0 }), { duration: 1800 })
  expect(invoke.mock.calls.some(([command]) => ['sync_skill_to_tool', 'unsync_skill_from_tool', 'set_skill_enabled', 'set_skill_tags'].includes(command))).toBe(false)
})

it('skips disabled and unbound sources while updating selected Git and bound local sources', async () => {
  skills = [fixture('git'), fixture('local', { source_type: 'local', source_ref: '/test/source' }),
    fixture('disabled', { enabled: false }), fixture('unbound', { source_type: 'local', source_ref: ' ' })]
  await openBulk()
  await clickButton(i18n.t('bulk.update'), true)
  expect(updatedIds()).toEqual(['git', 'local'])
  expect(report().getByText(i18n.t('bulk.updateResult', { updated: 2, unchanged: 0, skipped: 2, failed: 0 }))).toBeTruthy()
  expect(report().getByText(i18n.t('bulk.updateSkippedHelp'))).toBeTruthy()
  expect(skills[2].enabled).toBe(false)
})

it('explains why update is unavailable when none of the selected Skills can update', async () => {
  skills = [fixture('disabled', { enabled: false }), fixture('unbound', { source_type: 'local', source_ref: null })]
  await openBulk()
  const button = within(document.querySelector('.bulk-action-bar') as HTMLElement).getByRole('button', { name: i18n.t('bulk.update') }) as HTMLButtonElement
  expect(button.disabled).toBe(true)
  expect(button.title).toBe(i18n.t('bulk.updateHelp'))
  fireEvent.click(button)
  expect(updatedIds()).toEqual([])
})

it('continues after a source failure, reports pending tools, and keeps failed Skills selected for retry', async () => {
  skills = [fixture('failed'), fixture('pending')]
  update = async skill => {
    if (skill.id === 'failed') throw 'TARGET_MODIFIED|/test/modified'
    skill.description = 'Central copy updated'
    return { ...result(skill), pending_targets: ['codex'] }
  }
  await openBulk()
  await clickButton(i18n.t('bulk.update'), true)
  expect(updatedIds()).toEqual(['failed', 'pending'])
  expect(screen.getByText('Central copy updated')).toBeTruthy()
  expect(report().getByText(i18n.t('bulk.updateFailedTitle', { name: 'failed' }))).toBeTruthy()
  expect(report().getByText(i18n.t('errors.targetModified', { path: '/test/modified' }))).toBeTruthy()
  expect(report().getByText(i18n.t('deviceSync.skillUpdatedToolsPending', { name: 'pending', count: 1 }))).toBeTruthy()
  expect(screen.getAllByRole('checkbox', { name: i18n.t('bulk.toggleSkill') }).map(box => (box as HTMLInputElement).checked)).toEqual([true, false])
  update = async skill => result(skill)
  await clickButton(i18n.t('bulk.update'), true)
  expect(updatedIds()).toEqual(['failed', 'pending', 'failed'])
  expect(screen.queryByRole('region', { name: i18n.t('bulk.updateResultTitle') })).toBeNull()
})

it('waits for the current update before cancellation and keeps unprocessed Skills selected', async () => {
  skills = [fixture('one'), fixture('two')]
  let finish!: (result: UpdateResultDto) => void
  update = () => new Promise(resolve => { finish = resolve })
  await openBulk()
  await clickButton(i18n.t('bulk.update'), true)
  expect(updatedIds()).toEqual(['one'])
  expect(screen.getByText(i18n.t('bulk.updateTitle'))).toBeTruthy()
  expect(screen.getByText(i18n.t('bulk.updateProgress', { current: 1, total: 2, name: 'one' }))).toBeTruthy()
  await act(async () => {
    fireEvent.click(within(screen.getByRole('dialog')).getByRole('button', { name: i18n.t('cancel') }))
    await vi.advanceTimersByTimeAsync(0)
  })
  expect(screen.getByRole('dialog')).toBeTruthy()
  expect(screen.getByText(i18n.t('bulk.updateCancelling'))).toBeTruthy()
  expect((within(document.querySelector('.bulk-action-bar') as HTMLElement).getByRole('button', { name: i18n.t('bulk.update') }) as HTMLButtonElement).disabled).toBe(true)
  await act(async () => { finish(result(skills[0])); await vi.advanceTimersByTimeAsync(0) })
  expect(updatedIds()).toEqual(['one'])
  expect(screen.queryByRole('dialog')).toBeNull()
  expect(report().getByText(i18n.t('bulk.updateCancelled', { count: 1 }))).toBeTruthy()
  expect(screen.getAllByRole('checkbox', { name: i18n.t('bulk.toggleSkill') }).map(box => (box as HTMLInputElement).checked)).toEqual([false, true])
  update = async skill => result(skill)
  await clickButton(i18n.t('bulk.update'), true)
  expect(updatedIds()).toEqual(['one', 'two'])
})

it('finishes an unchanged batch without leaving a result panel to close', async () => {
  update = async skill => result(skill, false)
  await openBulk()
  await clickButton(i18n.t('bulk.update'), true)
  expect(updatedIds()).toEqual(['one', 'two', 'unselected'])
  expect(screen.queryByRole('region', { name: i18n.t('bulk.updateResultTitle') })).toBeNull()
  expect(document.querySelector('.bulk-action-bar')).toBeNull()
  expect(toast.success).toHaveBeenCalledWith(i18n.t('bulk.updateResult', { updated: 0, unchanged: 3, skipped: 0, failed: 0 }), { duration: 1800 })
})

it('keeps pending tool details visible even when every source update succeeds', async () => {
  skills = [fixture('pending')]
  update = async skill => ({ ...result(skill), pending_targets: ['codex'] })
  await openBulk()
  await clickButton(i18n.t('bulk.update'), true)
  await act(async () => { await vi.advanceTimersByTimeAsync(10_000) })
  expect(report().getByText(i18n.t('deviceSync.skillUpdatedToolsPending', { name: 'pending', count: 1 }))).toBeTruthy()
  expect(toast.success).not.toHaveBeenCalled()
  await clickButton(i18n.t('close'))
  expect(screen.queryByRole('region', { name: i18n.t('bulk.updateResultTitle') })).toBeNull()
})
