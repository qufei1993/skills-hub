import { memo, useEffect, useId, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import { AlertTriangle, X } from 'lucide-react'
import type { TFunction } from 'i18next'
import type { ManagedSkill, ToolOption } from './types'
import { getSkillSyncState } from './skillSyncStatus'
import { issueReasonKey } from './skillIssueState'

export default memo(function SkillIssueNotice({ skill, tools, t, compact = false }: { skill: ManagedSkill; tools: ToolOption[]; t: TFunction; compact?: boolean }) {
  const state = getSkillSyncState(skill)
  return ['source-error', 'partial', 'failed'].includes(state) ? <IssueContent key={skill.id} skill={skill} tools={tools} t={t} compact={compact} /> : null
})

function IssueContent({ skill, tools, t, compact }: { skill: ManagedSkill; tools: ToolOption[]; t: TFunction; compact: boolean }) {
  const state = getSkillSyncState(skill)
  const [position, setPosition] = useState<{ left: number; top: number; maxHeight: number } | null>(null)
  const trigger = useRef<HTMLButtonElement>(null)
  const popup = useRef<HTMLDivElement>(null)
  const id = useId()
  useEffect(() => {
    if (!position) return
    popup.current?.focus()
    const dismiss = (event: PointerEvent | FocusEvent) => {
      if (event.target instanceof Node && !popup.current?.contains(event.target) && !trigger.current?.contains(event.target)) setPosition(null)
    }
    const escape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') { event.stopPropagation(); setPosition(null); trigger.current?.focus() }
    }
    const reposition = (event: Event) => {
      if (event.target instanceof Node && popup.current?.contains(event.target)) return
      setPosition(null)
    }
    document.addEventListener('pointerdown', dismiss)
    document.addEventListener('focusin', dismiss)
    document.addEventListener('keydown', escape)
    window.addEventListener('resize', reposition)
    window.addEventListener('scroll', reposition, true)
    return () => {
      document.removeEventListener('pointerdown', dismiss)
      document.removeEventListener('focusin', dismiss)
      document.removeEventListener('keydown', escape)
      window.removeEventListener('resize', reposition)
      window.removeEventListener('scroll', reposition, true)
    }
  }, [position])
  if (!['source-error', 'partial', 'failed'].includes(state)) return null
  const targets = skill.targets.filter(target => target.status !== 'ok' && target.status !== 'disabled')
  const title = t(state === 'source-error' ? 'deviceSync.sourceNeedsAttention' : 'deviceSync.toolUpdatesPending', { count: targets.length })
  const content = <div className="skill-issue-content">
      {state === 'source-error' ? <p>{t(issueReasonKey(skill.source_error))}<br />{t('deviceSync.sourceIssueHelp')}</p> : null}
      {targets.map((target, index) => <p key={index}><strong>{tools.find(tool => tool.id === target.tool)?.label || target.tool}</strong><span>{t(issueReasonKey(target.last_error))}<br />{t('deviceSync.toolIssueHelp')}</span></p>)}
    </div>
  if (!compact) return <details className="skill-issue-notice">
    <summary><AlertTriangle size={14} aria-hidden="true" /><span>{title}</span><span>{t('deviceSync.viewIssueReason')}</span></summary>{content}
  </details>
  return <>
    <button ref={trigger} type="button" className="skill-issue-trigger" aria-label={t('deviceSync.viewIssueReason')} title={title} aria-expanded={!!position} aria-haspopup="dialog" aria-controls={position ? id : undefined}
      onClick={() => {
        if (position) { setPosition(null); return }
        const rect = trigger.current!.getBoundingClientRect()
        const top = Math.min(rect.bottom + 8, Math.max(12, window.innerHeight - 260))
        setPosition({ left: Math.max(12, Math.min(rect.left, window.innerWidth - 372)), top, maxHeight: window.innerHeight - top - 12 })
      }}><AlertTriangle size={16} aria-hidden="true" /></button>
    {position ? createPortal(<div ref={popup} id={id} className="skill-issue-popover" role="dialog" aria-labelledby={`${id}-title`} tabIndex={-1} style={position}>
      <div className="skill-issue-popover-heading"><AlertTriangle size={16} aria-hidden="true" /><strong id={`${id}-title`}>{title}</strong>
        <button type="button" aria-label={t('close')} onClick={() => { setPosition(null); trigger.current?.focus() }}><X size={16} aria-hidden="true" /></button>
      </div>{content}
    </div>, document.body) : null}
  </>
}
