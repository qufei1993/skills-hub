import { useId, useState } from 'react'
import { AlertCircle, CheckCheck, CheckCircle2, FileDiff, RotateCcw, Search, ShieldCheck } from 'lucide-react'
import type { TFunction } from 'i18next'
import type { ManagedSkill } from './types'
import type { UpdateCandidate } from './workspace/contentCheck'
import type { useUpdateCheck } from './workspace/useUpdateCheck'
import { formatUpdateCheckError } from './workspace/updateCheckError'

type Props = {
  check: ReturnType<typeof useUpdateCheck>
  skills: ManagedSkill[]
  selectedIds: string[]
  onSelectionChange: (ids: string[]) => void
  recoverableIds: string[]
  onReview: (plans: UpdateCandidate[]) => void
  onRestore?: (skill: ManagedSkill) => void
  blocked: boolean
  recoveryError?: boolean
  onRetryRecoveries?: () => void
  t: TFunction
}

export default function UpdateCandidates({ check, selectedIds: selected, onSelectionChange, skills, recoverableIds, onReview, onRestore, blocked, recoveryError, onRetryRecoveries, t }: Props) {
  const [view, setView] = useState<{ tab: 'pending' | 'recovery'; checkedAt: number | null }>({ tab: 'pending', checkedAt: null })
  const id = useId()
  const { state } = check
  const tab = state.phase !== 'idle' || view.checkedAt !== state.checkedAt ? 'pending' : view.tab
  const setTab = (tab: 'pending' | 'recovery') => setView({ tab, checkedAt: state.checkedAt })
  const w = (key: string, values?: Record<string, unknown>) => t(`updatesWorkflow.${key}`, values)
  const inProgress = state.phase !== 'idle'
  const busy = inProgress || blocked
  // Keep selection tied to the exact check result, never to a newly checked version.
  const selectedPlans = state.plans.filter(plan => selected.includes(plan.id))
  const recoveries = skills.filter(skill => recoverableIds.includes(skill.id))
  const appliedSuccessfully = Boolean(state.lastApply?.succeeded.length && !state.lastApply.failed.length &&
    !state.lastApply.reloadError && !state.lastApply.cancelled && !state.cancelled && !state.failures.length)
  const applyWarning = Boolean(state.lastApply && (state.lastApply.failed.length || state.lastApply.reloadError || state.lastApply.cancelled || state.lastApply.succeeded.some(item => item.pendingTargets.length)))
  const showApplied = appliedSuccessfully && state.plans.length === 0
  // A new check retains its previous result until the full scan finishes. Keep
  // that empty-state presentation steady while the status line shows progress.
  const showCurrent = state.allUpToDate || (state.phase === 'checking' && state.checkedAt !== null &&
    state.checkedCount === state.eligibleCount && !state.fromCache && !state.stale && !state.cancelled &&
    !state.plans.length && !state.failures.length && !state.lastApply?.reloadError)
  const checkedLabel = state.checkedAt ? w('checkedAt', { time: new Date(state.checkedAt).toLocaleString([], { month: 'numeric', day: 'numeric', hour: '2-digit', minute: '2-digit', second: '2-digit' }), count: state.checkedCount }) : w('notChecked')
  const outcome = !inProgress && !state.lastApply ? state.lastCheck : null
  const outcomeWarning = Boolean(outcome && (outcome.cancelled || outcome.failed || state.stale))
  const outcomeTitle = outcome ? w(state.stale ? 'resultExpired' : outcome.cancelled ? 'checkStopped' : outcome.failed === outcome.total ? 'checkAllFailed' : outcome.failed ? 'checkPartial' : 'checkCompleted') : ''
  const outcomeDuration = outcome ? outcome.elapsedMs < 10 ? w('durationBrief')
    : w('durationSeconds', { seconds: (outcome.elapsedMs / 1000).toFixed(outcome.elapsedMs < 1000 ? 2 : 1) }) : ''
  const outcomeTime = outcome ? new Date(outcome.finishedAt).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' }) : ''
  const checkDetail = inProgress ? state.currentSkill?.name || w('preparing')
    : state.stale && !showApplied ? w('stale') : outcome ? w('checkBreakdown', { completed: outcome.completed, total: outcome.total, changed: outcome.changed, unchanged: outcome.unchanged }) + (outcome.failed ? ` · ${w('checkFailureCount', { count: outcome.failed })}` : '')
    : state.cancelled ? w('cancelled') : state.stale && !showApplied ? w('stale') : state.fromCache ? w('cachedHint') : ''
  return <section className="updates-candidates" aria-label={w('title')}>
    <div className="updates-content-tabs" role="group" aria-label={w('sections')}>
      <button type="button" aria-pressed={tab === 'pending'} className={tab === 'pending' ? 'active' : ''} onClick={() => setTab('pending')}>{w('pending')}<span>{state.plans.length}</span></button>
      <button type="button" aria-pressed={tab === 'recovery'} className={tab === 'recovery' ? 'active' : ''} onClick={() => setTab('recovery')}>{w('recovery')}<span>{recoveries.length}</span></button>
    </div>
    {tab === 'pending' ? <>
      <div className={`updates-check-meta${inProgress ? ' is-busy' : ''}${outcome ? outcomeWarning ? ' has-warning' : ' is-complete' : ''}`}>
        <div className="updates-check-copy" role="status" aria-live="polite">
          <div className="updates-check-summary"><span className="updates-check-label">
            {outcome ? outcomeWarning ? <AlertCircle size={16} aria-hidden="true" /> : <CheckCircle2 size={16} aria-hidden="true" /> : null}
            {inProgress ? w(state.phase === 'checking' ? 'checking' : 'applying') : outcome ? outcomeTitle : checkedLabel}</span>
            {outcome ? <span className="updates-check-timing">{outcomeTime} · {outcomeDuration}</span> : null}
            {inProgress ? <span className="updates-check-count">{state.processed} / {state.total}</span> : null}
          </div>
          <span className={`updates-check-detail${!inProgress && (state.cancelled || state.stale) ? ' is-notice' : ''}`} title={checkDetail}>{checkDetail}</span>
        </div>
        <button type="button" className="btn btn-secondary btn-sm updates-check-stop" disabled={!inProgress || state.cancelled}
          aria-hidden={!inProgress} tabIndex={inProgress ? 0 : -1} onClick={check.cancel}>{w(state.phase === 'checking' ? 'stopCheck' : 'stop')}</button>
        <progress className="updates-check-progress" max={Math.max(1, state.total)} value={state.processed}
          aria-label={w('progress')} aria-hidden={!inProgress} />
      </div>
      {state.lastApply ? <div className={`updates-apply-result${applyWarning ? ' has-errors' : ''}`} role="status">
        {applyWarning ? <AlertCircle size={17} aria-hidden="true" /> : <CheckCheck size={17} aria-hidden="true" />}<div><strong>{w('applyResult', { succeeded: state.lastApply.succeeded.length, failed: state.lastApply.failed.length })}</strong>
          {state.lastApply.succeeded.filter(item => item.pendingTargets.length).map(item => <p key={item.skillId}>{w('targetsPending', { name: item.name, count: item.pendingTargets.length })}</p>)}
          {state.lastApply.failed.map(item => <p key={item.skillId}>{item.name}: {formatUpdateCheckError(item.error, t)}</p>)}
          {state.lastApply.cancelled ? <p>{w('cancelled')}</p> : null}
          {state.lastApply.reloadError ? <p>{w('reloadFailed')}</p> : null}
        </div>
      </div> : null}
      {state.plans.length ? <>
        <div className="updates-selection-bar">
          <label><input type="checkbox" disabled={busy} checked={selectedPlans.length === state.plans.length} onChange={event => onSelectionChange(event.target.checked ? state.plans.map(plan => plan.id) : [])} />{w('selectAll')}</label>
          <span className="updates-selection-count">{w('selectionCount', { count: selectedPlans.length, total: state.plans.length })}</span>
        </div>
        <ul className="updates-content-list">
          {state.plans.map(plan => <li className="updates-content-row updates-candidate-row" key={plan.id}>
            <input type="checkbox" aria-label={w('selectSkill', { name: plan.name })} disabled={busy} checked={selected.includes(plan.id)} onChange={event => onSelectionChange(event.target.checked ? [...selected, plan.id] : selected.filter(value => value !== plan.id))} />
            <div className="updates-content-identity"><strong title={plan.name}>{plan.name}</strong><span className="updates-content-description">{w('sourceChanged')} · {w('affectedDeployments', { count: plan.targetCount })}</span></div>
            <div className="updates-content-actions"><button type="button" className="btn btn-secondary updates-content-review" disabled={busy} onClick={event => { event.currentTarget.focus(); onReview([plan]) }}><FileDiff size={15} aria-hidden="true" />{w('review')}</button></div>
          </li>)}
        </ul>
      </> : <div className={`updates-check-empty${showCurrent ? ' is-current' : ''}`} aria-busy={inProgress}>
        {showCurrent ? <ShieldCheck size={26} aria-hidden="true" /> : <Search size={26} aria-hidden="true" />}
        <h2>{w(showApplied ? 'applied' : state.eligibleCount === 0 ? 'noSkills' : showCurrent ? 'upToDate' : state.failures.length ? 'noConfirmedChanges' : state.cancelled || state.stale ? 'incomplete' : 'ready')}</h2>
        <p>{w(showApplied ? 'appliedHint' : state.eligibleCount === 0 ? 'noSkillsHint' : showCurrent ? 'upToDateHint' : state.failures.length ? 'failedHint' : state.cancelled || state.stale ? 'incompleteHint' : 'readyHint', { count: state.eligibleCount })}</p>
      </div>}
      {state.failures.length ? <section className="updates-check-failures" aria-labelledby={`${id}-failures`}>
        <header><h3 id={`${id}-failures`}><AlertCircle size={16} aria-hidden="true" />{w('failures', { count: state.failures.length })}</h3><button type="button" className="btn btn-secondary btn-sm" disabled={busy} onClick={() => { void check.retryFailures() }}>{w('retryFailed')}</button></header>
        <p>{w('failedHint')}</p>
        <ul>{state.failures.map(item => <li key={item.skillId}><strong>{item.name}</strong><span>{formatUpdateCheckError(item.error, t)}</span></li>)}</ul>
      </section> : null}
    </> : <>
      <p className="updates-recovery-hint">{w('recoveryHint')}</p>
      {recoveryError ? <div className="updates-check-failures" role="alert"><p>{w('recoveryFailed')}</p><button type="button" className="btn btn-secondary btn-sm" onClick={onRetryRecoveries}>{w('retryFailed')}</button></div> : null}
      {recoveries.length ? <ul className="updates-content-list">{recoveries.map(skill => <li className="updates-content-row" key={skill.id}>
        <span className="updates-content-skill-icon"><RotateCcw size={17} aria-hidden="true" /></span>
        <div className="updates-content-identity"><strong title={skill.name}>{skill.name}</strong><span className="updates-content-description">{skill.enabled ? w('latestSnapshot') : t('disabled')}</span></div>
        <div className="updates-content-actions"><button type="button" className="btn btn-secondary" disabled={busy || !onRestore} onClick={event => { event.currentTarget.focus(); onRestore?.(skill) }}>{w('reviewRecovery')}</button></div>
      </li>)}</ul> : !recoveryError ? <div className="updates-check-empty"><RotateCcw size={26} aria-hidden="true" /><h2>{w('noRecovery')}</h2><p>{w('noRecoveryHint')}</p></div> : null}
    </>}
  </section>
}
