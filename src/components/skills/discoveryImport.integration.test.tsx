// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { toast } from 'sonner'
import App from '../../App'
import i18n from '../../i18n'
import type { ManagedSkill, OnboardingPlan, ToolInfoDto } from './types'

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/plugin-updater', () => ({ check: async () => null }))
vi.mock('@tauri-apps/api/app', () => ({ getVersion: async () => '1.0.1' }))
vi.mock('@tauri-apps/plugin-deep-link', () => ({ getCurrent: async () => null, onOpenUrl: async () => () => {} }))
vi.mock('sonner', () => ({ Toaster: () => null, toast: { success: vi.fn(), error: vi.fn(), warning: vi.fn() } }))

const tools: ToolInfoDto[] = ['codex', 'claude_code'].map(key => ({
  key, label: key, installed: true, enabled: true, is_custom: false,
  skills_dir: `/test/${key}/skills`, project_skills_dir: `.${key}/skills`,
  supports_project_scope: true, sync_mode: 'symlink',
}))
let plan: OnboardingPlan
let managedNames: Set<string>
let failOtherTarget: boolean
let sourceSyncError: string | null
let managedSkills: ManagedSkill[]

beforeEach(async () => {
  localStorage.clear()
  await i18n.changeLanguage('en')
  vi.stubGlobal('matchMedia', () => ({ matches: false, addEventListener: () => {}, removeEventListener: () => {} }))
  Object.assign(window, { __TAURI_INTERNALS__: { invoke } })
  vi.clearAllMocks()
  managedNames = new Set()
  failOtherTarget = false
  sourceSyncError = null
  managedSkills = []
  plan = {
    total_tools_scanned: 2, total_skills_found: 1,
    groups: [{ name: 'fresh', has_conflict: false, variants: [{
      tool: 'codex', name: 'fresh', path: '/test/codex/skills/fresh', is_link: false,
    }] }],
  }
  invoke.mockImplementation(async (command: string, args?: Record<string, unknown>) => {
    if (command === 'get_onboarding_plan') return structuredClone(plan)
    if (command === 'get_managed_skills') return structuredClone(managedSkills)
    if (command === 'get_tags' || command === 'get_recent_projects' || command === 'get_recycle_bin_items') return []
    if (command === 'source_conflict_preview') return {
      id: 'preview-fresh', skillId: 'fresh', name: 'fresh',
      sourcePath: args?.sourcePath, centralPath: '/test/central/fresh',
      central: { targets: [args?.sourcePath], blocked: null },
      tool: { targets: [args?.sourcePath], blocked: null },
      changes: [{ path: 'SKILL.md', kind: 'modified', before: 'central version\n', after: 'tool version\n' }],
    }
    if (command === 'get_tool_status') return { tools, installed: tools.map(tool => tool.key), newly_installed: [] }
    if (command === 'get_tool_config') return { disabled_builtin_tools: [], custom_tools: [] }
    if (command === 'get_discovery_scan_settings') return { sources: [], disabled_source_keys: [] }
    if (command === 'get_agent_access_status') return { installed: false, officialState: 'missing' }
    if (command === 'get_github_proxy_config') return { enabled: false, port: 7890, url: '', auto_detected: false }
    if (command === 'get_github_token_status') return { has_token: false }
    if (command === 'get_central_repo_path') return '/test/central'
    if (command === 'get_git_cache_cleanup_days') return 7
    if (command === 'get_git_cache_ttl_secs') return 3600
    if (command === 'get_auto_update_config' || command === 'get_auto_update_runtime') return { enabled: false, local_skill_count: 0, progress: { total: 0, succeeded: [], failed: [], running: null, pending: [] } }
    if (command === 'import_existing_skill') {
      if (managedNames.has(String(args?.name))) return null
      return { action: 'installed', name: args?.name, skill_id: args?.name, central_path: `/test/central/${args?.name}` }
    }
    if (command === 'sync_skill_to_tool') {
      if (args?.tool === 'codex' && sourceSyncError) throw sourceSyncError
      if (args?.tool === 'claude_code' && failOtherTarget) throw 'TOOL_NOT_WRITABLE|claude_code|/test/claude/skills/fresh'
      return { mode_used: 'symlink', target_path: '/test/claude/skills/fresh' }
    }
    throw new Error(`Unavailable in test: ${command}`)
  })
})

afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
  delete (window as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__
})

async function importDiscovered() {
  render(<App />)
  fireEvent.click(await screen.findByRole('button', { name: i18n.t('reviewImport') }))
  const dialog = await screen.findByRole('dialog', { name: i18n.t('importTitle') })
  fireEvent.click(within(dialog).getByRole('button', { name: i18n.t('importAndSync') }))
  await waitFor(() => expect(screen.queryByRole('dialog', { name: i18n.t('importTitle') })).toBeNull())
}

it('skips already managed Skills from a stale scan without deploying or deleting them', async () => {
  managedNames.add('fresh')
  await importDiscovered()
  expect(invoke.mock.calls.some(([command]) => command === 'sync_skill_to_tool')).toBe(false)
  expect(invoke.mock.calls.some(([command]) => command === 'delete_managed_skill')).toBe(false)
  await waitFor(() => expect(toast.success).toHaveBeenCalledWith(i18n.t('status.importCompletedWithSkipped', { imported: 0, managed: 1, sources: 0 }), expect.anything()))
  expect(toast.error).not.toHaveBeenCalled()
})

it('skips protected source targets and continues deploying to other tools', async () => {
  sourceSyncError = 'SKILL_TARGET_OVERLAPS_SOURCE|/test/codex/skills/fresh|sync target overlaps original local source'
  await importDiscovered()
  expect(invoke.mock.calls.filter(([command]) => command === 'sync_skill_to_tool').map(([, args]) => args.tool)).toEqual(['codex', 'claude_code'])
  await waitFor(() => expect(toast.success).toHaveBeenCalledWith(i18n.t('status.importCompletedWithSkipped', { imported: 1, managed: 0, sources: 1 }), expect.anything()))
  expect(toast.error).not.toHaveBeenCalled()
})

it('imports and syncs to the source tool as well as the other selected tools', async () => {
  await importDiscovered()
  const deployments = invoke.mock.calls.filter(([command]) => command === 'sync_skill_to_tool')
  expect(deployments.map(([, args]) => args.tool)).toEqual(['codex', 'claude_code'])
  expect(deployments[0][1]).toMatchObject({ overwrite: true, overwriteIfSameContent: true })
  await waitFor(() => expect(toast.success).toHaveBeenCalledWith(i18n.t('status.importCompleted'), expect.anything()))
  expect(toast.error).not.toHaveBeenCalled()
})

it('opens source conflicts after import and keeps a reopening action after closing', async () => {
  sourceSyncError = 'SKILL_SOURCE_TARGET_DIFFERS|/test/codex/skills/fresh'
  await importDiscovered()
  const dialog = await screen.findByRole('dialog', { name: i18n.t('sourceConflict.title') })
  expect(within(dialog).getByRole('region').textContent).toContain('central version')
  expect(within(dialog).getByRole('region').textContent).toContain('tool version')
  expect(toast.error).not.toHaveBeenCalled()
  fireEvent.click(within(dialog).getByRole('button', { name: i18n.t('close') }))
  expect(screen.queryByRole('dialog')).toBeNull()
  fireEvent.click(within(screen.getByRole('alert')).getByRole('button', { name: i18n.t('sourceConflict.title') }))
  expect(await screen.findByRole('dialog', { name: i18n.t('sourceConflict.title') })).toBeTruthy()
  fireEvent.keyDown(document, { key: 'Escape' })
  const message = i18n.t('errors.sourceTargetDiffers', { path: '/test/codex/skills/fresh' })
  expect(screen.getByRole('alert').textContent).toContain(message)
  expect(toast.success).not.toHaveBeenCalled()
  expect(invoke.mock.calls.filter(([command]) => command === 'sync_skill_to_tool').map(([, args]) => args.tool)).toEqual(['codex', 'claude_code'])
  fireEvent.click(within(screen.getByRole('alert')).getByRole('button', { name: i18n.t('close') }))
  expect(screen.queryByRole('alert')).toBeNull()
})

it('continues past managed Skills while retaining real deployment failures', async () => {
  plan.groups.unshift({ name: 'managed', has_conflict: false, variants: [{
    name: 'managed', tool: 'codex', path: '/test/codex/skills/managed', is_link: false,
  }] })
  plan.total_skills_found = 2
  managedNames.add('managed')
  failOtherTarget = true
  await importDiscovered()
  expect(invoke.mock.calls.filter(([command]) => command === 'import_existing_skill').map(([, args]) => args.name)).toEqual(['managed', 'fresh'])
  expect(invoke.mock.calls.filter(([command]) => command === 'sync_skill_to_tool').every(([, args]) => args.skillId === 'fresh')).toBe(true)
  expect(toast.error).toHaveBeenCalledWith(i18n.t('errors.syncFailedTitle', { name: 'fresh', tool: i18n.t('tools.claude_code') }), expect.anything())
  expect(toast.success).not.toHaveBeenCalled()
})

it('opens the conflict dialog directly from the source-tool sync icon without an error toast', async () => {
  prepareManagedConflict()
  render(<App />)
  const sync = await screen.findByRole('button', { name: `Codex · ${i18n.t('toolManagement.notSynced')}` })
  fireEvent.click(sync)
  const dialog = await screen.findByRole('dialog', { name: i18n.t('sourceConflict.title') })
  await waitFor(() => expect(within(dialog).getByRole('region').textContent).toContain('central version'))
  expect(within(dialog).getAllByRole('radio').every(radio => !(radio as HTMLInputElement).checked)).toBe(true)
  expect(toast.error).not.toHaveBeenCalled()
  expect(invoke.mock.calls.some(([command]) => command === 'source_conflict_execute')).toBe(false)
  fireEvent.keyDown(document, { key: 'Escape' })
  expect(screen.queryByRole('dialog')).toBeNull()
  expect(document.activeElement).toBe(sync)
  fireEvent.click(sync)
  expect(await screen.findByRole('dialog', { name: i18n.t('sourceConflict.title') })).toBeTruthy()
  expect(invoke.mock.calls.filter(([command]) => command === 'source_conflict_preview')).toHaveLength(2)
})

it('still reports other deployment failures when a batch opens a source conflict', async () => {
  sourceSyncError = 'SKILL_SOURCE_TARGET_DIFFERS|/test/codex/skills/fresh'
  failOtherTarget = true
  await importDiscovered()
  expect(await screen.findByRole('dialog', { name: i18n.t('sourceConflict.title') })).toBeTruthy()
  expect(toast.error).toHaveBeenCalledWith(i18n.t('errors.syncFailedTitle', { name: 'fresh', tool: i18n.t('tools.claude_code') }), expect.anything())
  expect(invoke.mock.calls.filter(([command]) => command === 'sync_skill_to_tool').map(([, args]) => args.tool)).toEqual(['codex', 'claude_code'])
})

function prepareManagedConflict() {
  sourceSyncError = 'SKILL_SOURCE_TARGET_DIFFERS|/test/codex/skills/fresh'
  managedSkills = [{ id: 'fresh', name: 'fresh', source_type: 'local',
    source_ref: '/test/codex/skills/fresh', central_path: '/test/central/fresh',
    created_at: 1, updated_at: 1, enabled: true, status: 'ok', tags: [], targets: [] }]
  plan = { groups: [], total_tools_scanned: 2, total_skills_found: 0 }
}

it('closes bulk target selection and opens the conflict without another entry click', async () => {
  prepareManagedConflict()
  render(<App />)
  await screen.findByText('fresh')
  fireEvent.click(screen.getByRole('button', { name: i18n.t('bulk.manage') }))
  fireEvent.click(screen.getByRole('button', { name: i18n.t('bulk.selectVisible') }))
  fireEvent.click(within(document.querySelector('.bulk-action-bar') as HTMLElement).getByRole('button', { name: i18n.t('bulk.sync') }))
  const selection = screen.getByRole('dialog')
  fireEvent.click(within(selection).getByRole('button', { name: /Codex/ }))
  fireEvent.click(within(selection).getByRole('button', { name: i18n.t('bulk.syncConfirm') }))
  const conflict = await screen.findByRole('dialog', { name: i18n.t('sourceConflict.title') })
  await waitFor(() => expect(within(conflict).getByRole('region').textContent).toContain('tool version'))
  expect(screen.getAllByRole('dialog')).toHaveLength(1)
  expect(toast.error).not.toHaveBeenCalled()
  expect(invoke.mock.calls.some(([command]) => command === 'source_conflict_execute')).toBe(false)
})

it('retains the remaining error summary when multiple source folders conflict', async () => {
  plan.groups.push({ name: 'second', has_conflict: false, variants: [{
    tool: 'codex', name: 'second', path: '/test/codex/skills/second', is_link: false,
  }] })
  plan.total_skills_found = 2
  const original = invoke.getMockImplementation()!
  invoke.mockImplementation(async (command: string, args?: Record<string, unknown>) => {
    if (command === 'sync_skill_to_tool' && args?.tool === 'codex') {
      throw `SKILL_SOURCE_TARGET_DIFFERS|/test/codex/skills/${args.name}`
    }
    return original(command, args)
  })
  await importDiscovered()
  const dialog = await screen.findByRole('dialog', { name: i18n.t('sourceConflict.title') })
  await waitFor(() => expect(within(dialog).getByRole('region').textContent).toContain('central version'))
  expect(invoke.mock.calls.find(([command]) => command === 'source_conflict_preview')?.[1]).toEqual({ sourcePath: '/test/codex/skills/fresh' })
  expect(toast.error).toHaveBeenCalledWith(i18n.t('errors.syncFailedTitle', { name: 'second', tool: i18n.t('tools.codex') }), expect.objectContaining({
    description: i18n.t('errors.sourceTargetDiffers', { path: '/test/codex/skills/second' }),
  }))
})
