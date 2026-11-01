import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react'
import type { ManagedSkill } from '../types'
import type { CheckSources, ContentCheckRunResult, ContentCheckSummary, UpdateCandidate } from './contentCheck'

type Invoke = <T>(command: string, args?: Record<string, unknown>) => Promise<T>
type SkillIdentity = Pick<ManagedSkill, 'id' | 'name'>
export type UpdateCheckFailure = { skillId: string; name: string; error: string }
export type UpdateCheckOutcome = {
  checkId: string
  finishedAt: number
  elapsedMs: number
  total: number
  completed: number
  changed: number
  unchanged: number
  failed: number
  cancelled: boolean
}
export type UpdateApplyResult = {
  succeeded: { skillId: string; name: string; pendingTargets: string[] }[]
  failed: UpdateCheckFailure[]
  cancelled: boolean
  reloadError?: string
}
type CheckedSkill = {
  invoke: Invoke
  checkSources: CheckSources
  localSignature: string
  candidate: UpdateCandidate
}
type InternalState = {
  phase: 'idle' | 'checking' | 'applying'
  records: Record<string, CheckedSkill>
  currentSkill: SkillIdentity | null
  processed: number
  total: number
  checkedAt: number | null
  cancelled: boolean
  fromCache: boolean
  lastApply: UpdateApplyResult | null
  lastCheck: UpdateCheckOutcome | null
}
export type UpdateCheckState = Omit<InternalState, 'records'> & {
  plans: UpdateCandidate[]
  failures: UpdateCheckFailure[]
  eligibleCount: number
  checkedCount: number
  stale: boolean
  complete: boolean
  allUpToDate: boolean
}
type Options = {
  skills: ManagedSkill[]
  invokeTauri: Invoke
  checkSources: CheckSources
  onReload: () => Promise<unknown>
  ready?: boolean
}
type Run = { kind: 'checking' | 'applying'; checkId: string; cancelled: boolean; invoke: Invoke }
type ApplyRequest = { skillId: string; expectedCheckId: string; planId?: string }
type CacheWork = {
  invoke: Invoke
  checkSources: CheckSources
  signatures: Map<string, string>
  revision: number
  consumed: boolean
  promise: Promise<ContentCheckRunResult | null>
}

function signature(skill: ManagedSkill) {
  // Backend signatures are opaque. Capture the local library snapshot as well
  // so a response cannot revive results after source/deployment metadata changes.
  return JSON.stringify([skill.id, skill.name, skill.enabled, skill.source_type,
    skill.source_ref, skill.source_revision, skill.source_subpath, skill.content_hash,
    skill.central_path, skill.updated_at, skill.status, skill.targets])
}
function errorText(error: unknown) {
  return error instanceof Error ? error.message : String(error)
}
function candidate(summary: ContentCheckSummary, checkId: string): UpdateCandidate {
  return { ...summary, checkId, id: JSON.stringify([checkId, summary.skillId, summary.checkedAt, summary.signature]) }
}
function stopRun(run: Run) {
  if (run.cancelled) return
  run.cancelled = true
  if (run.kind === 'checking') {
    // Cancellation is scoped to this run. The final response remains the source
    // of truth; a transport failure cannot make us claim that all work completed.
    void run.invoke('workspace_content_check_cancel', { checkId: run.checkId }).catch(() => {})
  }
}
const initialState: InternalState = {
  phase: 'idle', records: {}, currentSkill: null, processed: 0, total: 0,
  checkedAt: null, cancelled: false, fromCache: false, lastApply: null, lastCheck: null,
}

/** Lightweight source checks; full plans are prepared only for review or execution. */
export function useUpdateCheck({ skills, invokeTauri, checkSources, onReload, ready = true }: Options) {
  const [raw, setRaw] = useState<InternalState>(initialState)
  const rawRef = useRef(raw)
  const mounted = useRef(true)
  const runRef = useRef<Run | null>(null)
  const revision = useRef(0)
  const cacheWork = useRef<CacheWork | null>(null)
  const eligible = useMemo(() => skills.filter(skill => skill.enabled), [skills])
  const signatures = useMemo(() => new Map(eligible.map(skill => [skill.id, signature(skill)])), [eligible])
  const latest = useRef({ eligible, signatures, invokeTauri, checkSources, onReload })
  useLayoutEffect(() => { latest.current = { eligible, signatures, invokeTauri, checkSources, onReload } }, [eligible, signatures, invokeTauri, checkSources, onReload])
  useEffect(() => {
    mounted.current = true
    return () => {
      mounted.current = false
      if (runRef.current) stopRun(runRef.current)
    }
  }, [])
  const publish = useCallback((update: (state: InternalState) => InternalState) => {
    if (!mounted.current) return
    const next = update(rawRef.current)
    rawRef.current = next
    setRaw(next)
  }, [])

  useEffect(() => {
    if (!ready || !skills.length || revision.current > 0) return
    const snapshot = latest.current
    if (!cacheWork.current || cacheWork.current.invoke !== invokeTauri) {
      cacheWork.current = {
        invoke: invokeTauri, checkSources: snapshot.checkSources, signatures: new Map(snapshot.signatures),
        revision: revision.current, consumed: false,
        promise: invokeTauri<ContentCheckRunResult | null>('workspace_content_check_cached'),
      }
    }
    const work = cacheWork.current
    if (work.consumed) return
    let active = true
    void work.promise.then(result => {
      if (!active || !mounted.current || work.consumed) return
      work.consumed = true
      if (!result || runRef.current || revision.current !== work.revision || latest.current.invokeTauri !== work.invoke || latest.current.checkSources !== work.checkSources) return
      const records: InternalState['records'] = {}
      for (const summary of result.summaries) {
        const localSignature = work.signatures.get(summary.skillId)
        if (localSignature === undefined || latest.current.signatures.get(summary.skillId) !== localSignature) continue
        records[summary.skillId] = { invoke: work.invoke, checkSources: work.checkSources, localSignature, candidate: candidate(summary, result.checkId) }
      }
      publish(state => ({ ...state, records, checkedAt: result.checkedAt, processed: result.completed,
        total: result.total, cancelled: result.cancelled, fromCache: true }))
    }).catch(() => { if (active) work.consumed = true })
    return () => { active = false }
  }, [ready, skills.length, invokeTauri, publish])

  const scan = useCallback(async (retryOnly: boolean) => {
    if (runRef.current || !mounted.current) return
    const snapshot = latest.current
    const previous = rawRef.current
    const candidates = snapshot.eligible.filter(skill => !retryOnly ||
      (previous.records[skill.id]?.localSignature === snapshot.signatures.get(skill.id) && previous.records[skill.id]?.candidate.status === 'failed' &&
        previous.records[skill.id]?.invoke === snapshot.invokeTauri && previous.records[skill.id]?.checkSources === snapshot.checkSources))
    if (!candidates.length) return
    const startedAt = performance.now()
    const checkId = crypto.randomUUID()
    const run: Run = { kind: 'checking', checkId, cancelled: false, invoke: snapshot.invokeTauri }
    runRef.current = run
    revision.current += 1
    // Keep the last complete list and timestamp while progress changes. The new
    // summaries are published together, rather than removing/reordering rows.
    publish(state => ({ ...state, phase: 'checking', currentSkill: null, processed: 0, total: candidates.length, cancelled: false }))
    let result: ContentCheckRunResult
    try {
      result = await snapshot.checkSources(checkId, candidates.map(skill => skill.id), progress => {
        if (!mounted.current || runRef.current !== run || progress.checkId !== checkId ||
          latest.current.invokeTauri !== snapshot.invokeTauri || latest.current.checkSources !== snapshot.checkSources) return
        const current = candidates.find(skill => skill.id === progress.currentSkillId)
        publish(state => ({ ...state, processed: Math.max(state.processed, Math.min(candidates.length, progress.completed)),
          currentSkill: current ? { id: current.id, name: current.name } : null }))
      })
      if (result.checkId !== checkId) throw new Error('CHECK_FAILED')
    } catch (error) {
      result = { checkId, checkedAt: Date.now(), cancelled: run.cancelled, total: candidates.length, completed: candidates.length,
        summaries: candidates.map(skill => ({ skillId: skill.id, name: skill.name, signature: '', checkedAt: Date.now(), status: 'failed', targetCount: skill.targets.length, error: errorText(error) })) }
    }
    try {
      if (!mounted.current) return
      const cancelled = run.cancelled || result.cancelled
      const records: InternalState['records'] = retryOnly || cancelled ? { ...previous.records } : {}
      const summaries = new Map(result.summaries.map(summary => [summary.skillId, summary]))
      const lastCheck: UpdateCheckOutcome = {
        checkId, finishedAt: result.checkedAt, elapsedMs: Math.max(0, performance.now() - startedAt),
        total: candidates.length, completed: 0, changed: 0, unchanged: 0, failed: 0, cancelled,
      }
      for (const skill of candidates) {
        const localSignature = snapshot.signatures.get(skill.id)!
        const stillCurrent = latest.current.signatures.get(skill.id) === localSignature &&
          latest.current.invokeTauri === snapshot.invokeTauri && latest.current.checkSources === snapshot.checkSources
        if (!stillCurrent) {
          lastCheck.failed += 1
          lastCheck.completed += 1
          continue
        }
        const summary = summaries.get(skill.id)
        if (!summary && cancelled) continue
        const completed = summary ?? { skillId: skill.id, name: skill.name, signature: '', checkedAt: result.checkedAt,
          status: 'failed' as const, targetCount: skill.targets.length, error: 'CHECK_FAILED' }
        lastCheck[completed.status] += 1
        lastCheck.completed += 1
        records[skill.id] = { invoke: snapshot.invokeTauri, checkSources: snapshot.checkSources, localSignature, candidate: candidate(completed, checkId) }
      }
      publish(state => ({ ...state, records, phase: 'idle', currentSkill: null, processed: result.completed,
        total: result.total, checkedAt: result.checkedAt, cancelled: cancelled || (retryOnly && previous.cancelled),
        fromCache: retryOnly && previous.fromCache, lastApply: null, lastCheck }))
    } finally {
      if (runRef.current === run) runRef.current = null
    }
  }, [publish])
  const check = useCallback(() => scan(false), [scan])
  const retryFailures = useCallback(() => scan(true), [scan])
  const cancel = useCallback(() => {
    if (runRef.current) {
      stopRun(runRef.current)
      publish(state => ({ ...state, cancelled: true }))
    }
  }, [publish])

  const applyRequests = useCallback(async (requested: ApplyRequest[], prepare: boolean): Promise<UpdateApplyResult | null> => {
    if (runRef.current || !mounted.current || !requested.length) return null
    const snapshot = latest.current
    const before = rawRef.current
    const isCurrent = (record: CheckedSkill | undefined, skillId: string) =>
      record?.invoke === snapshot.invokeTauri && record.checkSources === snapshot.checkSources &&
      record.localSignature === latest.current.signatures.get(skillId) &&
      latest.current.invokeTauri === snapshot.invokeTauri && latest.current.checkSources === snapshot.checkSources
    // A direct action may only use a completed live check. A user-reviewed plan
    // keeps its existing per-item confirmation and backend fingerprint guards.
    const fresh = !prepare || (before.checkedAt !== null && !before.fromCache && !before.cancelled &&
      !before.lastApply?.reloadError && snapshot.eligible.every(skill => isCurrent(before.records[skill.id], skill.id)))
    if (!fresh) {
      const result: UpdateApplyResult = {
        succeeded: [], failed: requested.map(({ skillId }) => ({ skillId,
          name: before.records[skillId]?.candidate.name ?? skillId, error: 'PLAN_STALE' })),
        cancelled: before.cancelled,
        ...(before.lastApply?.reloadError ? { reloadError: before.lastApply.reloadError } : {}),
      }
      // Rejecting an action must not clear the invalidating condition. In
      // particular, repeated clicks cannot revive cancelled or unrefreshed data.
      publish(state => ({ ...state, lastApply: result }))
      return result
    }
    const run: Run = { kind: 'applying', checkId: '', cancelled: false, invoke: snapshot.invokeTauri }
    const result: UpdateApplyResult = { succeeded: [], failed: [], cancelled: false }
    runRef.current = run
    revision.current += 1
    publish(state => ({ ...state, phase: 'applying', currentSkill: null, processed: 0,
      total: requested.length, cancelled: false, lastApply: null }))
    try {
      for (const { skillId, expectedCheckId, planId: preparedId } of requested) {
        if (run.cancelled || !mounted.current) break
        const record = rawRef.current.records[skillId]
        const name = record?.candidate.name ?? snapshot.eligible.find(skill => skill.id === skillId)?.name ?? skillId
        publish(state => ({ ...state, currentSkill: { id: skillId, name } }))
        const valid = () => fresh && isCurrent(record, skillId) && record?.candidate.status === 'changed' &&
          record.candidate.id === expectedCheckId && rawRef.current.records[skillId] === record
        if (!valid()) {
          result.failed.push({ skillId, name, error: 'PLAN_STALE' })
        } else {
          let executing = false
          try {
            let planId = preparedId
            if (prepare) {
              const plan = await snapshot.invokeTauri<{ id: string; skillId: string; restore: boolean }>(
                'workspace_content_preview', { skillId, restore: false })
              // A preview is read-only with respect to installed content. If
              // cancelled while it was loading, never send the subsequent write.
              if (run.cancelled || !mounted.current) break
              if (!valid() || plan.skillId !== skillId || plan.restore || !plan.id) {
                result.failed.push({ skillId, name, error: 'PLAN_STALE' })
                publish(state => ({ ...state, processed: state.processed + 1 }))
                continue
              }
              planId = plan.id
            }
            executing = true
            const outcome = await snapshot.invokeTauri<{ changed: boolean; pending_targets?: string[] }>('workspace_content_execute', { id: planId })
            result.succeeded.push({ skillId, name, pendingTargets: outcome.pending_targets ?? [] })
            publish(state => ({ ...state, records: { ...state.records, [skillId]: {
              ...record, candidate: { ...record.candidate, status: 'unchanged' },
            } } }))
          } catch (error) {
            // Once a write has started its actual outcome must be retained even
            // if cancellation arrived meanwhile. Never retry an uncertain write.
            if (!executing && (run.cancelled || !mounted.current)) break
            const failure = { skillId, name, error: errorText(error) }
            result.failed.push(failure)
            publish(state => ({ ...state, records: { ...state.records, [skillId]: {
              ...record, candidate: { ...record.candidate, status: 'failed', error: failure.error },
            } } }))
          }
        }
        publish(state => ({ ...state, processed: state.processed + 1 }))
      }
      result.cancelled = run.cancelled
      if (mounted.current && result.succeeded.length) {
        try { await latest.current.onReload() } catch (error) { result.reloadError = errorText(error) }
      }
      return result
    } finally {
      result.cancelled = run.cancelled
      publish(state => ({ ...state, phase: 'idle', currentSkill: null, cancelled: run.cancelled, lastApply: result }))
      if (runRef.current === run) runRef.current = null
    }
  }, [publish])
  const apply = useCallback((planIds: Record<string, string>, expectedCheckIds: Record<string, string>) =>
    applyRequests(Object.entries(planIds).map(([skillId, planId]) => ({ skillId, planId, expectedCheckId: expectedCheckIds[skillId] })), false), [applyRequests])
  const update = useCallback((candidates: UpdateCandidate[]) =>
    applyRequests([...new Map(candidates.map(item => [item.skillId, { skillId: item.skillId, expectedCheckId: item.id }])).values()], true), [applyRequests])

  const state = useMemo<UpdateCheckState>(() => {
    const current = eligible.flatMap(skill => {
      const entry = raw.records[skill.id]
      return entry?.localSignature === signatures.get(skill.id) && entry.invoke === invokeTauri && entry.checkSources === checkSources ? [{ skillId: skill.id, entry }] : []
    })
    const plans = current.flatMap(({ entry }) => entry.candidate.status === 'changed' ? [entry.candidate] : [])
    const failures = current.flatMap(({ skillId, entry }) => entry.candidate.status === 'failed' ? [{ skillId, name: entry.candidate.name, error: entry.candidate.error ?? 'CHECK_FAILED' }] : [])
    const stale = !raw.cancelled && raw.checkedAt !== null && current.length !== eligible.length
    const complete = raw.phase === 'idle' && raw.checkedAt !== null && !raw.cancelled && !stale && !raw.fromCache
    return { phase: raw.phase, currentSkill: raw.currentSkill, processed: raw.processed, total: raw.total, checkedAt: raw.checkedAt,
      cancelled: raw.cancelled, fromCache: raw.fromCache, lastApply: raw.lastApply, lastCheck: raw.lastCheck, plans, failures, eligibleCount: eligible.length, checkedCount: current.length,
      stale, complete, allUpToDate: complete && eligible.length > 0 && plans.length === 0 && failures.length === 0 && !raw.lastApply?.reloadError }
  }, [raw, eligible, signatures, invokeTauri, checkSources])
  return { state, check, retryFailures, cancel, apply, update }
}
