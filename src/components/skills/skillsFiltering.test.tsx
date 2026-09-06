// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import App from '../../App'
import i18n from '../../i18n'
import type { ManagedSkill } from './types'

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
})
