// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { toast } from 'sonner'
import App from '../../App'
import i18n from '../../i18n'
import type { OnboardingPlan, ToolInfoDto } from './types'

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

beforeEach(async () => {
  localStorage.clear()
  await i18n.changeLanguage('en')
  vi.stubGlobal('matchMedia', () => ({ matches: false, addEventListener: () => {}, removeEventListener: () => {} }))
  Object.assign(window, { __TAURI_INTERNALS__: { invoke } })
  vi.clearAllMocks()
  managedNames = new Set()
  failOtherTarget = false
  plan = {
    total_tools_scanned: 2, total_skills_found: 1,
    groups: [{ name: 'fresh', has_conflict: false, variants: [{
      tool: 'codex', name: 'fresh', path: '/test/codex/skills/fresh', is_link: false,
    }] }],
  }
  invoke.mockImplementation(async (command: string, args?: Record<string, unknown>) => {
    if (command === 'get_onboarding_plan') return structuredClone(plan)
    if (command === 'get_managed_skills' || command === 'get_tags' || command === 'get_recent_projects') return []
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
      if (args?.tool === 'codex') throw 'SKILL_TARGET_OVERLAPS_SOURCE|/test/codex/skills/fresh|sync target overlaps original local source'
      if (failOtherTarget) throw 'TOOL_NOT_WRITABLE|claude_code|/test/claude/skills/fresh'
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
  expect(toast.success).toHaveBeenCalledWith(i18n.t('status.importCompletedWithSkipped', { imported: 0, managed: 1, sources: 0 }), expect.anything())
  expect(toast.error).not.toHaveBeenCalled()
})

it('skips protected source targets and continues deploying to other tools', async () => {
  await importDiscovered()
  expect(invoke.mock.calls.filter(([command]) => command === 'sync_skill_to_tool').map(([, args]) => args.tool)).toEqual(['codex', 'claude_code'])
  expect(toast.success).toHaveBeenCalledWith(i18n.t('status.importCompletedWithSkipped', { imported: 1, managed: 0, sources: 1 }), expect.anything())
  expect(toast.error).not.toHaveBeenCalled()
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
