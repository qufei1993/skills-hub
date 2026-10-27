// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import i18n from '../../../i18n'
import type { ManagedSkill, SkillPreset } from '../types'
import SkillUsage from './SkillUsage'

const skill: ManagedSkill = { id: 'alpha', name: 'Alpha', source_type: 'local', central_path: '/central/alpha', created_at: 1, updated_at: 1, enabled: true, status: 'ok', tags: [], targets: [] }
const preset: SkillPreset = { id: 'p1', revision: 1, name: 'Daily', description: '', members: [{ skillId: 'alpha', tools: ['codex'] }] }
beforeEach(async () => { await i18n.changeLanguage('en') })
afterEach(cleanup)

it('distinguishes loading from empty membership and hides results belonging to a previous Skill', async () => {
  let resolveFirst!: (value: SkillPreset[]) => void
  let resolveSecond!: (value: SkillPreset[]) => void
  const invoke = vi.fn().mockImplementationOnce(() => new Promise(done => { resolveFirst = done })).mockImplementationOnce(() => new Promise(done => { resolveSecond = done }))
  const view = render(<SkillUsage skill={skill} invokeTauri={invoke} t={i18n.t} />)
  fireEvent.click(screen.getByText('Usage locations and presets'))
  expect(screen.getByRole('status')).toBeTruthy()
  expect(screen.queryByText('No preset membership')).toBeNull()
  await act(async () => { resolveFirst([preset]) })
  expect(screen.getByText('Daily')).toBeTruthy()

  view.rerender(<SkillUsage skill={{ ...skill, id: 'beta' }} invokeTauri={invoke} t={i18n.t} />)
  expect(screen.getByRole('status')).toBeTruthy()
  expect(screen.queryByText('Daily')).toBeNull()
  await act(async () => { resolveSecond([]) })
  expect(screen.queryByRole('status')).toBeNull()
  expect(screen.getByText('No preset membership')).toBeTruthy()
})

it('offers retry after a membership load failure without claiming no membership', async () => {
  const invoke = vi.fn().mockRejectedValueOnce(new Error('read failed')).mockResolvedValueOnce([preset])
  render(<SkillUsage skill={skill} invokeTauri={invoke} t={i18n.t} />)
  fireEvent.click(screen.getByText('Usage locations and presets'))
  await screen.findByRole('alert')
  expect(screen.queryByText('No preset membership')).toBeNull()
  fireEvent.click(screen.getByRole('button', { name: 'Retry' }))
  await screen.findByText('Daily')
  expect(screen.queryByRole('alert')).toBeNull()
  expect(invoke).toHaveBeenCalledTimes(2)
})
