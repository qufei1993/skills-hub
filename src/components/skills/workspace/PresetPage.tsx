import { useEffect, useRef, useState } from 'react'
import type { TFunction } from 'i18next'
import { open } from '@tauri-apps/plugin-dialog'
import { ArrowLeft, ArrowRight, CheckCircle2, ChevronRight, CircleAlert, Clock3, Copy, Folder, Layers2, Plus, Search, Trash2 } from 'lucide-react'
import type { ApplicationRecord, ManagedSkill, PresetMember, SkillPreset, ToolOption, WorkspaceIntent, WorkspaceScope, WorkspaceUndoItem } from '../types'
import ToolIcon from '../ToolIcon'
import ConfirmActionModal from '../modals/ConfirmActionModal'
import WorkspaceDialog from './WorkspaceDialog'
import './workspace.css'

type Props = {
  onDirtyChange: (dirty: boolean) => void
  active: boolean; skills: ManagedSkill[]; tools: ToolOption[]; projects: string[]; intent?: WorkspaceIntent
  invokeTauri: <T>(command: string, args?: Record<string, unknown>) => Promise<T>
  onChanged: () => Promise<void>; onBack: () => void; onSkill: (skill: ManagedSkill) => void
  onProject: (scope: WorkspaceScope) => void; t: TFunction
}
type View = 'list' | 'detail' | 'edit' | 'select' | 'preview' | 'result' | 'undo'
const emptyPreset = (): SkillPreset => ({ id: '', name: '', description: '', members: [], revision: 0 })
const presetContent = (value: SkillPreset) => JSON.stringify({
  name: value.name, description: value.description,
  members: value.members.map(member => ({ skillId: member.skillId, tools: [...member.tools].sort() })).sort((a, b) => a.skillId.localeCompare(b.skillId)),
})

export default function PresetPage({ onDirtyChange, active, skills, tools, projects, intent, invokeTauri, onChanged, onBack, onSkill, onProject, t }: Props) {
  const [view, setView] = useState<View>(intent?.mode ?? 'list')
  const [returnView, setReturnView] = useState<View>('list')
  const [presets, setPresets] = useState<SkillPreset[]>([])
  const [history, setHistory] = useState<ApplicationRecord[]>([])
  const [loaded, setLoaded] = useState(false)
  const [loadError, setLoadError] = useState(false)
  const [historyLoaded, setHistoryLoaded] = useState(false)
  const [previewOrigin, setPreviewOrigin] = useState<'select' | 'result'>('select')
  const [historyError, setHistoryError] = useState(false)
  const [nameError, setNameError] = useState('')
  const [tab, setTab] = useState<'presets' | 'history'>('presets')
  const [query, setQuery] = useState('')
  const [memberQuery, setMemberQuery] = useState('')
  const [selectedOnly, setSelectedOnly] = useState(false)
  const [preset, setPreset] = useState<SkillPreset>(() => ({ ...emptyPreset(), members: intent?.members ?? [] }))
  const [savedPreset, setSavedPreset] = useState<SkillPreset | null>(null)
  const [editBaseline, setEditBaseline] = useState(() => presetContent(emptyPreset()))
  const [saveConflict, setSaveConflict] = useState(false)
  const [scope, setScope] = useState<'' | 'global' | 'project'>(intent?.scope?.kind ?? '')
  const [selectedProjects, setSelectedProjects] = useState<string[]>(intent?.scope?.kind === 'project' ? [intent.scope.path] : [])
  const [projectQuery, setProjectQuery] = useState('')
  const [extraProjects, setExtraProjects] = useState<string[]>([])
  const [overrideTools, setOverrideTools] = useState(false)
  const [commonTools, setCommonTools] = useState<string[]>([])
  const [record, setRecord] = useState<ApplicationRecord | null>(null)
  const [undo, setUndo] = useState<WorkspaceUndoItem[]>([])
  const [undoFeedback, setUndoFeedback] = useState<{ id: string; removed: number; retained: number } | null>(null)
  const [busy, setBusy] = useState(false)
  const busyRef = useRef(false)
  const [error, setError] = useState('')
  const [confirm, setConfirm] = useState<null | 'discard' | 'delete' | 'latest'>(null)
  const previousView = useRef(view)
  const heading = useRef<HTMLHeadingElement>(null)
  const body = useRef<HTMLDivElement>(null)
  const backButton = useRef<HTMLButtonElement>(null)
  const deleteButton = useRef<HTMLButtonElement>(null)
  const latestButton = useRef<HTMLButtonElement>(null)
  const dialogReturnFocus = useRef<HTMLButtonElement>(null)
  const remove = intent?.remove ?? false
  const w = (key: string, options?: Record<string, unknown>) => t(`workspace110.${key}`, options)
  const fail = (e: unknown) => {
    const text = String(e instanceof Error ? e.message : e)
    if (view === 'edit' && text.includes('preset name already exists')) { setNameError(w('duplicateName')); return }
    if (view === 'edit' && text.includes('PLAN_STALE')) { setSaveConflict(true); setError(w('presetConflict')); return }
    setError(text.includes('PLAN_STALE') ? w('stale') : text.includes('INVALID_ARGUMENT') ? w('invalid') : w('failed'))
  }
  const run = async (work: () => Promise<void>) => {
    if (busyRef.current) return
    busyRef.current = true; setBusy(true); setError('')
    try { await work() } catch (e) { fail(e) } finally { busyRef.current = false; setBusy(false) }
  }
  const reload = async () => {
    setLoadError(false)
    try { setPresets(await invokeTauri<SkillPreset[]>('workspace_presets')); setLoaded(true) }
    catch { setLoadError(true) }
    setHistoryLoaded(false)
  }
  useEffect(() => {
    if (!active) return
    let cancelled = false
    invokeTauri<SkillPreset[]>('workspace_presets').then(values => {
      if (!cancelled) { setPresets(values); setLoaded(true); setLoadError(false) }
    }).catch(() => { if (!cancelled) setLoadError(true) })
    return () => { cancelled = true }
  }, [active, invokeTauri])
  const needsHistory = view === 'detail' || view === 'result' || (view === 'list' && tab === 'history')
  useEffect(() => {
    if (!active || !needsHistory || historyLoaded) return
    let cancelled = false
    invokeTauri<ApplicationRecord[]>('workspace_history').then(values => {
      if (!cancelled) { setHistory(values); setHistoryLoaded(true); setHistoryError(false) }
    }).catch(() => { if (!cancelled) setHistoryError(true) })
    return () => { cancelled = true }
  }, [active, needsHistory, historyLoaded, invokeTauri])
  useEffect(() => {
    const previous = previousView.current
    previousView.current = view
    if (!active || ['select', 'preview', 'undo'].includes(view)) return
    if ((['select', 'preview'].includes(previous) && view === returnView) || (previous === 'undo' && view === 'result' && (record?.undoTargetCount ?? 0) > 0) || (previous === 'preview' && previewOrigin === 'result' && view === 'result')) return
    if (body.current) body.current.scrollTop = 0
    heading.current?.focus()
  }, [view, active, returnView, previewOrigin, record?.undoTargetCount])
  const dirty = view === 'edit' && presetContent(preset) !== editBaseline
  useEffect(() => { onDirtyChange(dirty) }, [dirty, onDirtyChange])
  if (!active) return null

  const filteredPresets = presets.filter(p => `${p.name} ${p.description} ${p.members.map(m => skills.find(s => s.id === m.skillId)?.name ?? m.skillId).join(' ')}`.toLowerCase().includes(query.toLowerCase()))
  const filteredHistory = history.filter(r => `${r.preset?.name ?? ''} ${r.items.map(i => `${i.name} ${i.scope.kind === 'project' ? i.scope.path : ''}`).join(' ')}`.toLowerCase().includes(query.toLowerCase()))
  const summary = (items: ApplicationRecord['items'], outcomes = false) => <span className="workspace-summary">
    <span>{w('memberCount', { count: new Set(items.map(i => i.skillId)).size })}</span>
    <span>{w('toolCount', { count: new Set(items.flatMap(i => i.tools)).size })}</span>
    {(outcomes ? ['pending', 'running', 'done', 'failed', 'blocked', 'interrupted', 'undone'] : ['add', 'existing', 'repair', 'remove', 'blocked']).map(value => {
      const count = items.filter(i => (outcomes ? i.status : i.action) === value).length
      return count ? <span key={value} className={outcomes ? 'workspace-outcome' : undefined} data-state={outcomes ? value : undefined}>{w(`${outcomes ? 'states' : 'actions'}.${value}`)} {count}</span> : null
    })}
  </span>
  const members = preset.members
  const filteredSkills = skills.filter(s => (!selectedOnly || members.some(m => m.skillId === s.id)) && `${s.name} ${s.description ?? ''}`.toLowerCase().includes(memberQuery.toLowerCase()))
  const setMembers = (next: PresetMember[]) => setPreset(p => ({ ...p, members: next }))
  const toggleMember = (id: string) => setMembers(members.some(m => m.skillId === id) ? members.filter(m => m.skillId !== id) : [...members, { skillId: id, tools: [...commonTools] }])
  const toggleTool = (id: string, tool: string) => setMembers(members.map(m => m.skillId !== id ? m : { ...m, tools: m.tools.includes(tool) ? m.tools.filter(v => v !== tool) : [...m.tools, tool] }))
  const requestConfirmation = (kind: 'discard' | 'delete' | 'latest') => {
    // macOS pointer clicks may leave focus outside the trigger button.
    const trigger = kind === 'delete' ? deleteButton.current : kind === 'latest' ? latestButton.current : backButton.current
    trigger?.focus({ preventScroll: true })
    setConfirm(kind)
  }
  const leavePreview = (next: () => void) => {
    if (view !== 'preview' || previewOrigin !== 'select' || !record || record.confirmed) { next(); return }
    void run(async () => {
      try { await invokeTauri('workspace_discard', { id: record.id }) }
      catch { setError(w('discardPreviewFailed')); return }
      setRecord(null)
      next()
    })
  }
  const back = () => {
    setError('')
    if (view === 'edit' && dirty) { requestConfirmation('discard'); return }
    if (view === 'edit' && savedPreset) setPreset(savedPreset)
    if (view === 'undo') { setView('result'); return }
    if (view === 'preview') {
      leavePreview(() => setView(previewOrigin))
      return
    }
    if (intent && (view === intent.mode || view === 'result' || view === 'detail')) { onBack(); return }
    if ((view === 'select' || view === 'result') && savedPreset) setPreset(savedPreset)
    setView(view === 'detail' ? 'list' : returnView)
  }
  const resetSelectionControls = () => {
    setMemberQuery(''); setSelectedOnly(false); setOverrideTools(false); setCommonTools([]); setError(''); setNameError(''); setSaveConflict(false); setRecord(null)
  }
  const startEdit = (value: SkillPreset, copy = false) => {
    resetSelectionControls()
    setReturnView(view); setSavedPreset(value.id ? value : null)
    setEditBaseline(presetContent(copy ? emptyPreset() : value))
    setSelectedOnly(value.members.length > 0)
    setPreset(copy ? { ...value, id: '', revision: 0, name: w('copyName', { name: value.name }) } : structuredClone(value))
    setView('edit')
  }
  const startApply = (value = savedPreset ?? preset) => {
    resetSelectionControls()
    setSelectedOnly(true)
    setScope(intent?.scope?.kind ?? '')
    setSelectedProjects(intent?.scope?.kind === 'project' ? [intent.scope.path] : [])
    setProjectQuery(''); setExtraProjects([])
    setPreset(structuredClone(value)); setSavedPreset(value.id ? value : null)
    setReturnView(view); setView('select')
  }
  const execute = async (plan: ApplicationRecord) => {
    // The backend revalidates the preview before touching any deployment.
    let result: ApplicationRecord
    try { result = await invokeTauri<ApplicationRecord>('workspace_execute', { id: plan.id }) }
    catch {
      // A rejected transport call may still have committed some targets. Recover
      // this journal instead of creating a new application that claims them again.
      let latest = plan
      try { latest = (await invokeTauri<ApplicationRecord[]>('workspace_history')).find(r => r.id === plan.id) ?? plan } catch { /* Keep the known journal ID for explicit recovery. */ }
      setRecord(latest); setView('result')
      if (latest.confirmed && latest.items.every(i => ['done', 'undone'].includes(i.status))) { await onChanged(); await reload() }
      else setError(w('executionUnknown'))
      return
    }
    setRecord(result); setView('result')
    await onChanged(); await reload()
  }
  const preview = async () => {
    const chosen = overrideTools ? members.map(m => ({ ...m, tools: commonTools })) : members
    if (!scope || !chosen.length || chosen.some(m => !m.tools.length) || (scope === 'project' && !selectedProjects.length)) { setError(w('invalid')); return }
    const result = await invokeTauri<ApplicationRecord>('workspace_preview', { request: { presetId: preset.id || null, members: chosen, scopes: scope === 'global' ? [{ kind: 'global' }] : selectedProjects.map(path => ({ kind: 'project', path })), remove } })
    setRecord(result); setPreviewOrigin('select')
    if (remove || result.items.some(item => item.status === 'blocked' || item.action === 'blocked')) setView('preview')
    else await execute(result)
  }
  const openRecord = (r: ApplicationRecord) => { setReturnView(view); setRecord(r); setView('result') }
  const save = async (value: SkillPreset) => {
    if (presets.some(p => p.id !== value.id && p.name.trim().toLowerCase() === value.name.trim().toLowerCase())) { setNameError(w('duplicateName')); return }
    const saved = await invokeTauri<SkillPreset>('workspace_save_preset', { preset: value })
    setPreset(saved); setSavedPreset(saved); setSaveConflict(false); setEditBaseline(presetContent(saved)); setView('detail')
    await reload()
  }
  const saveCopy = async () => {
    const latest = await invokeTauri<SkillPreset[]>('workspace_presets')
    const base = w('copyName', { name: preset.name })
    let name = base; let number = 2
    while (latest.some(value => value.name.toLowerCase() === name.toLowerCase())) name = `${base} ${number++}`
    await save({ ...preset, name, id: '', revision: 0 })
  }
  const loadLatest = async () => {
    const latest = await invokeTauri<SkillPreset[]>('workspace_presets')
    setPresets(latest)
    const value = latest.find(value => value.id === preset.id)
    setConfirm(null)
    if (!value) { setError(w('presetDeleted')); return }
    setPreset(structuredClone(value)); setSavedPreset(value); setEditBaseline(presetContent(value)); setSaveConflict(false)
  }
  const missingTools = overrideTools ? !commonTools.length : members.some(member => !member.tools.length)
  const prerequisite = view === 'edit'
    ? !preset.name.trim() ? w('needName') : !members.length ? w('needMembers') : missingTools ? w('needTools') : ''
    : !scope ? w('needScope') : scope === 'project' && !selectedProjects.length ? w('needProjects') : !members.length ? w('needMembers') : missingTools ? w('needTools') : ''
  const modalOpen = ['select', 'preview', 'undo'].includes(view)
  const pageView = view === 'undo' || (view === 'preview' && previewOrigin === 'result') ? 'result' : modalOpen ? returnView === 'detail' ? 'detail' : 'list' : view
  const title = pageView === 'list' ? w('title') : pageView === 'detail' ? preset.name : pageView === 'edit' && !preset.id ? w('new') : w(pageView)
  const chosenProjects = [...new Set([...projects, ...extraProjects, ...selectedProjects])].filter(p => p.toLowerCase().includes(projectQuery.toLowerCase()))
  const toolIdentity = (id: string): ToolOption => tools.find(tool => tool.id === id) ?? { id, label: id }
  const toolBadge = (id: string, iconOnly = false) => {
    const tool = toolIdentity(id)
    return <span key={id} className={iconOnly ? 'workspace-tool-avatar' : 'workspace-tool-badge'} title={tool.label}>
      <ToolIcon toolKey={id} label={tool.label} avatar={tool.avatar} />
      <span className={iconOnly ? 'sr-only' : undefined}>{tool.label}</span>
    </span>
  }
  const selectableTools = (selected: string[]): ToolOption[] => [...tools, ...selected.filter(id => !tools.some(tool => tool.id === id)).map(id => ({ id, label: id }))]
  const toolSelector = (selected: string[], toggle: (id: string) => void, label: string) => <div className="workspace-tool-choices" role="group" aria-label={label}>{selectableTools(selected).map(tool => <label key={tool.id}><input type="checkbox" checked={selected.includes(tool.id)} onChange={() => toggle(tool.id)} disabled={busy} /><ToolIcon toolKey={tool.id} label={tool.label} avatar={tool.avatar} /><span>{tool.label}</span></label>)}</div>
  const pathName = (path: string) => path.split(/[\\/]/).filter(Boolean).at(-1) ?? path
  const scopeName = (value: WorkspaceScope) => value.kind === 'global' ? w('global') : value.path
  const historyCard = (r: ApplicationRecord) => <button key={r.id} className="workspace-preset workspace-history-record" onClick={() => openRecord(r)}>
    <span className="workspace-preset-main">
      {view === 'detail' ? <time className="workspace-record-date">{new Date(r.createdAt).toLocaleString()}</time> : <strong>{r.preset?.name ?? w(r.remove ? 'manualRemove' : 'manual')}</strong>}
      <span className="workspace-record-scopes">{[...new Set(r.items.map(i => scopeName(i.scope)))].map(path => <span key={path} title={path}><Folder size={14} aria-hidden="true" />{pathName(path)}</span>)}</span>
      {summary(r.items, true)}
    </span>
    {view !== 'detail' ? <span className="workspace-preset-meta"><time>{new Date(r.createdAt).toLocaleString()}</time></span> : null}<ChevronRight size={16} aria-hidden="true" />
  </button>
  const itemReason = (item: ApplicationRecord['items'][number], fallback = `states.${item.status}`) => item.conflict
    ? w(`conflictReasons.${item.conflict.reason}`, { defaultValue: w('reasons.TARGET_CONFLICT') })
    : item.reason ? w(`reasons.${item.reason}`, { defaultValue: w('failed') }) : w(fallback)
  const targetDetails = (item: ApplicationRecord['items'][number]) => item.conflict
    ? <div className="workspace-conflict-target"><div className="workspace-tools">{toolBadge(item.conflict.agent)}</div><p className="workspace-path">{item.conflict.path}</p></div>
    : item.targets.length ? <details className="workspace-disclosure"><summary>{w('deploymentDetails')}</summary>{item.targets.map(target => <div key={target.path}><p className="workspace-path">{target.path}</p>{target.affected_agents.length > 1 ? <p className="workspace-note">{w('shared', { tools: target.affected_agents.map(id => toolIdentity(id).label).join(', ') })}</p> : null}</div>)}</details> : null
  const availableCount = record?.items.filter(item => ['pending', 'failed'].includes(item.status) && item.action !== 'blocked').length ?? 0
  const previewBlocked = view === 'preview' && availableCount === 0
  const previewHint = record?.remove
    ? w(previewBlocked ? 'nothingRemovableHint' : 'confirmHint')
    : w(previewBlocked ? previewOrigin === 'result' ? 'nothingApplicableHistoryHint' : 'nothingApplicableHint' : 'conflictHint')
  const table = (r: ApplicationRecord) => <div className="workspace-results">{[...new Set(r.items.map(i => scopeName(i.scope)))].map(path => {
    const items = r.items.filter(i => scopeName(i.scope) === path)
    const targetScope = items[0].scope
    return <section key={path} className="workspace-group">
      <div className="workspace-group-heading"><h2 title={path}>{targetScope.kind === 'global' ? w('global') : pathName(path)}</h2>{summary(items, true)}</div>
      {items.filter(item => !['done', 'undone'].includes(item.status)).map((item, i) => <div key={`${item.skillId}-${i}`} className="workspace-result-row"><div><strong>{item.name}</strong><p className="workspace-error">{itemReason(item)}</p>{targetDetails(item)}</div><span className="workspace-status" data-state={item.status}>{w(`states.${item.status}`)}</span></div>)}
      <details className="workspace-result-details"><summary>{w('viewDetails')}</summary>{items.map((item, i) => <div key={`${item.skillId}-${i}`} className="workspace-result-row">
        <div><strong>{item.name}</strong><div className="workspace-tools workspace-result-tools">{item.tools.map(id => toolBadge(id))}</div>{targetDetails(item)}</div>
        <div className="workspace-result-status"><span className="workspace-status" data-state={item.status}>{w(`states.${item.status}`)}</span><span className="workspace-meta">{w(`actions.${item.action}`)}</span></div>
      </div>)}</details>
      <div className="workspace-group-action"><button className="btn workspace-text-action" onClick={() => onProject(targetScope)}>{w('viewTarget')}<ArrowRight size={14} /></button></div>
    </section>
  })}</div>
  const presetHistory = history.filter(r => r.preset?.id === preset.id)
  const historyFeedback = historyError ? <div className="workspace-alert workspace-error" role="alert">{w('historyFailed')}<button className="btn btn-secondary" disabled={busy} onClick={() => void run(async () => { try { setHistory(await invokeTauri<ApplicationRecord[]>('workspace_history')); setHistoryLoaded(true); setHistoryError(false) } catch { setHistoryError(true) } })}>{w('reload')}</button></div> : !historyLoaded ? <p className="workspace-note" role="status">{w('historyLoading')}</p> : null
  const alert = error ? <div className="workspace-alert workspace-error" role="alert"><span>{error}</span>{saveConflict && view === 'edit' ? <div className="workspace-actions"><button className="btn btn-secondary" disabled={busy || Boolean(prerequisite)} onClick={() => void run(saveCopy)}>{w('saveCopy')}</button><button ref={latestButton} className="btn btn-secondary" disabled={busy} onClick={() => requestConfirmation('latest')}>{w('loadLatest')}</button></div> : null}</div> : null
  const memberEditor = <section className="workspace-section workspace-member-editor">
    <div className="workspace-section-heading"><h2>{w('membersTitle')}</h2><span className="workspace-count">{w('memberCount', { count: members.length })}</span></div>
    {view === 'select' ? <label className="workspace-check"><input type="checkbox" checked={overrideTools} onChange={e => setOverrideTools(e.target.checked)} />{w('override')}</label> : null}
    {overrideTools || !selectedOnly ? <div className="workspace-default-tools"><span>{w(overrideTools ? 'commonTools' : 'defaultTools')}</span>{toolSelector(commonTools, id => setCommonTools(a => a.includes(id) ? a.filter(v => v !== id) : [...a, id]), w(overrideTools ? 'commonTools' : 'defaultTools'))}</div> : null}
    <div className="workspace-toolbar"><label className="workspace-search"><Search size={16} /><input aria-label={w('searchMembers')} placeholder={w('searchMembers')} value={memberQuery} onChange={e => setMemberQuery(e.target.value)} /></label><button className="btn btn-secondary" onClick={() => { setSelectedOnly(!selectedOnly); setMemberQuery('') }}>{w(selectedOnly ? 'addMembers' : 'doneSelecting')}</button>{!selectedOnly ? <button className="btn workspace-text-action" disabled={!filteredSkills.some(s => !members.some(m => m.skillId === s.id))} onClick={() => setMembers([...members, ...filteredSkills.filter(s => !members.some(m => m.skillId === s.id)).map(s => ({ skillId: s.id, tools: [...commonTools] }))])}>{w('selectResults')}</button> : null}</div>
    <div className="workspace-members">{filteredSkills.map(skill => { const member = members.find(m => m.skillId === skill.id); return <div className={`workspace-member${member ? ' is-selected' : ''}`} key={skill.id}>
      <label><input type="checkbox" aria-label={skill.name} checked={Boolean(member)} onChange={() => toggleMember(skill.id)} /><span className="workspace-member-identity"><strong>{skill.name}</strong>{skill.description ? <small title={skill.description}>{skill.description}</small> : null}{!skill.enabled ? <small>{w('disabled')}</small> : null}</span></label>
      {member ? <div className="workspace-member-assignment">{!overrideTools ? <><div className="workspace-tools">{member.tools.map(id => toolBadge(id))}{!member.tools.length ? <span className="workspace-error">{w('chooseTools')}</span> : null}</div><details className="workspace-disclosure"><summary aria-label={w('adjustMemberTools', { name: skill.name })}>{w('adjustTools')}</summary>{toolSelector(member.tools, tool => toggleTool(skill.id, tool), skill.name)}</details></> : <span className="workspace-note">{commonTools.map(id => toolIdentity(id).label).join(' · ') || w('chooseTools')}</span>}</div> : null}
    </div> })}{filteredSkills.length === 0 ? <p className="workspace-empty">{w('noResults')}</p> : null}</div>
    {members.filter(m => !skills.some(s => s.id === m.skillId)).map(m => <div className="workspace-member" key={m.skillId}><span>{w('missing')} · {m.skillId}</span><button className="btn btn-secondary" onClick={() => toggleMember(m.skillId)}>{w('removeMember')}</button></div>)}
  </section>
  const appliedTools = [...new Set(overrideTools ? commonTools : members.flatMap(m => m.tools))]
  const closeDialog = () => leavePreview(() => {
    if (view === 'preview' && previewOrigin === 'result') setView('result')
    else if (intent && view !== 'undo') onBack()
    else if (view === 'preview') {
      setError('')
      if (savedPreset) setPreset(savedPreset)
      setView(returnView)
    } else back()
  })
  return <>
    <section className={`workspace-page workspace-view-${pageView}`} inert={modalOpen || confirm !== null} aria-busy={busy || (!loaded && !loadError)}>
      <header className="workspace-heading">
        <div className="workspace-title">{pageView !== 'list' || intent ? <button ref={backButton} className="btn btn-secondary workspace-back" aria-label={w('back')} title={w('back')} onClick={back} disabled={busy}><ArrowLeft size={18} /></button> : <span className="workspace-page-symbol" aria-hidden="true"><Layers2 size={24} /></span>}<div><h1 tabIndex={-1} ref={heading}>{title}</h1><p>{pageView === 'detail' ? `${w('memberCount', { count: members.length })} · ${w('toolCount', { count: new Set(members.flatMap(m => m.tools)).size })}` : w(pageView === 'edit' ? 'editorIntro' : pageView === 'result' ? 'resultIntro' : 'intro')}</p></div></div>
        <div className="workspace-actions">{pageView === 'list' ? <button className="btn btn-primary" onClick={() => startEdit(emptyPreset())}><Plus size={16} />{w('new')}</button> : null}{pageView === 'detail' ? <><button className="btn btn-secondary" onClick={() => startEdit(preset)}>{w('edit')}</button><button className="btn btn-primary" onClick={event => { dialogReturnFocus.current = event.currentTarget; event.currentTarget.focus(); startApply() }}>{w('apply')}<ArrowRight size={16} /></button></> : null}</div>
      </header>
      <div ref={body} className="workspace-body">
        {!modalOpen ? alert : null}
        {busy && !modalOpen ? <p className="workspace-note" role="status">{w('working')}</p> : null}
        {pageView === 'list' ? <>
          <div className="workspace-directory-controls"><div className="workspace-tabs" role="group" aria-label={w('title')}><button aria-pressed={tab === 'presets'} onClick={() => setTab('presets')}><Layers2 size={16} />{w('presets')}</button><button aria-pressed={tab === 'history'} onClick={() => setTab('history')}><Clock3 size={16} />{w('history')}</button></div><label className="workspace-search"><Search size={16} /><input aria-label={w('search')} placeholder={w('search')} value={query} onChange={e => setQuery(e.target.value)} /></label></div>
          {tab === 'presets' ? <>
            {loadError ? <div role="alert" className="workspace-alert workspace-error">{w('failed')}<button className="btn btn-secondary" onClick={() => void run(reload)}>{w('reload')}</button></div> : !loaded ? <p className="workspace-note" role="status">{w('presetsLoading')}</p> : <>
              <span className="workspace-count">{w('presetCount', { count: filteredPresets.length })}</span>
              {filteredPresets.length ? <div className="workspace-preset-grid">{filteredPresets.map(p => <article key={p.id} className="workspace-preset-card">
                <button className="workspace-card-open" aria-label={p.name} onClick={() => { setPreset(p); setSavedPreset(p); setView('detail') }}><span className="workspace-card-heading"><span className="workspace-card-symbol" aria-hidden="true"><Layers2 size={21} /></span><span className="workspace-card-title"><strong>{p.name}</strong><span>{w('memberCount', { count: p.members.length })} · {w('toolCount', { count: new Set(p.members.flatMap(m => m.tools)).size })}</span></span><ChevronRight size={17} /></span>{p.description ? <span className="workspace-card-description">{p.description}</span> : null}<span className="workspace-card-preview">{p.members.slice(0, 3).map(m => skills.find(skill => skill.id === m.skillId)?.name ?? w('missing')).join(' · ')}{p.members.length > 3 ? ` · +${p.members.length - 3}` : ''}</span><span className="workspace-tools">{[...new Set(p.members.flatMap(m => m.tools))].map(id => toolBadge(id))}</span></button>
                <div className="workspace-card-footer"><button className="btn btn-secondary" aria-label={w('applyNamed', { name: p.name })} onClick={event => { dialogReturnFocus.current = event.currentTarget; event.currentTarget.focus(); startApply(p) }}>{w('apply')}<ArrowRight size={14} /></button></div>
              </article>)}</div> : <div className="workspace-empty"><p>{w(presets.length ? 'noMatches' : 'empty')}</p>{!presets.length ? <p>{w('emptyHelp')}</p> : null}</div>}
            </>}
          </> : <><p className="workspace-note">{w('historyRetention')}</p>{historyFeedback}{historyLoaded ? filteredHistory.length ? <div className="workspace-list">{filteredHistory.map(historyCard)}</div> : <div className="workspace-empty"><p>{w(history.length ? 'noMatches' : 'noHistory')}</p></div> : null}</>}
        </> : null}
        {pageView === 'detail' ? <div className="workspace-detail-layout"><section className="workspace-section workspace-detail-members">
          <div className="workspace-section-heading"><h2>{w('membersTitle')}</h2></div>{preset.description ? <p className="workspace-note">{preset.description}</p> : null}
          <div className="workspace-members">{members.map(m => { const skill = skills.find(s => s.id === m.skillId); return <div className="workspace-member" key={m.skillId}><div className="workspace-detail-identity"><span className="workspace-skill-symbol" aria-hidden="true"><Folder size={19} /></span><div className="workspace-member-identity"><button className="workspace-member-link" disabled={!skill} onClick={() => skill && onSkill(skill)}>{skill?.name ?? w('missing')}</button>{skill?.description ? <small title={skill.description}>{skill.description}</small> : null}</div></div><div className="workspace-tools">{m.tools.map(id => toolBadge(id))}</div></div> })}</div>
          <div className="workspace-definition-actions"><button className="btn workspace-text-action" onClick={() => startEdit(preset, true)}><Copy size={14} />{w('copy')}</button><button ref={deleteButton} className="btn workspace-text-action workspace-delete-action" onClick={() => requestConfirmation('delete')}><Trash2 size={14} />{w('delete')}</button></div>
          {historyFeedback}{historyLoaded ? presetHistory.length ? <details className="workspace-disclosure workspace-history-disclosure"><summary>{w('presetHistory')} · {w('recordCount', { count: presetHistory.length })}</summary><div className="workspace-list">{presetHistory.slice(0, 3).map(historyCard)}</div>{presetHistory.length > 3 ? <button className="btn workspace-text-action" onClick={() => { setQuery(preset.name); setTab('history'); setView('list') }}>{w('history')}</button> : null}</details> : <p className="workspace-note">{w('noHistory')}</p> : null}
        </section></div> : null}
        {view === 'edit' ? <fieldset disabled={busy} className="workspace-form workspace-editor"><section className="workspace-section workspace-editor-info"><h2>{w('presetDetails')}</h2><div className="workspace-definition"><label className="workspace-field">{w('name')}<input value={preset.name} aria-invalid={Boolean(nameError)} aria-describedby={nameError ? 'workspace-name-error' : undefined} onChange={e => { setPreset(p => ({ ...p, name: e.target.value })); setNameError('') }} /></label>{nameError ? <p id="workspace-name-error" className="workspace-error" role="alert">{nameError}</p> : null}<label className="workspace-field">{w('description')}<textarea rows={3} value={preset.description} onChange={e => setPreset(p => ({ ...p, description: e.target.value }))} /></label></div><p className="workspace-definition-hint">{w('definitionOnly')}</p></section>{memberEditor}</fieldset> : null}
        {pageView === 'result' && record ? <><div className="workspace-completion" data-state={record.items.some(i => !['done', 'undone'].includes(i.status)) ? 'warning' : 'success'}>{record.items.some(i => !['done', 'undone'].includes(i.status)) ? <CircleAlert size={22} aria-hidden="true" /> : <CheckCircle2 size={22} aria-hidden="true" />}<div><strong>{w(record.items.length && record.items.every(i => i.status === 'undone') ? 'undoComplete' : record.items.some(i => !['done', 'undone'].includes(i.status)) ? 'completedWithIssues' : 'completed')}</strong><p>{record.preset?.name ?? w('manual')}</p></div></div>{undoFeedback?.id === record.id ? <p className="workspace-note" role="status">{w('undoSummary', { removed: undoFeedback.removed, retained: undoFeedback.retained })}</p> : null}{table(record)}</> : null}
      </div>
      {view === 'edit' ? <footer className="workspace-footer"><div className="workspace-footer-summary"><span>{w('memberCount', { count: members.length })}</span><p id="workspace-prerequisite">{prerequisite || w('saveHint')}</p></div><button className="btn btn-primary" aria-describedby="workspace-prerequisite" disabled={busy || Boolean(prerequisite) || saveConflict} onClick={() => void run(() => save(preset))}>{w('save')}</button></footer> : null}
      {pageView === 'result' && record ? <footer className="workspace-footer">{!record.remove && record.undoTargetCount > 0 ? <button className="btn btn-secondary" disabled={busy} onClick={event => { dialogReturnFocus.current = event.currentTarget; event.currentTarget.focus(); void run(async () => { setUndo(await invokeTauri<WorkspaceUndoItem[]>('workspace_preview_undo', { id: record.id })); setView('undo') }) }}>{w('undo')}</button> : null}{record.items.some(i => !['done', 'undone'].includes(i.status)) ? <button className="btn btn-secondary" disabled={busy} onClick={event => { dialogReturnFocus.current = event.currentTarget; event.currentTarget.focus(); void run(async () => { setRecord(await invokeTauri<ApplicationRecord>('workspace_recheck', { id: record.id })); setPreviewOrigin('result'); setView('preview') }) }}>{w('recheck')}</button> : null}<button className="btn btn-primary" disabled={busy} onClick={back}>{w('done')}</button></footer> : null}
    </section>
    {view === 'select' || view === 'preview' ? <WorkspaceDialog returnFocusRef={dialogReturnFocus} title={w(view === 'preview' ? record?.remove ? previewBlocked ? 'nothingRemovable' : 'confirmRemove' : previewBlocked ? 'nothingApplicable' : 'reviewConflicts' : 'applyDialog')} closeLabel={w('close')} busy={busy} onClose={closeDialog} footer={<>
      {view === 'select' ? <div className="workspace-footer-summary"><p id="workspace-apply-prerequisite">{prerequisite || w('applySafeHint')}</p></div> : null}
      <button className="btn btn-secondary" disabled={busy} onClick={closeDialog}>{w(view === 'preview' && previewOrigin === 'result' ? 'backToResult' : 'cancel')}</button>
      {view === 'preview' && previewOrigin === 'select' ? <button className={`btn ${previewBlocked ? 'btn-primary' : 'btn-secondary'}`} disabled={busy} onClick={back}>{w('backToSelection')}</button> : null}
      {view === 'select' ? <button className="btn btn-primary" aria-describedby="workspace-apply-prerequisite" disabled={busy || Boolean(prerequisite)} onClick={() => void run(preview)}>{w(busy ? 'working' : 'applyNow')}</button> : record && availableCount > 0 ? <button className={`btn ${record.remove ? 'btn-danger' : 'btn-primary'}`} disabled={busy} onClick={() => void run(() => execute(record))}>{w(record.remove ? 'confirmRemove' : 'applyAvailable')}</button> : null}
    </>}>
      {alert}
      {view === 'select' ? <fieldset disabled={busy} className="workspace-form">
        <div className="workspace-apply-summary"><Layers2 size={20} /><div><strong>{preset.name || w('manual')}</strong><p>{w('memberCount', { count: members.length })} · {w('toolCount', { count: appliedTools.length })}</p></div></div>
        <div className="workspace-scope-options" role="radiogroup" aria-label={w('scope')}>{(['global', 'project'] as const).map(value => <label key={value}><input type="radio" name="workspace-scope" aria-label={w(value)} checked={scope === value} onChange={() => setScope(value)} /><span><strong>{w(value)}</strong><small>{w(value === 'global' ? 'globalHint' : 'projectHint')}</small></span></label>)}</div>
        {scope === 'project' ? <section className="workspace-section"><div className="workspace-toolbar">{chosenProjects.length > 5 || projectQuery ? <label className="workspace-search"><Search size={16} /><input aria-label={w('searchProjects')} placeholder={w('searchProjects')} value={projectQuery} onChange={e => setProjectQuery(e.target.value)} /></label> : <span className="workspace-count">{w('projectCount', { count: selectedProjects.length })}</span>}<button className="btn btn-secondary" onClick={() => void run(async () => { const path = await open({ directory: true, multiple: false }); if (typeof path === 'string') { setExtraProjects(p => [...p, path]); setSelectedProjects(p => [...new Set([...p, path])]) } })}><Folder size={15} />{w('chooseProject')}</button></div>{chosenProjects.length ? <div className="workspace-projects">{chosenProjects.map(path => <label key={path}><input type="checkbox" aria-label={path} checked={selectedProjects.includes(path)} onChange={() => setSelectedProjects(p => p.includes(path) ? p.filter(v => v !== path) : [...p, path])} /><span className="workspace-project-identity"><strong>{pathName(path)}</strong><span className="workspace-path" title={path}>{path}</span></span></label>)}</div> : <p className="workspace-note">{w('noProjectResults')}</p>}</section> : null}
        <p className="workspace-note">{w('sharedScopeHint')}</p>
        <details className="workspace-disclosure workspace-adjustments" open={intent && !preset.id ? true : undefined}><summary>{w('adjustApplication')}</summary>{memberEditor}</details>
      </fieldset> : record ? <><p className="workspace-note">{previewHint}</p><div className="workspace-members workspace-conflicts">{record.items.filter(i => record.remove || i.status === 'blocked').map((item, i) => <div className="workspace-member" key={`${item.skillId}-${i}`}><strong>{item.name} · {item.scope.kind === 'global' ? w('global') : pathName(item.scope.path)}</strong><p className="workspace-error">{itemReason(item, `actions.${item.action}`)}</p>{targetDetails(item)}</div>)}</div>{!record.remove && availableCount > 0 ? <p className="workspace-note">{w('availableCount', { count: availableCount })}</p> : null}</> : null}
    </WorkspaceDialog> : null}
    {view === 'undo' && record ? <WorkspaceDialog returnFocusRef={dialogReturnFocus} title={w('undo')} closeLabel={w('close')} busy={busy} onClose={back} footer={<><button className="btn btn-secondary" disabled={busy} onClick={back}>{w('cancel')}</button><button className="btn btn-danger" disabled={busy || !undo.some(i => i.canRemove)} onClick={() => void run(async () => { const result = await invokeTauri<WorkspaceUndoItem[]>('workspace_undo', { id: record.id }); setUndo(result); setUndoFeedback({ id: record.id, removed: result.filter(item => item.canRemove).length, retained: result.filter(item => !item.canRemove && item.reason !== 'alreadyUndone').length }); setRecord({ ...record, undoTargetCount: result.filter(item => !item.canRemove && item.reason !== 'alreadyUndone').length }); setView('result'); const latest = (await invokeTauri<ApplicationRecord[]>('workspace_history')).find(r => r.id === record.id); if (latest) setRecord(latest); await onChanged(); await reload() })}>{w('confirmUndo')}</button></>}>
      {alert}<p className="workspace-note">{w('undoHint')}</p>
      {undo.some(i => i.canRemove) ? <div className="workspace-members workspace-undo-list">{undo.filter(i => i.canRemove).map(item => <div className="workspace-member" key={item.targetId}><strong>{pathName(item.path)}</strong><div className="workspace-tools">{toolBadge(item.tool)}</div><details className="workspace-disclosure"><summary>{w('deploymentDetails')}</summary><p className="workspace-path">{item.path}</p></details></div>)}</div> : <p className="workspace-note">{w('nothingToUndo')}</p>}
      {undo.some(i => !i.canRemove && i.reason !== 'alreadyUndone') ? <section className="workspace-section workspace-undo-list"><h2>{w('protectedTargets')}</h2>{undo.filter(i => !i.canRemove && i.reason !== 'alreadyUndone').map(item => <div className="workspace-member" key={item.targetId}><strong>{pathName(item.path)}</strong><div className="workspace-tools">{toolBadge(item.tool)}</div><p className="workspace-note">{w(`undoReasons.${item.reason}`)}</p><details className="workspace-disclosure"><summary>{w('deploymentDetails')}</summary><p className="workspace-path">{item.path}</p></details></div>)}</section> : null}
      {undo.some(i => i.reason === 'alreadyUndone') ? <details className="workspace-disclosure"><summary>{w('alreadyUndoneCount', { count: undo.filter(i => i.reason === 'alreadyUndone').length })}</summary>{undo.filter(i => i.reason === 'alreadyUndone').map(item => <p key={item.targetId} className="workspace-path">{item.path}</p>)}</details> : null}
    </WorkspaceDialog> : null}
    <ConfirmActionModal open={confirm !== null} loading={busy} returnFocusRef={confirm === 'delete' ? deleteButton : confirm === 'latest' ? latestButton : backButton} title={w(confirm === 'delete' ? 'delete' : confirm === 'latest' ? 'loadLatest' : 'discard')} body={w(confirm === 'delete' ? 'deleteHint' : confirm === 'latest' ? 'loadLatestHint' : 'discardHint')} cancelLabel={w('cancel')} confirmLabel={w('confirm')} onRequestClose={() => setConfirm(null)} onConfirm={() => { if (confirm === 'latest') void run(loadLatest); else if (confirm === 'delete') void run(async () => { await invokeTauri('workspace_delete_preset', { id: preset.id, revision: preset.revision }); await reload(); setConfirm(null); setView('list') }); else { setConfirm(null); if (savedPreset) setPreset(savedPreset); if (intent) onBack(); else setView(returnView) } }} />
  </>
}
