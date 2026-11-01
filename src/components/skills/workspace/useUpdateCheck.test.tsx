// @vitest-environment jsdom
import { act, cleanup, renderHook } from '@testing-library/react'
import { StrictMode, type ReactNode } from 'react'
import { afterEach, expect, it, vi } from 'vitest'
import type { ManagedSkill } from '../types'
import type { CheckSources, ContentCheckRunResult, ContentCheckSummary } from './contentCheck'
import { useUpdateCheck } from './useUpdateCheck'

afterEach(() => {
  cleanup()
  vi.restoreAllMocks()
})
const skill = (id: string, extra: Partial<ManagedSkill> = {}): ManagedSkill => ({
  id, name: id, source_type: 'local', source_ref: `/source/${id}`, central_path: `/central/${id}`,
  created_at: 1, updated_at: 1, enabled: true, status: 'ok', targets: [], tags: [], ...extra,
})
const summary = (id: string, status: ContentCheckSummary['status'] = 'changed', extra: Partial<ContentCheckSummary> = {}): ContentCheckSummary => ({
  skillId: id, name: id, status, signature: `fingerprint-${id}`, checkedAt: 1000, targetCount: 2, ...extra,
})
const resultFor = (checkId: string, summaries: ContentCheckSummary[], extra: Partial<ContentCheckRunResult> = {}): ContentCheckRunResult => ({
  checkId, summaries, cancelled: false, total: summaries.length, completed: summaries.length, checkedAt: 1000, ...extra,
})
const checkFor = (...summaries: ContentCheckSummary[]) => vi.fn<CheckSources>(async checkId => resultFor(checkId, summaries))
type Invoke = Parameters<typeof useUpdateCheck>[0]['invokeTauri']
const mockInvoke = (implementation: (command: string, args?: Record<string, unknown>) => Promise<unknown>) => {
  const mocked = vi.fn<Invoke>(async <T,>(command: string, args?: Record<string, unknown>) => await implementation(command, args) as T)
  // Vitest preserves the implementation but erases generic call signatures.
  return mocked as typeof mocked & Invoke
}
const idleInvoke = () => mockInvoke(async () => null)
const reload = async () => {}
const deferred = <T,>() => {
  let resolve!: (value: T) => void
  const promise = new Promise<T>(done => { resolve = done })
  return { promise, resolve }
}
const selection = (plans: { skillId: string; id: string }[]) => Object.fromEntries(plans.map(item => [item.skillId, item.id]))

it('only reads cached observations on mount and explicitly checks enabled sources in one lightweight call', async () => {
  const checkSources = checkFor(summary('changed'), summary('same', 'unchanged'))
  const invoke = idleInvoke()
  const { result } = renderHook(() => useUpdateCheck({ skills: [skill('changed'), skill('same'), skill('disabled', { enabled: false })], invokeTauri: invoke, checkSources, onReload: reload }))
  expect(invoke.mock.calls).toEqual([['workspace_content_check_cached']])
  expect(checkSources).not.toHaveBeenCalled()
  expect(result.current.state.plans).toEqual([])
  expect(result.current.state.lastCheck).toBeNull()
  await act(async () => { await result.current.check() })
  expect(checkSources).toHaveBeenCalledTimes(1)
  expect(checkSources.mock.calls[0][1]).toEqual(['changed', 'same'])
  expect(result.current.state.plans.map(item => item.skillId)).toEqual(['changed'])
  expect(result.current.state.plans[0]).not.toHaveProperty('changes')
  expect(result.current.state.checkedCount).toBe(2)
  expect(result.current.state.complete).toBe(true)
  expect(result.current.state.allUpToDate).toBe(false)
  expect(invoke.mock.calls.every(([command]) => command === 'workspace_content_check_cached')).toBe(true)
})

it('publishes a persistent result for immediate checks and distinguishes repeated checks in the same minute', async () => {
  let clock = 100
  vi.spyOn(performance, 'now').mockImplementation(() => clock)
  const finishedAt = new Date('2026-10-26T12:34:01Z').getTime()
  const checkSources = vi.fn<CheckSources>(async checkId => {
    clock += 7
    return resultFor(checkId, [summary('changed'), summary('current', 'unchanged')], { checkedAt: finishedAt })
  })
  const invoke = idleInvoke()
  const { result, rerender } = renderHook(() => useUpdateCheck({ skills: [skill('changed'), skill('current')], invokeTauri: invoke, checkSources, onReload: reload }))
  await act(async () => { await result.current.check() })
  const first = result.current.state.lastCheck
  expect(first).toEqual({
    checkId: checkSources.mock.calls[0][0], finishedAt, elapsedMs: 7,
    total: 2, completed: 2, changed: 1, unchanged: 1, failed: 0, cancelled: false,
  })
  expect(result.current.state.phase).toBe('idle')
  clock += 60_000
  rerender()
  expect(result.current.state.lastCheck).toBe(first)
  checkSources.mockImplementationOnce(async checkId => {
    clock += 5
    return resultFor(checkId, [summary('changed'), summary('current', 'unchanged')], { checkedAt: finishedAt + 1000 })
  })
  await act(async () => { await result.current.check() })
  expect(result.current.state.lastCheck).toEqual({
    ...first, checkId: checkSources.mock.calls[1][0], finishedAt: finishedAt + 1000, elapsedMs: 5,
  })
  expect(result.current.state.lastCheck?.checkId).not.toBe(first?.checkId)
})

it('keeps rows and checked time stable during a new check and atomically publishes its final summaries', async () => {
  const second = deferred<ContentCheckRunResult>()
  const checkSources = checkFor(summary('alpha'), summary('beta', 'unchanged'))
  const invoke = idleInvoke()
  const { result } = renderHook(() => useUpdateCheck({ skills: [skill('alpha'), skill('beta')], invokeTauri: invoke, checkSources, onReload: reload }))
  await act(async () => { await result.current.check() })
  const previous = result.current.state.plans[0]
  const previousOutcome = result.current.state.lastCheck
  checkSources.mockImplementationOnce(() => second.promise)
  let checking!: Promise<void>
  act(() => { checking = result.current.check(); void result.current.check(); void result.current.retryFailures() })
  expect(checkSources).toHaveBeenCalledTimes(2)
  const [checkId, , progress] = checkSources.mock.calls[1]
  act(() => { progress({ checkId, phase: 'checking', total: 2, completed: 1, currentSkillId: 'beta', summary: summary('beta') }) })
  expect(result.current.state.plans).toEqual([previous])
  expect(result.current.state.checkedAt).toBe(1000)
  expect(result.current.state.lastCheck).toBe(previousOutcome)
  expect(result.current.state.processed).toBe(1)
  expect(result.current.state.currentSkill?.id).toBe('beta')
  await act(async () => { second.resolve(resultFor(checkId, [summary('alpha', 'unchanged'), summary('beta')], { checkedAt: 2000 })); await checking })
  expect(result.current.state.plans.map(item => item.skillId)).toEqual(['beta'])
  expect(result.current.state.checkedAt).toBe(2000)
  expect(result.current.state.lastCheck).toMatchObject({ checkId, finishedAt: 2000, completed: 2, changed: 1, unchanged: 1 })
  act(() => { progress({ checkId, phase: 'checking', total: 2, completed: 0, currentSkillId: 'alpha' }) })
  expect(result.current.state.currentSkill).toBeNull()
  expect(result.current.state.processed).toBe(2)
})

it('only claims current content after a successful live check, never from a cached observation', async () => {
  const cached = resultFor('previous', [summary('alpha', 'unchanged')])
  const invoke = vi.fn().mockResolvedValue(cached)
  const checkSources = checkFor(summary('alpha', 'unchanged'))
  const { result } = renderHook(() => useUpdateCheck({ skills: [skill('alpha')], invokeTauri: invoke, checkSources, onReload: reload }))
  await act(async () => {})
  expect(result.current.state.checkedAt).toBe(1000)
  expect(result.current.state.fromCache).toBe(true)
  expect(result.current.state.complete).toBe(false)
  expect(result.current.state.allUpToDate).toBe(false)
  expect(result.current.state.lastCheck).toBeNull()
  await act(async () => { await result.current.check() })
  expect(result.current.state.fromCache).toBe(false)
  expect(result.current.state.allUpToDate).toBe(true)
  expect(result.current.state.lastCheck).toMatchObject({ completed: 1, unchanged: 1, failed: 0, cancelled: false })
})

it('hydrates only after library readiness and nonempty Skills, reusing the read during StrictMode effects', async () => {
  const invoke = vi.fn().mockResolvedValue(resultFor('previous', [summary('alpha')]))
  const checkSources = checkFor(summary('alpha'))
  const wrapper = ({ children }: { children: ReactNode }) => <StrictMode>{children}</StrictMode>
  const { result, rerender } = renderHook(({ skills, ready }) => useUpdateCheck({ skills, ready, invokeTauri: invoke, checkSources, onReload: reload }),
    { initialProps: { skills: [] as ManagedSkill[], ready: false }, wrapper })
  expect(invoke).not.toHaveBeenCalled()
  rerender({ skills: [skill('alpha')], ready: false })
  expect(invoke).not.toHaveBeenCalled()
  rerender({ skills: [skill('alpha')], ready: true })
  await act(async () => {})
  expect(result.current.state.plans[0].checkId).toBe('previous')
  rerender({ skills: [skill('alpha')], ready: true })
  expect(invoke).toHaveBeenCalledTimes(1)
  expect(checkSources).not.toHaveBeenCalled()
})

it('does not let a late cached read overwrite a newer user-initiated check', async () => {
  const cached = deferred<ContentCheckRunResult>()
  const invoke = vi.fn().mockReturnValue(cached.promise)
  const checkSources = checkFor(summary('alpha', 'unchanged'))
  const { result } = renderHook(() => useUpdateCheck({ skills: [skill('alpha')], invokeTauri: invoke, checkSources, onReload: reload }))
  await act(async () => { await result.current.check() })
  await act(async () => { cached.resolve(resultFor('previous', [summary('alpha')])); await cached.promise })
  expect(result.current.state.plans).toEqual([])
  expect(result.current.state.allUpToDate).toBe(true)
  expect(result.current.state.fromCache).toBe(false)
})

it('does not hydrate over a completed check when readiness arrives later', async () => {
  const invoke = idleInvoke()
  const checkSources = checkFor(summary('alpha', 'unchanged'))
  const { result, rerender } = renderHook(({ ready }) => useUpdateCheck({ skills: [skill('alpha')], ready, invokeTauri: invoke, checkSources, onReload: reload }), { initialProps: { ready: false } })
  await act(async () => { await result.current.check() })
  rerender({ ready: true })
  expect(invoke).not.toHaveBeenCalled()
  expect(result.current.state.allUpToDate).toBe(true)
})

it('filters cached responses against the library snapshot captured when loading began', async () => {
  const cached = deferred<ContentCheckRunResult>()
  const invoke = vi.fn().mockReturnValue(cached.promise)
  const checkSources = checkFor(summary('alpha'))
  const original = skill('alpha')
  const { result, rerender } = renderHook(({ skills }) => useUpdateCheck({ skills, invokeTauri: invoke, checkSources, onReload: reload }), { initialProps: { skills: [original] } })
  rerender({ skills: [{ ...original, source_ref: '/new-source' }] })
  await act(async () => { cached.resolve(resultFor('previous', [summary('alpha')])); await cached.promise })
  expect(result.current.state.plans).toEqual([])
  expect(result.current.state.stale).toBe(true)
  expect(result.current.state.allUpToDate).toBe(false)
  expect(result.current.state.lastCheck).toBeNull()
})

it('retries only failed sources while preserving successful summaries and their selection identity', async () => {
  const invoke = idleInvoke()
  const checkSources = checkFor(summary('alpha'), summary('beta', 'failed', { error: 'INVALID_SOURCE' }))
  const { result } = renderHook(() => useUpdateCheck({ skills: [skill('alpha'), skill('beta')], invokeTauri: invoke, checkSources, onReload: reload }))
  await act(async () => { await result.current.check() })
  const originalId = result.current.state.plans[0].id
  expect(result.current.state.failures).toEqual([{ skillId: 'beta', name: 'beta', error: 'INVALID_SOURCE' }])
  expect(result.current.state.allUpToDate).toBe(false)
  expect(result.current.state.lastCheck).toMatchObject({ total: 2, completed: 2, changed: 1, unchanged: 0, failed: 1, cancelled: false })
  checkSources.mockImplementationOnce(async checkId => resultFor(checkId, [summary('beta', 'unchanged')]))
  await act(async () => { await result.current.retryFailures() })
  expect(checkSources.mock.calls.map(([, ids]) => ids)).toEqual([['alpha', 'beta'], ['beta']])
  expect(result.current.state.plans[0].id).toBe(originalId)
  expect(result.current.state.failures).toEqual([])
  expect(result.current.state.checkedCount).toBe(2)
  expect(result.current.state.lastCheck).toMatchObject({ total: 1, completed: 1, changed: 0, unchanged: 1, failed: 0, cancelled: false })
})

it('scopes cancellation to the active batch and retains unvisited previous rows as incomplete', async () => {
  const invoke = idleInvoke()
  const checkSources = checkFor(summary('alpha'), summary('beta'))
  const { result } = renderHook(() => useUpdateCheck({ skills: [skill('alpha'), skill('beta')], invokeTauri: invoke, checkSources, onReload: reload }))
  await act(async () => { await result.current.check() })
  const beta = result.current.state.plans[1]
  const pending = deferred<ContentCheckRunResult>()
  checkSources.mockImplementationOnce(() => pending.promise)
  let checking!: Promise<void>
  act(() => { checking = result.current.check(); result.current.cancel(); result.current.cancel(); void result.current.check() })
  const checkId = checkSources.mock.calls[1][0]
  expect(invoke.mock.calls.filter(([command]) => command === 'workspace_content_check_cancel')).toEqual([['workspace_content_check_cancel', { checkId }]])
  expect(checkSources).toHaveBeenCalledTimes(2)
  expect(result.current.state.phase).toBe('checking')
  await act(async () => { pending.resolve(resultFor(checkId, [summary('alpha', 'unchanged')], { cancelled: true, total: 2, completed: 1 })); await checking })
  expect(result.current.state.plans).toEqual([beta])
  expect(result.current.state.cancelled).toBe(true)
  expect(result.current.state.stale).toBe(false)
  expect(result.current.state.complete).toBe(false)
  expect(result.current.state.allUpToDate).toBe(false)
  expect(result.current.state.lastCheck).toMatchObject({ checkId, total: 2, completed: 1, changed: 0, unchanged: 1, failed: 0, cancelled: true })
})

it('never publishes stale source results or progress from a different run', async () => {
  const pending = deferred<ContentCheckRunResult>()
  const invoke = idleInvoke()
  const checkSources = vi.fn<CheckSources>(() => pending.promise)
  const original = skill('alpha')
  const { result, rerender } = renderHook(({ skills }) => useUpdateCheck({ skills, invokeTauri: invoke, checkSources, onReload: reload }), { initialProps: { skills: [original] } })
  let checking!: Promise<void>
  act(() => { checking = result.current.check() })
  const [checkId, , progress] = checkSources.mock.calls[0]
  act(() => { progress({ checkId: 'different-run', phase: 'checking', completed: 1, total: 1, currentSkillId: 'alpha' }) })
  expect(result.current.state.processed).toBe(0)
  rerender({ skills: [{ ...original, source_ref: '/new-source' }] })
  await act(async () => { pending.resolve(resultFor(checkId, [summary('alpha')])); await checking })
  expect(result.current.state.plans).toEqual([])
  expect(result.current.state.stale).toBe(true)
  expect(result.current.state.lastCheck).toMatchObject({ total: 1, completed: 1, changed: 0, unchanged: 0, failed: 1 })
})

it.each(['source_revision', 'source_subpath', 'content_hash'])('invalidates summaries and confirmation when %s changes without an updated_at change', async field => {
  const original = { ...skill('alpha'), [field]: 'old-value' }
  const invoke = idleInvoke()
  const checkSources = checkFor(summary('alpha'))
  const { result, rerender } = renderHook(({ skills }) => useUpdateCheck({ skills, invokeTauri: invoke, checkSources, onReload: reload }), { initialProps: { skills: [original] } })
  await act(async () => { await result.current.check() })
  const expected = selection(result.current.state.plans)
  const changed = { ...original, [field]: 'new-value' }
  expect(changed.updated_at).toBe(original.updated_at)
  rerender({ skills: [changed] })
  expect(result.current.state.plans).toEqual([])
  expect(result.current.state.stale).toBe(true)
  await act(async () => { await result.current.apply({ alpha: 'actual-plan-alpha' }, expected) })
  expect(invoke.mock.calls.some(([command]) => command === 'workspace_content_execute')).toBe(false)
  expect(result.current.state.lastApply?.failed).toEqual([{ skillId: 'alpha', name: 'alpha', error: 'PLAN_STALE' }])
})

it('does not keep current claims after an added Skill or transport scope change', async () => {
  const original = [skill('alpha')]
  const invoke = idleInvoke()
  const checkSources = checkFor(summary('alpha', 'unchanged'))
  const { result, rerender } = renderHook(({ skills, invokeTauri }) => useUpdateCheck({ skills, invokeTauri, checkSources, onReload: reload }), { initialProps: { skills: original, invokeTauri: invoke } })
  await act(async () => { await result.current.check() })
  expect(result.current.state.allUpToDate).toBe(true)
  rerender({ skills: [...original, skill('beta')], invokeTauri: invoke })
  expect(result.current.state.allUpToDate).toBe(false)
  rerender({ skills: original, invokeTauri: idleInvoke() })
  expect(result.current.state.stale).toBe(true)
})

it('cancels an active batch on unmount and ignores its subsequent progress and completion', async () => {
  const pending = deferred<ContentCheckRunResult>()
  const invoke = idleInvoke()
  const checkSources = vi.fn<CheckSources>(() => pending.promise)
  const { result, unmount } = renderHook(() => useUpdateCheck({ skills: [skill('alpha')], invokeTauri: invoke, checkSources, onReload: reload }))
  let checking!: Promise<void>
  act(() => { checking = result.current.check() })
  const [checkId, , progress] = checkSources.mock.calls[0]
  unmount()
  expect(invoke).toHaveBeenLastCalledWith('workspace_content_check_cancel', { checkId })
  await act(async () => { progress({ checkId, phase: 'completed', total: 1, completed: 1 }); pending.resolve(resultFor(checkId, [summary('alpha')])); await checking })
  expect(checkSources).toHaveBeenCalledTimes(1)
})

it('applies the preview plan only against its selected summary and preserves partial failures and pending targets', async () => {
  const invoke = mockInvoke(async (command, args) => {
    if (command === 'workspace_content_execute') {
      if (args?.id === 'actual-beta') throw new Error('PLAN_STALE')
      return { changed: true, pending_targets: ['/tool/alpha'] }
    }
    return null
  })
  const onReload = vi.fn(reload)
  const checkSources = checkFor(summary('alpha'), summary('beta'))
  const { result } = renderHook(() => useUpdateCheck({ skills: [skill('alpha'), skill('beta')], invokeTauri: invoke, checkSources, onReload }))
  await act(async () => { await result.current.check() })
  const expected = selection(result.current.state.plans)
  await act(async () => { await result.current.apply({ alpha: 'actual-alpha', beta: 'actual-beta' }, expected) })
  expect(invoke.mock.calls.filter(([command]) => command === 'workspace_content_execute')).toEqual([
    ['workspace_content_execute', { id: 'actual-alpha' }], ['workspace_content_execute', { id: 'actual-beta' }],
  ])
  expect(result.current.state.plans).toEqual([])
  expect(result.current.state.failures).toEqual([{ skillId: 'beta', name: 'beta', error: 'PLAN_STALE' }])
  expect(result.current.state.lastApply).toEqual({ succeeded: [{ skillId: 'alpha', name: 'alpha', pendingTargets: ['/tool/alpha'] }], failed: [{ skillId: 'beta', name: 'beta', error: 'PLAN_STALE' }], cancelled: false })
  expect(onReload).toHaveBeenCalledOnce()
  expect(result.current.state.allUpToDate).toBe(false)
})

it('rejects confirmation from an earlier check even when its content signature is unchanged, without removing the new summary', async () => {
  const invoke = idleInvoke()
  const checkSources = checkFor(summary('alpha'))
  const { result } = renderHook(() => useUpdateCheck({ skills: [skill('alpha')], invokeTauri: invoke, checkSources, onReload: reload }))
  await act(async () => { await result.current.check() })
  const old = selection(result.current.state.plans)
  await act(async () => { await result.current.check() })
  const current = result.current.state.plans[0]
  expect(current.id).not.toBe(old.alpha)
  await act(async () => { await result.current.apply({ alpha: 'actual-alpha' }, old) })
  expect(result.current.state.lastApply?.failed[0].error).toBe('PLAN_STALE')
  expect(result.current.state.plans).toEqual([current])
  expect(invoke.mock.calls.some(([command]) => command === 'workspace_content_execute')).toBe(false)
})

it('blocks checks while applying and only cancels the remaining execution without cancelling unrelated batches', async () => {
  const executing = deferred<{ changed: boolean }>()
  const invoke = mockInvoke(command => command === 'workspace_content_execute' ? executing.promise : Promise.resolve(null))
  const checkSources = checkFor(summary('alpha'), summary('beta'))
  const { result } = renderHook(() => useUpdateCheck({ skills: [skill('alpha'), skill('beta')], invokeTauri: invoke, checkSources, onReload: reload }))
  await act(async () => { await result.current.check() })
  const expected = selection(result.current.state.plans)
  let applying!: ReturnType<typeof result.current.apply>
  act(() => { applying = result.current.apply({ alpha: 'actual-alpha', beta: 'actual-beta' }, expected); void result.current.check(); result.current.cancel() })
  expect(checkSources).toHaveBeenCalledTimes(1)
  expect(invoke.mock.calls.some(([command]) => command === 'workspace_content_check_cancel')).toBe(false)
  await act(async () => { executing.resolve({ changed: true }); await applying })
  expect(invoke.mock.calls.filter(([command]) => command === 'workspace_content_execute')).toHaveLength(1)
  expect(result.current.state.lastApply?.cancelled).toBe(true)
  expect(result.current.state.plans.map(item => item.skillId)).toEqual(['beta'])
  expect(result.current.state.allUpToDate).toBe(false)
})

it('records a refresh failure separately from a successful execution and does not claim current content', async () => {
  const invoke = mockInvoke(async command => command === 'workspace_content_execute' ? { changed: true } : null)
  const checkSources = checkFor(summary('alpha'))
  const onReload = vi.fn(async () => { throw new Error('reload unavailable') })
  const { result } = renderHook(() => useUpdateCheck({ skills: [skill('alpha')], invokeTauri: invoke, checkSources, onReload }))
  await act(async () => { await result.current.check() })
  await act(async () => { await result.current.apply({ alpha: 'actual-alpha' }, selection(result.current.state.plans)) })
  expect(result.current.state.lastApply?.succeeded).toHaveLength(1)
  expect(result.current.state.lastApply?.failed).toEqual([])
  expect(result.current.state.lastApply?.reloadError).toBe('reload unavailable')
  expect(result.current.state.allUpToDate).toBe(false)
})

it('reports whole-check transport failures and missing summaries as failures instead of current content', async () => {
  const invoke = idleInvoke()
  const checkSources = checkFor(summary('alpha', 'unchanged'))
  checkSources.mockRejectedValueOnce(new Error('CHECK_FAILED'))
  const { result } = renderHook(() => useUpdateCheck({ skills: [skill('alpha'), skill('beta')], invokeTauri: invoke, checkSources, onReload: reload }))
  await act(async () => { await result.current.check() })
  expect(result.current.state.failures.map(item => item.skillId)).toEqual(['alpha', 'beta'])
  expect(result.current.state.allUpToDate).toBe(false)
  expect(result.current.state.lastCheck).toMatchObject({ total: 2, completed: 2, changed: 0, unchanged: 0, failed: 2, cancelled: false })
  await act(async () => { await result.current.check() })
  expect(result.current.state.failures).toEqual([{ skillId: 'beta', name: 'beta', error: 'CHECK_FAILED' }])
  expect(result.current.state.allUpToDate).toBe(false)
  expect(result.current.state.lastCheck).toMatchObject({ total: 2, completed: 2, changed: 0, unchanged: 1, failed: 1, cancelled: false })
})

it('ignores nonmatching run results and never requests checks for an empty enabled library', async () => {
  const invoke = idleInvoke()
  const checkSources = vi.fn<CheckSources>(async () => resultFor('wrong-run', [summary('alpha', 'unchanged')]))
  const { result, rerender } = renderHook(({ skills }) => useUpdateCheck({ skills, invokeTauri: invoke, checkSources, onReload: reload }), { initialProps: { skills: [skill('alpha')] } })
  await act(async () => { await result.current.check() })
  expect(result.current.state.failures[0].error).toBe('CHECK_FAILED')
  rerender({ skills: [] })
  await act(async () => { await result.current.check() })
  expect(checkSources).toHaveBeenCalledTimes(1)
  expect(result.current.state.allUpToDate).toBe(false)
})

it('directly updates only the selected confirmed change through preview and execute, once per Skill', async () => {
  const invoke = mockInvoke(async (command, args) => {
    if (command === 'workspace_content_preview') return { id: `prepared-${args?.skillId}`, skillId: args?.skillId, restore: false }
    if (command === 'workspace_content_execute') return { changed: true, pending_targets: ['/copy/beta'] }
    return null
  })
  const onReload = vi.fn(reload)
  const checkSources = checkFor(summary('alpha'), summary('beta'), summary('current', 'unchanged'), summary('failed', 'failed', { error: 'INVALID_SOURCE' }))
  const { result } = renderHook(() => useUpdateCheck({ skills: [skill('alpha'), skill('beta'), skill('current'), skill('failed'), skill('disabled', { enabled: false })], invokeTauri: invoke, checkSources, onReload }))
  await act(async () => { await result.current.check() })
  const chosen = result.current.state.plans.find(item => item.skillId === 'beta')!
  await act(async () => { await result.current.update([chosen, chosen]) })
  expect(invoke.mock.calls).toEqual([
    ['workspace_content_check_cached'],
    ['workspace_content_preview', { skillId: 'beta', restore: false }],
    ['workspace_content_execute', { id: 'prepared-beta' }],
  ])
  expect(result.current.state.lastApply).toEqual({ succeeded: [{ skillId: 'beta', name: 'beta', pendingTargets: ['/copy/beta'] }], failed: [], cancelled: false })
  expect(result.current.state.plans.map(item => item.skillId)).toEqual(['alpha'])
  expect(result.current.state.failures).toEqual([{ skillId: 'failed', name: 'failed', error: 'INVALID_SOURCE' }])
  expect(onReload).toHaveBeenCalledOnce()
})

it.each(['unchecked', 'cached', 'cancelled', 'stale'] as const)('rejects a direct action on %s observations before any preview or write', async mode => {
  const invoke = mockInvoke(async command => mode === 'cached' && command === 'workspace_content_check_cached' ? resultFor('history', [summary('alpha')]) : null)
  const checkSources = checkFor(summary('alpha'))
  if (mode === 'cancelled') checkSources.mockImplementation(async id => resultFor(id, [summary('alpha')], { cancelled: true }))
  const { result, rerender } = renderHook(({ skills }) => useUpdateCheck({ skills, invokeTauri: invoke, checkSources, onReload: reload }), { initialProps: { skills: [skill('alpha')] } })
  await act(async () => {})
  if (mode !== 'unchecked' && mode !== 'cached') await act(async () => { await result.current.check() })
  const chosen = result.current.state.plans[0] ?? { ...summary('alpha'), id: 'not-checked', checkId: 'not-checked' }
  if (mode === 'stale') rerender({ skills: [skill('alpha'), skill('newly-added')] })
  await act(async () => { await result.current.update([chosen]); await result.current.update([chosen]) })
  expect(invoke.mock.calls.every(([command]) => command === 'workspace_content_check_cached')).toBe(true)
  expect(result.current.state.lastApply?.succeeded).toEqual([])
  expect(result.current.state.lastApply?.failed).toEqual([{ skillId: 'alpha', name: 'alpha', error: 'PLAN_STALE' }])
})

it('continues with other confirmed updates after preview failure and records the failure without executing its plan', async () => {
  const invoke = mockInvoke(async (command, args) => {
    if (command === 'workspace_content_preview') {
      if (args?.skillId === 'alpha') throw new Error('TARGET_MODIFIED|/copy/alpha')
      return { id: 'prepared-beta', skillId: 'beta', restore: false }
    }
    if (command === 'workspace_content_execute') return { changed: true }
    return null
  })
  const onReload = vi.fn(reload)
  const checkSources = checkFor(summary('alpha'), summary('beta'))
  const { result } = renderHook(() => useUpdateCheck({ skills: [skill('alpha'), skill('beta')], invokeTauri: invoke, checkSources, onReload }))
  await act(async () => { await result.current.check() })
  await act(async () => { await result.current.update(result.current.state.plans) })
  expect(invoke.mock.calls.filter(([command]) => command !== 'workspace_content_check_cached')).toEqual([
    ['workspace_content_preview', { skillId: 'alpha', restore: false }],
    ['workspace_content_preview', { skillId: 'beta', restore: false }],
    ['workspace_content_execute', { id: 'prepared-beta' }],
  ])
  expect(result.current.state.lastApply).toEqual({ succeeded: [{ skillId: 'beta', name: 'beta', pendingTargets: [] }], failed: [{ skillId: 'alpha', name: 'alpha', error: 'TARGET_MODIFIED|/copy/alpha' }], cancelled: false })
  expect(result.current.state.failures).toEqual([{ skillId: 'alpha', name: 'alpha', error: 'TARGET_MODIFIED|/copy/alpha' }])
  expect(onReload).toHaveBeenCalledOnce()
})

it('holds one apply lock across preview and execution, and cancellation during preview prevents every write', async () => {
  const preview = deferred<{ id: string; skillId: string; restore: boolean }>()
  const invoke = mockInvoke(command => command === 'workspace_content_preview' ? preview.promise : Promise.resolve(null))
  const onReload = vi.fn(reload)
  const checkSources = checkFor(summary('alpha'), summary('beta'))
  const { result } = renderHook(() => useUpdateCheck({ skills: [skill('alpha'), skill('beta')], invokeTauri: invoke, checkSources, onReload }))
  await act(async () => { await result.current.check() })
  const chosen = result.current.state.plans
  let updating!: ReturnType<typeof result.current.update>
  act(() => {
    updating = result.current.update(chosen)
    void result.current.update(chosen)
    void result.current.apply({ alpha: 'unrelated-plan' }, selection(chosen))
    void result.current.check()
    result.current.cancel()
  })
  expect(result.current.state.phase).toBe('applying')
  expect(checkSources).toHaveBeenCalledTimes(1)
  await act(async () => { preview.resolve({ id: 'prepared-alpha', skillId: 'alpha', restore: false }); await updating })
  expect(invoke.mock.calls.filter(([command]) => command !== 'workspace_content_check_cached')).toEqual([
    ['workspace_content_preview', { skillId: 'alpha', restore: false }],
  ])
  expect(result.current.state.lastApply).toEqual({ succeeded: [], failed: [], cancelled: true })
  expect(result.current.state.plans).toEqual(chosen)
  expect(result.current.state.phase).toBe('idle')
  expect(onReload).not.toHaveBeenCalled()
})

it.each(['metadata', 'transport'] as const)('revalidates %s after preparing a direct update and never executes a stale plan', async changed => {
  const preview = deferred<{ id: string; skillId: string; restore: boolean }>()
  const invoke = mockInvoke(command => command === 'workspace_content_preview' ? preview.promise : Promise.resolve(null))
  const replacement = idleInvoke()
  const original = skill('alpha')
  const checkSources = checkFor(summary('alpha'))
  const { result, rerender } = renderHook(({ skills, invokeTauri }) => useUpdateCheck({ skills, invokeTauri, checkSources, onReload: reload }), { initialProps: { skills: [original], invokeTauri: invoke } })
  await act(async () => { await result.current.check() })
  let updating!: ReturnType<typeof result.current.update>
  act(() => { updating = result.current.update(result.current.state.plans) })
  rerender({ skills: [changed === 'metadata' ? { ...original, source_revision: 'new-revision' } : original], invokeTauri: changed === 'transport' ? replacement : invoke })
  await act(async () => { preview.resolve({ id: 'prepared-alpha', skillId: 'alpha', restore: false }); await updating })
  expect(invoke.mock.calls.some(([command]) => command === 'workspace_content_execute')).toBe(false)
  expect(replacement).not.toHaveBeenCalled()
  expect(result.current.state.lastApply?.succeeded).toEqual([])
  expect(result.current.state.lastApply?.failed).toEqual([{ skillId: 'alpha', name: 'alpha', error: 'PLAN_STALE' }])
})

it('never executes a direct preview that completes after unmount', async () => {
  const preview = deferred<{ id: string; skillId: string; restore: boolean }>()
  const invoke = mockInvoke(command => command === 'workspace_content_preview' ? preview.promise : Promise.resolve(null))
  const onReload = vi.fn(reload)
  const checkSources = checkFor(summary('alpha'))
  const { result, unmount } = renderHook(() => useUpdateCheck({ skills: [skill('alpha')], invokeTauri: invoke, checkSources, onReload }))
  await act(async () => { await result.current.check() })
  let updating!: ReturnType<typeof result.current.update>
  act(() => { updating = result.current.update(result.current.state.plans) })
  unmount()
  await act(async () => { preview.resolve({ id: 'prepared-alpha', skillId: 'alpha', restore: false }); expect(await updating).toEqual({ succeeded: [], failed: [], cancelled: true }) })
  expect(invoke.mock.calls.some(([command]) => command === 'workspace_content_execute')).toBe(false)
  expect(onReload).not.toHaveBeenCalled()
})

it('retains a failed direct write after cancellation, does not retry it, and leaves the remaining item untouched', async () => {
  let reject!: (error: Error) => void
  const execute = new Promise<never>((_, fail) => { reject = fail })
  const invoke = mockInvoke(async (command, args) => {
    if (command === 'workspace_content_preview') return { id: `prepared-${args?.skillId}`, skillId: args?.skillId, restore: false }
    if (command === 'workspace_content_execute') return execute
    return null
  })
  const onReload = vi.fn(reload)
  const checkSources = checkFor(summary('alpha'), summary('beta'))
  const { result } = renderHook(() => useUpdateCheck({ skills: [skill('alpha'), skill('beta')], invokeTauri: invoke, checkSources, onReload }))
  await act(async () => { await result.current.check() })
  let updating!: ReturnType<typeof result.current.update>
  await act(async () => { updating = result.current.update(result.current.state.plans); await Promise.resolve() })
  expect(invoke.mock.calls.filter(([command]) => command === 'workspace_content_execute')).toHaveLength(1)
  act(() => { result.current.cancel() })
  await act(async () => { reject(new Error('transport unavailable')); await updating })
  expect(result.current.state.lastApply).toEqual({ succeeded: [], failed: [{ skillId: 'alpha', name: 'alpha', error: 'transport unavailable' }], cancelled: true })
  expect(result.current.state.plans.map(item => item.skillId)).toEqual(['beta'])
  expect(invoke.mock.calls.filter(([command]) => command === 'workspace_content_execute')).toHaveLength(1)
  expect(onReload).not.toHaveBeenCalled()
})

it('reports refresh failure independently from a successful direct write and blocks another unrefreshed direct update', async () => {
  const invoke = mockInvoke(async (command, args) => {
    if (command === 'workspace_content_preview') return { id: `prepared-${args?.skillId}`, skillId: args?.skillId, restore: false }
    return command === 'workspace_content_execute' ? { changed: true } : null
  })
  const checkSources = checkFor(summary('alpha'), summary('beta'))
  const onReload = vi.fn(async () => { throw new Error('refresh unavailable') })
  const { result } = renderHook(() => useUpdateCheck({ skills: [skill('alpha'), skill('beta')], invokeTauri: invoke, checkSources, onReload }))
  await act(async () => { await result.current.check() })
  await act(async () => { await result.current.update([result.current.state.plans[0]]) })
  expect(result.current.state.lastApply).toEqual({ succeeded: [{ skillId: 'alpha', name: 'alpha', pendingTargets: [] }], failed: [], cancelled: false, reloadError: 'refresh unavailable' })
  expect(result.current.state.allUpToDate).toBe(false)
  await act(async () => { await result.current.update(result.current.state.plans); await result.current.update(result.current.state.plans) })
  expect(invoke.mock.calls.filter(([command]) => command === 'workspace_content_execute')).toHaveLength(1)
  expect(result.current.state.lastApply?.reloadError).toBe('refresh unavailable')
  expect(result.current.state.lastApply?.failed).toEqual([{ skillId: 'beta', name: 'beta', error: 'PLAN_STALE' }])
})


it.each([
  { id: 'wrong-skill', skillId: 'beta', restore: false },
  { id: 'restore-plan', skillId: 'alpha', restore: true },
  { id: '', skillId: 'alpha', restore: false },
])('rejects a mismatched direct preview before execution: %j', async preview => {
  const invoke = mockInvoke(async command => command === 'workspace_content_preview' ? preview : null)
  const checkSources = checkFor(summary('alpha'))
  const { result } = renderHook(() => useUpdateCheck({ skills: [skill('alpha')], invokeTauri: invoke, checkSources, onReload: reload }))
  await act(async () => { await result.current.check() })
  const chosen = result.current.state.plans[0]
  await act(async () => { await result.current.update([chosen]) })
  expect(invoke.mock.calls.some(([command]) => command === 'workspace_content_execute')).toBe(false)
  expect(result.current.state.plans).toEqual([chosen])
  expect(result.current.state.lastApply?.failed).toEqual([{ skillId: 'alpha', name: 'alpha', error: 'PLAN_STALE' }])
})
