// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, expect, it, vi } from 'vitest'
import type { TFunction } from 'i18next'
import SkillCard from './SkillCard'
import type { ManagedSkill } from './types'

afterEach(cleanup)

const skill: ManagedSkill = {
  id: 'one', name: 'one', source_type: 'local', source_ref: '/projects/one', central_path: '/managed/one',
  created_at: 1, updated_at: 1, status: 'ok', enabled: true, tags: [],
  targets: [{ tool: 'codex', scope: 'global', mode: 'symlink', status: 'ok', target_path: '/codex/one' }],
}
const props = {
  installedTools: [{ id: 'codex', label: 'Codex' }, { id: 'cursor', label: 'Cursor' }],
  loading: false, bulkMode: false, bulkSelected: false, getGithubInfo: () => null,
  getSkillSourceLabel: () => '/projects/one', formatRelative: () => 'now', onUpdate: vi.fn(), onDelete: vi.fn(),
  onToggleEnabled: vi.fn(), onToggleTool: vi.fn(), onOpenScope: vi.fn(), onOpenDetail: vi.fn(), onEditTags: vi.fn(),
  onToggleBulkSelection: vi.fn(), getSkillScope: () => 'global' as const, getSkillProjects: () => [],
  t: ((key: string) => key) as TFunction,
}

it('shows the tool name and sync state immediately on hover and focus', () => {
  render(<SkillCard {...props} skill={skill} />)
  const codex = screen.getByRole('button', { name: 'Codex · toolManagement.synced' })
  expect(codex.getAttribute('title')).toBeNull()
  expect(screen.queryByRole('tooltip')).toBeNull()

  fireEvent.mouseEnter(codex)
  const tooltip = screen.getByRole('tooltip')
  expect(tooltip.textContent).toBe('Codex · toolManagement.synced')
  expect(tooltip.parentElement).toBe(document.body)
  fireEvent.mouseLeave(codex)
  expect(screen.queryByRole('tooltip')).toBeNull()

  fireEvent.focus(screen.getByRole('button', { name: 'Cursor · toolManagement.notSynced' }))
  expect(screen.getByRole('tooltip').textContent).toBe('Cursor · toolManagement.notSynced')
  fireEvent.scroll(window)
  expect(screen.queryByRole('tooltip')).toBeNull()
})

it('keeps the native title on disabled Skills, where hover events are not delivered', () => {
  render(<SkillCard {...props} skill={{ ...skill, enabled: false }} />)
  const codex = screen.getByRole('button', { name: 'Codex · toolManagement.synced' }) as HTMLButtonElement
  expect(codex.disabled).toBe(true)
  expect(codex.getAttribute('title')).toBe('Codex · toolManagement.synced')
})
