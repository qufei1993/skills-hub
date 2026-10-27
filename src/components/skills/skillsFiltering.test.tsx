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
  fireEvent.click(await screen.findByText('Daily'))
  fireEvent.click(screen.getByRole('button', { name: i18n.t('workspace110.apply') }))

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

it('returns a library Skill to the library after leaving a preset member through the sidebar', async () => {
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
  fireEvent.click(screen.getByRole('button', { name: 'global-writing' }))
  fireEvent.click(screen.getByRole('button', { name: i18n.t('detail.back') }))
  expect(screen.getByRole('heading', { name: 'Daily' })).toBeTruthy()

  fireEvent.click(screen.getByRole('button', { name: 'global-writing' }))
  fireEvent.click(screen.getByRole('button', { name: 'My Skills 2' }))
  fireEvent.click(screen.getByRole('button', { name: 'project-code' }))
  fireEvent.click(screen.getByRole('button', { name: i18n.t('detail.back') }))
  expect(screen.getByPlaceholderText('Search skills...')).toBeTruthy()
  expect(screen.getByRole('button', { name: 'project-code' })).toBeTruthy()
  expect(screen.queryByRole('heading', { name: 'Daily' })).toBeNull()
})

it('opens target repair from Skill details with the selected project preserved', async () => {
  const projectPath = '/test/project-alpha'
  const broken = { ...skills[1], targets: [{ ...skills[1].targets[0], project_path: projectPath, status: 'error', last_error: 'SKILL_ISSUE|unsafeTarget' }] }
  const original = invoke.getMockImplementation()!
  invoke.mockImplementation(async (command: string) => {
    if (command === 'get_managed_skills') return [skills[0], broken]
    if (command === 'get_recent_projects') return [projectPath]
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
  expect((screen.getByRole('radio', { name: i18n.t('workspace110.project') }) as HTMLInputElement).checked).toBe(true)
  expect((screen.getByRole('checkbox', { name: projectPath }) as HTMLInputElement).checked).toBe(true)
  expect((screen.getByRole('checkbox', { name: /project-code/ }) as HTMLInputElement).checked).toBe(true)
})

it.each(['global', 'project', 'multiple'])('repairs a preset member’s actual failed project instead of the old %s library filter', async previousScope => {
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
  fireEvent.click(screen.getByRole('button', { name: broken.name }))
  fireEvent.click(screen.getByText(i18n.t('deviceSync.viewIssueReason')))
  fireEvent.click(screen.getByRole('button', { name: i18n.t('workspace110.repairTargets') }))
  if (previousScope === 'multiple') {
    expect((screen.getByRole('radio', { name: i18n.t('workspace110.project') }) as HTMLInputElement).checked).toBe(false)
    expect((screen.getByRole('radio', { name: i18n.t('workspace110.global') }) as HTMLInputElement).checked).toBe(false)
    fireEvent.click(screen.getByRole('button', { name: i18n.t('workspace110.applyNow') }))
    expect(invoke.mock.calls.some(([command]) => command === 'workspace_preview')).toBe(false)
    return
  }
  expect((screen.getByRole('radio', { name: i18n.t('workspace110.project') }) as HTMLInputElement).checked).toBe(true)
  expect((screen.getByRole('checkbox', { name: beta }) as HTMLInputElement).checked).toBe(true)
  expect((screen.getByRole('checkbox', { name: alpha }) as HTMLInputElement).checked).toBe(false)
  fireEvent.click(screen.getByRole('button', { name: i18n.t('workspace110.applyNow') }))
  await vi.waitFor(() => expect(invoke.mock.calls.filter(([command]) => command === 'workspace_preview').map(([, args]) => args)).toEqual([{ request: {
    presetId: null, members: [{ skillId: broken.id, tools: ['codex'] }], scopes: [{ kind: 'project', path: beta }], remove: false,
  } }]))
})
