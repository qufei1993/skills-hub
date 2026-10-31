import { useEffect, useRef, useState } from 'react'
import { AlertCircle, ChevronRight, FileText, LoaderCircle } from 'lucide-react'
import type { TFunction } from 'i18next'
import type { ManagedSkill } from '../types'
import ConfirmActionModal from '../modals/ConfirmActionModal'
import ContentFileDiff from './ContentFileDiff'
import './workspace-overlays.css'

type EntryMetadata = { kind: 'file' | 'directory'; mode?: number; readonly?: boolean }
export type ContentPlan = { id: string; skillId: string; name: string; changes: { path: string; kind: string; before: string | null; after: string | null; beforeMetadata?: EntryMetadata | null; afterMetadata?: EntryMetadata | null }[]; targets: string[]; restore: boolean }
type Props = { preparedPlans?: ContentPlan[]; skills: ManagedSkill[]; restore: boolean; invokeTauri: <T>(command: string, args?: Record<string, unknown>) => Promise<T>; onConfirm: (plans: Record<string, string>) => Promise<void>; onClose: () => void; t: TFunction }
type PreviewFailure = Pick<ManagedSkill, 'id' | 'name'> & { conflictPath?: string }
type PreviewResult = { plans: ContentPlan[]; failed: PreviewFailure[] }
type PreviewWork = { key: string; selection: string; invoke: Props['invokeTauri']; result: Promise<PreviewResult> }
type PreviewState = Omit<PreviewWork, 'result'> & { result: PreviewResult }
export default function ContentReview({ preparedPlans, skills, restore, invokeTauri, onConfirm, onClose, t }: Props) {
  const [attempt, setAttempt] = useState(0)
  const [preview, setPreview] = useState<PreviewState | null>(null)
  const [submissionError, setSubmissionError] = useState<string | null>(null)
  const [saving, setSaving] = useState(false)
  const work = useRef<PreviewWork | null>(null)
  const submitting = useRef(false)
  const stopped = useRef(false)
  const selection = JSON.stringify([skills.map(({ id, name }) => [id, name]), restore, preparedPlans?.map(plan => plan.id)])
  const key = `${selection}:${attempt}`
  const current = preview?.selection === selection && preview.invoke === invokeTauri ? preview : null
  const plans = current?.result.plans ?? []
  const failed = current?.result.failed ?? []
  const changedCount = plans.filter(plan => plan.changes.length > 0).length
  const fileCount = plans.reduce((count, plan) => count + plan.changes.length, 0)
  const busy = current?.key !== key || saving
  const w = (key: string) => t(`workspace110.${key}`)
  useEffect(() => {
    let cancelled = false
    if (work.current?.key !== key || work.current.invoke !== invokeTauri) {
      const previous = work.current
      const retryFailed = previous?.selection === selection && previous.invoke === invokeTauri
      const result = (async (): Promise<PreviewResult> => {
        // A replaced request must release its backend lock before this one starts.
        const prior = await previous?.result
        if (stopped.current) return { plans: [], failed: [] }
        // Reuse a caller-provided preview when available. Execution still
        // validates its backend fingerprint before changing any content.
        if (preparedPlans && skills.every(skill => preparedPlans.some(plan => plan.skillId === skill.id && plan.restore === restore))) {
          return { plans: preparedPlans.filter(plan => skills.some(skill => skill.id === plan.skillId)), failed: [] }
        }
        const retained = retryFailed ? prior : undefined
        const plans = new Map(retained?.plans.map(plan => [plan.skillId, plan]))
        const failed: PreviewResult['failed'] = []
        // Preview persists a plan under the same exclusive lock as other writes.
        for (const skill of retained?.failed ?? skills) {
          if (stopped.current) break
          try {
            plans.set(skill.id, await invokeTauri<ContentPlan>('workspace_content_preview', { skillId: skill.id, restore }))
          } catch (error) {
            const message = String(error instanceof Error ? error.message : error)
            const conflictPath = /^(?:TARGET_MODIFIED|TARGET_EXISTS)\|(.+)$/s.exec(message)?.[1]
            failed.push({ id: skill.id, name: skill.name, conflictPath })
          }
        }
        return { plans: skills.flatMap(skill => plans.has(skill.id) ? [plans.get(skill.id)!] : []), failed }
      })()
      work.current = { key, selection, invoke: invokeTauri, result }
    }
    // StrictMode replays effects without cancelling the Tauri command: subscribe
    // to the existing work instead of issuing a second competing request.
    const request = work.current
    void request.result.then(result => { if (!cancelled && !stopped.current) setPreview({ ...request, result }) })
    return () => { cancelled = true }
  }, [skills, restore, invokeTauri, key, selection, preparedPlans])
  return <ConfirmActionModal open className="workspace-overlay workspace-content-modal" loading={saving} intent="default" title={w(restore ? 'restoreContent' : 'reviewUpdate')} cancelLabel={w('cancel')} confirmLabel={w(restore ? 'restoreContent' : 'confirmUpdate')} confirmDisabled={busy || !plans.length} onRequestClose={() => {
    if (submitting.current) return
    // Only explicit user dismissal stops work. StrictMode effect cleanup merely
    // unsubscribes, so its replay can still reuse the same serial preview work.
    stopped.current = true
    onClose()
  }} onConfirm={() => {
    if (submitting.current || busy || stopped.current) return
    submitting.current = true; setSaving(true); setSubmissionError(null)
    void onConfirm(Object.fromEntries(plans.map(p => [p.skillId, p.id]))).then(onClose).catch(() => setSubmissionError(key)).finally(() => { setSaving(false); submitting.current = false })
  }} body={<div className="content-review" aria-busy={busy}>
    {busy ? <p className="workspace-overlay-status" role="status"><LoaderCircle size={16} className="spin" aria-hidden="true" />{w('working')}</p> : null}
    {submissionError === key ? <div role="alert" className="workspace-overlay-error"><AlertCircle size={16} aria-hidden="true" /><p>{w('failed')}</p></div> : null}
    {failed.length ? <div className="workspace-overlay-notice">
      <div role="alert" className="workspace-overlay-error"><AlertCircle size={16} aria-hidden="true" /><p>{w('contentBlocked')} <strong>{failed.map(skill => skill.name).join(', ')}</strong></p></div>
      {failed.some(skill => skill.conflictPath) ? <div className="content-review-conflicts">
        <p>{w('reasons.TARGET_CONFLICT')}</p>
        <ul className="workspace-overlay-paths">{failed.filter(skill => skill.conflictPath).map(skill => <li key={skill.id}><strong>{skill.name}</strong><code>{skill.conflictPath}</code></li>)}</ul>
      </div> : null}
      <button type="button" className="btn btn-secondary" disabled={busy} onClick={() => setAttempt(value => value + 1)}>{w('recheck')}</button>
    </div> : null}
    {plans.length ? <div className="content-review-overview"><p>{t('workspace110.reviewSummary', { total: plans.length, changed: changedCount, files: fileCount })}</p>{!busy && !fileCount ? <span>{w('reviewNoChanges')}</span> : null}</div> : null}
    <div className="content-review-list">{plans.map(plan => <section className="content-review-skill" key={plan.id}>
      <header className="content-review-heading"><h3>{plan.name}</h3><span className={`content-review-state${plan.changes.length ? ' has-changes' : ''}`}>{plan.changes.length ? t('workspace110.contentFiles', { count: plan.changes.length }) : w('noFileChanges')}</span></header>
      <div className="content-review-impact">
        {plan.targets.length ? <details><summary>{t('workspace110.diff.affectedTargets', { count: plan.targets.length })}</summary><p>{w('contentImpact')}</p><ul className="workspace-overlay-paths">{plan.targets.map(path => <li key={path}><code>{path}</code></li>)}</ul></details> : <p>{w('centralContentOnly')}</p>}
      </div>
      {plan.changes.length > 0 ? <div className="content-review-files">{plan.changes.map((change, index) => <details className="content-review-file" key={change.path} open={plans.length === 1 && index === 0}>
        <summary><ChevronRight className="content-review-chevron" size={14} aria-hidden="true" /><FileText size={15} aria-hidden="true" /><code>{change.path}</code><span className={`content-change-kind content-change-${change.kind}`}>{w(`fileKinds.${change.kind}`)}</span></summary>
        <ContentFileDiff change={change} t={t} />
      </details>)}</div> : null}
    </section>)}</div>
  </div>} />
}
