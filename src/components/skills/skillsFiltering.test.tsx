// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import App from '../../App'
import i18n from '../../i18n'
import type { ApplicationRecord, ManagedSkill, OnboardingPlan, SkillPreset } from './types'
import collectionIndex from './collections/fixtures/index.json'

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/plugin-updater', () => ({ check: async () => null }))
vi.mock('@tauri-apps/api/app', () => ({ getVersion: async () => '0.10.0' }))

const tags = [
  { id: 1, name: 'writing', skill_count: 1, updated_at: 0 },
  { id: 2, name: 'code', skill_count: 1, updated_at: 0 },
]
const makeSkill = (id: string, scope: 'global' | 'project', tagIds: number[]): ManagedSkill => ({
  id, name: id, source_type: 'local', central_path: `/skills/${id}`,
  created_at: 0, updated_at: 0, enabled: true, status: 'ok',
  tags: tags.filter((tag) => tagIds.includes(tag.id)),
  targets: [{ scope, tool: 'codex', mode: 'copy', status: 'ok', target_path: `/targets/${id}` }],
})
const skills = [makeSkill('global-writing', 'global', [1]), makeSkill('project-code', 'project', [2])]

beforeEach(async () => {
  vi.stubGlobal('matchMedia', () => ({ matches: false, addEventListener: () => {}, removeEventListener: () => {} }))
  localStorage.clear()
  await i18n.changeLanguage('en')
  Object.assign(window, { __TAURI_INTERNALS__: { invoke } })
  invoke.mockReset()
  invoke.mockImplementation(async (command: string) => {
    if (command === 'get_managed_skills') return skills
    if (command === 'get_tags') return tags
    if (command === 'get_onboarding_plan') return { groups: [], total_skills_found: 0 }
    if (command === 'get_recent_projects') return []
    throw new Error(`Unavailable in test: ${command}`)
  })
})
afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
  delete (window as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__
})
async function setup() {
  render(<App />)
  await screen.findByText('global-writing')
  await screen.findByRole('button', { name: 'Tags 2' })
}
const scope = (name: string) => fireEvent.click(screen.getByRole('button', { name }))

function withLibraryNotices() {
  const plan: OnboardingPlan = {
    total_tools_scanned: 1,
    total_skills_found: 1,
    groups: [{ name: 'discovered-local-skill', has_conflict: false, variants: [{
      name: 'discovered-local-skill', tool: 'codex', path: '/external/skills/discovered-local-skill', is_link: false,
    }] }],
  }
  const original = invoke.getMockImplementation()!
  invoke.mockImplementation(async (command: string) => {
    if (command === 'get_onboarding_plan') return structuredClone(plan)
    if (command === 'get_agent_access_status') return { installed: false, officialState: 'missing' }
    if (command === 'get_tool_status') return { tools: [], installed: [], newly_installed: [] }
    if (command === 'get_tool_config') return { disabled_builtin_tools: [], custom_tools: [] }
    if (command === 'get_discovery_scan_settings') return { sources: [], disabled_source_keys: [] }
    if (command === 'get_website_collections') return collectionIndex
    if (command === 'cache_website_collections') return undefined
    return original(command)
  })
  return plan
}

describe('My Skills contextual notices', () => {
  it.each(['list', 'cards'] as const)('keeps setup and discovery outside the Skills region in %s view', async view => {
    withLibraryNotices()
    await setup()
    fireEvent.click(screen.getByRole('button', { name: i18n.t(`viewMode.${view}`) }))
    const aiNotice = await screen.findByText(i18n.t('aiManagement.notice.title'))
    const discoveryNotice = screen.getByText(i18n.t('discoveredTitle'))
    const skillRegion = screen.getAllByRole('region', { name: i18n.t('navMySkills') })
      .find(region => within(region).queryByText('global-writing'))!
    expect(skillRegion).toBeTruthy()
    expect(skillRegion.contains(aiNotice)).toBe(false)
    expect(skillRegion.contains(discoveryNotice)).toBe(false)
    expect(within(skillRegion).getByText('global-writing')).toBeTruthy()
    expect(within(skillRegion).getByText('project-code')).toBeTruthy()
    expect(within(skillRegion).queryByText('discovered-local-skill')).toBeNull()
    expect(screen.getByRole('button', { name: 'All 2' })).toBeTruthy()
  })

  it('remembers a dismissed discovery notice while keeping detected Skills available from Add Skills', async () => {
    const plan = withLibraryNotices()
    const originalPlan = structuredClone(plan)
    const page = render(<App />)
    await screen.findByText('global-writing')
    const closeNotice = await screen.findByRole('button', { name: i18n.t('discovery.dismissNotice') })
    const callsBeforeDismiss = invoke.mock.calls.length
    fireEvent.click(closeNotice)
    expect(screen.queryByText(i18n.t('discoveredTitle'))).toBeNull()
    expect(localStorage.getItem('skills-discovery-notice-dismissed-v1')).toBe('true')
    expect(invoke.mock.calls.slice(callsBeforeDismiss)).toEqual([])

    fireEvent.click(screen.getByRole('button', { name: i18n.t('addSkills') }))
    const detected = await screen.findByRole('button', { name: i18n.t('discovery.detected', { count: 1 }) })
    fireEvent.click(detected)
    const modalTitle = await screen.findByText(i18n.t('importTitle'))
    const importDialog = modalTitle.closest('.modal') as HTMLElement
    expect(within(importDialog).getByText('discovered-local-skill')).toBeTruthy()
    expect(within(importDialog).getByText('/external/skills/discovered-local-skill')).toBeTruthy()
    fireEvent.click(within(importDialog).getAllByRole('button', { name: i18n.t('close') })[0])
    fireEvent.click(screen.getByRole('button', { name: 'My Skills 2' }))
    await screen.findByText('global-writing')
    expect(screen.queryByText(i18n.t('discoveredTitle'))).toBeNull()

    page.unmount()
    render(<App />)
    await screen.findByText('global-writing')
    await waitFor(() => expect(invoke.mock.calls.filter(([command]) => command === 'get_onboarding_plan').length).toBeGreaterThan(1))
    expect(screen.queryByText(i18n.t('discoveredTitle'))).toBeNull()
    expect(plan).toEqual(originalPlan)
    expect(invoke.mock.calls.some(([command]) => ['set_discovery_scan_config', 'import_existing_skill'].includes(command))).toBe(false)
  })
})

describe('My Skills filter integration', () => {
  it('keeps zero-count selected tags removable and clears filters from an empty result', async () => {
    await setup()
    fireEvent.click(screen.getByRole('button', { name: 'Tags' }))
    fireEvent.click(screen.getByRole('button', { name: 'writing 1' }))
    scope('Project 1')
    expect((screen.getByRole('button', { name: 'writing 0' }) as HTMLButtonElement).disabled).toBe(false)
    expect(screen.getByText('No Skills match the current filters.')).toBeTruthy()
    fireEvent.click(screen.getAllByRole('button', { name: 'Clear filters' })[0])
    expect(screen.getByText('global-writing')).toBeTruthy()
    expect(screen.getByText('project-code')).toBeTruthy()
  })

  it('does not enable bulk actions for selections hidden by scope or search', async () => {
    await setup()
    scope('Global 1')
    fireEvent.click(screen.getByRole('button', { name: 'Bulk' }))
    fireEvent.click(screen.getByRole('button', { name: 'Select all' }))
    scope('Project 1')
    expect((screen.getByRole('button', { name: 'Delete' }) as HTMLButtonElement).disabled).toBe(true)
    scope('Global 1')
    expect((screen.getByRole('button', { name: 'Delete' }) as HTMLButtonElement).disabled).toBe(false)
    fireEvent.change(screen.getByPlaceholderText('Search skills...'), { target: { value: 'no match' } })
    expect((screen.getByRole('button', { name: 'Delete' }) as HTMLButtonElement).disabled).toBe(true)
  })

  it('resets the old scope and search when opening a tag from management', async () => {
    await setup()
    scope('Project 1')
    fireEvent.change(screen.getByPlaceholderText('Search skills...'), { target: { value: 'no match' } })
    fireEvent.click(screen.getByRole('button', { name: 'Tags 2' }))
    const row = screen.getByText('writing').closest('.tags-table-row')
    expect(row).toBeTruthy()
    fireEvent.click(within(row as HTMLElement).getByRole('button', { name: /writing/ }))
    expect(screen.getByText('global-writing')).toBeTruthy()
    expect(screen.getByRole('button', { name: 'All 2' }).getAttribute('aria-pressed')).toBe('true')
    expect((screen.getByPlaceholderText('Search skills...') as HTMLInputElement).value).toBe('')
  })

  it('keeps library scopes focused on management and carries the selected project into single-Skill targets', async () => {
    const deployed = [makeSkill('global-writing', 'global', []), makeSkill('alpha-only', 'project', []), makeSkill('beta-only', 'project', [])]
    deployed[1].targets[0].project_path = '/test/project-alpha'
    deployed[2].targets[0].project_path = '/test/project-beta'
    const original = invoke.getMockImplementation()!
    invoke.mockImplementation(async (command: string) => {
      if (command === 'get_managed_skills') return deployed
      if (command === 'get_recent_projects') return ['/test/project-alpha']
      if (command === 'get_tool_status') return { tools: [], installed: [], newly_installed: [] }
      if (command === 'get_tool_config') return { disabled_builtin_tools: [], custom_tools: [] }
      if (command === 'workspace_presets' || command === 'workspace_history') return []
      return original(command)
    })
    render(<App />)
    await screen.findByText('alpha-only')
    const expectManagementOnly = () => {
      for (const name of ['Configure project Skills', 'Save current configuration', 'Save as preset']) {
        expect(screen.queryByRole('button', { name })).toBeNull()
      }
      expect(screen.queryByRole('button', { name: i18n.t('workspace110.addExisting') })).toBeNull()
    }
    expectManagementOnly()
    scope('Global 1')
    expectManagementOnly()
    scope('Project 2')
    expectManagementOnly()

    fireEvent.click(screen.getByRole('button', { name: i18n.t('workspace110.project') }))
    fireEvent.change(screen.getByRole('combobox', { name: i18n.t('workspace110.searchProjects') }), { target: { value: 'project-alpha' } })
    expect(screen.queryByRole('option', { name: /\/test\/project-beta/ })).toBeNull()
    fireEvent.click(screen.getByRole('option', { name: /\/test\/project-alpha/ }))
    expect(screen.getByText('alpha-only')).toBeTruthy()
    expect(screen.queryByText('beta-only')).toBeNull()
    expect(screen.queryByText('global-writing')).toBeNull()
    expectManagementOnly()

    fireEvent.click(screen.getByRole('button', { name: i18n.t('scope.projectCount', { count: 1 }) }))
    expect((screen.getByRole('radio', { name: i18n.t('workspace110.project') }) as HTMLInputElement).checked).toBe(true)
    expect((screen.getByRole('checkbox', { name: '/test/project-alpha' }) as HTMLInputElement).checked).toBe(true)
    expect((screen.getByRole('checkbox', { name: '/test/project-beta' }) as HTMLInputElement).checked).toBe(false)
    fireEvent.click(screen.getByRole('button', { name: i18n.t('workspace110.cancel') }))
    expect(screen.getByText('alpha-only')).toBeTruthy()
    expect(screen.queryByText('beta-only')).toBeNull()

    fireEvent.click(screen.getByRole('button', { name: i18n.t('workspace110.project') }))
    fireEvent.click(screen.getByRole('option', { name: i18n.t('workspace110.allProjects') }))
    expect(screen.getByText('alpha-only')).toBeTruthy()
    expect(screen.getByText('beta-only')).toBeTruthy()
    expect(screen.queryByText('global-writing')).toBeNull()
    expectManagementOnly()

    fireEvent.click(screen.getByRole('button', { name: i18n.t('workspace110.project') }))
    fireEvent.click(screen.getByRole('option', { name: /\/test\/project-alpha/ }))
    fireEvent.click(screen.getByRole('button', { name: i18n.t('filters.clear') }))
    expect(screen.getByRole('button', { name: 'All 3' }).getAttribute('aria-pressed')).toBe('true')
    expect(screen.getByText('global-writing')).toBeTruthy()
    expect(screen.getByText('alpha-only')).toBeTruthy()
    expect(screen.getByText('beta-only')).toBeTruthy()
    expect(screen.queryByRole('button', { name: i18n.t('workspace110.project') })).toBeNull()
    expectManagementOnly()

    fireEvent.click(screen.getByRole('button', { name: i18n.t('scope.globalBadge') }))
    const applicationScope = screen.getByRole('radio', { name: i18n.t('workspace110.project') }) as HTMLInputElement
    expect(applicationScope.checked).toBe(false)
    fireEvent.click(applicationScope)
    expect((screen.getByRole('checkbox', { name: '/test/project-alpha' }) as HTMLInputElement).checked).toBe(false)
    expect((screen.getByRole('checkbox', { name: '/test/project-beta' }) as HTMLInputElement).checked).toBe(false)
    expect(invoke.mock.calls.some(([command]) => ['workspace_execute', 'sync_skill_to_tool', 'unsync_skill_from_tool'].includes(command))).toBe(false)
  })
})

it.each([
  { name: 'empty', recentProjects: [], expectedPaths: ['/test/project-alpha', '/test/project-beta'] },
  { name: 'overlapping', recentProjects: ['/test/project-alpha', '/test/recent-only'], expectedPaths: ['/test/project-alpha', '/test/recent-only', '/test/project-beta'] },
])('offers deployed projects when applying a preset with $name recent projects', async ({ recentProjects, expectedPaths }) => {
  const deployed = [makeSkill('alpha', 'project', []), makeSkill('beta', 'project', [])]
  deployed[0].targets = ['codex', 'cursor'].map(tool => ({
    tool, scope: 'project', project_path: '/test/project-alpha',
    target_path: `/test/project-alpha/.${tool}/skills/alpha`, mode: 'copy', status: 'ok',
  }))
  deployed[1].targets[0].project_path = '/test/project-beta'
  const preset: SkillPreset = {
    id: 'daily', name: 'Daily', description: '', revision: 1,
    members: deployed.map(skill => ({ skillId: skill.id, tools: ['codex'] })),
  }
  const preview: ApplicationRecord = {
    id: 'preview', preset, createdAt: 1, confirmed: false, remove: false, undoTargetCount: 0,
    items: ['/test/project-alpha', '/test/project-beta'].flatMap(path => preset.members.map(member => ({
      skillId: member.skillId, name: member.skillId, tools: member.tools, scope: { kind: 'project' as const, path },
      targets: [], action: 'add', status: 'pending', reason: null,
    }))),
  }
  const original = invoke.getMockImplementation()!
  invoke.mockImplementation(async (command: string) => {
    if (command === 'get_managed_skills') return deployed
    if (command === 'get_recent_projects') return recentProjects
    if (command === 'get_tool_status') return { tools: [], installed: [], newly_installed: [] }
    if (command === 'get_tool_config') return { disabled_builtin_tools: [], custom_tools: [] }
    if (command === 'workspace_presets') return [preset]
    if (command === 'workspace_history') return []
    if (command === 'workspace_preview') return preview
    if (command === 'workspace_execute') return { ...preview, confirmed: true, undoTargetCount: 4, items: preview.items.map(item => ({ ...item, status: 'done' })) }
    return original(command)
  })
  render(<App />)
  await screen.findByText('alpha')
  fireEvent.click(screen.getByRole('button', { name: i18n.t('workspace110.title') }))
  fireEvent.click(await screen.findByRole('button', { name: i18n.t('workspace110.applyNamed', { name: 'Daily' }) }))

  const projectScope = screen.getByRole('radio', { name: i18n.t('workspace110.project') }) as HTMLInputElement
  expect(projectScope.checked).toBe(false)
  fireEvent.click(projectScope)
  for (const path of expectedPaths) {
    const choices = screen.getAllByRole('checkbox', { name: path })
    expect(choices).toHaveLength(1)
    expect((choices[0] as HTMLInputElement).checked).toBe(false)
  }
  expect(screen.queryByText(i18n.t('workspace110.noProjectResults'))).toBeNull()
  fireEvent.click(screen.getByRole('checkbox', { name: '/test/project-alpha' }))
  fireEvent.click(screen.getByRole('checkbox', { name: '/test/project-beta' }))
  fireEvent.click(screen.getByRole('button', { name: i18n.t('workspace110.applyNow') }))
  await screen.findByRole('heading', { name: i18n.t('workspace110.result') })
  expect(invoke.mock.calls.filter(([command]) => command === 'workspace_preview').map(([, args]) => args)).toEqual([{ request: {
    presetId: preset.id, members: preset.members,
    scopes: [{ kind: 'project', path: '/test/project-alpha' }, { kind: 'project', path: '/test/project-beta' }],
    remove: false,
  } }])
  expect(invoke.mock.calls.filter(([command]) => command === 'workspace_execute').map(([, args]) => args)).toEqual([{ id: preview.id }])
})

it('keeps preset summaries passive and returns Skill details to the library', async () => {
  const preset: SkillPreset = {
    id: 'daily', name: 'Daily', description: '', revision: 1,
    members: [{ skillId: skills[0].id, tools: ['codex'] }],
  }
  const original = invoke.getMockImplementation()!
  invoke.mockImplementation(async (command: string) => {
    if (command === 'workspace_presets') return [preset]
    if (command === 'workspace_history') return []
    if (command === 'get_skill_files') return []
    return original(command)
  })
  await setup()
  fireEvent.click(screen.getByRole('button', { name: i18n.t('workspace110.title') }))
  fireEvent.click(await screen.findByText('Daily'))
  expect(screen.getByRole('heading', { name: i18n.t('workspace110.title') })).toBeTruthy()
  expect(screen.queryByRole('button', { name: 'global-writing' })).toBeNull()
  fireEvent.click(screen.getByRole('button', { name: i18n.t('workspace110.editNamed', { name: 'Daily' }) }))
  expect(screen.getByRole('checkbox', { name: 'global-writing' })).toBeTruthy()
  fireEvent.click(screen.getByRole('button', { name: i18n.t('workspace110.back') }))
  expect(screen.getByRole('heading', { name: i18n.t('workspace110.title') })).toBeTruthy()
  fireEvent.click(screen.getByRole('button', { name: 'My Skills 2' }))
  fireEvent.click(screen.getByRole('button', { name: 'global-writing' }))
  fireEvent.click(screen.getByRole('button', { name: i18n.t('detail.back') }))
  expect(screen.getByPlaceholderText('Search skills...')).toBeTruthy()
  fireEvent.click(screen.getByRole('button', { name: 'project-code' }))
  fireEvent.click(screen.getByRole('button', { name: i18n.t('detail.back') }))
  expect(screen.getByPlaceholderText('Search skills...')).toBeTruthy()
  expect(screen.getByRole('button', { name: 'project-code' })).toBeTruthy()
})

it('opens target repair from Skill details with the selected project preserved', async () => {
  const projectPath = '/test/project-alpha'
  const broken = { ...skills[1], targets: [{ ...skills[1].targets[0], project_path: projectPath, status: 'error', last_error: 'SKILL_ISSUE|unsafeTarget' }] }
  const original = invoke.getMockImplementation()!
  invoke.mockImplementation(async (command: string) => {
    if (command === 'get_managed_skills') return [skills[0], broken]
    if (command === 'get_recent_projects') return [projectPath]
    if (command === 'get_tool_status') return { tools: [], installed: [], newly_installed: [] }
    if (command === 'workspace_presets' || command === 'workspace_history' || command === 'get_skill_files') return []
    return original(command)
  })
  await setup()
  scope('Project 1')
  fireEvent.click(screen.getByRole('button', { name: i18n.t('workspace110.project') }))
  fireEvent.click(screen.getByRole('option', { name: /\/test\/project-alpha/ }))
  fireEvent.click(screen.getByRole('button', { name: 'project-code' }))
  fireEvent.click(screen.getByText(i18n.t('deviceSync.viewIssueReason')))
  fireEvent.click(screen.getByRole('button', { name: i18n.t('workspace110.repairTargets') }))
  const repair = screen.getByRole('dialog', { name: i18n.t('workspace110.repairTargets') })
  expect(within(repair).queryByRole('radio')).toBeNull()
  expect(within(repair).getByText('project-alpha')).toBeTruthy()
  expect(within(repair).getByText('project-code')).toBeTruthy()
  expect(document.querySelector('.detail-body')).toBeTruthy()
})

it.each(['global', 'project', 'multiple'])('repairs the actual failed targets after leaving preset management with a previous %s library filter', async previousScope => {
  const alpha = '/test/project-alpha'
  const beta = '/test/project-beta'
  const broken = { ...skills[1], targets: [{ ...skills[1].targets[0], project_path: beta, status: 'error', last_error: 'SKILL_ISSUE|unsafeTarget' }] }
  if (previousScope === 'multiple') broken.targets.push({ ...broken.targets[0], scope: 'global', project_path: '' })
  const preset: SkillPreset = { id: 'repair', name: 'Repair preset', description: '', revision: 1, members: [{ skillId: broken.id, tools: ['codex'] }] }
  const original = invoke.getMockImplementation()!
  invoke.mockImplementation(async (command: string) => {
    if (command === 'get_managed_skills') return [skills[0], broken]
    if (command === 'get_recent_projects') return [alpha, beta]
    if (command === 'workspace_presets') return [preset]
    if (command === 'workspace_history' || command === 'get_skill_files') return []
    return original(command)
  })
  await setup()
  if (previousScope === 'global') scope('Global 1')
  else {
    scope('Project 1')
    fireEvent.click(screen.getByRole('button', { name: i18n.t('workspace110.project') }))
    fireEvent.click(screen.getByRole('option', { name: /\/test\/project-alpha/ }))
  }
  fireEvent.click(screen.getByRole('button', { name: i18n.t('workspace110.title') }))
  fireEvent.click(await screen.findByText('Repair preset'))
  expect(screen.getByRole('heading', { name: i18n.t('workspace110.title') })).toBeTruthy()
  expect(screen.queryByRole('button', { name: broken.name })).toBeNull()
  fireEvent.click(screen.getByRole('button', { name: 'My Skills 2' }))
  scope('All 2')
  fireEvent.click(screen.getByRole('button', { name: broken.name }))
  fireEvent.click(screen.getByText(i18n.t('deviceSync.viewIssueReason')))
  fireEvent.click(screen.getByRole('button', { name: i18n.t('workspace110.repairTargets') }))
  if (previousScope === 'multiple') {
    expect((screen.getByRole('radio', { name: i18n.t('workspace110.project') }) as HTMLInputElement).checked).toBe(false)
    expect((screen.getByRole('radio', { name: i18n.t('workspace110.global') }) as HTMLInputElement).checked).toBe(false)
    fireEvent.click(screen.getByRole('button', { name: i18n.t('workspace110.repairToolConfirm') }))
    expect(invoke.mock.calls.some(([command]) => command === 'workspace_preview')).toBe(false)
    return
  }
  const repair = screen.getByRole('dialog', { name: i18n.t('workspace110.repairTargets') })
  expect(within(repair).queryByRole('radio')).toBeNull()
  expect(within(repair).getByText('project-beta')).toBeTruthy()
  expect(within(repair).queryByText('project-alpha')).toBeNull()
  fireEvent.click(screen.getByRole('button', { name: i18n.t('workspace110.repairToolConfirm') }))
  await vi.waitFor(() => expect(invoke.mock.calls.filter(([command]) => command === 'workspace_preview').map(([, args]) => args)).toEqual([{ request: {
    presetId: null, members: [{ skillId: broken.id, tools: ['codex'] }], scopes: [{ kind: 'project', path: beta }], remove: false,
  } }]))
})

describe('Library bulk tool assignment', () => {
  const toolStatus = {
    tools: ['codex', 'cursor', 'claude_code'].map(key => ({ key, label: key, installed: true, enabled: true, is_custom: false, skills_dir: `/test/${key}`, project_skills_dir: `.${key}/skills`, supports_project_scope: true, sync_mode: 'copy' })),
    installed: ['codex', 'cursor', 'claude_code'], newly_installed: [],
  }
  function prepareTools(deployed: ManagedSkill[], failure?: string) {
    const original = invoke.getMockImplementation()!
    invoke.mockImplementation(async (command: string, args?: Record<string, unknown>) => {
      if (command === 'get_managed_skills') return structuredClone(deployed)
      if (command === 'get_tool_status') return toolStatus
      if (command === 'get_tool_config') return { disabled_builtin_tools: [], custom_tools: [] }
      if (command === 'sync_skill_to_tool' || command === 'unsync_skill_from_tool') {
        if (args?.skillId === failure) throw new Error('Tool directory is unavailable')
        return undefined
      }
      return original(command)
    })
  }
  async function openTools() {
    fireEvent.click(screen.getByRole('button', { name: i18n.t('bulk.manage') }))
    fireEvent.click(within(document.querySelector('.bulk-action-bar') as HTMLElement).getByRole('button', { name: i18n.t('bulk.selectVisible') }))
    fireEvent.click(within(document.querySelector('.bulk-action-bar') as HTMLElement).getByRole('button', { name: i18n.t('bulk.sync') }))
    return (await screen.findByText(i18n.t('bulk.syncTitle'))).closest('[role=dialog]') as HTMLElement
  }
  const toolWrites = () => invoke.mock.calls.filter(([command]) => ['sync_skill_to_tool', 'unsync_skill_from_tool'].includes(command)).map(([command, args]) => [command, args])
  const expectNoPresetCommands = () => expect(invoke.mock.calls.some(([command]) => ['workspace_presets', 'workspace_history', 'workspace_save_preset', 'workspace_preview', 'workspace_execute'].includes(command))).toBe(false)

  it('opens the original Tools dialog, cancels without writes, and resets choices when reopened', async () => {
    prepareTools(skills)
    await setup()
    const dialog = await openTools()
    expect(within(dialog).getByText(i18n.t('bulk.syncSubtitle'))).toBeTruthy()
    expect(within(dialog).getByRole('button', { name: /Codex/ }).classList.contains('selected')).toBe(true)
    fireEvent.click(within(dialog).getByRole('button', { name: /Cursor/ }))
    fireEvent.click(within(dialog).getByRole('button', { name: i18n.t('cancel') }))
    expect(toolWrites()).toEqual([])
    expect(screen.getByText('global-writing')).toBeTruthy()
    fireEvent.click(within(document.querySelector('.bulk-action-bar') as HTMLElement).getByRole('button', { name: i18n.t('bulk.sync') }))
    const reopened = (await screen.findByText(i18n.t('bulk.syncTitle'))).closest('[role=dialog]') as HTMLElement
    expect(within(reopened).getByRole('button', { name: /Cursor/ }).classList.contains('selected')).toBe(false)
    expectNoPresetCommands()
  })

  it('applies checked tools and removes unchecked tools in each selected Skill’s existing scope', async () => {
    const deployed = structuredClone(skills)
    deployed[1].targets[0].project_path = '/test/project-alpha'
    prepareTools(deployed)
    await setup()
    const dialog = await openTools()
    fireEvent.click(within(dialog).getByRole('button', { name: /Codex/ }))
    fireEvent.click(within(dialog).getByRole('button', { name: /Cursor/ }))
    fireEvent.click(within(dialog).getByRole('button', { name: i18n.t('bulk.syncConfirm') }))
    await vi.waitFor(() => expect(document.querySelector('.bulk-action-bar')).toBeNull())
    expect(toolWrites()).toEqual([
      ['unsync_skill_from_tool', { skillId: 'global-writing', tool: 'codex', scope: 'global', projectPath: undefined }],
      ['sync_skill_to_tool', { sourcePath: '/skills/global-writing', skillId: 'global-writing', tool: 'cursor', name: 'global-writing', overwriteIfSameContent: true, scope: 'global' }],
      ['unsync_skill_from_tool', { skillId: 'project-code', tool: 'codex', scope: 'project', projectPath: '/test/project-alpha' }],
      ['sync_skill_to_tool', { sourcePath: '/skills/project-code', skillId: 'project-code', tool: 'cursor', name: 'project-code', overwriteIfSameContent: true, scope: 'project', projectPath: '/test/project-alpha' }],
    ])
    expect(screen.getByText('global-writing')).toBeTruthy()
    expectNoPresetCommands()
  })

  it.each(['global', 'project'] as const)('limits tool changes to the current %s filter', async filter => {
    const value = makeSkill('global-writing', 'global', [])
    value.targets.push(
      { ...value.targets[0], scope: 'project', project_path: '/test/project-alpha', target_path: '/test/project-alpha/skills/global-writing' },
      { ...value.targets[0], scope: 'project', project_path: '/test/project-beta', target_path: '/test/project-beta/skills/global-writing' },
    )
    prepareTools([value])
    render(<App />)
    await screen.findByText('global-writing')
    fireEvent.click(screen.getByRole('button', { name: filter === 'global' ? 'Global 1' : 'Project 1' }))
    if (filter === 'project') {
      fireEvent.click(screen.getByRole('button', { name: i18n.t('workspace110.project') }))
      fireEvent.click(screen.getByRole('option', { name: /\/test\/project-alpha/ }))
    }
    const dialog = await openTools()
    fireEvent.click(within(dialog).getByRole('button', { name: /Codex/ }))
    fireEvent.click(within(dialog).getByRole('button', { name: /Cursor/ }))
    fireEvent.click(within(dialog).getByRole('button', { name: i18n.t('bulk.syncConfirm') }))
    await vi.waitFor(() => expect(document.querySelector('.bulk-action-bar')).toBeNull())
    expect(toolWrites()).toEqual([
      ['unsync_skill_from_tool', { skillId: value.id, tool: 'codex', scope: filter, projectPath: filter === 'project' ? '/test/project-alpha' : undefined }],
      ['sync_skill_to_tool', { sourcePath: value.central_path, skillId: value.id, tool: 'cursor', name: value.name, overwriteIfSameContent: true, scope: filter, ...(filter === 'project' ? { projectPath: '/test/project-alpha' } : {}) }],
    ])
    expectNoPresetCommands()
  })

  it('keeps tools that share the selected tool’s directory instead of removing their deployment', async () => {
    const value = makeSkill('global-writing', 'global', [])
    value.targets.push({ ...value.targets[0], tool: 'cursor' })
    prepareTools([value])
    const original = invoke.getMockImplementation()!
    invoke.mockImplementation(async (command: string, args?: Record<string, unknown>) => command === 'get_tool_status'
      ? { ...toolStatus, tools: toolStatus.tools.map(tool => tool.key === 'cursor' ? { ...tool, skills_dir: '/test/codex' } : tool) }
      : original(command, args))
    render(<App />)
    await screen.findByText('global-writing')
    const dialog = await openTools()
    fireEvent.click(within(dialog).getByRole('button', { name: /Cursor/ }))
    fireEvent.click(within(dialog).getByRole('button', { name: i18n.t('bulk.syncConfirm') }))
    await vi.waitFor(() => expect(document.querySelector('.bulk-action-bar')).toBeNull())
    expect(toolWrites().map(([command, args]) => [command, args.tool, args.scope])).toEqual([
      ['sync_skill_to_tool', 'codex', 'global'], ['sync_skill_to_tool', 'cursor', 'global'],
    ])
    expectNoPresetCommands()
  })

  it('allows clearing all tools and retains selected Skills when an assignment fails', async () => {
    prepareTools(skills, 'global-writing')
    await setup()
    const dialog = await openTools()
    fireEvent.click(within(dialog).getByRole('button', { name: /Codex/ }))
    fireEvent.click(within(dialog).getByRole('button', { name: i18n.t('bulk.syncConfirm') }))
    await vi.waitFor(() => expect(toolWrites()).toHaveLength(2))
    await vi.waitFor(() => expect(screen.queryByText(i18n.t('bulk.syncTitle'))).toBeNull())
    expect(toolWrites().every(([command]) => command === 'unsync_skill_from_tool')).toBe(true)
    expect(document.querySelector('.bulk-action-bar')).toBeTruthy()
    expect(screen.getAllByRole('checkbox', { name: i18n.t('bulk.toggleSkill') }).every(input => (input as HTMLInputElement).checked)).toBe(true)
    expectNoPresetCommands()
  })
})

it.each(['list', 'cards'] as const)('removes from the selected project directly without dialogs in %s view', async view => {
  const alpha = '/test/project-alpha', beta = '/test/project-beta'
  const skill = makeSkill('project-skill', 'global', [])
  skill.targets.push(
    { ...skill.targets[0], scope: 'project', project_path: alpha, target_path: `${alpha}/codex/project-skill` },
    { ...skill.targets[0], tool: 'cursor', scope: 'project', project_path: alpha, target_path: `${alpha}/cursor/project-skill` },
    { ...skill.targets[0], tool: 'claude_code', scope: 'project', project_path: beta, target_path: `${beta}/claude/project-skill` },
  )
  const originalTargets = structuredClone(skill.targets)
  const plan: ApplicationRecord = { id: 'remove-alpha', preset: null, createdAt: 1, confirmed: false, remove: true, undoTargetCount: 0,
    items: [{ skillId: skill.id, name: skill.name, tools: ['codex', 'cursor'], scope: { kind: 'project', path: alpha }, targets: [], action: 'remove', status: 'pending', reason: null }],
  }
  const original = invoke.getMockImplementation()!
  invoke.mockImplementation(async (command: string) => {
    if (command === 'get_managed_skills') return structuredClone([skill])
    if (command === 'get_tool_status') return { tools: [], installed: [], newly_installed: [] }
    if (command === 'get_tool_config') return { disabled_builtin_tools: [], custom_tools: [] }
    if (command === 'workspace_presets' || command === 'workspace_history') return []
    if (command === 'workspace_preview') return plan
    if (command === 'workspace_execute') {
      skill.targets = skill.targets.filter(target => target.project_path !== alpha)
      return { ...plan, confirmed: true, items: plan.items.map(item => ({ ...item, status: 'done' })) }
    }
    return original(command)
  })
  render(<App />)
  await screen.findByText(skill.name)
  fireEvent.click(screen.getByRole('button', { name: i18n.t(`viewMode.${view}`) }))
  fireEvent.click(screen.getByRole('button', { name: 'Project 1' }))
  fireEvent.click(screen.getByRole('button', { name: i18n.t('workspace110.project') }))
  fireEvent.click(screen.getByRole('option', { name: new RegExp(alpha) }))
  const removeButton = screen.getByRole('button', { name: i18n.t('workspace110.removeTargets') })
  fireEvent.click(removeButton)
  expect(screen.queryByRole('dialog')).toBeNull()
  expect(document.querySelector('.skills-library')).toBeTruthy()
  expect(document.querySelector('.workspace-preset-grid')).toBeNull()
  await waitFor(() => expect(skill.targets).toEqual(originalTargets.filter(target => target.project_path !== alpha)))
  await waitFor(() => expect(document.querySelector('main')?.hasAttribute('inert')).toBe(false))
  expect(screen.queryByRole('dialog')).toBeNull()
  expect(invoke.mock.calls.filter(([command]) => command === 'workspace_preview').map(([, args]) => args)).toEqual([{ request: {
    presetId: null, members: [{ skillId: skill.id, tools: ['codex', 'cursor'] }], scopes: [{ kind: 'project', path: alpha }], remove: true,
  } }])
  expect(invoke.mock.calls.filter(([command]) => command === 'workspace_execute').map(([, args]) => args)).toEqual([{ id: plan.id }])
  expect(invoke.mock.calls.some(([command]) => ['delete_managed_skill', 'workspace_save_preset', 'workspace_presets'].includes(command))).toBe(false)
  expect(skill.targets).toEqual(originalTargets.filter(target => target.project_path !== alpha))
  expect(skill.targets.map(target => target.project_path ?? 'global')).toEqual(['global', beta])
})

describe('Library contextual tool actions', () => {
  const alpha = '/test/project-alpha', beta = '/test/project-beta'
  function prepare(state: 'synced' | 'missing' | 'failed', options: { fail?: boolean; shared?: boolean } = {}) {
    const skill = makeSkill('tool-action', 'global', [])
    skill.source_type = 'git'; skill.source_ref = 'https://example.test/skills.git'
    const target = skill.targets[0]
    const targetFor = (tool: string, scope: 'global' | 'project', path?: string, status = 'ok') => ({ ...target, tool, scope, project_path: path, status, target_path: `${path ?? '/test/global'}/${options.shared ? 'shared' : tool}/tool-action` })
    skill.targets = [
      targetFor('cursor', 'global'),
      targetFor('cursor', 'project', alpha),
      targetFor('codex', 'project', beta),
      ...(state === 'missing' ? [] : [
        targetFor('codex', 'global', undefined, state === 'failed' ? 'error' : 'ok'),
        targetFor('codex', 'project', alpha, state === 'failed' ? 'error' : 'ok'),
      ]),
    ]
    const original = invoke.getMockImplementation()!
    invoke.mockImplementation(async (command: string, args?: Record<string, unknown>) => {
      if (command === 'get_managed_skills') return structuredClone([skill])
      if (command === 'get_tool_config') return { disabled_builtin_tools: [], custom_tools: [] }
      if (command === 'get_tool_status') return { tools: ['codex', 'cursor'].map(key => ({ key, label: key, enabled: true, installed: true, skills_dir: `/test/${options.shared ? 'shared' : key}`, project_skills_dir: `.${options.shared ? 'shared' : key}/skills`, supports_project_scope: true, sync_mode: 'copy', is_custom: false })), installed: ['codex', 'cursor'], newly_installed: [] }
      if (command === 'sync_skill_to_tool' || command === 'unsync_skill_from_tool') {
        if (options.fail) throw new Error('TOOL_NOT_WRITABLE|codex|/test/readonly')
        const data = args as { tool: string; sharedTools?: string[]; scope: 'global' | 'project'; projectPath?: string }
        const affected = data.sharedTools ?? [data.tool]
        skill.targets = skill.targets.filter(target => !(affected.includes(target.tool) && target.scope === data.scope && target.project_path === data.projectPath))
        if (command === 'sync_skill_to_tool') skill.targets.push(...affected.map(tool => targetFor(tool, data.scope, data.projectPath)))
        return
      }
      if (command === 'workspace_presets' || command === 'workspace_history') return []
      return original(command)
    })
    return skill
  }
  async function selectProject() {
    fireEvent.click(screen.getByRole('button', { name: 'Project 1' }))
    fireEvent.click(screen.getByRole('button', { name: i18n.t('workspace110.project') }))
    fireEvent.click(screen.getByRole('option', { name: new RegExp(alpha) }))
  }
  it.each(['list', 'cards'] as const)('toggles tools directly in %s view without opening confirmation or result dialogs', async view => {
    const skill = prepare('synced')
    render(<App />)
    await screen.findByText('tool-action')
    fireEvent.click(screen.getByRole('button', { name: i18n.t(`viewMode.${view}`) }))
    await selectProject()
    const toolButton = () => within(document.querySelector('.skill-tool-avatars') as HTMLElement).getByRole('button', { name: /Codex/ })
    fireEvent.click(toolButton())
    await waitFor(() => expect(skill.targets.some(t => t.tool === 'codex' && t.project_path === alpha)).toBe(false))
    await waitFor(() => expect(toolButton().getAttribute('aria-pressed')).toBe('false'))
    expect(screen.queryByRole('dialog')).toBeNull()
    fireEvent.click(toolButton())
    await waitFor(() => expect(toolButton().getAttribute('aria-pressed')).toBe('true'))
    expect(skill.targets.some(t => t.tool === 'codex' && t.scope === 'global')).toBe(true)
    expect(skill.targets.some(t => t.tool === 'codex' && t.project_path === beta)).toBe(true)
    expect(skill.targets.some(t => t.tool === 'cursor' && t.project_path === alpha)).toBe(true)
    expect(invoke.mock.calls.filter(([c]) => ['sync_skill_to_tool', 'unsync_skill_from_tool'].includes(c)).map(([c, args]) => [c, args])).toEqual([
      ['unsync_skill_from_tool', { skillId: skill.id, tool: 'codex', scope: 'project', projectPath: alpha }],
      ['sync_skill_to_tool', { skillId: skill.id, sourcePath: skill.central_path, name: skill.name, tool: 'codex', scope: 'project', projectPath: alpha, overwriteIfSameContent: true }],
    ])
    expect(invoke.mock.calls.some(([c]) => ['workspace_preview', 'workspace_execute', 'workspace_presets', 'workspace_save_preset'].includes(c))).toBe(false)
    expect(screen.queryByRole('dialog')).toBeNull()
  })
  it.each([
    ['global', 'synced'], ['project', 'synced'], ['global', 'missing'], ['project', 'missing'], ['global', 'failed'], ['project', 'failed'], ['all', 'failed'],
  ] as const)('immediately applies %s tool %s actions using the existing synchronization commands', async (context, state) => {
    const skill = prepare(state)
    render(<App />)
    await screen.findByText('tool-action')
    if (context === 'project') await selectProject()
    else if (context === 'global') fireEvent.click(screen.getByRole('button', { name: 'Global 1' }))
    fireEvent.click(within(document.querySelector('.skill-tool-avatars') as HTMLElement).getByRole('button', { name: /Codex/ }))
    const command = state === 'synced' ? 'unsync_skill_from_tool' : 'sync_skill_to_tool'
    const paths = context === 'all' ? [alpha, beta] : context === 'project' ? [alpha] : [undefined]
    await waitFor(() => expect(invoke.mock.calls.filter(([c]) => c === command)).toHaveLength(paths.length))
    expect(invoke.mock.calls.filter(([c]) => c === command).map(([,args]) => args)).toEqual(paths.map(projectPath => ({
      skillId: skill.id, tool: 'codex', scope: context === 'global' ? 'global' : 'project',
      ...(projectPath ? { projectPath } : {}),
      ...(state !== 'synced' ? { sourcePath: skill.central_path, name: skill.name, overwriteIfSameContent: true } : {}),
    })))
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull())
    expect(document.querySelector('.skills-library')).toBeTruthy()
    expect(invoke.mock.calls.some(([c]) => ['workspace_preview', 'workspace_execute', 'workspace_presets', 'workspace_save_preset'].includes(c))).toBe(false)
  })
  it('keeps failures visible and allows retry directly from the tool icon', async () => {
    prepare('failed', { fail: true })
    render(<App />)
    await screen.findByText('tool-action')
    await selectProject()
    const trigger = () => within(document.querySelector('.skill-tool-avatars') as HTMLElement).getByRole('button', { name: /Codex/ })
    fireEvent.click(trigger())
    await screen.findByText(i18n.t('errors.toolNotWritable', { tool: 'codex', path: '/test/readonly' }))
    await waitFor(() => expect((trigger() as HTMLButtonElement).disabled).toBe(false))
    expect(screen.queryByRole('dialog')).toBeNull()
    fireEvent.click(trigger())
    await waitFor(() => expect(invoke.mock.calls.filter(([c]) => c === 'sync_skill_to_tool')).toHaveLength(2))
    expect(invoke.mock.calls.some(([c]) => c === 'unsync_skill_from_tool')).toBe(false)
  })
  it('preserves the existing shared-directory warning before affecting another tool', async () => {
    prepare('synced', { shared: true })
    render(<App />)
    await screen.findByText('tool-action')
    await selectProject()
    fireEvent.click(within(document.querySelector('.skill-tool-avatars') as HTMLElement).getByRole('button', { name: /Codex/ }))
    await screen.findByRole('dialog')
    expect(invoke.mock.calls.some(([c]) => ['sync_skill_to_tool', 'unsync_skill_from_tool', 'workspace_preview'].includes(c))).toBe(false)
    expect(screen.queryByRole('heading', { name: i18n.t('workspace110.removeTargets') })).toBeNull()
  })
  it.each([
    ['global', 'missing'], ['project', 'missing'], ['global', 'synced'], ['project', 'synced'], ['global', 'failed'], ['project', 'failed'],
  ] as const)('applies the confirmed shared-tool group together for %s %s targets', async (context, state) => {
    const skill = prepare(state, { shared: true })
    const before = structuredClone(skill.targets)
    render(<App />)
    await screen.findByText('tool-action')
    if (context === 'project') await selectProject()
    else fireEvent.click(screen.getByRole('button', { name: 'Global 1' }))
    const toolButton = (tool: string) => within(document.querySelector('.skill-tool-avatars') as HTMLElement).getByRole('button', { name: new RegExp(tool) })
    fireEvent.click(toolButton('Codex'))
    let dialog = await screen.findByRole('dialog')
    expect(within(dialog).getByText(i18n.t('sharedDirConfirm', { tool: 'Codex', others: 'Cursor' }))).toBeTruthy()
    fireEvent.click(within(dialog).getByRole('button', { name: i18n.t('cancel') }))
    expect(skill.targets).toEqual(before)
    expect(invoke.mock.calls.some(([c]) => ['sync_skill_to_tool', 'unsync_skill_from_tool'].includes(c))).toBe(false)
    fireEvent.click(toolButton('Codex'))
    dialog = await screen.findByRole('dialog')
    fireEvent.click(within(dialog).getByRole('button', { name: i18n.t('confirm') }))
    const command = state === 'synced' ? 'unsync_skill_from_tool' : 'sync_skill_to_tool'
    if (state === 'synced') {
      // Removing the last tools also removes this Skill from the current scope's list.
      await waitFor(() => expect(screen.queryByRole('button', { name: skill.name })).toBeNull())
    } else {
      await waitFor(() => expect(toolButton('Codex').getAttribute('aria-pressed')).toBe('true'))
      await waitFor(() => expect(toolButton('Cursor').getAttribute('aria-pressed')).toBe('true'))
    }
    expect(invoke.mock.calls.filter(([c]) => c === command).map(([,args]) => args)).toEqual([{
      skillId: skill.id, tool: 'codex', sharedTools: ['codex', 'cursor'], scope: context,
      ...(context === 'project' ? { projectPath: alpha } : {}),
      ...(state !== 'synced' ? { sourcePath: skill.central_path, name: skill.name, overwriteIfSameContent: true } : {}),
    }])
    expect(skill.targets.filter(t => context === 'global' ? t.scope !== 'global' : t.project_path !== alpha))
      .toEqual(before.filter(t => context === 'global' ? t.scope !== 'global' : t.project_path !== alpha))
    expect(screen.queryByRole('dialog')).toBeNull()
  })
  it('keeps both shared tool states unchanged when group synchronization fails', async () => {
    const skill = prepare('missing', { shared: true, fail: true })
    const before = structuredClone(skill.targets)
    render(<App />)
    await screen.findByText('tool-action')
    await selectProject()
    fireEvent.click(within(document.querySelector('.skill-tool-avatars') as HTMLElement).getByRole('button', { name: /Codex/ }))
    const dialog = await screen.findByRole('dialog')
    fireEvent.click(within(dialog).getByRole('button', { name: i18n.t('confirm') }))
    await screen.findByText(i18n.t('errors.toolNotWritable', { tool: 'codex', path: '/test/readonly' }))
    expect(skill.targets).toEqual(before)
    expect(invoke.mock.calls.filter(([c]) => c === 'sync_skill_to_tool')).toHaveLength(1)
    expect(invoke.mock.calls.find(([c]) => c === 'sync_skill_to_tool')?.[1].sharedTools).toEqual(['codex', 'cursor'])
  })
  it('routes the card issue repair to failed tools in the selected project and restores its trigger on cancel', async () => {
    prepare('failed')
    render(<App />)
    await screen.findByText('tool-action')
    await selectProject()
    const trigger = screen.getByRole('button', { name: i18n.t('deviceSync.viewIssueReason') })
    fireEvent.click(trigger)
    fireEvent.click(screen.getByRole('button', { name: i18n.t('workspace110.repairTargets') }))
    const dialog = await screen.findByRole('dialog', { name: i18n.t('workspace110.repairTargets') })
    expect(within(dialog).getByText('project-alpha')).toBeTruthy()
    expect(within(dialog).getByText('Codex')).toBeTruthy()
    expect(within(dialog).queryByText('Cursor')).toBeNull()
    expect(within(dialog).queryByRole('radio')).toBeNull()
    expect(document.querySelector('.skill-issue-popover')).toBeNull()
    fireEvent.click(within(dialog).getByRole('button', { name: i18n.t('workspace110.cancel') }))
    await waitFor(() => expect(document.activeElement).toBe(trigger))
    expect(invoke.mock.calls.some(([command]) => ['workspace_preview', 'workspace_execute', 'workspace_presets'].includes(command))).toBe(false)
  })
  it('opens target management over the current project without adding unrelated Skills', async () => {
    prepare('synced')
    render(<App />)
    await screen.findByText('tool-action')
    await selectProject()
    fireEvent.click(screen.getByRole('button', { name: i18n.t('workspace110.manageTargets') }))
    const dialog = await screen.findByRole('dialog', { name: i18n.t('workspace110.manageTargets') })
    expect(document.querySelector('.skills-library')).toBeTruthy()
    expect(within(dialog).getByRole('radio', { name: i18n.t('workspace110.project') }).getAttribute('checked')).not.toBeNull()
    expect(within(dialog).queryByText(i18n.t('workspace110.applyDialog'))).toBeNull()
    expect(within(dialog).queryByRole('button', { name: i18n.t('workspace110.addMembers') })).toBeNull()
    expect(within(dialog).queryByLabelText(i18n.t('workspace110.override'))).toBeNull()
    fireEvent.click(within(dialog).getByRole('button', { name: i18n.t('workspace110.cancel') }))
    expect(invoke.mock.calls.some(([command]) => ['workspace_preview', 'workspace_execute', 'workspace_presets'].includes(command))).toBe(false)
  })
})
