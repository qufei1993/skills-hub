// @vitest-environment jsdom
import { StrictMode, useState } from 'react'
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import i18n from '../../../i18n'
import type { ManagedSkill } from '../types'
import ContentReview, { type ContentPlan } from './ContentReview'

const skills: ManagedSkill[] = ['alpha', 'beta'].map(name => ({
  id: name, name, source_type: 'local', central_path: `/central/${name}`,
  created_at: 1, updated_at: 1, enabled: true, status: 'ok', tags: [], targets: [],
}))

function plan(skillId: string, restore = false): ContentPlan {
  return { id: `${skillId}-${restore ? 'restore' : 'update'}`, skillId, name: skillId, changes: [], targets: [], restore }
}

function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>(done => { resolve = done })
  return { promise, resolve }
}

// The real preview command takes the application's exclusive operation lock.
// Hold its first response explicitly so overlapping invocations fail deterministically.
function lockedPreviewBackend() {
  const gate = deferred<void>()
  let locked = false
  const invoke = vi.fn()
  invoke.mockImplementation(async (_command: string, args?: Record<string, unknown>) => {
    if (locked) throw new Error('OPERATION_BUSY|another Skills Hub operation is already in progress')
    locked = true
    try {
      await gate.promise
      return plan(String(args?.skillId), Boolean(args?.restore))
    } finally {
      locked = false
    }
  })
  return { invoke, release: () => gate.resolve() }
}

beforeEach(async () => { await i18n.changeLanguage('en') })
afterEach(cleanup)

it('reviews the exact checked plans without fetching and replacing their versions', async () => {
  const invoke = vi.fn()
  const onConfirm = vi.fn(async () => {})
  const preparedPlans = [plan('alpha'), plan('beta')]
  render(<ContentReview skills={skills.slice(0, 1)} restore={false} preparedPlans={preparedPlans} invokeTauri={invoke} onConfirm={onConfirm} onClose={vi.fn()} t={i18n.t} />)

  await screen.findByRole('heading', { name: 'alpha' })
  expect(screen.queryByRole('heading', { name: 'beta' })).toBeNull()
  expect(invoke).not.toHaveBeenCalled()
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: 'Confirm update' })) })
  expect(onConfirm).toHaveBeenCalledWith({ alpha: 'alpha-update' })
})

it('fetches a restore preview instead of using a prepared update plan', async () => {
  const invoke = vi.fn().mockResolvedValue(plan('alpha', true))
  render(<ContentReview skills={skills.slice(0, 1)} restore preparedPlans={[plan('alpha')]} invokeTauri={invoke} onConfirm={vi.fn(async () => {})} onClose={vi.fn()} t={i18n.t} />)
  await screen.findByRole('heading', { name: 'alpha' })
  expect(invoke).toHaveBeenCalledWith('workspace_content_preview', { skillId: 'alpha', restore: true })
})

it('previews every selected Skill without competing for the backend write lock', async () => {
  const backend = lockedPreviewBackend()
  const onConfirm = vi.fn(async () => {})
  render(<ContentReview skills={skills} restore={false} invokeTauri={backend.invoke} onConfirm={onConfirm} onClose={vi.fn()} t={i18n.t} />)
  await act(async () => { backend.release() })

  expect(screen.getByRole('heading', { name: 'alpha' })).toBeTruthy()
  expect(screen.getByRole('heading', { name: 'beta' })).toBeTruthy()
  expect(screen.queryByRole('alert')).toBeNull()
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: 'Confirm update' })) })
  expect(onConfirm).toHaveBeenCalledWith({ alpha: 'alpha-update', beta: 'beta-update' })
})

it('reuses the in-flight preview and loads every selected Skill through StrictMode effect replay', async () => {
  const backend = lockedPreviewBackend()
  render(<StrictMode><ContentReview skills={skills} restore={false} invokeTauri={backend.invoke} onConfirm={vi.fn(async () => {})} onClose={vi.fn()} t={i18n.t} /></StrictMode>)
  await waitFor(() => expect(backend.invoke).toHaveBeenCalledTimes(1))
  await act(async () => { backend.release() })

  expect(screen.getByRole('heading', { name: 'alpha' })).toBeTruthy()
  expect(screen.getByRole('heading', { name: 'beta' })).toBeTruthy()
  expect(screen.queryByRole('alert')).toBeNull()
  expect(backend.invoke.mock.calls.map(([, args]) => args.skillId)).toEqual(['alpha', 'beta'])
})

it.each(['cancel', 'escape'])('stops lazy previews after a user %s while the current request finishes', async close => {
  const backend = lockedPreviewBackend()
  const onClose = vi.fn()
  const onConfirm = vi.fn(async () => {})
  function Dialog() {
    const [open, setOpen] = useState(true)
    return open ? <ContentReview skills={skills} restore={false} invokeTauri={backend.invoke} onConfirm={onConfirm}
      onClose={() => { onClose(); setOpen(false) }} t={i18n.t} /> : null
  }
  render(<StrictMode><Dialog /></StrictMode>)
  await waitFor(() => expect(backend.invoke).toHaveBeenCalledTimes(1))
  expect((screen.getByRole('button', { name: 'Confirm update' }) as HTMLButtonElement).disabled).toBe(true)
  if (close === 'cancel') fireEvent.click(screen.getByRole('button', { name: 'Cancel' }))
  else fireEvent.keyDown(document, { key: 'Escape' })
  expect(onClose).toHaveBeenCalledOnce()
  expect(screen.queryByRole('dialog')).toBeNull()
  await act(async () => { backend.release() })
  expect(backend.invoke.mock.calls.map(([, args]) => args.skillId)).toEqual(['alpha'])
  expect(onConfirm).not.toHaveBeenCalled()
})

it.each([
  { name: 'selected Skill', nextSkill: skills[1], restore: false },
  { name: 'restore mode', nextSkill: skills[0], restore: true },
])('waits for the old command and ignores its plan when $name changes', async ({ nextSkill, restore }) => {
  const oldResult = deferred<ContentPlan>()
  const newResult = deferred<ContentPlan>()
  const invoke = vi.fn().mockImplementationOnce(() => oldResult.promise).mockImplementationOnce(() => newResult.promise)
  const onConfirm = vi.fn(async () => {})
  const props = { invokeTauri: invoke, onConfirm, onClose: vi.fn(), t: i18n.t }
  const view = render(<ContentReview {...props} skills={[skills[0]]} restore={false} />)
  await waitFor(() => expect(invoke).toHaveBeenCalledTimes(1))

  view.rerender(<ContentReview {...props} skills={[nextSkill]} restore={restore} />)
  expect(invoke).toHaveBeenCalledTimes(1)
  expect(screen.queryByRole('heading')).toBeNull()
  expect((screen.getByRole('button', { name: restore ? 'Restore pre-update content' : 'Confirm update' }) as HTMLButtonElement).disabled).toBe(true)
  await act(async () => { oldResult.resolve(plan('alpha')) })
  expect(invoke).toHaveBeenCalledTimes(2)
  expect(screen.queryByRole('heading')).toBeNull()
  expect(invoke).toHaveBeenLastCalledWith('workspace_content_preview', { skillId: nextSkill.id, restore })
  await act(async () => { newResult.resolve(plan(nextSkill.id, restore)) })

  expect(screen.getByRole('heading', { name: nextSkill.name })).toBeTruthy()
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: restore ? 'Restore pre-update content' : 'Confirm update' })) })
  expect(onConfirm).toHaveBeenCalledWith({ [nextSkill.id]: `${nextSkill.id}-${restore ? 'restore' : 'update'}` })
})

it('retains successful plans and explicitly rechecks only failed Skills', async () => {
  const retry = deferred<ContentPlan>()
  const invoke = vi.fn()
    .mockResolvedValueOnce(plan('alpha'))
    .mockRejectedValueOnce(new Error('INVALID_SOURCE'))
    .mockImplementationOnce(() => retry.promise)
  const onConfirm = vi.fn(async () => {})
  render(<ContentReview skills={skills} restore={false} invokeTauri={invoke} onConfirm={onConfirm} onClose={vi.fn()} t={i18n.t} />)
  await screen.findByRole('heading', { name: 'alpha' })
  expect(screen.getByRole('alert').textContent).toContain('beta')
  expect((screen.getByRole('button', { name: 'Confirm update' }) as HTMLButtonElement).disabled).toBe(false)

  fireEvent.click(screen.getByRole('button', { name: 'Preview unfinished items again' }))
  expect(screen.getByRole('heading', { name: 'alpha' })).toBeTruthy()
  expect((screen.getByRole('button', { name: 'Confirm update' }) as HTMLButtonElement).disabled).toBe(true)
  await waitFor(() => expect(invoke).toHaveBeenCalledTimes(3))
  expect(invoke.mock.calls.map(([, args]) => args.skillId)).toEqual(['alpha', 'beta', 'beta'])
  await act(async () => { retry.resolve(plan('beta')) })
  expect(screen.queryByRole('alert')).toBeNull()
  expect(screen.getAllByRole('heading').map(heading => heading.textContent)).toEqual(['alpha', 'beta'])
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: 'Confirm update' })) })
  expect(onConfirm).toHaveBeenCalledWith({ alpha: 'alpha-update', beta: 'beta-update' })
})

it('keeps all-failed previews open with a disabled confirm action and a retry path', async () => {
  const invoke = vi.fn().mockRejectedValue(new Error('INVALID_SOURCE'))
  const onConfirm = vi.fn(async () => {})
  render(<ContentReview skills={skills} restore={false} invokeTauri={invoke} onConfirm={onConfirm} onClose={vi.fn()} t={i18n.t} />)
  const alert = await screen.findByRole('alert')
  expect(alert.textContent).toContain('alpha, beta')
  expect((screen.getByRole('button', { name: 'Confirm update' }) as HTMLButtonElement).disabled).toBe(true)
  expect((screen.getByRole('button', { name: 'Preview unfinished items again' }) as HTMLButtonElement).disabled).toBe(false)
  expect(onConfirm).not.toHaveBeenCalled()
})

it('confirms only successfully previewed Skills after a partial failure', async () => {
  const invoke = vi.fn().mockResolvedValueOnce(plan('alpha')).mockRejectedValueOnce(new Error('INVALID_SOURCE'))
  const onConfirm = vi.fn(async () => {})
  render(<ContentReview skills={skills} restore={false} invokeTauri={invoke} onConfirm={onConfirm} onClose={vi.fn()} t={i18n.t} />)
  await screen.findByRole('alert')
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: 'Confirm update' })) })
  expect(onConfirm).toHaveBeenCalledWith({ alpha: 'alpha-update' })
})

it('exposes actual changed lines in one labelled keyboard-accessible comparison', async () => {
  const invoke = vi.fn().mockResolvedValue({ ...plan('alpha'), changes: [{ path: 'SKILL.md', kind: 'modified', before: 'Previous instructions', after: 'Updated instructions' }] })
  render(<ContentReview skills={skills.slice(0, 1)} restore={false} invokeTauri={invoke} onConfirm={vi.fn(async () => {})} onClose={vi.fn()} t={i18n.t} />)
  await screen.findByRole('heading', { name: 'alpha' })
  const comparison = screen.getByRole('region', { name: 'SKILL.md · Line-by-line changes' })
  expect(comparison.textContent).toContain('Previous instructions')
  expect(comparison.textContent).toContain('Updated instructions')
  expect(comparison.tabIndex).toBe(0)
  expect(comparison.closest('details')?.open).toBe(true)
  expect(screen.getByText('− 1 removed')).toBeTruthy()
  expect(screen.getByText('+ 1 added')).toBeTruthy()
  expect(comparison.querySelector('.content-diff-line-removed mark')?.textContent).toBe('Previous')
  expect(comparison.querySelector('.content-diff-line-added mark')?.textContent).toBe('Updated')
})

it.each(['TARGET_MODIFIED', 'TARGET_EXISTS'])('shows a safe conflict location for %s and retains a retry action', async category => {
  const target = '/project/.agents/skills/alpha'
  const invoke = vi.fn().mockRejectedValue(`${category}|${target}`)
  render(<ContentReview skills={skills.slice(0, 1)} restore={false} invokeTauri={invoke} onConfirm={vi.fn(async () => {})} onClose={vi.fn()} t={i18n.t} />)
  expect(await screen.findByText(target)).toBeTruthy()
  expect(screen.getByText(i18n.t('workspace110.reasons.TARGET_CONFLICT'))).toBeTruthy()
  expect((screen.getByRole('button', { name: 'Confirm update' }) as HTMLButtonElement).disabled).toBe(true)
  expect(screen.getByRole('button', { name: 'Preview unfinished items again' })).toBeTruthy()
})

it('shows file permissions and directory entries without presenting them as missing binary files', async () => {
  const invoke = vi.fn().mockResolvedValue({ ...plan('alpha'), changes: [
    { path: 'run.sh', kind: 'modified', before: 'echo ready', after: 'echo ready', beforeMetadata: { kind: 'file', mode: 0o644 }, afterMetadata: { kind: 'file', mode: 0o755 } },
    { path: 'empty/', kind: 'added', before: null, after: null, beforeMetadata: null, afterMetadata: { kind: 'directory' } },
  ] })
  render(<ContentReview skills={skills.slice(0, 1)} restore={false} invokeTauri={invoke} onConfirm={vi.fn(async () => {})} onClose={vi.fn()} t={i18n.t} />)
  expect(await screen.findByText('Permissions 0644')).toBeTruthy()
  expect(screen.getByText('Permissions 0755')).toBeTruthy()
  expect(screen.getByText('Directory entry')).toBeTruthy()
  expect(screen.queryByText(i18n.t('workspace110.unchanged'))).toBeNull()
})
