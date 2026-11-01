import { useEffect, useRef, useState } from 'react'
import { AlertCircle, CheckCircle2, ChevronRight, FileText, LoaderCircle } from 'lucide-react'
import type { TFunction } from 'i18next'
import type { SourceConflictChoice, SourceConflictOutcome, SourceConflictPreview } from '../types'
import ConfirmActionModal from '../modals/ConfirmActionModal'
import ContentFileDiff from './ContentFileDiff'
import './workspace-overlays.css'

type Props = {
  sourcePath: string
  invokeTauri: <T>(command: string, args?: Record<string, unknown>) => Promise<T>
  onChanged: () => Promise<void>
  onClose: () => void
  t: TFunction
}
type Failure = { code: string; recovery?: boolean; recoveryPaths?: string[] }
function failure(error: unknown, execution = false): Failure {
  try {
    const parsed = JSON.parse(error instanceof Error ? error.message : String(error))
    return { code: parsed.code ?? 'INTERNAL_ERROR', recovery: Boolean(parsed.details?.recovery?.length), recoveryPaths: parsed.details?.recovery?.filter((path: unknown) => typeof path === 'string') }
  } catch { return { code: execution ? 'UNKNOWN_RESULT' : 'INTERNAL_ERROR' } }
}

export default function SourceConflictDialog({ sourcePath, invokeTauri, onChanged, onClose, t }: Props) {
  const [attempt, setAttempt] = useState(0)
  const [preview, setPreview] = useState<SourceConflictPreview | null>(null)
  const [choice, setChoice] = useState<SourceConflictChoice | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<Failure | null>(null)
  const [result, setResult] = useState<SourceConflictOutcome | null>(null)
  const [refreshFailed, setRefreshFailed] = useState(false)
  const guard = useRef(false)
  const request = useRef<{ key: string; promise: Promise<SourceConflictPreview> } | null>(null)
  const w = (key: string) => t(`sourceConflict.${key}`)
  useEffect(() => {
    let cancelled = false
    const key = `${sourcePath}:${attempt}`
    if (request.current?.key !== key) request.current = { key, promise: invokeTauri<SourceConflictPreview>('source_conflict_preview', { sourcePath }) }
    void request.current.promise.then(plan => { if (!cancelled) { setPreview(plan); setError(null) } }).catch(error => { if (!cancelled) setError(failure(error)) })
    return () => { cancelled = true }
  }, [sourcePath, attempt, invokeTauri])
  const recheck = () => { setPreview(null); setError(null); setAttempt(value => value + 1) }
  const selected = choice && preview ? preview[choice] : null
  const submit = () => {
    if (!preview || !choice || selected?.blocked || guard.current) return
    const reconciling = error?.code === 'UNKNOWN_RESULT'
    guard.current = true; setBusy(true); setError(null)
    void (async () => {
      try {
        const outcome = await invokeTauri<SourceConflictOutcome>('source_conflict_execute', { id: preview.id, choice })
        setResult(outcome)
        try { await onChanged() } catch { setRefreshFailed(true) }
      } catch (error) {
        const next = failure(error, true)
        setError(reconciling && next.code === 'OPERATION_BUSY' ? { ...next, code: 'UNKNOWN_RESULT' } : next)
      }
      finally { guard.current = false; setBusy(false) }
    })()
  }
  return <ConfirmActionModal open className="workspace-overlay workspace-content-modal source-conflict-modal" intent="default" loading={busy}
    title={w(result ? 'completed' : 'title')} cancelLabel={t('close')} confirmLabel={w(result ? 'done' : 'resolve')}
    confirmDisabled={!result && (!preview || !choice || Boolean(selected?.blocked) || Boolean(error))}
    onRequestClose={() => { if (!guard.current) onClose() }}
    onConfirm={() => { if (result) onClose(); else submit() }} body={<div className="source-conflict-body" aria-busy={busy || (!preview && !error)}>
      {result ? <>
        <p className="workspace-overlay-status" role="status"><CheckCircle2 size={18} aria-hidden="true" />{t('sourceConflict.success', { name: result.name })}</p>
        {refreshFailed ? <div role="alert"><p>{w('refreshFailed')}</p><button type="button" className="btn btn-secondary" disabled={busy} onClick={() => {
          if (guard.current) return
          guard.current = true; setBusy(true)
          void onChanged().then(() => setRefreshFailed(false)).finally(() => { guard.current = false; setBusy(false) }).catch(() => {})
        }}>{w('refresh')}</button></div> : null}
      </> : <>
        {!preview && !error ? <p className="workspace-overlay-status" role="status"><LoaderCircle size={16} className="spin" aria-hidden="true" />{w('loading')}</p> : null}
        {error ? <div className="workspace-overlay-notice" role="alert">
          <div className="workspace-overlay-error"><AlertCircle size={16} aria-hidden="true" /><p>{w(error.recovery ? 'recoveryRequired' : error.code === 'UNKNOWN_RESULT' ? 'unknown' : error.code === 'PLAN_STALE' ? 'stale' : error.code === 'OPERATION_BUSY' ? 'busy' : 'failed')}</p></div>
          {error.recoveryPaths?.length ? <ul className="workspace-overlay-paths">{error.recoveryPaths.map(path => <li key={path}><code>{path}</code></li>)}</ul> : null}
          <button type="button" className="btn btn-secondary" disabled={busy} onClick={error.code === 'UNKNOWN_RESULT' ? submit : recheck}>{w(error.code === 'UNKNOWN_RESULT' ? 'verifyResult' : 'recheck')}</button>
        </div> : null}
        {preview ? <>
          <header><h3>{preview.name}</h3><p>{w('intro')}</p></header>
          <fieldset className="source-conflict-choices" disabled={busy || error?.code === 'UNKNOWN_RESULT'}>
            <legend>{w('choose')}</legend>
            {(['central', 'tool'] as const).map(value => <label key={value} className={`source-conflict-choice${choice === value ? ' is-selected' : ''}`}>
              <input type="radio" name="source-conflict-choice" value={value} checked={choice === value} disabled={Boolean(preview[value].blocked)} onChange={() => setChoice(value)} />
              <span><strong>{w(value === 'central' ? 'keepCentral' : 'keepTool')}</strong><span>{w(value === 'central' ? 'centralHelp' : 'toolHelp')}</span>
                {preview[value].blocked ? <span className="source-conflict-blocked">{w('blocked')}{preview[value].blocked?.path ? <code>{preview[value].blocked?.path}</code> : null}</span> : null}
              </span>
            </label>)}
          </fieldset>
          {selected && !selected.blocked ? <details className="source-conflict-impact" open>
            <summary>{t('sourceConflict.affected', { count: selected.targets.length })}</summary>
            <ul className="workspace-overlay-paths">{selected.targets.map(path => <li key={path}><code>{path}</code></li>)}</ul>
          </details> : null}
          <details><summary>{w('locations')}</summary><dl className="source-conflict-locations"><dt>{w('central')}</dt><dd><code>{preview.centralPath}</code></dd><dt>{w('tool')}</dt><dd><code>{preview.sourcePath}</code></dd></dl></details>
          <div className="content-review-files">
            <h4>{t('sourceConflict.fileCount', { count: preview.changes.length })}</h4>
            {preview.changes.length === 0 ? <p>{w('identical')}</p> : preview.changes.map((change, index) => <details className="content-review-file" key={change.path} open={index === 0}>
              <summary><ChevronRight className="content-review-chevron" size={14} aria-hidden="true" /><FileText size={15} aria-hidden="true" /><code>{change.path}</code><span className={`content-change-kind content-change-${change.kind}`}>{t(`workspace110.fileKinds.${change.kind}`)}</span></summary>
              <ContentFileDiff change={change} t={t} labels={{ before: w('central'), after: w('tool') }} />
            </details>)}
          </div>
        </> : null}
      </>}
    </div>} />
}
