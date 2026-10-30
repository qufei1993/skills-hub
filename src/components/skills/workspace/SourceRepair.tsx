import { useEffect, useId, useRef, useState } from 'react'
import { open } from '@tauri-apps/plugin-dialog'
import { AlertCircle, ArrowLeft, FolderOpen, LoaderCircle } from 'lucide-react'
import type { TFunction } from 'i18next'
import type { ManagedSkill } from '../types'
import ConfirmActionModal from '../modals/ConfirmActionModal'
import './workspace-overlays.css'
type Plan = { id: string; sourceRef: string; targets: string[] }
export default function SourceRepair({ skill, invokeTauri, onChanged, onClose, t }: { skill: ManagedSkill; invokeTauri: <T>(command: string, args?: Record<string, unknown>) => Promise<T>; onChanged: () => Promise<void>; onClose: () => void; t: TFunction }) {
  const [kind, setKind] = useState(skill.source_type === 'git' ? 'git' : 'local')
  const [reference, setReference] = useState('')
  const [subpath, setSubpath] = useState('')
  const [plan, setPlan] = useState<Plan | null>(null)
  const [busy, setBusy] = useState(false)
  const guard = useRef(false)
  const referenceInput = useRef<HTMLInputElement>(null)
  const planHeading = useRef<HTMLHeadingElement>(null)
  const returnToFields = useRef(false)
  const fieldId = useId()
  const [error, setError] = useState('')
  const w = (key: string) => t(`workspace110.${key}`)
  useEffect(() => {
    if (plan) planHeading.current?.focus({ preventScroll: true })
    else if (returnToFields.current) {
      referenceInput.current?.focus({ preventScroll: true })
      returnToFields.current = false
    }
  }, [plan])
  return <ConfirmActionModal open className="workspace-overlay workspace-source-modal" intent="default" loading={busy} title={w('repairSource')} cancelLabel={w('cancel')} confirmLabel={w(plan ? 'confirm' : 'preview')} confirmDisabled={!reference.trim()} onRequestClose={onClose} onConfirm={() => {
    if (guard.current) return
    guard.current = true; setBusy(true); setError('')
    void (async () => {
      try { if (plan) { await invokeTauri('workspace_source_execute', { id: plan.id }); await onChanged(); onClose() } else setPlan(await invokeTauri<Plan>('workspace_source_preview', { skillId: skill.id, kind, reference, subpath: subpath || null })) }
      catch { setError(w('sourceFailed')); setPlan(null) } finally { setBusy(false); guard.current = false }
    })()
  }} body={<div className="source-repair" aria-busy={busy}>
    <header className="source-repair-heading"><h3>{skill.name}</h3><p>{w('sourcePreserved')}</p></header>
    {error ? <div role="alert" className="workspace-overlay-error"><AlertCircle size={16} aria-hidden="true" /><p>{error}</p></div> : null}
    {busy ? <p role="status" className="workspace-overlay-status"><LoaderCircle size={16} className="spin" aria-hidden="true" />{w('working')}</p> : null}
    {plan ? <div className="source-repair-preview">
      <section><h4 ref={planHeading} tabIndex={-1}>{w('validatedSource')}</h4><p className="source-repair-location"><code>{plan.sourceRef}</code></p></section>
      <section><h4>{w('retainedDeployments')}</h4>{plan.targets.length ? <ul className="workspace-overlay-paths">{plan.targets.map(path => <li key={path}><code>{path}</code></li>)}</ul> : <p className="workspace-overlay-muted">{w('noDeployments')}</p>}</section>
      <button type="button" className="btn btn-secondary source-repair-back" disabled={busy} onClick={() => { returnToFields.current = true; setPlan(null) }}><ArrowLeft size={14} aria-hidden="true" />{w('back')}</button>
    </div> : <fieldset className="source-repair-fields" disabled={busy}>
      <div className="source-repair-field source-repair-kind"><label htmlFor={`${fieldId}-kind`}>{w('sourceType')}</label><select id={`${fieldId}-kind`} value={kind} onChange={e => setKind(e.target.value)}><option value="local">{w('localSource')}</option><option value="git">Git</option></select></div>
      <div className="source-repair-field"><label htmlFor={`${fieldId}-reference`}>{w('sourceLocation')}</label><div className="source-repair-location-row"><input ref={referenceInput} id={`${fieldId}-reference`} value={reference} onChange={e => setReference(e.target.value)} spellCheck={false} autoComplete="off" />{kind === 'local' ? <button type="button" className="btn btn-secondary" onClick={async () => { const path = await open({ directory: true, multiple: false }); if (typeof path === 'string') setReference(path) }}><FolderOpen size={15} aria-hidden="true" />{w('chooseSource')}</button> : null}</div></div>
      {kind === 'git' ? <div className="source-repair-field"><label htmlFor={`${fieldId}-subpath`}>{w('sourceSubpath')}</label><input id={`${fieldId}-subpath`} value={subpath} onChange={e => setSubpath(e.target.value)} spellCheck={false} autoComplete="off" /></div> : null}
    </fieldset>}
  </div>} />
}
