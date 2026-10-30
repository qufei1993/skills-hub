// @vitest-environment jsdom
import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import i18n from '../../i18n'
import SkillCard from './SkillCard'
import type { ManagedSkill } from './types'

const skill: ManagedSkill = {
  id: 'mixed', name: 'Mixed scope', enabled: true, status: 'ok', source_type: 'local',
  central_path: '/central/mixed', created_at: 1, updated_at: 1, tags: [],
  targets: [
    { tool: 'codex', scope: 'global', mode: 'symlink', status: 'ok', target_path: '/home/.codex/skills/mixed' },
    { tool: 'cursor', scope: 'global', mode: 'copy', status: 'ok', target_path: '/home/.cursor/skills/mixed' },
    { tool: 'codex', scope: 'project', project_path: '/project', mode: 'copy', status: 'ok', target_path: '/project/.agents/skills/mixed' },
  ],
}
const props = {
  installedTools: [{ id: 'codex', label: 'Codex' }, { id: 'cursor', label: 'Cursor' }],
  loading: false, bulkMode: false, bulkSelected: false,
  getGithubInfo: () => null, getSkillSourceLabel: () => 'Local source', formatRelative: () => 'Now',
  onUpdate: vi.fn(), onDelete: vi.fn(), onToggleEnabled: vi.fn(), onToggleTool: vi.fn(),
  onOpenScope: vi.fn(), onOpenDetail: vi.fn(), onEditTags: vi.fn(), onToggleBulkSelection: vi.fn(),
  getSkillScope: (): 'global' | 'project' => 'project', getSkillProjects: () => ['/project'],
}

beforeEach(async () => { await i18n.changeLanguage('en') })
afterEach(cleanup)

it('shows tools from both global and project deployments in an all-scopes card', () => {
  render(<SkillCard {...props} skill={skill} t={i18n.t} />)
  for (const name of ['Codex', 'Cursor']) {
    expect(screen.getByRole('button', { name: `${name} · ${i18n.t('toolManagement.synced')}` }).getAttribute('aria-pressed')).toBe('true')
  }
  expect(screen.getByText(i18n.t('syncTargetsCount', { synced: 2, total: 2 }))).toBeTruthy()
})

it.each(['global', 'project'] as const)('uses only the targets passed for a %s scope card', scope => {
  render(<SkillCard {...props} skill={{ ...skill, targets: skill.targets.filter(target => target.scope === scope) }} getSkillScope={() => scope} t={i18n.t} />)
  const cursorState = scope === 'global' ? 'synced' : 'notSynced'
  expect(screen.getByRole('button', { name: `Cursor · ${i18n.t(`toolManagement.${cursorState}`)}` }).getAttribute('aria-pressed')).toBe(String(scope === 'global'))
})

it('does not hide a failed global target behind a successful project target', () => {
  const targets = skill.targets.map(target => target.tool === 'codex' && target.scope === 'global' ? { ...target, status: 'error' } : target)
  render(<SkillCard {...props} skill={{ ...skill, targets }} t={i18n.t} />)
  expect(screen.getByRole('button', { name: `Codex · ${i18n.t('toolManagement.syncPartialFailed')}` }).getAttribute('aria-pressed')).toBe('false')
})
