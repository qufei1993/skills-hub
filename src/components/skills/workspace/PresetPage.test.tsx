// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import i18n from '../../../i18n'
import PresetPage from './PresetPage'
import type { ApplicationRecord, ManagedSkill, SkillPreset, WorkspaceUndoItem } from '../types'
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }))
const skills: ManagedSkill[] = ['alpha', 'beta'].map(name => ({ id: name, name, source_type: 'local', central_path: `/central/${name}`, created_at: 1, updated_at: 1, enabled: true, status: 'ok', tags: [], targets: [] }))
const preset: SkillPreset = { id: 'p1', revision: 1, name: 'Daily', description: '', members: [{ skillId: 'alpha', tools: ['codex'] }, { skillId: 'beta', tools: ['cursor'] }] }
const invoke = vi.fn()
const onBack = vi.fn()
const props = { active: true, skills, tools: [{ id: 'codex', label: 'Codex' }, { id: 'cursor', label: 'Cursor' }], projects: ['/project'], invokeTauri: invoke, onChanged: async () => {}, onBack, onSkill: vi.fn(), onProject: vi.fn(), onDirtyChange: vi.fn() }
const expand = (name: string) => {
  const control = screen.getByText(name, { selector: 'summary' })
  if (!control.parentElement?.hasAttribute('open')) fireEvent.click(control)
}
const toolChoice = (group: string, tool: string) => {
  const control = screen.queryByLabelText(`Adjust tools for ${group}`)
  if (control && !control.parentElement?.hasAttribute('open')) fireEvent.click(control)
  return within(screen.getByRole('group', { name: group })).getByRole('checkbox', { name: tool }) as HTMLInputElement
}
const adjustApplication = () => expand('Adjust for this application')
const simulateInertBlur = () => {
  const setAttribute = Element.prototype.setAttribute
  const blurred = vi.fn()
  vi.spyOn(Element.prototype, 'setAttribute').mockImplementation(function (this: Element, name: string, value: string) {
    setAttribute.call(this, name, value)
    // jsdom does not implement the WebKit focus loss caused by an inert ancestor.
    if (name === 'inert' && document.activeElement instanceof HTMLElement && this.contains(document.activeElement)) {
      document.activeElement.blur()
      blurred()
    }
  })
  return blurred
}

const applicationPlan = (overrides: Partial<ApplicationRecord> = {}): ApplicationRecord => ({
  id: 'a1', preset, createdAt: 1, confirmed: false, remove: false, undoTargetCount: 0,
  items: preset.members.map(member => ({ skillId: member.skillId, name: member.skillId, tools: member.tools, scope: { kind: 'global' }, targets: [], action: 'add', status: 'pending', reason: null })),
  ...overrides,
})
beforeEach(async () => { await i18n.changeLanguage('en'); invoke.mockReset(); invoke.mockImplementation(async command => command === 'workspace_presets' ? [preset] : []); onBack.mockReset() })
afterEach(() => { cleanup(); vi.restoreAllMocks() })
it('loads presets independently and does not read application history until requested', async () => {
  let resolvePresets!: (value: SkillPreset[]) => void
  let resolveHistory!: (value: ApplicationRecord[]) => void
  const pendingPresets = new Promise<SkillPreset[]>(resolve => { resolvePresets = resolve })
  const pendingHistory = new Promise<ApplicationRecord[]>(resolve => { resolveHistory = resolve })
  invoke.mockImplementation(command => command === 'workspace_presets' ? pendingPresets : pendingHistory)
  render(<PresetPage {...props} t={i18n.t} />)
  expect(screen.queryByText('No presets yet. Save a reusable set of Skills.')).toBeNull()
  expect(invoke.mock.calls.some(([command]) => command === 'workspace_history')).toBe(false)

  await act(async () => { resolvePresets([preset]) })
  expect(screen.getByRole('button', { name: 'Apply Daily' })).toBeTruthy()
  expect(invoke.mock.calls.some(([command]) => command === 'workspace_history')).toBe(false)
  fireEvent.click(screen.getByRole('button', { name: 'All application records' }))
  expect(screen.queryByText('No application records yet.')).toBeNull()
  await act(async () => { resolveHistory([]) })
  expect(screen.getByText('No application records yet.')).toBeTruthy()
})
it('keeps history failures local while presets remain usable and retries the history read', async () => {
  let historyReads = 0
  let rejectPrevious!: (error: Error) => void
  const previousRead = new Promise<ApplicationRecord[]>((_, reject) => { rejectPrevious = reject })
  invoke.mockImplementation(async command => {
    if (command === 'workspace_history') {
      if (++historyReads === 1) throw new Error('read failed')
      if (historyReads === 2) return previousRead
      return []
    }
    return [preset]
  })
  render(<PresetPage {...props} t={i18n.t} />)
  await screen.findByRole('button', { name: 'Apply Daily' })
  expect(screen.queryByRole('alert')).toBeNull()
  fireEvent.click(screen.getByRole('button', { name: 'All application records' }))
  await screen.findByRole('alert')
  expect(screen.queryByText('No application records yet.')).toBeNull()
  fireEvent.click(screen.getByRole('button', { name: 'Presets' }))
  expect(screen.getByRole('button', { name: 'Apply Daily' })).toBeTruthy()
  fireEvent.click(screen.getByRole('button', { name: 'Apply Daily' }))
  expect(screen.getByRole('dialog', { name: 'Apply preset' })).toBeTruthy()
  fireEvent.click(screen.getByRole('button', { name: 'Cancel' }))
  fireEvent.click(screen.getByRole('button', { name: 'All application records' }))
  fireEvent.click(screen.getByRole('button', { name: 'Reload records' }))
  expect(await screen.findByText('No application records yet.')).toBeTruthy()
  await act(async () => { rejectPrevious(new Error('stale read failed')) })
  expect(screen.queryByRole('alert')).toBeNull()
})
it('keeps a failed preset read distinct from an empty directory', async () => {
  invoke.mockRejectedValue(new Error('preset read failed'))
  render(<PresetPage {...props} t={i18n.t} />)
  await screen.findByRole('alert')
  expect(screen.queryByText('No presets yet. Save a reusable set of Skills.')).toBeNull()
  invoke.mockResolvedValue([])
  fireEvent.click(screen.getByRole('button', { name: 'Reload records' }))
  expect(await screen.findByText('No presets yet. Save a reusable set of Skills.')).toBeTruthy()
})
it('shows each preset card\'s own members and tools and opens the corresponding detail', async () => {
  const daily = { ...preset, members: [{ skillId: 'alpha', tools: ['codex'] }] }
  const weekend = { ...preset, id: 'p2', name: 'Weekend', members: [{ skillId: 'beta', tools: ['cursor'] }] }
  invoke.mockImplementation(async command => command === 'workspace_presets' ? [daily, weekend] : [])
  render(<PresetPage {...props} t={i18n.t} />)

  const dailyCard = await screen.findByRole('button', { name: 'Daily' })
  const weekendCard = screen.getByRole('button', { name: 'Weekend' })
  expect(dailyCard.tagName).toBe('BUTTON')
  expect(weekendCard.tagName).toBe('BUTTON')
  expect(within(dailyCard).getByText('alpha')).toBeTruthy()
  expect(within(dailyCard).queryByText('beta')).toBeNull()
  expect(within(dailyCard).getByTitle('Codex')).toBeTruthy()
  expect(within(dailyCard).queryByTitle('Cursor')).toBeNull()
  expect(within(weekendCard).getByText('beta')).toBeTruthy()
  expect(within(weekendCard).queryByText('alpha')).toBeNull()
  expect(within(weekendCard).getByTitle('Cursor')).toBeTruthy()
  expect(within(weekendCard).queryByTitle('Codex')).toBeNull()

  fireEvent.click(dailyCard)
  expect(screen.getByRole('heading', { level: 1, name: 'Daily' })).toBeTruthy()
  expect(screen.getByRole('button', { name: 'alpha' })).toBeTruthy()
  expect(screen.queryByRole('button', { name: 'beta' })).toBeNull()
  expect(screen.getByText('Codex')).toBeTruthy()
  expect(screen.queryByText('Cursor')).toBeNull()

  fireEvent.click(screen.getByRole('button', { name: 'Back' }))
  fireEvent.click(screen.getByRole('button', { name: 'Weekend' }))
  expect(screen.getByRole('heading', { level: 1, name: 'Weekend' })).toBeTruthy()
  expect(screen.getByRole('button', { name: 'beta' })).toBeTruthy()
  expect(screen.queryByRole('button', { name: 'alpha' })).toBeNull()
  expect(screen.getByText('Cursor')).toBeTruthy()
  expect(screen.queryByText('Codex')).toBeNull()
  expect(invoke.mock.calls.some(([command]) => ['workspace_save_preset', 'workspace_preview', 'workspace_execute'].includes(command))).toBe(false)
})
it('summarizes actual history outcomes with their scopes and target count', async () => {
  const record: ApplicationRecord = {
    id: 'mixed-outcomes', preset, createdAt: 1, confirmed: true, remove: false, undoTargetCount: 0,
    items: ['done', 'failed', 'blocked', 'undone'].map((status, index): ApplicationRecord['items'][number] => ({
      skillId: index % 2 ? 'beta' : 'alpha', name: index % 2 ? 'beta' : 'alpha', tools: ['codex'],
      scope: index < 2 ? { kind: 'global' } : { kind: 'project', path: '/project' },
      targets: [{ agents: ['codex'], affected_agents: ['codex'], path: `/target-${index}`, mode: 'copy', exists: true }],
      // Every item was planned as an addition; the summary must describe what actually happened.
      action: 'add', status, reason: null,
    })),
  }
  invoke.mockImplementation(async command => command === 'workspace_history' ? [record] : [])
  render(<PresetPage {...props} t={i18n.t} />)
  fireEvent.click(screen.getByRole('button', { name: 'All application records' }))

  const historyCard = await screen.findByRole('button', { name: /Daily/ })
  for (const status of ['Complete', 'Failed', 'Blocked', 'Undone']) {
    expect(historyCard.textContent).toMatch(new RegExp(`${status}\\s+1`))
  }
  expect(historyCard.textContent).toContain('Global')
  expect(within(historyCard).getByTitle('/project').textContent).toBe('project')
  expect(historyCard.textContent).toContain('2 Skills')
  expect(historyCard.textContent).not.toMatch(/Add\s+4/)
  fireEvent.click(historyCard)
  expect(screen.getByRole('heading', { name: 'project' }).getAttribute('title')).toBe('/project')
})
it('labels common tools as applying to every selected member only while the override is enabled', () => {
  render(<PresetPage {...props} intent={{ mode: 'select', members: preset.members }} t={i18n.t} />)
  adjustApplication()
  const add = screen.queryByRole('button', { name: 'Add Skills' })
  if (add) fireEvent.click(add)
  expect(screen.getByText('Default tools for newly selected members')).toBeTruthy()
  expect(screen.queryByText('Common tools for this application')).toBeNull()

  fireEvent.click(screen.getByLabelText('Use common tools for this application'))
  expect(screen.getByText('Common tools for this application')).toBeTruthy()
  expect(screen.queryByText('Default tools for newly selected members')).toBeNull()

  fireEvent.click(screen.getByLabelText('Use common tools for this application'))
  expect(screen.getByText('Default tools for newly selected members')).toBeTruthy()
  expect(screen.queryByText('Common tools for this application')).toBeNull()
})
it('describes unmatched project searches separately from Skill searches', () => {
  render(<PresetPage {...props} projects={['/project', '/a', '/b', '/c', '/d', '/e']} intent={{ mode: 'select', members: preset.members, scope: { kind: 'project', path: '/project' } }} t={i18n.t} />)
  fireEvent.change(screen.getByLabelText('Search project paths'), { target: { value: 'missing-project' } })
  expect(screen.getByText('No matching projects.')).toBeTruthy()
  expect(screen.queryByText('No matching Skills.')).toBeNull()
  adjustApplication()
  expect(screen.getByRole('checkbox', { name: 'alpha' })).toBeTruthy()
})
it('saves exact per-member tools without any deployment', async () => {
  render(<PresetPage {...props} intent={{ mode: 'edit', members: preset.members }} t={i18n.t} />)
  fireEvent.change(screen.getByLabelText('Name'), { target: { value: 'Project configuration' } })
  invoke.mockImplementation(async (command, args) => command === 'workspace_save_preset' ? { ...args.preset, id: 'new', revision: 1 } : command === 'workspace_presets' ? [preset] : [])
  fireEvent.click(screen.getByRole('button', { name: 'Save preset' }))
  await waitFor(() => expect(invoke).toHaveBeenCalledWith('workspace_save_preset', { preset: { ...preset, id: '', revision: 0, name: 'Project configuration' } }))
  expect(invoke.mock.calls.some(([name]) => ['workspace_execute', 'sync_skill_to_tool'].includes(name))).toBe(false)
})
it('keeps each member selected and isolates its tool changes when saving an existing preset', async () => {
  invoke.mockImplementation(async (command, args) => command === 'workspace_save_preset' ? { ...args.preset, revision: 2 } : command === 'workspace_presets' ? [preset] : [])
  render(<PresetPage {...props} t={i18n.t} />)
  fireEvent.click(await screen.findByText('Daily'))
  fireEvent.click(screen.getByRole('button', { name: 'Edit preset' }))

  fireEvent.click(toolChoice('alpha', 'Cursor'))
  fireEvent.click(toolChoice('alpha', 'Codex'))
  expect(toolChoice('alpha', 'Cursor').checked).toBe(true)
  expect(toolChoice('alpha', 'Codex').checked).toBe(false)
  expect(toolChoice('beta', 'Cursor').checked).toBe(true)
  expect(toolChoice('beta', 'Codex').checked).toBe(false)
  fireEvent.click(toolChoice('beta', 'Codex'))
  expect(toolChoice('alpha', 'Codex').checked).toBe(false)
  for (const name of ['alpha', 'beta']) {
    expect((screen.getByRole('checkbox', { name }) as HTMLInputElement).checked).toBe(true)
  }

  fireEvent.click(screen.getByRole('button', { name: 'Save preset' }))
  await screen.findByRole('heading', { level: 1, name: 'Daily' })
  expect(invoke).toHaveBeenCalledWith('workspace_save_preset', { preset: {
    ...preset,
    members: [{ skillId: 'alpha', tools: ['cursor'] }, { skillId: 'beta', tools: ['cursor', 'codex'] }],
  } })
  expect(invoke.mock.calls.some(([name]) => ['workspace_preview', 'workspace_execute', 'sync_skill_to_tool'].includes(name))).toBe(false)
})
it('uses default tools only for newly selected members and preserves existing mappings', async () => {
  invoke.mockImplementation(async (command, args) => command === 'workspace_save_preset' ? { ...args.preset, id: 'new', revision: 1 } : command === 'workspace_presets' ? [preset] : [])
  render(<PresetPage {...props} intent={{ mode: 'edit', members: [preset.members[0]] }} t={i18n.t} />)
  fireEvent.change(screen.getByLabelText('Name'), { target: { value: 'New mappings' } })
  const defaults = 'Default tools for newly selected members'
  fireEvent.click(toolChoice(defaults, 'Cursor'))
  expect(toolChoice('alpha', 'Codex').checked).toBe(true)
  expect(toolChoice('alpha', 'Cursor').checked).toBe(false)
  expect(screen.queryByRole('group', { name: 'beta' })).toBeNull()

  fireEvent.click(screen.getByRole('checkbox', { name: 'beta' }))
  expect(toolChoice('beta', 'Codex').checked).toBe(false)
  expect(toolChoice('beta', 'Cursor').checked).toBe(true)
  fireEvent.click(toolChoice(defaults, 'Cursor'))
  fireEvent.click(toolChoice(defaults, 'Codex'))
  expect(toolChoice('alpha', 'Codex').checked).toBe(true)
  expect(toolChoice('alpha', 'Cursor').checked).toBe(false)
  expect(toolChoice('beta', 'Codex').checked).toBe(false)
  expect(toolChoice('beta', 'Cursor').checked).toBe(true)

  fireEvent.click(screen.getByRole('button', { name: 'Save preset' }))
  await screen.findByRole('heading', { level: 1, name: 'New mappings' })
  expect(invoke).toHaveBeenCalledWith('workspace_save_preset', { preset: {
    id: '', revision: 0, name: 'New mappings', description: '', members: preset.members,
  } })
})
it('prefills the source project after saving its configuration and resets cancelled application choices', async () => {
  invoke.mockImplementation(async (command, args) => command === 'workspace_save_preset' ? { ...args.preset, id: 'new', revision: 1 } : command === 'workspace_presets' ? [preset] : [])
  render(<PresetPage {...props} projects={['/project', '/other-project']} intent={{ mode: 'edit', members: preset.members, scope: { kind: 'project', path: '/project' } }} t={i18n.t} />)
  fireEvent.change(screen.getByLabelText('Name'), { target: { value: 'Project configuration' } })
  fireEvent.click(screen.getByRole('button', { name: 'Save preset' }))
  fireEvent.click(await screen.findByRole('button', { name: 'Apply to…' }))
  expect((screen.getByRole('radio', { name: 'Projects' }) as HTMLInputElement).checked).toBe(true)
  expect((screen.getByLabelText('/project') as HTMLInputElement).checked).toBe(true)
  fireEvent.click(screen.getByLabelText('/project'))
  fireEvent.click(screen.getByLabelText('/other-project'))
  fireEvent.click(screen.getByRole('radio', { name: 'Global' }))
  fireEvent.click(screen.getByRole('button', { name: 'Cancel' }))
  fireEvent.click(screen.getByRole('button', { name: 'Apply to…' }))
  expect((screen.getByRole('radio', { name: 'Projects' }) as HTMLInputElement).checked).toBe(true)
  expect((screen.getByLabelText('/project') as HTMLInputElement).checked).toBe(true)
  expect((screen.getByLabelText('/other-project') as HTMLInputElement).checked).toBe(false)
})
it('requires an explicit destination and preflights exact saved mappings before applying once', async () => {
  let resolvePreview!: (record: ApplicationRecord) => void
  const pendingPreview = new Promise<ApplicationRecord>(resolve => { resolvePreview = resolve })
  const reviewed = applicationPlan()
  const completed = { ...reviewed, confirmed: true, undoTargetCount: 2, items: reviewed.items.map(item => ({ ...item, status: 'done' })) }
  invoke.mockImplementation(async command => command === 'workspace_presets' ? [preset] : command === 'workspace_preview' ? pendingPreview : command === 'workspace_execute' ? completed : [])
  render(<PresetPage {...props} t={i18n.t} />)
  fireEvent.click(await screen.findByRole('button', { name: 'Apply Daily' }))
  const dialog = screen.getByRole('dialog', { name: 'Apply preset' })
  const apply = within(dialog).getByRole('button', { name: 'Apply now' }) as HTMLButtonElement
  expect(apply.disabled).toBe(true)
  expect(apply.closest('footer')?.textContent).toMatch(/scope/i)
  fireEvent.click(screen.getByRole('radio', { name: 'Projects' }))
  expect(apply.disabled).toBe(true)
  expect(apply.closest('footer')?.textContent).toMatch(/project/i)
  fireEvent.click(screen.getByLabelText('/project'))
  expect(apply.disabled).toBe(false)
  fireEvent.click(apply)
  fireEvent.click(apply)
  expect(invoke.mock.calls.filter(([command]) => command === 'workspace_preview')).toHaveLength(1)
  expect(invoke.mock.calls.some(([command]) => command === 'workspace_execute')).toBe(false)
  await act(async () => { resolvePreview(reviewed) })
  await screen.findByRole('heading', { name: 'Application result' })
  expect(invoke).toHaveBeenCalledWith('workspace_preview', { request: { presetId: 'p1', members: preset.members, scopes: [{ kind: 'project', path: '/project' }], remove: false } })
  expect(invoke.mock.calls.filter(([command]) => command === 'workspace_execute')).toEqual([['workspace_execute', { id: reviewed.id }]])
  expect(screen.queryByRole('dialog', { name: 'Apply preset' })).toBeNull()
  expect(screen.queryByRole('button', { name: 'Confirm application' })).toBeNull()
  expect(invoke.mock.calls.some(([command]) => command === 'workspace_preview_undo')).toBe(false)
})
it('keeps blocked targets in the application dialog and requires explicit consent before applying available Skills', async () => {
  const reviewed = applicationPlan()
  reviewed.items[1] = {
    ...reviewed.items[1], status: 'blocked', reason: 'TARGET_CONFLICT',
    conflict: { agent: 'cursor', path: '/project/.cursor/skills/beta', reason: 'target_modified' },
  }
  invoke.mockImplementation(async command => command === 'workspace_presets' ? [preset] : command === 'workspace_preview' ? reviewed : command === 'workspace_execute' ? { ...reviewed, confirmed: true } : [])
  render(<PresetPage {...props} t={i18n.t} />)
  fireEvent.click(await screen.findByRole('button', { name: 'Apply Daily' }))
  fireEvent.click(screen.getByRole('radio', { name: 'Global' }))
  fireEvent.click(screen.getByRole('button', { name: 'Apply now' }))
  const continueApply = await screen.findByRole('button', { name: 'Apply available Skills' })
  const conflictDialog = screen.getByRole('dialog', { name: 'Some Skills need attention' })
  const details = within(conflictDialog)
  expect(details.getByText('Cursor')).toBeTruthy()
  const path = details.getByText('/project/.cursor/skills/beta')
  expect(path.closest('details')?.open ?? true).toBe(true)
  expect(details.getByText('This tool’s deployed copy has been modified. It has been kept unchanged.')).toBeTruthy()
  expect(details.getByText(i18n.t('workspace110.availableCount', { count: 1 }))).toBeTruthy()
  expect(details.queryByText('Deployment locations', { selector: 'summary' })).toBeNull()
  expect(invoke.mock.calls.some(([command]) => command === 'workspace_execute')).toBe(false)
  fireEvent.click(continueApply)
  await screen.findByRole('heading', { name: 'Application result' })
  expect(invoke).toHaveBeenCalledWith('workspace_execute', { id: reviewed.id })
})
it.each([
  { language: 'en', title: 'No Skills can be applied' },
  { language: 'zh', title: '本次无法应用' },
  { language: 'ko', title: '적용할 수 있는 스킬이 없습니다' },
])('explains an entirely blocked legacy plan without impossible actions in $language', async ({ language, title }) => {
  await i18n.changeLanguage(language)
  const reviewed = applicationPlan()
  reviewed.items = reviewed.items.map(item => ({ ...item, status: 'blocked', reason: 'TARGET_CONFLICT' }))
  invoke.mockImplementation(async command => command === 'workspace_presets' ? [preset] : command === 'workspace_preview' ? reviewed : [])
  render(<PresetPage {...props} t={i18n.t} />)
  fireEvent.click(await screen.findByRole('button', { name: i18n.t('workspace110.applyNamed', { name: 'Daily' }) }))
  fireEvent.click(screen.getByRole('radio', { name: i18n.t('workspace110.global') }))
  fireEvent.click(screen.getByRole('button', { name: i18n.t('workspace110.applyNow') }))
  const dialog = within(await screen.findByRole('dialog', { name: title }))
  expect(dialog.queryByRole('button', { name: i18n.t('workspace110.applyAvailable') })).toBeNull()
  expect(dialog.queryByText(i18n.t('workspace110.availableCount', { count: 0 }))).toBeNull()
  expect(dialog.queryByText(i18n.t('workspace110.conflictHint'))).toBeNull()
  expect(dialog.queryByText(i18n.t('workspace110.deploymentDetails'), { selector: 'summary' })).toBeNull()
  expect(dialog.getByRole('button', { name: i18n.t('workspace110.backToSelection') })).toBeTruthy()
  fireEvent.click(dialog.getByRole('button', { name: i18n.t('workspace110.cancel') }))
  await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull())
  expect(invoke).toHaveBeenCalledWith('workspace_discard', { id: reviewed.id })
  expect(invoke.mock.calls.some(([command]) => command === 'workspace_execute')).toBe(false)
})
it('returns from a blocked preview to the same project and temporary tool choices without applying', async () => {
  const reviewed = applicationPlan()
  reviewed.items = reviewed.items.map(item => ({ ...item, status: 'blocked', reason: 'TARGET_CONFLICT' }))
  invoke.mockImplementation(async command => command === 'workspace_presets' ? [preset] : command === 'workspace_preview' ? reviewed : [])
  render(<PresetPage {...props} t={i18n.t} />)
  fireEvent.click(await screen.findByRole('button', { name: 'Apply Daily' }))
  fireEvent.click(screen.getByRole('radio', { name: 'Projects' }))
  fireEvent.click(screen.getByLabelText('/project'))
  adjustApplication()
  fireEvent.click(screen.getByLabelText('Use common tools for this application'))
  fireEvent.click(screen.getByLabelText('Codex'))
  fireEvent.click(screen.getByRole('button', { name: 'Apply now' }))
  const blocked = await screen.findByRole('dialog', { name: 'No Skills can be applied' })
  fireEvent.click(within(blocked).getByRole('button', { name: 'Back to selection' }))
  expect(await screen.findByRole('dialog', { name: 'Apply preset' })).toBeTruthy()
  expect(invoke).toHaveBeenCalledWith('workspace_discard', { id: reviewed.id })
  expect((screen.getByRole('radio', { name: 'Projects' }) as HTMLInputElement).checked).toBe(true)
  expect((screen.getByLabelText('/project') as HTMLInputElement).checked).toBe(true)
  adjustApplication()
  expect((screen.getByLabelText('Use common tools for this application') as HTMLInputElement).checked).toBe(true)
  expect((screen.getByLabelText('Codex') as HTMLInputElement).checked).toBe(true)
  expect((screen.getByLabelText('Cursor') as HTMLInputElement).checked).toBe(false)
  expect(invoke).toHaveBeenCalledWith('workspace_preview', {
    request: { presetId: preset.id, members: preset.members.map(member => ({ ...member, tools: ['codex'] })), scopes: [{ kind: 'project', path: '/project' }], remove: false },
  })
  expect(invoke.mock.calls.filter(([command]) => command === 'workspace_preview')).toHaveLength(1)
  expect(invoke.mock.calls.some(([command]) => command === 'workspace_execute')).toBe(false)
})
it.each(['Cancel', 'Close', 'Escape'])('discards an unexecuted preview before closing through %s', async closeAction => {
  const reviewed = applicationPlan()
  reviewed.items = reviewed.items.map(item => ({ ...item, status: 'blocked', reason: 'TARGET_CONFLICT' }))
  let finishDiscard!: () => void
  const pendingDiscard = new Promise<void>(resolve => { finishDiscard = resolve })
  invoke.mockImplementation(async command => {
    if (command === 'workspace_presets') return [preset]
    if (command === 'workspace_preview') return reviewed
    if (command === 'workspace_discard') return pendingDiscard
    return []
  })
  render(<PresetPage {...props} t={i18n.t} />)
  const trigger = await screen.findByRole('button', { name: 'Apply Daily' })
  fireEvent.click(trigger)
  fireEvent.click(screen.getByRole('radio', { name: 'Global' }))
  fireEvent.click(screen.getByRole('button', { name: 'Apply now' }))
  const dialog = await screen.findByRole('dialog', { name: 'No Skills can be applied' })
  if (closeAction === 'Escape') fireEvent.keyDown(document, { key: 'Escape' })
  else fireEvent.click(within(dialog).getByRole('button', { name: closeAction }))
  await waitFor(() => expect(invoke).toHaveBeenCalledWith('workspace_discard', { id: reviewed.id }))
  expect(screen.getByRole('dialog', { name: 'No Skills can be applied' })).toBe(dialog)
  expect((within(dialog).getByRole('button', { name: 'Cancel' }) as HTMLButtonElement).disabled).toBe(true)
  fireEvent.keyDown(document, { key: 'Escape' })
  expect(invoke.mock.calls.filter(([command]) => command === 'workspace_discard')).toHaveLength(1)

  await act(async () => { finishDiscard() })
  expect(screen.queryByRole('dialog')).toBeNull()
  expect(document.activeElement).toBe(trigger)
  expect(invoke.mock.calls.some(([command]) => command === 'workspace_execute')).toBe(false)
})

it('keeps a preview visible when discarding fails and lets the user retry without applying', async () => {
  const reviewed = applicationPlan()
  reviewed.items = reviewed.items.map(item => ({ ...item, status: 'blocked', reason: 'TARGET_CONFLICT' }))
  let discardAttempts = 0
  invoke.mockImplementation(async command => {
    if (command === 'workspace_presets') return [preset]
    if (command === 'workspace_preview') return reviewed
    if (command === 'workspace_discard' && ++discardAttempts === 1) throw new Error('journal cleanup unavailable')
    return []
  })
  render(<PresetPage {...props} t={i18n.t} />)
  fireEvent.click(await screen.findByRole('button', { name: 'Apply Daily' }))
  fireEvent.click(screen.getByRole('radio', { name: 'Projects' }))
  fireEvent.click(screen.getByLabelText('/project'))
  fireEvent.click(screen.getByRole('button', { name: 'Apply now' }))
  const dialog = await screen.findByRole('dialog', { name: 'No Skills can be applied' })
  fireEvent.click(within(dialog).getByRole('button', { name: 'Back to selection' }))
  expect((await within(dialog).findByRole('alert')).textContent).toContain('Could not clear this preview. Your selection is kept. Try closing or going back again.')
  expect(screen.getByRole('dialog', { name: 'No Skills can be applied' })).toBe(dialog)
  expect(screen.queryByRole('dialog', { name: 'Apply preset' })).toBeNull()
  expect(invoke.mock.calls.some(([command]) => command === 'workspace_execute')).toBe(false)

  fireEvent.click(within(dialog).getByRole('button', { name: 'Back to selection' }))
  const selection = await screen.findByRole('dialog', { name: 'Apply preset' })
  expect(within(selection).queryByRole('alert')).toBeNull()
  expect((within(selection).getByRole('checkbox', { name: '/project' }) as HTMLInputElement).checked).toBe(true)
  expect(invoke.mock.calls.filter(([command]) => command === 'workspace_discard')).toEqual([
    ['workspace_discard', { id: reviewed.id }], ['workspace_discard', { id: reviewed.id }],
  ])
  fireEvent.click(within(selection).getByRole('button', { name: 'Cancel' }))
  await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull())
  expect(invoke.mock.calls.filter(([command]) => command === 'workspace_discard')).toHaveLength(2)
})

it('does not count completed history items as available when every unfinished item is blocked', async () => {
  const history = applicationPlan({ confirmed: true })
  history.items[0] = { ...history.items[0], status: 'done' }
  history.items[1] = {
    ...history.items[1], status: 'blocked', reason: 'TARGET_CONFLICT',
    conflict: { agent: 'cursor', path: '/project/.cursor/skills/beta', reason: 'target_modified' },
  }
  invoke.mockImplementation(async command => command === 'workspace_presets' ? [preset] : command === 'workspace_history' ? [history] : command === 'workspace_recheck' ? history : [])
  render(<PresetPage {...props} t={i18n.t} />)
  fireEvent.click(screen.getByRole('button', { name: 'All application records' }))
  fireEvent.click(await screen.findByRole('button', { name: /Daily/ }))
  fireEvent.click(screen.getByRole('button', { name: 'Preview unfinished items again' }))
  const dialog = within(await screen.findByRole('dialog', { name: 'No Skills can be applied' }))
  expect(dialog.queryByRole('button', { name: 'Apply available Skills' })).toBeNull()
  expect(dialog.queryByRole('button', { name: 'Back to selection' })).toBeNull()
  expect(dialog.queryByText(i18n.t('workspace110.availableCount', { count: 1 }))).toBeNull()
  expect(dialog.queryByText(i18n.t('workspace110.conflictHint'))).toBeNull()
  fireEvent.click(dialog.getByRole('button', { name: 'Back to result' }))
  expect(screen.queryByRole('dialog')).toBeNull()
  expect(screen.getByRole('heading', { name: 'Application result' })).toBeTruthy()
  expect(invoke).toHaveBeenCalledWith('workspace_recheck', { id: history.id })
  expect(invoke.mock.calls.some(([command]) => ['workspace_preview', 'workspace_execute', 'workspace_discard'].includes(command))).toBe(false)
})
it('requires confirmation for removals even when the preview has no blocked items', async () => {
  const reviewed = applicationPlan({ remove: true })
  reviewed.items = reviewed.items.map(item => ({ ...item, action: 'remove' }))
  invoke.mockImplementation(async command => command === 'workspace_preview' ? reviewed : command === 'workspace_execute' ? { ...reviewed, confirmed: true } : [])
  render(<PresetPage {...props} intent={{ mode: 'select', members: preset.members, scope: { kind: 'global' }, remove: true }} t={i18n.t} />)
  fireEvent.click(screen.getByRole('button', { name: 'Apply now' }))
  const confirm = await screen.findByRole('button', { name: 'Remove selected targets' })
  expect(invoke.mock.calls.some(([command]) => command === 'workspace_execute')).toBe(false)
  fireEvent.click(confirm)
  await screen.findByRole('heading', { name: 'Application result' })
  expect(invoke).toHaveBeenCalledWith('workspace_execute', { id: reviewed.id })
})
it.each(['Edit preset', 'Duplicate'])('clears application overrides and filters before %s', async action => {
  render(<PresetPage {...props} t={i18n.t} />)
  fireEvent.click(await screen.findByText('Daily'))
  fireEvent.click(screen.getByRole('button', { name: 'Apply to…' }))
  adjustApplication()
  fireEvent.click(screen.getByLabelText('Use common tools for this application'))
  fireEvent.click(screen.getByLabelText('Codex'))
  fireEvent.change(screen.getByLabelText('Search Skills'), { target: { value: 'alpha' } })
  fireEvent.click(screen.getByRole('button', { name: 'Add Skills' }))
  fireEvent.click(screen.getByRole('button', { name: 'Cancel' }))
  fireEvent.click(screen.getByRole('button', { name: action }))

  expect(screen.getByRole('heading', { level: 1, name: action === 'Duplicate' ? 'New preset' : 'Edit preset' })).toBeTruthy()
  expect((screen.getByLabelText('Search Skills') as HTMLInputElement).value).toBe('')
  expect(screen.getByRole('button', { name: 'Add Skills' })).toBeTruthy()
  expect(screen.queryByRole('group', { name: 'Default tools for newly selected members' })).toBeNull()
  fireEvent.click(screen.getByRole('button', { name: 'Add Skills' }))
  expect(toolChoice('Default tools for newly selected members', 'Codex').checked).toBe(false)
  expect(toolChoice('Default tools for newly selected members', 'Cursor').checked).toBe(false)
  expect(toolChoice('alpha', 'Codex').checked).toBe(true)
  expect(toolChoice('alpha', 'Cursor').checked).toBe(false)
  expect(toolChoice('beta', 'Codex').checked).toBe(false)
  expect(toolChoice('beta', 'Cursor').checked).toBe(true)
})
it('clears temporary selection controls when creating a new preset', async () => {
  render(<PresetPage {...props} t={i18n.t} />)
  fireEvent.click(await screen.findByText('Daily'))
  fireEvent.click(screen.getByRole('button', { name: 'Apply to…' }))
  adjustApplication()
  fireEvent.click(screen.getByLabelText('Use common tools for this application'))
  fireEvent.click(screen.getByLabelText('Codex'))
  fireEvent.change(screen.getByLabelText('Search Skills'), { target: { value: 'alpha' } })
  fireEvent.click(screen.getByRole('button', { name: 'Add Skills' }))
  fireEvent.click(screen.getByRole('button', { name: 'Cancel' }))
  fireEvent.click(screen.getByRole('button', { name: 'Back' }))
  fireEvent.click(screen.getByRole('button', { name: 'New preset' }))

  const heading = screen.getByRole('heading', { level: 1, name: 'New preset' })
  expect(document.activeElement).toBe(heading)
  expect((screen.getByLabelText('Search Skills') as HTMLInputElement).value).toBe('')
  expect(screen.getByRole('button', { name: 'Done selecting' })).toBeTruthy()
  expect((screen.getByLabelText('Codex') as HTMLInputElement).checked).toBe(false)
  fireEvent.click(screen.getByRole('checkbox', { name: 'beta' }))
  expect(screen.getByLabelText('Adjust tools for beta').parentElement?.hasAttribute('open')).toBe(false)
  expect(screen.getByText('Choose tools for this member.')).toBeTruthy()
})
it.each(['Daily', 'Weekend'])('starts a fresh %s application from its saved mappings after cancelling temporary changes', async name => {
  const secondPreset: SkillPreset = { ...preset, id: 'p2', name: 'Weekend', members: [{ skillId: 'alpha', tools: ['cursor'] }] }
  const reviewed = applicationPlan()
  invoke.mockImplementation(async command => command === 'workspace_presets' ? [preset, secondPreset] : command === 'workspace_preview' ? reviewed : command === 'workspace_execute' ? { ...reviewed, confirmed: true } : [])
  render(<PresetPage {...props} t={i18n.t} />)
  fireEvent.click(await screen.findByRole('button', { name: 'Apply Daily' }))
  adjustApplication()
  fireEvent.click(screen.getByRole('checkbox', { name: 'beta' }))
  fireEvent.click(screen.getByLabelText('Use common tools for this application'))
  fireEvent.click(screen.getByLabelText('Cursor'))
  fireEvent.click(screen.getByRole('radio', { name: 'Projects' }))
  fireEvent.click(screen.getByLabelText('/project'))
  fireEvent.change(screen.getByLabelText('Search Skills'), { target: { value: 'alpha' } })
  fireEvent.click(screen.getByRole('button', { name: 'Cancel' }))
  fireEvent.click(screen.getByRole('button', { name: `Apply ${name}` }))
  expect((screen.getByRole('radio', { name: 'Global' }) as HTMLInputElement).checked).toBe(false)
  expect((screen.getByRole('radio', { name: 'Projects' }) as HTMLInputElement).checked).toBe(false)
  expect(screen.getByText('Adjust for this application', { selector: 'summary' }).parentElement?.hasAttribute('open')).toBe(false)
  adjustApplication()
  expect((screen.getByLabelText('Use common tools for this application') as HTMLInputElement).checked).toBe(false)
  expect((screen.getByLabelText('Search Skills') as HTMLInputElement).value).toBe('')
  fireEvent.click(screen.getByRole('radio', { name: 'Projects' }))
  expect((screen.getByLabelText('/project') as HTMLInputElement).checked).toBe(false)
  fireEvent.click(screen.getByLabelText('/project'))
  fireEvent.click(screen.getByRole('button', { name: 'Apply now' }))
  await screen.findByRole('heading', { name: 'Application result' })
  const expected = name === 'Daily' ? preset : secondPreset
  expect(invoke).toHaveBeenCalledWith('workspace_preview', { request: { presetId: expected.id, members: expected.members, scopes: [{ kind: 'project', path: '/project' }], remove: false } })
  expect(invoke.mock.calls.some(([command]) => command === 'workspace_save_preset')).toBe(false)
})
it('retains destination and temporary member mappings after a failed preview and retries from the same draft', async () => {
  let previews = 0
  const reviewed = applicationPlan()
  invoke.mockImplementation(async command => {
    if (command === 'workspace_presets') return [preset]
    if (command === 'workspace_preview') {
      if (++previews === 1) throw new Error('source unavailable')
      return reviewed
    }
    if (command === 'workspace_execute') return { ...reviewed, confirmed: true }
    return []
  })
  render(<PresetPage {...props} t={i18n.t} />)
  fireEvent.click(await screen.findByRole('button', { name: 'Apply Daily' }))
  adjustApplication()
  fireEvent.click(screen.getByRole('checkbox', { name: 'beta' }))
  fireEvent.click(screen.getByLabelText('Use common tools for this application'))
  fireEvent.click(screen.getByLabelText('Cursor'))
  fireEvent.click(screen.getByRole('radio', { name: 'Projects' }))
  fireEvent.click(screen.getByLabelText('/project'))
  fireEvent.click(screen.getByRole('button', { name: 'Apply now' }))
  await screen.findByRole('alert')
  expect(screen.getByRole('dialog', { name: 'Apply preset' })).toBeTruthy()
  expect((screen.getByRole('radio', { name: 'Projects' }) as HTMLInputElement).checked).toBe(true)
  expect((screen.getByLabelText('/project') as HTMLInputElement).checked).toBe(true)
  expect((screen.getByLabelText('Use common tools for this application') as HTMLInputElement).checked).toBe(true)
  expect((screen.getByLabelText('Cursor') as HTMLInputElement).checked).toBe(true)
  expect(invoke.mock.calls.some(([command]) => command === 'workspace_execute')).toBe(false)
  fireEvent.click(screen.getByRole('button', { name: 'Apply now' }))
  await screen.findByRole('heading', { name: 'Application result' })
  expect(invoke.mock.calls.filter(([command]) => command === 'workspace_preview').map(([, args]) => args)).toEqual(Array(2).fill({ request: { presetId: 'p1', members: [{ skillId: 'alpha', tools: ['cursor'] }], scopes: [{ kind: 'project', path: '/project' }], remove: false } }))
})
it('keeps older history accessible and finds immutable deleted-preset snapshots', async () => {
  const records = Array.from({ length: 25 }, (_, i) => ({ id: `history-${i}`, preset: { ...preset, name: `Original ${i}` }, createdAt: i, confirmed: true, remove: false, undoTargetCount: 0, items: [] }))
  invoke.mockImplementation(async command => command === 'workspace_history' ? records : [])
  render(<PresetPage {...props} t={i18n.t} />)
  await act(async () => {})
  fireEvent.click(screen.getByRole('button', { name: 'All application records' }))
  expect(await screen.findByText('Original 24')).toBeTruthy()
  fireEvent.change(screen.getByLabelText('Search presets, members or records'), { target: { value: 'Original 24' } })
  expect(screen.queryByText('Original 0')).toBeNull()
  fireEvent.click(screen.getByText('Original 24'))
  expect(screen.getByRole('heading', { name: 'Application result' })).toBeTruthy()
})
it('counts unique Skills in an application record spanning multiple projects', async () => {
  const record: ApplicationRecord = {
    id: 'multi-project', preset, createdAt: 1, confirmed: true, remove: false, undoTargetCount: 0,
    items: ['/first-project', '/second-project'].flatMap(path => preset.members.map(member => ({
      skillId: member.skillId, name: member.skillId, tools: member.tools,
      scope: { kind: 'project' as const, path }, targets: [], action: 'add' as const,
      status: 'done' as const, reason: null,
    }))),
  }
  invoke.mockImplementation(async command => command === 'workspace_history' ? [record] : [])
  render(<PresetPage {...props} t={i18n.t} />)
  fireEvent.click(screen.getByRole('button', { name: 'All application records' }))
  await screen.findByText('Daily')
  expect(screen.getByText('2 Skills')).toBeTruthy()
  expect(screen.queryByText('4 Skills')).toBeNull()
})
it('opens an application result at the top after scrolling through history', async () => {
  const record: ApplicationRecord = {
    id: 'multi-project', preset, createdAt: 1, confirmed: true, remove: false, undoTargetCount: 0,
    items: ['/first-project', '/second-project'].flatMap(path => preset.members.map(member => ({
      skillId: member.skillId, name: member.skillId, tools: member.tools,
      scope: { kind: 'project' as const, path }, targets: [], action: 'add',
      status: 'done', reason: null,
    }))),
  }
  const records = [
    ...Array.from({ length: 20 }, (_, index) => ({ ...record, id: `earlier-${index}`, preset: { ...preset, name: `Earlier ${index}` } })),
    record,
  ]
  invoke.mockImplementation(async command => command === 'workspace_history' ? records : [])
  const { container } = render(<PresetPage {...props} t={i18n.t} />)
  fireEvent.click(screen.getByRole('button', { name: 'All application records' }))
  const historyCard = await screen.findByRole('button', { name: /Daily/ })
  const body = container.querySelector<HTMLElement>('.workspace-body')!
  // jsdom does not lay out scrollable elements, so reproduce the observed history offset.
  body.scrollTop = 292
  fireEvent.scroll(body)
  expect(body.scrollTop).toBe(292)

  fireEvent.click(historyCard)

  expect(screen.getByRole('heading', { name: 'first-project' })).toBeTruthy()
  for (const summary of screen.getAllByText('View details', { selector: 'summary' })) expect(summary.parentElement?.hasAttribute('open')).toBe(false)
  expect(document.activeElement).toBe(screen.getByRole('heading', { name: 'Application result' }))
  expect(body.scrollTop).toBe(0)
})
it('does not discard an edit without confirmation and returns to the source', async () => {
  render(<PresetPage {...props} intent={{ mode: 'edit', members: preset.members }} t={i18n.t} />)
  fireEvent.click(screen.getByRole('button', { name: 'Back' }))
  expect(onBack).not.toHaveBeenCalled()
  fireEvent.click(screen.getByRole('button', { name: 'Cancel' }))
  expect(screen.getByLabelText('Name')).toBeTruthy()
  fireEvent.click(screen.getByRole('button', { name: 'Back' }))
  fireEvent.click(screen.getByRole('button', { name: 'Confirm' }))
  expect(onBack).toHaveBeenCalledOnce()
})
it.each(['discard', 'delete'])('returns focus to the %s confirmation trigger after Escape', async kind => {
  render(<PresetPage {...props} t={i18n.t} />)
  fireEvent.click(await screen.findByText('Daily'))
  if (kind === 'discard') {
    fireEvent.click(screen.getByRole('button', { name: 'Edit preset' }))
    fireEvent.change(screen.getByLabelText('Name'), { target: { value: 'Unsaved change' } })
    screen.getByLabelText('Name').focus()
  } else {
    (document.activeElement as HTMLElement).blur()
    expect(document.activeElement).toBe(document.body)
  }
  const trigger = screen.getByRole('button', { name: kind === 'discard' ? 'Back' : 'Delete preset' })
  // Pointer clicks on macOS do not necessarily focus buttons before opening the dialog.
  expect(document.activeElement).not.toBe(trigger)
  fireEvent.click(trigger)
  expect(document.activeElement).toBe(screen.getByRole('button', { name: 'Cancel' }))
  fireEvent.keyDown(document, { key: 'Escape' })
  expect(screen.queryByRole('dialog')).toBeNull()
  expect(document.activeElement).toBe(trigger)
  expect(onBack).not.toHaveBeenCalled()
  expect(invoke.mock.calls.some(([command]) => command === 'workspace_delete_preset')).toBe(false)
})
it('keeps secondary detail actions outside the header and returns focus after cancelling deletion', async () => {
  render(<PresetPage {...props} t={i18n.t} />)
  fireEvent.click(await screen.findByText('Daily'))
  const header = screen.getByRole('heading', { level: 1, name: 'Daily' }).closest('header')!
  expect(within(header).getByRole('button', { name: 'Apply to…' })).toBeTruthy()
  expect(within(header).getByRole('button', { name: 'Edit preset' })).toBeTruthy()
  expect(within(header).queryByRole('button', { name: 'Duplicate' })).toBeNull()
  expect(within(header).queryByRole('button', { name: 'Delete preset' })).toBeNull()
  expect(screen.getByRole('button', { name: 'Duplicate' })).toBeTruthy()
  const trigger = screen.getByRole('button', { name: 'Delete preset' })

  fireEvent.click(trigger)
  const dialog = screen.getByRole('dialog', { name: 'Delete preset' })
  fireEvent.click(within(dialog).getByRole('button', { name: 'Cancel' }))

  expect(screen.queryByRole('dialog')).toBeNull()
  expect(document.activeElement).toBe(trigger)
  expect(screen.getByRole('heading', { level: 1, name: 'Daily' })).toBeTruthy()
  expect(screen.getByRole('button', { name: 'alpha' })).toBeTruthy()
  expect(screen.getByRole('button', { name: 'beta' })).toBeTruthy()
  expect(invoke.mock.calls.some(([command]) => command === 'workspace_delete_preset')).toBe(false)
})
it('shows an explicit empty search result without hiding the search controls', async () => {
  render(<PresetPage {...props} t={i18n.t} />)
  await screen.findByText('Daily')
  fireEvent.change(screen.getByLabelText('Search presets, members or records'), { target: { value: 'not present' } })
  expect(screen.getByText('No matching presets or records.')).toBeTruthy()
  fireEvent.change(screen.getByLabelText('Search presets, members or records'), { target: { value: '' } })
  expect(screen.getByText('Daily')).toBeTruthy()
})

it('leaves a blank new preset without a discard prompt', async () => {
  const onDirtyChange = vi.fn()
  render(<PresetPage {...props} onDirtyChange={onDirtyChange} t={i18n.t} />)
  await screen.findByText('Daily')
  fireEvent.click(screen.getByRole('button', { name: 'New preset' }))

  expect(onDirtyChange).toHaveBeenLastCalledWith(false)
  fireEvent.click(screen.getByRole('button', { name: 'Back' }))
  expect(screen.queryByRole('dialog')).toBeNull()
  expect(screen.getByRole('heading', { level: 1, name: 'Skill presets' })).toBeTruthy()
  expect(invoke.mock.calls.some(([command]) => command === 'workspace_save_preset')).toBe(false)
})

it.each(['untouched', 'restored fields', 'restored members and tool order'])('leaves an %s existing preset without reporting unsaved changes', async state => {
  const existing: SkillPreset = { ...preset, description: 'Reusable setup', members: [{ skillId: 'alpha', tools: ['codex', 'cursor'] }, preset.members[1]] }
  const onDirtyChange = vi.fn()
  invoke.mockImplementation(async command => command === 'workspace_presets' ? [existing] : [])
  render(<PresetPage {...props} onDirtyChange={onDirtyChange} t={i18n.t} />)
  fireEvent.click(await screen.findByText('Daily'))
  fireEvent.click(screen.getByRole('button', { name: 'Edit preset' }))
  expect(onDirtyChange).toHaveBeenLastCalledWith(false)

  if (state === 'restored fields') {
    fireEvent.change(screen.getByLabelText('Name'), { target: { value: 'Changed name' } })
    fireEvent.change(screen.getByLabelText('Purpose (optional)'), { target: { value: 'Changed purpose' } })
    expect(onDirtyChange).toHaveBeenLastCalledWith(true)
    fireEvent.change(screen.getByLabelText('Name'), { target: { value: existing.name } })
    fireEvent.change(screen.getByLabelText('Purpose (optional)'), { target: { value: existing.description } })
  }
  if (state === 'restored members and tool order') {
    fireEvent.click(toolChoice('alpha', 'Codex'))
    expect(onDirtyChange).toHaveBeenLastCalledWith(true)
    fireEvent.click(toolChoice('alpha', 'Codex'))
    fireEvent.click(screen.getByRole('checkbox', { name: 'alpha' }))
    fireEvent.click(screen.getByRole('button', { name: 'Add Skills' }))
    fireEvent.click(screen.getByRole('checkbox', { name: 'alpha' }))
    fireEvent.click(toolChoice('alpha', 'Cursor'))
    fireEvent.click(toolChoice('alpha', 'Codex'))
  }

  expect(onDirtyChange).toHaveBeenLastCalledWith(false)
  fireEvent.click(screen.getByRole('button', { name: 'Back' }))
  expect(screen.queryByRole('dialog')).toBeNull()
  expect(screen.getByRole('heading', { level: 1, name: 'Daily' })).toBeTruthy()
  expect(invoke.mock.calls.some(([command]) => command === 'workspace_save_preset')).toBe(false)
})

it('protects an unsaved duplicate even before its fields are changed', async () => {
  const onDirtyChange = vi.fn()
  render(<PresetPage {...props} onDirtyChange={onDirtyChange} t={i18n.t} />)
  fireEvent.click(await screen.findByText('Daily'))
  fireEvent.click(screen.getByRole('button', { name: 'Duplicate' }))

  expect(onDirtyChange).toHaveBeenLastCalledWith(true)
  fireEvent.click(screen.getByRole('button', { name: 'Back' }))
  expect(screen.getByRole('dialog')).toBeTruthy()
  fireEvent.click(screen.getByRole('button', { name: 'Cancel' }))
  expect((screen.getByLabelText('Name') as HTMLInputElement).value).toBe('Daily copy')
  expect(toolChoice('alpha', 'Codex').checked).toBe(true)
  expect(toolChoice('beta', 'Cursor').checked).toBe(true)
})

it('focuses application on the saved members in a library of 24 Skills and can choose other Skills', async () => {
  const library: ManagedSkill[] = [...skills, ...Array.from({ length: 22 }, (_, index) => ({ ...skills[0], id: `extra-${index}`, name: `GitHub workflow ${index + 1}`, source_type: 'git' as const, description: `Representative GitHub Skill ${index + 1}` }))]
  render(<PresetPage {...props} skills={library} t={i18n.t} />)
  fireEvent.click(await screen.findByText('Daily'))
  fireEvent.click(screen.getByRole('button', { name: 'Apply to…' }))
  expect(screen.getByText('Adjust for this application', { selector: 'summary' }).parentElement?.hasAttribute('open')).toBe(false)
  expect(screen.queryByRole('checkbox', { name: 'GitHub workflow 22' })).toBeNull()
  adjustApplication()

  expect(screen.getByRole('button', { name: 'Add Skills' })).toBeTruthy()
  expect(screen.getByRole('checkbox', { name: 'alpha' })).toBeTruthy()
  expect(screen.getByRole('checkbox', { name: 'beta' })).toBeTruthy()
  for (const skill of library.slice(2)) expect(screen.queryByRole('checkbox', { name: skill.name })).toBeNull()

  fireEvent.click(screen.getByRole('button', { name: 'Add Skills' }))
  for (const skill of library) expect(screen.getByRole('checkbox', { name: skill.name })).toBeTruthy()
  fireEvent.change(screen.getByLabelText('Search Skills'), { target: { value: 'GitHub workflow 22' } })
  expect(screen.getByRole('checkbox', { name: 'GitHub workflow 22' })).toBeTruthy()
  expect(screen.queryByRole('checkbox', { name: 'alpha' })).toBeNull()
  fireEvent.click(screen.getByRole('button', { name: 'Cancel' }))
  expect(screen.getByRole('heading', { level: 1, name: 'Daily' })).toBeTruthy()
  expect(invoke.mock.calls.some(([command]) => ['workspace_save_preset', 'workspace_preview', 'workspace_execute'].includes(command))).toBe(false)
})

it('focuses editing on the three saved members at the end of a 24-Skill library and preserves their mappings when adding another', async () => {
  const extraSkills: ManagedSkill[] = Array.from({ length: 22 }, (_, index) => ({ ...skills[0], id: `extra-${index}`, name: `GitHub workflow ${index + 1}`, source_type: 'git', description: `Representative GitHub Skill ${index + 1}` }))
  const library = [...extraSkills, ...skills]
  const existing: SkillPreset = { ...preset, members: [...preset.members, { skillId: 'extra-21', tools: ['codex', 'cursor'] }] }
  invoke.mockImplementation(async (command, args) => command === 'workspace_save_preset' ? { ...args.preset, revision: 2 } : command === 'workspace_presets' ? [existing] : [])
  render(<PresetPage {...props} skills={library} t={i18n.t} />)
  fireEvent.click(await screen.findByText('Daily'))
  fireEvent.click(screen.getByRole('button', { name: 'Edit preset' }))

  expect(screen.getByRole('button', { name: 'Add Skills' })).toBeTruthy()
  expect(screen.queryByRole('group', { name: 'Default tools for newly selected members' })).toBeNull()
  for (const skill of library) {
    if (existing.members.some(member => member.skillId === skill.id)) expect(screen.getByRole('checkbox', { name: skill.name })).toBeTruthy()
    else expect(screen.queryByRole('checkbox', { name: skill.name })).toBeNull()
  }
  fireEvent.change(screen.getByLabelText('Search Skills'), { target: { value: 'alpha' } })
  expect(screen.queryByRole('checkbox', { name: 'beta' })).toBeNull()
  fireEvent.click(screen.getByRole('button', { name: 'Add Skills' }))

  expect(screen.getByRole('button', { name: 'Done selecting' })).toBeTruthy()
  expect((screen.getByLabelText('Search Skills') as HTMLInputElement).value).toBe('')
  expect(toolChoice('Default tools for newly selected members', 'Codex').checked).toBe(false)
  expect(toolChoice('Default tools for newly selected members', 'Cursor').checked).toBe(false)
  for (const skill of library) expect(screen.getByRole('checkbox', { name: skill.name })).toBeTruthy()
  expect(toolChoice('alpha', 'Codex').checked).toBe(true)
  expect(toolChoice('alpha', 'Cursor').checked).toBe(false)
  expect(toolChoice('beta', 'Codex').checked).toBe(false)
  expect(toolChoice('beta', 'Cursor').checked).toBe(true)
  expect(toolChoice('GitHub workflow 22', 'Codex').checked).toBe(true)
  expect(toolChoice('GitHub workflow 22', 'Cursor').checked).toBe(true)
  fireEvent.click(screen.getByRole('checkbox', { name: 'GitHub workflow 6' }))
  fireEvent.click(toolChoice('GitHub workflow 6', 'Cursor'))
  fireEvent.click(screen.getByRole('button', { name: 'Done selecting' }))
  expect(screen.queryByRole('checkbox', { name: 'GitHub workflow 1' })).toBeNull()
  expect(screen.getByRole('checkbox', { name: 'GitHub workflow 6' })).toBeTruthy()
  expect(screen.queryByRole('group', { name: 'Default tools for newly selected members' })).toBeNull()
  expect(toolChoice('alpha', 'Codex').checked).toBe(true)
  fireEvent.click(screen.getByRole('button', { name: 'Save preset' }))

  await screen.findByRole('heading', { level: 1, name: 'Daily' })
  expect(invoke).toHaveBeenCalledWith('workspace_save_preset', { preset: { ...existing, members: [...existing.members, { skillId: 'extra-5', tools: ['cursor'] }] } })
  expect(invoke.mock.calls.some(([command]) => ['workspace_preview', 'workspace_execute'].includes(command))).toBe(false)
})

it('explains missing member tools and common tools before enabling application', async () => {
  render(<PresetPage {...props} intent={{ mode: 'select', members: [{ skillId: 'alpha', tools: [] }], scope: { kind: 'global' } }} t={i18n.t} />)
  const review = screen.getByRole('button', { name: 'Apply now' }) as HTMLButtonElement
  expect(review.disabled).toBe(true)
  expect(review.closest('footer')?.textContent).toMatch(/tool/i)
  expect(screen.queryByRole('alert')).toBeNull()

  adjustApplication()
  fireEvent.click(toolChoice('alpha', 'Codex'))
  expect(review.disabled).toBe(false)
  fireEvent.click(screen.getByLabelText('Use common tools for this application'))
  expect(review.disabled).toBe(true)
  expect(review.closest('footer')?.textContent).toMatch(/tool/i)
  fireEvent.click(screen.getByRole('checkbox', { name: 'Cursor' }))
  expect(review.disabled).toBe(false)
  expect(invoke.mock.calls.some(([command]) => command === 'workspace_preview')).toBe(false)
})

it('saves a conflicting draft as a separate preset without overwriting the latest version', async () => {
  const onDirtyChange = vi.fn()
  const latest: SkillPreset = { ...preset, revision: 2, description: 'Updated elsewhere', members: [preset.members[1]] }
  const existingCopy: SkillPreset = { ...preset, id: 'copy', name: 'Daily copy' }
  let stored = [preset, existingCopy]
  invoke.mockImplementation(async (command, args) => {
    if (command === 'workspace_presets') return stored
    if (command !== 'workspace_save_preset') return []
    const draft = args.preset as SkillPreset
    if (draft.id === preset.id) {
      stored = [latest, existingCopy]
      throw new Error('PLAN_STALE')
    }
    if (stored.some(item => item.name.toLowerCase() === draft.name.toLowerCase())) throw new Error('INVALID_ARGUMENT: preset name already exists')
    const saved = { ...draft, id: 'recovered-draft', revision: 1 }
    stored = [...stored, saved]
    return saved
  })
  render(<PresetPage {...props} onDirtyChange={onDirtyChange} t={i18n.t} />)
  fireEvent.click(await screen.findByText('Daily'))
  fireEvent.click(screen.getByRole('button', { name: 'Edit preset' }))
  fireEvent.change(screen.getByLabelText('Purpose (optional)'), { target: { value: 'My unsaved purpose' } })
  fireEvent.click(toolChoice('alpha', 'Cursor'))
  fireEvent.click(screen.getByRole('button', { name: 'Save preset' }))
  const recover = await screen.findByRole('button', { name: 'Save as new preset' })

  expect((screen.getByLabelText('Purpose (optional)') as HTMLTextAreaElement).value).toBe('My unsaved purpose')
  expect(toolChoice('alpha', 'Codex').checked).toBe(true)
  expect(toolChoice('alpha', 'Cursor').checked).toBe(true)
  expect(screen.getByRole('button', { name: 'Load latest preset' })).toBeTruthy()
  fireEvent.click(recover)
  await waitFor(() => expect(stored).toHaveLength(3))

  const savedCopy = stored.find(item => item.id === 'recovered-draft')!
  expect(savedCopy.name).not.toBe(preset.name)
  expect(savedCopy.name).not.toBe(existingCopy.name)
  expect(savedCopy.description).toBe('My unsaved purpose')
  expect(savedCopy.members).toEqual([{ skillId: 'alpha', tools: ['codex', 'cursor'] }, preset.members[1]])
  expect(stored.find(item => item.id === preset.id)).toEqual(latest)
  expect(invoke.mock.calls.filter(([command]) => command === 'workspace_save_preset').map(([, args]) => ({ id: args.preset.id, revision: args.preset.revision }))).toEqual([{ id: 'p1', revision: 1 }, { id: '', revision: 0 }])
  expect(await screen.findByRole('heading', { level: 1, name: savedCopy.name })).toBeTruthy()
  expect(screen.queryByRole('alert')).toBeNull()
  expect(onDirtyChange).toHaveBeenLastCalledWith(false)
})

it('loads the latest preset only after confirmation and resets the dirty baseline', async () => {
  const latest: SkillPreset = { ...preset, revision: 2, name: 'Daily updated elsewhere', description: 'Latest purpose', members: [preset.members[1]] }
  const onDirtyChange = vi.fn()
  let stored = [preset]
  invoke.mockImplementation(async command => {
    if (command === 'workspace_presets') return stored
    if (command === 'workspace_save_preset') {
      stored = [latest]
      throw new Error('PLAN_STALE')
    }
    return []
  })
  render(<PresetPage {...props} onDirtyChange={onDirtyChange} t={i18n.t} />)
  fireEvent.click(await screen.findByText('Daily'))
  fireEvent.click(screen.getByRole('button', { name: 'Edit preset' }))
  fireEvent.change(screen.getByLabelText('Purpose (optional)'), { target: { value: 'My draft purpose' } })
  fireEvent.click(screen.getByRole('button', { name: 'Save preset' }))
  const loadLatest = await screen.findByRole('button', { name: 'Load latest preset' })
  expect(onDirtyChange).toHaveBeenLastCalledWith(true)

  fireEvent.click(loadLatest)
  const confirmation = screen.getByRole('dialog', { name: 'Load latest preset' })
  expect((screen.getByLabelText('Name') as HTMLInputElement).value).toBe('Daily')
  expect((screen.getByLabelText('Purpose (optional)') as HTMLTextAreaElement).value).toBe('My draft purpose')
  fireEvent.click(within(confirmation).getByRole('button', { name: 'Cancel' }))
  expect(screen.queryByRole('dialog')).toBeNull()
  expect(document.activeElement).toBe(loadLatest)
  expect((screen.getByLabelText('Purpose (optional)') as HTMLTextAreaElement).value).toBe('My draft purpose')
  expect(onDirtyChange).toHaveBeenLastCalledWith(true)

  fireEvent.click(loadLatest)
  fireEvent.click(within(screen.getByRole('dialog')).getByRole('button', { name: 'Confirm' }))
  await waitFor(() => expect((screen.getByLabelText('Name') as HTMLInputElement).value).toBe(latest.name))
  expect((screen.getByLabelText('Purpose (optional)') as HTMLTextAreaElement).value).toBe(latest.description)
  expect(screen.queryByRole('checkbox', { name: 'alpha' })).toBeNull()
  fireEvent.click(screen.getByRole('button', { name: 'Add Skills' }))
  expect((screen.getByRole('checkbox', { name: 'alpha' }) as HTMLInputElement).checked).toBe(false)
  expect((screen.getByRole('checkbox', { name: 'beta' }) as HTMLInputElement).checked).toBe(true)
  expect(toolChoice('beta', 'Cursor').checked).toBe(true)
  expect(onDirtyChange).toHaveBeenLastCalledWith(false)
  expect(screen.queryByRole('alert')).toBeNull()
  fireEvent.click(screen.getByRole('button', { name: 'Back' }))
  expect(screen.queryByRole('dialog')).toBeNull()
  expect(screen.getByRole('heading', { level: 1, name: latest.name })).toBeTruthy()
  expect(invoke.mock.calls.filter(([command]) => command === 'workspace_save_preset')).toHaveLength(1)
})

it('distinguishes manual removals from manual applications in history', async () => {
  const history: ApplicationRecord[] = [false, true].map(remove => ({
    id: remove ? 'removal' : 'application', preset: null, createdAt: 1, confirmed: true, remove, undoTargetCount: 0,
    items: [{ skillId: 'alpha', name: 'alpha', tools: ['codex'], scope: { kind: 'project', path: '/project' }, targets: [], action: remove ? 'remove' : 'add', status: 'done', reason: null }],
  }))
  invoke.mockImplementation(async command => command === 'workspace_history' ? history : [])
  render(<PresetPage {...props} t={i18n.t} />)
  fireEvent.click(screen.getByRole('button', { name: 'All application records' }))

  expect(await screen.findByRole('button', { name: /Manual application/ })).toBeTruthy()
  const removal = screen.getByRole('button', { name: /Manual removal/ })
  expect(removal.textContent).not.toContain('Manual application')
  fireEvent.click(removal)
  expand('View details')
  expect(screen.getByText('Remove')).toBeTruthy()
  expect(screen.queryByRole('button', { name: 'Undo additions' })).toBeNull()
})

const undoReceipt = (targetId: string, reason: string): WorkspaceUndoItem => ({
  targetId, skillId: 'alpha', path: `/project/${targetId}`, tool: targetId === 'codex' ? 'codex' : 'cursor',
  scope: { kind: 'project', path: '/project' }, canRemove: reason === 'newTarget', reason,
})
const undoRecord = (id: string, name: string): ApplicationRecord => ({
  id, preset: { ...preset, name }, createdAt: 1, confirmed: true, remove: false, undoTargetCount: 1,
  items: [{ skillId: 'alpha', name: 'alpha', tools: ['codex', 'cursor'], scope: { kind: 'project', path: '/project' }, targets: [], action: 'repair', status: 'done', reason: null }],
})

it.each(['no additions', 'all additions undone'])('uses receipt metadata to hide undo for %s without inspecting files', async scenario => {
  const record = { ...undoRecord('inspected', 'Inspected application'), undoTargetCount: 0 }
  record.items = [
    { ...record.items[0], action: 'existing', status: 'done' },
    ...(scenario === 'all additions undone' ? [{ ...record.items[0], skillId: 'beta', name: 'beta', action: 'add', status: 'undone' }] : []),
  ]
  invoke.mockImplementation(async command => command === 'workspace_history' ? [record] : [])
  render(<PresetPage {...props} t={i18n.t} />)
  fireEvent.click(screen.getByRole('button', { name: 'All application records' }))
  fireEvent.click(await screen.findByRole('button', { name: /Inspected application/ }))
  expect(screen.queryByRole('button', { name: 'Undo additions' })).toBeNull()
  expect(screen.getByRole('heading', { name: 'Application result' })).toBeTruthy()
  expect(invoke.mock.calls.some(([command]) => command === 'workspace_preview_undo')).toBe(false)
})

it.each(['newTarget', 'laterUse'])('keeps partially undone repair receipts reviewable when the remaining target is %s', async reason => {
  const record = undoRecord('partial', 'Partially undone application')
  const receipts = [undoReceipt('codex', 'alreadyUndone'), undoReceipt('cursor', reason)]
  invoke.mockImplementation(async command => command === 'workspace_history' ? [record] : command === 'workspace_preview_undo' ? receipts : [])
  render(<PresetPage {...props} t={i18n.t} />)
  fireEvent.click(screen.getByRole('button', { name: 'All application records' }))
  fireEvent.click(await screen.findByRole('button', { name: /Partially undone application/ }))

  const undo = screen.getByRole('button', { name: 'Undo additions' }) as HTMLButtonElement
  await waitFor(() => expect(undo.disabled).toBe(false))
  fireEvent.click(undo)
  await screen.findByRole('heading', { name: 'Undo additions' })
  const dialog = screen.getByRole('dialog', { name: 'Undo additions' })
  const alreadyUndone = within(dialog).getByText('1 deployments already undone', { selector: 'summary' })
  expect(alreadyUndone.parentElement?.hasAttribute('open')).toBe(false)
  fireEvent.click(alreadyUndone)
  expect(within(dialog).getByText('/project/codex')).toBeTruthy()
  for (const details of within(dialog).getAllByText('Deployment locations', { selector: 'summary' })) fireEvent.click(details)
  expect(within(dialog).getByText('/project/cursor')).toBeTruthy()
  if (reason === 'newTarget') expect(within(dialog).getByTitle('Cursor')).toBeTruthy()
  else expect(within(dialog).getByRole('heading', { name: 'Kept for your protection' })).toBeTruthy()
  expect((screen.getByRole('button', { name: 'Confirm undo' }) as HTMLButtonElement).disabled).toBe(reason !== 'newTarget')
})

it('keeps undo review available for retry when receipt inspection fails', async () => {
  const record = undoRecord('retry', 'Retry receipt inspection')
  let inspections = 0
  invoke.mockImplementation(async command => {
    if (command === 'workspace_history') return [record]
    if (command === 'workspace_preview_undo') {
      inspections += 1
      if (inspections === 1) throw new Error('receipt inspection unavailable')
      return [undoReceipt('codex', 'newTarget')]
    }
    return []
  })
  render(<PresetPage {...props} t={i18n.t} />)
  fireEvent.click(screen.getByRole('button', { name: 'All application records' }))
  fireEvent.click(await screen.findByRole('button', { name: /Retry receipt inspection/ }))

  const undo = screen.getByRole('button', { name: 'Undo additions' }) as HTMLButtonElement
  await waitFor(() => expect(undo.disabled).toBe(false))
  expect(inspections).toBe(0)
  fireEvent.click(undo)
  await screen.findByRole('alert')
  expect(screen.getByRole('heading', { name: 'Application result' })).toBeTruthy()
  expect(inspections).toBe(1)
  fireEvent.click(undo)
  await screen.findByRole('heading', { name: 'Undo additions' })
  expect(inspections).toBe(2)
  expect((screen.getByRole('button', { name: 'Confirm undo' }) as HTMLButtonElement).disabled).toBe(false)
})

it('binds undo availability and explicit inspection to the selected history record', async () => {
  const first = { ...undoRecord('first', 'First application'), undoTargetCount: 0 }
  const second = undoRecord('second', 'Second application')
  invoke.mockImplementation(async (command, args) => {
    if (command === 'workspace_history') return [first, second]
    if (command === 'workspace_preview_undo') {
      expect(args.id).toBe(second.id)
      return [undoReceipt('cursor', 'newTarget')]
    }
    return []
  })
  render(<PresetPage {...props} t={i18n.t} />)
  fireEvent.click(screen.getByRole('button', { name: 'All application records' }))
  fireEvent.click(await screen.findByRole('button', { name: /First application/ }))
  expect(screen.queryByRole('button', { name: 'Undo additions' })).toBeNull()
  fireEvent.click(screen.getByRole('button', { name: 'Back' }))
  fireEvent.click(screen.getByRole('button', { name: /Second application/ }))
  const undo = screen.getByRole('button', { name: 'Undo additions' }) as HTMLButtonElement
  expect(undo.disabled).toBe(false)
  expect(invoke.mock.calls.some(([command]) => command === 'workspace_preview_undo')).toBe(false)
  fireEvent.click(undo)
  await screen.findByRole('heading', { name: 'Undo additions' })
  expect(invoke.mock.calls.filter(([command]) => command === 'workspace_preview_undo')).toEqual([['workspace_preview_undo', { id: second.id }]])
  fireEvent.click(within(screen.getByRole('dialog', { name: 'Undo additions' })).getByText('Deployment locations', { selector: 'summary' }))
  expect(screen.getByText('/project/cursor')).toBeTruthy()
})

it('removes the undo action after confirming all owned additions while preserving existing deployments', async () => {
  let record = undoRecord('confirmed-undo', 'Application to undo')
  record.items = [...record.items, { ...record.items[0], skillId: 'beta', name: 'beta', action: 'existing' }]
  let receipts = [undoReceipt('codex', 'newTarget')]
  invoke.mockImplementation(async command => {
    if (command === 'workspace_history') return [record]
    if (command === 'workspace_preview_undo') return receipts
    if (command === 'workspace_undo') {
      record = { ...record, undoTargetCount: 0, items: [{ ...record.items[0], status: 'undone' }, record.items[1]] }
      const reviewed = receipts
      receipts = [undoReceipt('codex', 'alreadyUndone')]
      return reviewed
    }
    return []
  })
  render(<PresetPage {...props} t={i18n.t} />)
  fireEvent.click(screen.getByRole('button', { name: 'All application records' }))
  fireEvent.click(await screen.findByRole('button', { name: /Application to undo/ }))
  const undo = screen.getByRole('button', { name: 'Undo additions' }) as HTMLButtonElement
  await waitFor(() => expect(undo.disabled).toBe(false))
  fireEvent.click(undo)
  fireEvent.click(await screen.findByRole('button', { name: 'Confirm undo' }))

  await screen.findByRole('heading', { name: 'Application result' })
  await waitFor(() => expect(screen.queryByRole('button', { name: 'Undo additions' })).toBeNull())
  expect(record.items.map(item => item.status)).toEqual(['undone', 'done'])
  expect(invoke.mock.calls.filter(([command]) => command === 'workspace_undo')).toHaveLength(1)
})


it.each(['already loaded', 'added concurrently'])('reports a duplicate preset name next to the field when the name was %s', async scenario => {
  invoke.mockImplementation(async command => {
    if (command === 'workspace_presets') return scenario === 'already loaded' ? [preset] : []
    if (command === 'workspace_save_preset') throw new Error('INVALID_ARGUMENT: preset name already exists')
    return []
  })
  render(<PresetPage {...props} intent={{ mode: 'edit', members: preset.members }} t={i18n.t} />)
  await act(async () => {})
  fireEvent.change(screen.getByLabelText('Name'), { target: { value: 'Daily' } })
  fireEvent.click(screen.getByRole('button', { name: 'Save preset' }))
  const alert = await screen.findByRole('alert')
  const name = screen.getByLabelText('Name')
  expect(alert.textContent).toContain('name already exists')
  expect(name.getAttribute('aria-invalid')).toBe('true')
  expect(name.getAttribute('aria-describedby')).toBe(alert.id)
  expect(screen.queryByRole('button', { name: 'Reload records' })).toBeNull()
  expect((name as HTMLInputElement).value).toBe('Daily')
  expect(toolChoice('alpha', 'Codex').checked).toBe(true)
  if (scenario === 'already loaded') expect(invoke.mock.calls.some(([command]) => command === 'workspace_save_preset')).toBe(false)
  fireEvent.change(name, { target: { value: 'Daily alternative' } })
  expect(name.getAttribute('aria-invalid')).toBe('false')
  expect(screen.queryByRole('alert')).toBeNull()
})

it('keeps a completed application visible if refreshing the surrounding library fails', async () => {
  const reviewed = applicationPlan()
  const completed = { ...reviewed, confirmed: true, undoTargetCount: 2, items: reviewed.items.map(item => ({ ...item, status: 'done' })) }
  invoke.mockImplementation(async command => command === 'workspace_presets' ? [preset] : command === 'workspace_preview' ? reviewed : command === 'workspace_execute' ? completed : [])
  render(<PresetPage {...props} onChanged={async () => { throw new Error('library refresh unavailable') }} t={i18n.t} />)
  fireEvent.click(await screen.findByRole('button', { name: 'Apply Daily' }))
  fireEvent.click(screen.getByRole('radio', { name: 'Global' }))
  fireEvent.click(screen.getByRole('button', { name: 'Apply now' }))
  expect(await screen.findByRole('heading', { name: 'Application result' })).toBeTruthy()
  expect(screen.getByText('Application complete')).toBeTruthy()
  expect(screen.getByRole('alert')).toBeTruthy()
  expect(screen.queryByRole('dialog')).toBeNull()
  expect(screen.queryByRole('button', { name: 'Apply now' })).toBeNull()
  expect(invoke.mock.calls.filter(([command]) => command === 'workspace_execute')).toHaveLength(1)
})

it('rechecks historical removals as removals and returns to their result when cancelled', async () => {
  const history = applicationPlan({ id: 'old-removal', preset: null, confirmed: true, remove: true })
  history.items = history.items.map(item => ({ ...item, action: 'remove', status: 'failed', reason: 'TARGET_CONFLICT' }))
  const reviewed = { ...history, confirmed: false, items: history.items.map(item => ({ ...item, status: 'pending', reason: null })) }
  invoke.mockImplementation(async command => command === 'workspace_history' ? [history] : command === 'workspace_recheck' ? reviewed : command === 'workspace_execute' ? { ...reviewed, confirmed: true } : [])
  render(<PresetPage {...props} t={i18n.t} />)
  fireEvent.click(screen.getByRole('button', { name: 'All application records' }))
  fireEvent.click(await screen.findByRole('button', { name: /Manual removal/ }))
  fireEvent.click(screen.getByRole('button', { name: 'Preview unfinished items again' }))
  const dialog = await screen.findByRole('dialog', { name: 'Remove selected targets' })
  const confirm = within(dialog).getByRole('button', { name: 'Remove selected targets' })
  expect(confirm.classList.contains('btn-danger')).toBe(true)
  expect(within(dialog).queryByRole('button', { name: 'Apply available Skills' })).toBeNull()
  expect(invoke.mock.calls.some(([command]) => command === 'workspace_execute')).toBe(false)
  fireEvent.click(within(dialog).getByRole('button', { name: 'Back to result' }))
  expect(screen.queryByRole('dialog')).toBeNull()
  expect(screen.getByRole('heading', { name: 'Application result' })).toBeTruthy()
  expect(screen.queryByRole('button', { name: 'Apply now' })).toBeNull()
})


it.each(['available', 'unavailable'])('recovers a rejected execution through the same journal when history is %s', async historyState => {
  const reviewed = applicationPlan()
  const partial = { ...reviewed, confirmed: true, undoTargetCount: 1, items: reviewed.items.map((item, index) => ({ ...item, status: index ? 'interrupted' : 'done' })) }
  const rechecked = { ...partial, items: partial.items.map(item => item.status === 'done' ? item : { ...item, status: 'pending' }) }
  const completed = { ...partial, undoTargetCount: 2, items: partial.items.map(item => ({ ...item, status: 'done' })) }
  let executions = 0
  invoke.mockImplementation(async command => {
    if (command === 'workspace_presets') return [preset]
    if (command === 'workspace_preview') return reviewed
    if (command === 'workspace_execute') {
      if (++executions === 1) throw new Error('transport interrupted after partial write')
      return completed
    }
    if (command === 'workspace_history') {
      if (historyState === 'unavailable') throw new Error('history unavailable')
      return [executions > 1 ? completed : partial]
    }
    if (command === 'workspace_recheck') return rechecked
    return []
  })
  render(<PresetPage {...props} t={i18n.t} />)
  fireEvent.click(await screen.findByRole('button', { name: 'Apply Daily' }))
  fireEvent.click(screen.getByRole('radio', { name: 'Global' }))
  fireEvent.click(screen.getByRole('button', { name: 'Apply now' }))
  expect(await screen.findByRole('heading', { name: 'Application result' })).toBeTruthy()
  expect(screen.getByText('Application could not be confirmed. This run was saved; recheck it before continuing.')).toBeTruthy()
  expect(screen.queryByRole('dialog')).toBeNull()
  expect(screen.queryByRole('button', { name: 'Apply now' })).toBeNull()
  if (historyState === 'available') expect(screen.getByRole('button', { name: 'Undo additions' })).toBeTruthy()
  fireEvent.click(screen.getByRole('button', { name: 'Preview unfinished items again' }))
  const continueApply = await screen.findByRole('button', { name: 'Apply available Skills' })
  const review = within(screen.getByRole('dialog'))
  expect(review.getByText(i18n.t('workspace110.availableCount', { count: 1 }))).toBeTruthy()
  expect(review.queryByText(i18n.t('workspace110.availableCount', { count: 2 }))).toBeNull()
  fireEvent.click(continueApply)
  await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull())
  expect(screen.getByText('Application complete')).toBeTruthy()
  expect(invoke.mock.calls.filter(([command]) => command === 'workspace_preview')).toHaveLength(1)
  expect(invoke.mock.calls.filter(([command]) => command === 'workspace_execute').map(([, args]) => args)).toEqual([{ id: reviewed.id }, { id: reviewed.id }])
  expect(invoke).toHaveBeenCalledWith('workspace_recheck', { id: reviewed.id })
  fireEvent.click(screen.getByRole('button', { name: 'Done' }))
  expect(invoke.mock.calls.some(([command]) => command === 'workspace_discard')).toBe(false)
})

it('does not discard the known journal when closing an execution with an unknown outcome', async () => {
  const reviewed = applicationPlan()
  invoke.mockImplementation(async command => {
    if (command === 'workspace_presets') return [preset]
    if (command === 'workspace_preview') return reviewed
    if (command === 'workspace_execute' || command === 'workspace_history') throw new Error('transport unavailable')
    return []
  })
  render(<PresetPage {...props} t={i18n.t} />)
  fireEvent.click(await screen.findByRole('button', { name: 'Apply Daily' }))
  fireEvent.click(screen.getByRole('radio', { name: 'Global' }))
  fireEvent.click(screen.getByRole('button', { name: 'Apply now' }))
  await screen.findByText('Application could not be confirmed. This run was saved; recheck it before continuing.')
  const done = screen.getByRole('button', { name: 'Done' }) as HTMLButtonElement
  await waitFor(() => expect(done.disabled).toBe(false))
  fireEvent.click(done)
  expect(await screen.findByRole('button', { name: 'Apply Daily' })).toBeTruthy()
  expect(invoke.mock.calls.filter(([command]) => command === 'workspace_execute')).toEqual([['workspace_execute', { id: reviewed.id }]])
  expect(invoke.mock.calls.some(([command]) => command === 'workspace_discard')).toBe(false)
})

it('accepts a fully completed journal recovered after an execution transport error without an impossible retry instruction', async () => {
  const reviewed = applicationPlan()
  const completed = { ...reviewed, confirmed: true, undoTargetCount: 2, items: reviewed.items.map(item => ({ ...item, status: 'done' })) }
  invoke.mockImplementation(async command => {
    if (command === 'workspace_presets') return [preset]
    if (command === 'workspace_preview') return reviewed
    if (command === 'workspace_execute') throw new Error('response lost after completing the application')
    if (command === 'workspace_history') return [completed]
    return []
  })
  render(<PresetPage {...props} t={i18n.t} />)
  fireEvent.click(await screen.findByRole('button', { name: 'Apply Daily' }))
  fireEvent.click(screen.getByRole('radio', { name: 'Global' }))
  fireEvent.click(screen.getByRole('button', { name: 'Apply now' }))
  await screen.findByRole('heading', { name: 'Application result' })
  expect(screen.getByText('Application complete')).toBeTruthy()
  expect(screen.queryByText('Application could not be confirmed. This run was saved; recheck it before continuing.')).toBeNull()
  expect(screen.queryByRole('button', { name: 'Preview unfinished items again' })).toBeNull()
  expect(screen.getByRole('button', { name: 'Undo additions' })).toBeTruthy()
  expect(invoke.mock.calls.filter(([command]) => command === 'workspace_preview')).toHaveLength(1)
  expect(invoke.mock.calls.filter(([command]) => command === 'workspace_execute')).toHaveLength(1)
  fireEvent.click(screen.getByRole('button', { name: 'Done' }))
  expect(invoke.mock.calls.some(([command]) => command === 'workspace_discard')).toBe(false)
})

it.each(['directory', 'detail'])('returns focus to the %s application trigger after inert blurs it and Escape closes the dialog', async origin => {
  const blurred = simulateInertBlur()
  render(<PresetPage {...props} t={i18n.t} />)
  await screen.findByRole('button', { name: 'Apply Daily' })
  if (origin === 'detail') fireEvent.click(screen.getByRole('button', { name: 'Daily' }))
  const trigger = screen.getByRole('button', { name: origin === 'directory' ? 'Apply Daily' : 'Apply to…' })
  expect(document.activeElement).not.toBe(trigger)
  fireEvent.click(trigger)
  const dialog = screen.getByRole('dialog', { name: 'Apply preset' })
  expect(blurred).toHaveBeenCalledOnce()
  expect(document.activeElement).toBe(dialog)
  fireEvent.keyDown(document, { key: 'Tab' })
  expect(document.activeElement).toBe(within(dialog).getByRole('button', { name: 'Close' }))
  fireEvent.keyDown(document, { key: 'Escape' })
  expect(screen.queryByRole('dialog')).toBeNull()
  expect(document.activeElement).toBe(trigger)
  expect(invoke.mock.calls.some(([command]) => ['workspace_preview', 'workspace_execute', 'workspace_save_preset'].includes(command))).toBe(false)
})

it.each(['history', 'library'])('closes a successful undo confirmation even if refreshing %s subsequently fails', async failedRead => {
  let record = undoRecord('undo-before-refresh-failure', 'Undo then refresh')
  let undone = false
  const reviewed = [undoReceipt('codex', 'newTarget')]
  invoke.mockImplementation(async command => {
    if (command === 'workspace_history') {
      if (undone && failedRead === 'history') throw new Error('history read unavailable')
      return [record]
    }
    if (command === 'workspace_preview_undo') return reviewed
    if (command === 'workspace_undo') {
      undone = true
      record = { ...record, undoTargetCount: 0, items: record.items.map(item => ({ ...item, status: 'undone' })) }
      // The backend returns the reviewed items from before removal, not fresh receipt states.
      return reviewed
    }
    return []
  })
  render(<PresetPage {...props} onChanged={async () => { if (failedRead === 'library') throw new Error('library refresh unavailable') }} t={i18n.t} />)
  fireEvent.click(screen.getByRole('button', { name: 'All application records' }))
  fireEvent.click(await screen.findByRole('button', { name: /Undo then refresh/ }))
  fireEvent.click(screen.getByRole('button', { name: 'Undo additions' }))
  fireEvent.click(await screen.findByRole('button', { name: 'Confirm undo' }))
  await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull())
  expect(screen.getByRole('heading', { name: 'Application result' })).toBeTruthy()
  expect(screen.getByRole('alert')).toBeTruthy()
  expect(screen.queryByRole('button', { name: 'Confirm undo' })).toBeNull()
  expect(screen.queryByRole('button', { name: 'Undo additions' })).toBeNull()
  expect(invoke.mock.calls.filter(([command]) => command === 'workspace_undo')).toHaveLength(1)
})

it('returns to the existing result when a source-launched application recheck is cancelled', async () => {
  const reviewed = applicationPlan({ preset: null })
  const failed = { ...reviewed, confirmed: true, items: reviewed.items.map(item => ({ ...item, status: 'failed', reason: 'TARGET_CONFLICT' })) }
  invoke.mockImplementation(async command => command === 'workspace_preview' || command === 'workspace_recheck' ? reviewed : command === 'workspace_execute' ? failed : command === 'workspace_history' ? [failed] : [])
  render(<PresetPage {...props} intent={{ mode: 'select', members: preset.members, scope: { kind: 'global' } }} t={i18n.t} />)
  fireEvent.click(screen.getByRole('button', { name: 'Apply now' }))
  await screen.findByRole('heading', { name: 'Application result' })
  fireEvent.click(screen.getByRole('button', { name: 'Preview unfinished items again' }))
  const dialog = await screen.findByRole('dialog', { name: 'Some Skills need attention' })
  fireEvent.click(within(dialog).getByRole('button', { name: 'Back to result' }))
  expect(screen.queryByRole('dialog')).toBeNull()
  expect(screen.getByRole('heading', { name: 'Application result' })).toBeTruthy()
  expect(onBack).not.toHaveBeenCalled()
  expect(invoke.mock.calls.filter(([command]) => command === 'workspace_preview')).toHaveLength(1)
  expect(invoke.mock.calls.filter(([command]) => command === 'workspace_execute')).toHaveLength(1)
})

it('reports the removed and protected counts after a partial undo and keeps that feedback on its own record', async () => {
  let record = { ...undoRecord('partial-with-feedback', 'Partially undo changed files'), undoTargetCount: 2 }
  const unrelated = { ...undoRecord('other-record', 'Another application'), undoTargetCount: 0 }
  let receipts = [undoReceipt('codex', 'newTarget'), undoReceipt('cursor', 'contentChanged')]
  invoke.mockImplementation(async command => {
    if (command === 'workspace_history') return [record, unrelated]
    if (command === 'workspace_preview_undo') return receipts
    if (command === 'workspace_undo') {
      const reviewed = receipts
      record = { ...record, undoTargetCount: 1 }
      // A partially undone item remains done while its protected receipt is retained.
      receipts = [undoReceipt('codex', 'alreadyUndone'), undoReceipt('cursor', 'contentChanged')]
      return reviewed
    }
    return []
  })
  render(<PresetPage {...props} t={i18n.t} />)
  fireEvent.click(screen.getByRole('button', { name: 'All application records' }))
  fireEvent.click(await screen.findByRole('button', { name: /Partially undo changed files/ }))
  fireEvent.click(screen.getByRole('button', { name: 'Undo additions' }))
  fireEvent.click(await screen.findByRole('button', { name: 'Confirm undo' }))

  const message = i18n.t('workspace110.undoSummary', { removed: 1, retained: 1 })
  const feedback = await screen.findByText(message)
  expect(message).toContain('1')
  expect(feedback.closest('[role="status"]')).toBeTruthy()
  expect(screen.queryByRole('dialog')).toBeNull()
  await waitFor(() => expect((screen.getByRole('button', { name: 'Undo additions' }) as HTMLButtonElement).disabled).toBe(false))
  expect(record.items[0].status).toBe('done')
  expect(invoke.mock.calls.filter(([command]) => command === 'workspace_undo')).toEqual([['workspace_undo', { id: record.id }]])

  fireEvent.click(screen.getByRole('button', { name: 'Undo additions' }))
  const dialog = await screen.findByRole('dialog', { name: 'Undo additions' })
  expect(within(dialog).getByRole('heading', { name: 'Kept for your protection' })).toBeTruthy()
  expect(within(dialog).getByText(i18n.t('workspace110.undoReasons.contentChanged'))).toBeTruthy()
  expect((within(dialog).getByRole('button', { name: 'Confirm undo' }) as HTMLButtonElement).disabled).toBe(true)
  fireEvent.click(within(dialog).getByRole('button', { name: 'Cancel' }))
  fireEvent.click(screen.getByRole('button', { name: 'Back' }))
  fireEvent.click(screen.getByRole('button', { name: /Another application/ }))
  expect(screen.queryByText(message)).toBeNull()
})


it.each(['Undo additions', 'Preview unfinished items again'])('restores the %s result trigger after inert blurs it while opening the dialog', async action => {
  const blurred = simulateInertBlur()
  const record = undoRecord('focus-result', 'Result focus restoration')
  if (action === 'Preview unfinished items again') record.items = record.items.map(item => ({ ...item, status: 'failed' }))
  invoke.mockImplementation(async command => {
    if (command === 'workspace_history') return [record]
    if (command === 'workspace_preview_undo') return [undoReceipt('codex', 'newTarget')]
    if (command === 'workspace_recheck') return { ...record, items: record.items.map(item => ({ ...item, status: 'pending' })) }
    return []
  })
  render(<PresetPage {...props} t={i18n.t} />)
  fireEvent.click(screen.getByRole('button', { name: 'All application records' }))
  fireEvent.click(await screen.findByRole('button', { name: /Result focus restoration/ }))
  const trigger = screen.getByRole('button', { name: action })
  fireEvent.click(trigger)
  const dialog = await screen.findByRole('dialog')
  expect(blurred).toHaveBeenCalledOnce()
  expect(document.activeElement).toBe(dialog)
  fireEvent.keyDown(document, { key: 'Escape' })
  expect(screen.queryByRole('dialog')).toBeNull()
  expect(document.activeElement).toBe(trigger)
  expect(invoke.mock.calls.some(([command]) => ['workspace_execute', 'workspace_undo'].includes(command))).toBe(false)
})
