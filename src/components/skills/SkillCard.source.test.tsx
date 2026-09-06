// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, expect, it, vi } from 'vitest'
import type { TFunction } from 'i18next'
import SkillCard from './SkillCard'
import type { ManagedSkill } from './types'

afterEach(cleanup)
it('shows unbound sync sources without pretending the managed copy is a source', () => {
  const skill: ManagedSkill = { id: 'one', name: 'one', source_type: 'local', source_ref: null, central_path: '/managed/one', created_at: 1, updated_at: 1, status: 'ok', enabled: true, tags: [], targets: [] }
  const update = vi.fn()
  const props = { installedTools: [], loading: false, bulkMode: false, bulkSelected: false, getGithubInfo: () => null, getSkillSourceLabel: () => '/managed/one', formatRelative: () => 'now', onUpdate: update, onDelete: vi.fn(), onToggleEnabled: vi.fn(), onToggleTool: vi.fn(), onOpenScope: vi.fn(), onOpenDetail: vi.fn(), onEditTags: vi.fn(), onToggleBulkSelection: vi.fn(), getSkillScope: () => 'global' as const, getSkillProjects: () => [], t: ((key: string) => key) as TFunction }
  const { rerender } = render(<SkillCard {...props} skill={skill} />)
  expect(screen.getByText('deviceSync.unboundSource')).toBeTruthy()
  const button = screen.getByRole('button', { name: 'update' }) as HTMLButtonElement
  expect(button.disabled).toBe(true)
  fireEvent.click(button)
  expect(update).not.toHaveBeenCalled()
  expect(screen.queryByRole('button', { name: 'deviceSync.viewIssueReason' })).toBeNull()
  rerender(<SkillCard {...props} skill={{ ...skill, source_ref: '/projects/one' }} />)
  expect((screen.getByRole('button', { name: 'update' }) as HTMLButtonElement).disabled).toBe(false)
  expect(screen.queryByText('deviceSync.unboundSource')).toBeNull()
})
