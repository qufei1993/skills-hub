import { useEffect, useState } from 'react'
import { ChevronRight, LoaderCircle } from 'lucide-react'
import type { TFunction } from 'i18next'
import type { ManagedSkill, SkillPreset } from '../types'
import './workspace-overlays.css'

type Props = { skill: ManagedSkill; invokeTauri: <T>(command: string, args?: Record<string, unknown>) => Promise<T>; t: TFunction }
type MembershipResult = { skillId: string; invoke: Props['invokeTauri']; attempt: number; names: string[]; failed: boolean }
export default function SkillUsage({ skill, invokeTauri, t }: Props) {
  const [attempt, setAttempt] = useState(0)
  const [result, setResult] = useState<MembershipResult | null>(null)
  const current = result?.skillId === skill.id && result.invoke === invokeTauri && result.attempt === attempt ? result : null
  const w = (key: string) => t(`workspace110.${key}`)
  useEffect(() => {
    let cancelled = false
    const request = { skillId: skill.id, invoke: invokeTauri, attempt }
    invokeTauri<SkillPreset[]>('workspace_presets').then(presets => {
      if (!cancelled) setResult({ ...request, names: presets.filter(p => p.members.some(m => m.skillId === skill.id)).map(p => p.name), failed: false })
    }).catch(() => { if (!cancelled) setResult({ ...request, names: [], failed: true }) })
    return () => { cancelled = true }
  }, [skill.id, invokeTauri, attempt])
  return <details className="skill-usage"><summary><ChevronRight size={14} aria-hidden="true" />{w('usage')}</summary><div className="skill-usage-content">
    <section className="skill-usage-section"><h3>{w('usageLocations')}</h3>{skill.targets.length ? <ul className="skill-usage-targets">{skill.targets.map(target => <li key={`${target.tool}:${target.target_path}`}><div><strong>{target.tool}</strong><span>{w(target.scope === 'project' ? 'project' : 'global')}</span></div><code>{target.target_path}</code></li>)}</ul> : <p className="workspace-overlay-muted">{w('noDeployments')}</p>}</section>
    <section className="skill-usage-section" aria-busy={!current}><h3>{w('memberOf')}</h3>{!current ? <p className="workspace-overlay-status" role="status"><LoaderCircle size={14} className="spin" aria-hidden="true" />{w('usageLoading')}</p> : current.failed ? <div className="skill-usage-failure"><p role="alert">{w('usageFailed')}</p><button type="button" className="btn btn-secondary" onClick={() => setAttempt(value => value + 1)}>{w('retry')}</button></div> : current.names.length ? <ul className="skill-usage-presets">{current.names.map(name => <li key={name}>{name}</li>)}</ul> : <p className="workspace-overlay-muted">{w('noPreset')}</p>}</section>
  </div></details>
}
