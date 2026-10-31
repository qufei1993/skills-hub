import { useEffect, useEffectEvent, useRef, useState, type RefObject } from 'react'
import type { TFunction } from 'i18next'
import { open } from '@tauri-apps/plugin-dialog'
import { toast } from 'sonner'
import { ArrowLeft, ArrowRight, CheckCircle2, ChevronRight, CircleAlert, Clock3, Folder, Layers2, Pencil, Plus, Search, Trash2 } from 'lucide-react'
import type { ApplicationRecord, ManagedSkill, PresetMember, SkillPreset, ToolOption, WorkspaceIntent, WorkspaceScope, WorkspaceUndoItem } from '../types'
import ToolIcon from '../ToolIcon'
import ConfirmActionModal from '../modals/ConfirmActionModal'
import WorkspaceDialog from './WorkspaceDialog'
import PresetMoreActions from './PresetMoreActions'
import './workspace.css'

type Props = {
  overlayOnly?: boolean; returnFocusRef?: RefObject<HTMLElement | null>
  onDirtyChange: (dirty: boolean) => void
  active: boolean; skills: ManagedSkill[]; tools: ToolOption[]; projects: string[]; intent?: WorkspaceIntent
  invokeTauri: <T>(command: string, args?: Record<string, unknown>) => Promise<T>
  onChanged: () => Promise<void>; onBack: () => void
  onRemoved?: () => void
  onProject: (scope: WorkspaceScope) => void; t: TFunction
}
type View = 'list' | 'edit' | 'select' | 'preview' | 'result' | 'undo' | 'removing'
const emptyPreset = (): SkillPreset => ({ id: '', name: '', description: '', members: [], revision: 0 })
const presetContent = (value: SkillPreset) => JSON.stringify({
  name: value.name, description: value.description,
  members: value.members.map(member => ({ skillId: member.skillId, tools: [...member.tools].sort() })).sort((a, b) => a.skillId.localeCompare(b.skillId)),
})

export default function PresetPage({ overlayOnly = false, returnFocusRef, onDirtyChange, active, skills, tools, projects, intent, invokeTauri, onChanged, onBack, onRemoved, onProject, t }: Props) {
  const immediateRemoval = Boolean(overlayOnly && intent?.remove && intent.scope)
  const [view, setView] = useState<View>(immediateRemoval ? 'removing' : intent?.mode ?? 'list')
  const [returnToSource, setReturnToSource] = useState(Boolean(intent))
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
  const [selectedOnly, setSelectedOnly] = useState(Boolean(overlayOnly && intent?.members.length))
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
  // null follows available tools until the user customizes the defaults.
  const [customDefaultTools, setCustomDefaultTools] = useState<string[] | null>(null)
  const [record, setRecord] = useState<ApplicationRecord | null>(null)
  const [undo, setUndo] = useState<WorkspaceUndoItem[]>([])
  const [undoFeedback, setUndoFeedback] = useState<{ id: string; removed: number; retained: number } | null>(null)
  const [busy, setBusy] = useState(false)
  const [copying, setCopying] = useState(false)
  const busyRef = useRef(false)
  const removalStarted = useRef(false)
  const [error, setError] = useState('')
  const [deleteTarget, setDeleteTarget] = useState<SkillPreset | null>(null)
  const [confirm, setConfirm] = useState<null | 'discard' | 'delete' | 'latest'>(null)
  const previousView = useRef(view)
  const heading = useRef<HTMLHeadingElement>(null)
  const body = useRef<HTMLDivElement>(null)
  const backButton = useRef<HTMLButtonElement>(null)
  const deleteButton = useRef<HTMLButtonElement>(null)
  const deleteCompleted = useRef(false)
  const latestButton = useRef<HTMLButtonElement>(null)
  const dialogReturnFocus = useRef<HTMLElement | null>(returnFocusRef?.current ?? null)
  const remove = intent?.remove ?? false
  const repairing = overlayOnly && intent?.operation === 'repair'
  const directRemoval = remove && Boolean(intent?.scope)
  const scopedConfirmation = directRemoval || (overlayOnly && Boolean(intent?.scope) && (intent?.operation === 'tool' || intent?.operation === 'repair'))
  const w = (key: string, options?: Record<string, unknown>) => t(`workspace110.${key}`, options)
  const fail = (e: unknown) => {
    const text = String(e instanceof Error ? e.message : e)
    if (text.includes('preset name already exists')) { if (view === 'edit') setNameError(w('duplicateName')); else setError(w('duplicateName')); return }
    if (view === 'edit' && text.includes('PLAN_STALE')) { setSaveConflict(true); setError(w('presetConflict')); return }
    setError(text.includes('PLAN_STALE') ? w('stale') : text.includes('INVALID_ARGUMENT') ? w('invalid') : w('failed'))
  }
  const run = async (work: () => Promise<void>) => {
    if (busyRef.current) return
    busyRef.current = true; setBusy(true); setError('')
    try { await work() } catch (e) { fail(e) } finally { busyRef.current = false; setBusy(false) }
  }
  const startRemoval = useEffectEvent(async () => {
    const notification = toast.loading(w('working'))
    try { await preview() }
    catch (e) { fail(e); setView(current => current === 'removing' ? 'select' : current) }
    finally { toast.dismiss(notification) }
  })
  useEffect(() => {
    if (!active || !immediateRemoval || removalStarted.current) return
    // The library click authorizes this scoped operation. Strict Mode must not
    // create a second removal journal or repeat a write.
    removalStarted.current = true
    void startRemoval()
  }, [active, immediateRemoval])
  const reload = async () => {
    if (overlayOnly) return
    setLoadError(false)
    try { setPresets(await invokeTauri<SkillPreset[]>('workspace_presets')); setLoaded(true) }
    catch { setLoadError(true) }
    setHistoryLoaded(false)
  }
  useEffect(() => {
    if (!active || overlayOnly) return
    let cancelled = false
    invokeTauri<SkillPreset[]>('workspace_presets').then(values => {
      if (!cancelled) { setPresets(values); setLoaded(true); setLoadError(false) }
    }).catch(() => { if (!cancelled) setLoadError(true) })
    return () => { cancelled = true }
  }, [active, overlayOnly, invokeTauri])
  const needsHistory = !overlayOnly && (view === 'result' || (view === 'list' && tab === 'history'))
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
    if ((['select', 'preview'].includes(previous) && view === 'list') || (previous === 'undo' && view === 'result' && (record?.undoTargetCount ?? 0) > 0) || (previous === 'preview' && previewOrigin === 'result' && view === 'result')) return
    if (body.current) body.current.scrollTop = 0
    heading.current?.focus()
  }, [view, active, previewOrigin, record?.undoTargetCount])
  useEffect(() => {
    if (confirm === null && deleteCompleted.current) {
      deleteCompleted.current = false
      heading.current?.focus()
    }
  }, [confirm])
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
  const defaultTools = customDefaultTools ?? tools.map(tool => tool.id)
  const members = preset.members
  const repairMembers = repairing ? members.map(member => ({ ...member, tools: member.tools.filter(tool => skills.find(skill => skill.id === member.skillId)?.targets.some(target => target.tool === tool && target.status !== 'ok' && target.status !== 'disabled' && (scope === 'global' ? target.scope === 'global' : target.scope === 'project' && selectedProjects.includes(target.project_path ?? '')))) })).filter(member => member.tools.length > 0) : members
  const filteredSkills = skills.filter(s => (!selectedOnly || members.some(m => m.skillId === s.id)) && `${s.name} ${s.description ?? ''}`.toLowerCase().includes(memberQuery.toLowerCase()))
  const selectedResultCount = filteredSkills.filter(skill => members.some(member => member.skillId === skill.id)).length
  const allResultsSelected = filteredSkills.length > 0 && selectedResultCount === filteredSkills.length
  const someResultsSelected = selectedResultCount > 0 && !allResultsSelected
  const setMembers = (next: PresetMember[]) => setPreset(p => ({ ...p, members: next }))
  const toggleResults = () => {
    if (allResultsSelected) {
      const resultIds = new Set(filteredSkills.map(skill => skill.id))
      setMembers(members.filter(member => !resultIds.has(member.skillId)))
    } else {
      setMembers([...members, ...filteredSkills.filter(skill => !members.some(member => member.skillId === skill.id)).map(skill => ({ skillId: skill.id, tools: [...defaultTools] }))])
    }
  }
  const toggleMember = (id: string) => setMembers(members.some(m => m.skillId === id) ? members.filter(m => m.skillId !== id) : [...members, { skillId: id, tools: [...defaultTools] }])
  const toggleDefaultTool = (id: string) => setCustomDefaultTools(values => {
    const selected = values ?? tools.map(tool => tool.id)
    return selected.includes(id) ? selected.filter(value => value !== id) : [...selected, id]
  })
  const toggleTool = (id: string, tool: string) => setMembers(members.map(m => m.skillId !== id ? m : { ...m, tools: m.tools.includes(tool) ? m.tools.filter(v => v !== tool) : [...m.tools, tool] }))
  const requestConfirmation = (kind: 'discard' | 'delete' | 'latest') => {
    // macOS pointer clicks may leave focus outside the trigger button.
    const trigger = kind === 'delete' ? deleteButton.current : kind === 'latest' ? latestButton.current : backButton.current
    trigger?.focus({ preventScroll: true })
    if (kind === 'delete') setError('')
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
    if ((intent && view === 'list') || (returnToSource && (view === intent?.mode || view === 'result'))) { onBack(); return }
    if ((view === 'select' || view === 'result') && savedPreset) setPreset(savedPreset)
    setView('list')
  }
  const resetSelectionControls = () => {
    setMemberQuery(''); setSelectedOnly(false); setOverrideTools(false); setCommonTools([]); setCustomDefaultTools(null); setError(''); setNameError(''); setSaveConflict(false); setRecord(null)
  }
  const startEdit = (value: SkillPreset) => {
    resetSelectionControls()
    setReturnToSource(false)
    setSavedPreset(value.id ? value : null)
    setEditBaseline(presetContent(value))
    setSelectedOnly(value.members.length > 0)
    setPreset(structuredClone(value))
    setView('edit')
  }
  const startApply = (value: SkillPreset) => {
    resetSelectionControls()
    setReturnToSource(false)
    setSelectedOnly(true)
    setScope(intent?.scope?.kind ?? '')
    setSelectedProjects(intent?.scope?.kind === 'project' ? [intent.scope.path] : [])
    setProjectQuery(''); setExtraProjects([])
    setPreset(structuredClone(value)); setSavedPreset(value.id ? value : null)
    setView('select')
  }
  const execute = async (plan: ApplicationRecord) => {
    const finish = async (result: ApplicationRecord) => {
      setRecord(result)
      if (immediateRemoval && result.confirmed && result.items.every(i => ['done', 'undone'].includes(i.status))) {
        try { await onChanged(); await reload() }
        catch (e) { setView('result'); throw e }
        onRemoved?.(); onBack()
      } else {
        setView('result')
        await onChanged(); await reload()
      }
    }
    // The backend revalidates the preview before touching any deployment.
    let result: ApplicationRecord
    try { result = await invokeTauri<ApplicationRecord>('workspace_execute', { id: plan.id }) }
    catch {
      // A rejected transport call may still have committed some targets. Recover
      // this journal instead of creating a new application that claims them again.
      let latest = plan
      try { latest = (await invokeTauri<ApplicationRecord[]>('workspace_history')).find(r => r.id === plan.id) ?? plan } catch { /* Keep the known journal ID for explicit recovery. */ }
      if (latest.confirmed && latest.items.every(i => ['done', 'undone'].includes(i.status))) await finish(latest)
      else { setRecord(latest); setView('result'); setError(w('executionUnknown')) }
      return
    }
    await finish(result)
  }
  const preview = async () => {
    const chosen = repairing ? repairMembers : overrideTools ? members.map(m => ({ ...m, tools: commonTools })) : members
    if (!scope || !chosen.length || chosen.some(m => !m.tools.length) || (scope === 'project' && !selectedProjects.length)) { setError(w('invalid')); return }
    const result = await invokeTauri<ApplicationRecord>('workspace_preview', { request: { presetId: preset.id || null, members: chosen, scopes: scope === 'global' ? [{ kind: 'global' }] : selectedProjects.map(path => ({ kind: 'project', path })), remove } })
    setRecord(result); setPreviewOrigin('select')
    const hasBlocked = result.items.some(item => item.status === 'blocked' || item.action === 'blocked')
    const hasAvailable = result.items.some(item => ['pending', 'failed'].includes(item.status) && item.action !== 'blocked')
    if ((directRemoval || (overlayOnly && remove)) && !hasBlocked && hasAvailable) await execute(result)
    else if (remove || hasBlocked) setView('preview')
    else await execute(result)
  }
  const openRecord = (r: ApplicationRecord) => { setRecord(r); setView('result') }
  const save = async (value: SkillPreset) => {
    if (view === 'edit' && presets.some(p => p.id !== value.id && p.name.trim().toLowerCase() === value.name.trim().toLowerCase())) { setNameError(w('duplicateName')); return }
    const saved = await invokeTauri<SkillPreset>('workspace_save_preset', { preset: value })
    setPreset(saved); setSavedPreset(saved); setSaveConflict(false); setEditBaseline(presetContent(saved)); setView('list'); setReturnToSource(false); setTab('presets'); setQuery('')
    await reload()
  }
  const saveCopy = async (source = preset) => {
    const latest = await invokeTauri<SkillPreset[]>('workspace_presets')
    const base = w('copyName', { name: source.name })
    let name = base; let number = 2
    while (latest.some(value => value.name.toLowerCase() === name.toLowerCase())) name = `${base} ${number++}`
    await save({ ...source, name, id: '', revision: 0 })
  }
  const copyFromList = async (source: SkillPreset) => {
    if (busyRef.current) return
    setCopying(true)
    try { await run(() => saveCopy(source)) } finally { setCopying(false) }
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
    : !scope ? w('needScope') : scope === 'project' && !selectedProjects.length ? w('needProjects') : !members.length ? w('needMembers') : repairing && !repairMembers.length ? w('noRepairTargets') : missingTools ? w('needTools') : ''
  const modalOpen = ['select', 'preview', 'undo'].includes(view)
  const pageView = view === 'undo' || (view === 'preview' && previewOrigin === 'result') ? 'result' : modalOpen ? 'list' : view
  const title = pageView === 'list' ? w('title') : pageView === 'edit' && !preset.id ? w('new') : w(pageView)
  const chosenProjects = [...new Set([...projects, ...extraProjects, ...selectedProjects])].filter(p => p.toLowerCase().includes(projectQuery.toLowerCase()))
  const toolIdentity = (id: string): ToolOption => tools.find(tool => tool.id === id) ?? { id, label: id }
  const toolBadge = (id: string, iconOnly = false) => {
    const tool = toolIdentity(id)
    return <span key={id} className={iconOnly ? 'workspace-tool-avatar' : 'workspace-tool-badge'} title={tool.label}>
      <ToolIcon toolKey={id} label={tool.label} avatar={tool.avatar} />
      <span className={iconOnly ? 'sr-only' : undefined}>{tool.label}</span>
    </span>
  }
  // Compact identity for scanning: a few icons plus an overflow count whose title names the rest.
  const toolStack = (ids: string[], limit = 4) => {
    const rest = ids.slice(limit)
    return <span className="workspace-tool-stack">
      {ids.slice(0, limit).map(id => toolBadge(id, true))}
      {rest.length ? <span className="workspace-tool-more" title={rest.map(id => toolIdentity(id).label).join(', ')}>+{rest.length}</span> : null}
    </span>
  }
  const selectableTools = (selected: string[]): ToolOption[] => [...tools, ...selected.filter(id => !tools.some(tool => tool.id === id)).map(id => ({ id, label: id }))]
  const toolSelector = (selected: string[], toggle: (id: string) => void, label: string) => <div className="workspace-tool-choices" role="group" aria-label={label}>{selectableTools(selected).map(tool => <label key={tool.id}><input type="checkbox" checked={selected.includes(tool.id)} onChange={() => toggle(tool.id)} disabled={busy} /><ToolIcon toolKey={tool.id} label={tool.label} avatar={tool.avatar} /><span>{tool.label}</span></label>)}</div>
  const undoScopeLabel = (scope: WorkspaceScope) => scope.kind === 'global' ? w('global') : `${t('scope.project')} · ${pathName(scope.path)}`
  const removableUndo = undo.filter(item => item.canRemove)
  const retainedUndo = undo.filter(item => !item.canRemove && item.reason !== 'alreadyUndone')
  const completedUndo = undo.filter(item => item.reason === 'alreadyUndone')
  const undoSkillCount = new Set([...removableUndo, ...retainedUndo].map(item => item.skillId)).size
  const undoList = (items: WorkspaceUndoItem[]) => {
    const groups = new Map<string, WorkspaceUndoItem[]>()
    for (const item of items) {
      const group = groups.get(item.skillId) ?? []
      group.push(item)
      groups.set(item.skillId, group)
    }
    return <div className="workspace-undo-groups">{[...groups].map(([skillId, targets]) => {
      const name = record?.items.find(item => item.skillId === skillId)?.name ?? skills.find(skill => skill.id === skillId)?.name ?? skillId
      const locations = new Map<string, WorkspaceUndoItem[]>()
      for (const target of targets) {
        const key = JSON.stringify([target.scope.kind, target.scope.kind === 'project' ? target.scope.path : null, target.reason])
        const group = locations.get(key) ?? []
        group.push(target)
        locations.set(key, group)
      }
      return <section className="workspace-undo-group" key={skillId} aria-label={name}>
        <header><div className="workspace-undo-identity"><h3>{name}</h3>{locations.size === 1 ? <span className="workspace-undo-scope">{undoScopeLabel(targets[0].scope)}</span> : null}</div><span className="workspace-count">{w('deploymentCount', { count: targets.length })}</span></header>
        <ul>{[...locations].map(([key, location]) => {
          const item = location[0]
          return <li key={key} className="workspace-undo-location">
            {locations.size > 1 ? <span className="workspace-undo-scope">{undoScopeLabel(item.scope)}</span> : null}
            <div className="workspace-undo-tools">{[...new Set(location.map(target => target.tool))].map(tool => toolBadge(tool))}</div>
            {!item.canRemove && item.reason !== 'alreadyUndone' ? <p className="workspace-undo-reason">{w(`undoReasons.${item.reason}`)}</p> : null}
          </li>
        })}</ul>
      </section>
    })}</div>
  }
  const pathName = (path: string) => path.split(/[\\/]/).filter(Boolean).at(-1) ?? path
  const scopeName = (value: WorkspaceScope) => value.kind === 'global' ? w('global') : value.path
  const historyCard = (r: ApplicationRecord) => <button key={r.id} className="workspace-preset workspace-history-record" onClick={() => openRecord(r)}>
    <span className="workspace-preset-main">
      <strong>{r.preset?.name ?? w(r.remove ? 'manualRemove' : 'manual')}</strong>
      <span className="workspace-record-scopes">{[...new Set(r.items.map(i => scopeName(i.scope)))].map(path => <span key={path} title={path}><Folder size={14} aria-hidden="true" />{pathName(path)}</span>)}</span>
    </span>
    {summary(r.items, true)}
    <span className="workspace-preset-meta"><time>{new Date(r.createdAt).toLocaleString()}</time></span><ChevronRight size={16} aria-hidden="true" />
  </button>
  const targetMemberList = (values: PresetMember[]) => <div className="workspace-target-members">{values.map(member => <div className="workspace-target-member" key={member.skillId}><strong>{skills.find(skill => skill.id === member.skillId)?.name ?? member.skillId}</strong><div className="workspace-tools">{member.tools.map(tool => toolBadge(tool))}</div></div>)}</div>
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
      <div className="workspace-group-heading"><div><h2 title={path}>{targetScope.kind === 'global' ? w('global') : pathName(path)}</h2>{summary(items, true)}</div><button className="btn workspace-text-action" onClick={() => onProject(targetScope)}>{w('viewTarget')}<ArrowRight size={14} /></button></div>
      <div className="workspace-result-list" role="list" aria-label={w('membersTitle')}>{items.map((item, i) => <div key={`${item.skillId}-${i}`} className="workspace-result-row" role="listitem">
        <strong>{item.name}</strong>
        <div className="workspace-tools workspace-result-tools">{item.tools.map(id => toolBadge(id))}</div>
        <div className="workspace-result-status"><span className="workspace-meta">{w(`actions.${item.action}`)}</span><span className="workspace-status" data-state={item.status}>{w(`states.${item.status}`)}</span></div>
        {(!['done', 'undone'].includes(item.status) || item.conflict || item.targets.length > 0) ? <div className="workspace-result-detail">{!['done', 'undone'].includes(item.status) ? <p className="workspace-error">{itemReason(item)}</p> : null}{targetDetails(item)}</div> : null}
      </div>)}</div>
    </section>
  })}</div>
  const resultHasIssues = record?.items.some(item => !['done', 'undone'].includes(item.status)) ?? false
  const completion = record ? <div className="workspace-completion" data-state={resultHasIssues ? 'warning' : 'success'}>{resultHasIssues ? <CircleAlert size={18} aria-hidden="true" /> : <CheckCircle2 size={18} aria-hidden="true" />}<strong>{w(record.items.length && record.items.every(item => item.status === 'undone') ? 'undoComplete' : record.remove ? resultHasIssues ? 'removeCompletedWithIssues' : 'removeComplete' : resultHasIssues ? 'completedWithIssues' : 'completed')}</strong></div> : null
  const historyFeedback = historyError ? <div className="workspace-alert workspace-error" role="alert">{w('historyFailed')}<button className="btn btn-secondary" disabled={busy} onClick={() => void run(async () => { try { setHistory(await invokeTauri<ApplicationRecord[]>('workspace_history')); setHistoryLoaded(true); setHistoryError(false) } catch { setHistoryError(true) } })}>{w('reload')}</button></div> : !historyLoaded ? <p className="workspace-note" role="status">{w('historyLoading')}</p> : null
  const alert = error ? <div className="workspace-alert workspace-error" role="alert"><span>{error}</span>{saveConflict && view === 'edit' ? <div className="workspace-actions"><button className="btn btn-secondary" disabled={busy || Boolean(prerequisite)} onClick={() => void run(() => saveCopy())}>{w('saveCopy')}</button><button ref={latestButton} className="btn btn-secondary" disabled={busy} onClick={() => requestConfirmation('latest')}>{w('loadLatest')}</button></div> : null}</div> : null
  const memberEditor = <section className="workspace-section workspace-member-editor">
    <div className="workspace-section-heading"><h2>{w('membersTitle')}</h2><span className="workspace-count">{w('memberCount', { count: members.length })}</span></div>
    {view === 'select' && !overlayOnly ? <label className="workspace-check"><input type="checkbox" checked={overrideTools} onChange={e => setOverrideTools(e.target.checked)} />{w('override')}</label> : null}
    {!overlayOnly && (overrideTools || !selectedOnly) ? <div className="workspace-default-tools"><span>{w(overrideTools ? 'commonTools' : 'defaultTools')}</span>{toolSelector(overrideTools ? commonTools : defaultTools, id => { if (overrideTools) setCommonTools(values => values.includes(id) ? values.filter(value => value !== id) : [...values, id]); else toggleDefaultTool(id) }, w(overrideTools ? 'commonTools' : 'defaultTools'))}</div> : null}
    <div className="workspace-toolbar"><label className="workspace-search"><Search size={16} /><input aria-label={w('searchMembers')} placeholder={w('searchMembers')} value={memberQuery} onChange={e => setMemberQuery(e.target.value)} /></label>{!overlayOnly ? <button className="btn btn-secondary" onClick={() => { setSelectedOnly(!selectedOnly); setMemberQuery('') }}>{w(selectedOnly ? 'addMembers' : 'doneSelecting')}</button> : null}</div>
    <div className="workspace-members">{!overlayOnly ? <div className="workspace-member-selection"><label className="workspace-select-results"><input type="checkbox" ref={element => { if (element) element.indeterminate = someResultsSelected }} checked={allResultsSelected} aria-checked={someResultsSelected ? 'mixed' : allResultsSelected} disabled={busy || filteredSkills.length === 0} onChange={toggleResults} /><span>{w(allResultsSelected ? 'clearResults' : 'selectResults')}</span></label><span className="workspace-count" role="status">{w('selectedResults', { selected: selectedResultCount, total: filteredSkills.length })}</span></div> : null}{filteredSkills.map(skill => { const member = members.find(m => m.skillId === skill.id); const MemberIdentity = overlayOnly ? 'div' : 'label'; return <div className={`workspace-member${member ? ' is-selected' : ''}`} key={skill.id}>
      <MemberIdentity className="workspace-member-label">{!overlayOnly ? <input type="checkbox" aria-label={skill.name} checked={Boolean(member)} onChange={() => toggleMember(skill.id)} /> : null}<span className="workspace-member-identity"><strong>{skill.name}</strong>{skill.description ? <small title={skill.description}>{skill.description}</small> : null}{!skill.enabled ? <small>{w('disabled')}</small> : null}</span></MemberIdentity>
      {member ? <div className="workspace-member-assignment">{!overrideTools ? <>{toolSelector(member.tools, tool => toggleTool(skill.id, tool), skill.name)}{!member.tools.length ? <span className="workspace-error">{w('chooseTools')}</span> : null}</> : <span className="workspace-note">{commonTools.map(id => toolIdentity(id).label).join(' · ') || w('chooseTools')}</span>}</div> : null}
    </div> })}{filteredSkills.length === 0 ? <p className="workspace-empty">{w('noResults')}</p> : null}</div>
    {members.filter(m => !skills.some(s => s.id === m.skillId)).map(m => <div className="workspace-member" key={m.skillId}><span>{w('missing')} · {m.skillId}</span><button className="btn btn-secondary" onClick={() => toggleMember(m.skillId)}>{w('removeMember')}</button></div>)}
  </section>
  const selectionTitle = remove ? directRemoval && intent?.scope?.kind === 'project' ? 'removeTargets' : 'confirmRemove' : overlayOnly ? intent?.operation === 'tool' ? 'syncToolTitle' : intent?.operation === 'repair' ? 'repairTargets' : 'manageTargets' : 'applyDialog'
  const selectionConfirm = remove ? 'confirmRemove' : overlayOnly ? intent?.operation === 'tool' ? 'syncToolConfirm' : intent?.operation === 'repair' ? 'repairToolConfirm' : 'confirmTargets' : 'applyNow'
  const appliedTools = [...new Set(overrideTools ? commonTools : members.flatMap(m => m.tools))]
  const closeDialog = () => leavePreview(() => {
    if (view === 'preview' && previewOrigin === 'result') setView('result')
    else if (returnToSource && view !== 'undo') onBack()
    else if (view === 'preview') {
      setError('')
      if (savedPreset) setPreset(savedPreset)
      setView('list')
    } else back()
  })
  return <>
    {!overlayOnly ? <section className={`workspace-page workspace-view-${pageView}`} inert={modalOpen || confirm !== null} aria-busy={busy || (!loaded && !loadError)}>
      <header className="workspace-heading">
        <div className="workspace-title">{pageView !== 'list' || intent ? <button ref={backButton} className="btn btn-secondary workspace-back" aria-label={w('back')} title={w('back')} onClick={back} disabled={busy}><ArrowLeft size={18} /></button> : <span className="workspace-page-symbol" aria-hidden="true"><Layers2 size={24} /></span>}<div><h1 tabIndex={-1} ref={heading}>{title}</h1><p>{pageView === 'result' && record ? <>{record.preset?.name ?? w(record.remove ? 'manualRemove' : 'manual')} · <time dateTime={new Date(record.createdAt).toISOString()}>{new Date(record.createdAt).toLocaleString()}</time></> : w(pageView === 'edit' ? 'editorIntro' : 'intro')}</p></div></div>
        <div className="workspace-actions">{pageView === 'result' ? completion : null}{pageView === 'list' ? <button className="btn btn-primary" disabled={busy} onClick={() => startEdit(emptyPreset())}><Plus size={16} />{w('new')}</button> : null}</div>
      </header>
      <div ref={body} className="workspace-body">
        {!modalOpen ? alert : null}
        {busy && !modalOpen ? <p className="workspace-note" role="status">{w('working')}</p> : null}
        {pageView === 'list' ? <>
          <div className="workspace-directory-controls"><div className="workspace-tabs" role="group" aria-label={w('title')}><button disabled={copying} aria-pressed={tab === 'presets'} onClick={() => setTab('presets')}><Layers2 size={16} />{w('presets')}{loaded ? <span className="workspace-tab-count" aria-hidden="true">{presets.length}</span> : null}</button><button disabled={copying} aria-pressed={tab === 'history'} onClick={() => setTab('history')}><Clock3 size={16} />{w('history')}</button></div><label className="workspace-search"><Search size={16} /><input disabled={copying} aria-label={w('search')} placeholder={w('search')} value={query} onChange={e => setQuery(e.target.value)} /></label></div>
          {tab === 'presets' ? <>
            {loadError ? <div role="alert" className="workspace-alert workspace-error">{w('failed')}<button className="btn btn-secondary" onClick={() => void run(reload)}>{w('reload')}</button></div> : !loaded ? <p className="workspace-note" role="status">{w('presetsLoading')}</p> : <>
              {filteredPresets.length ? <div className="workspace-preset-grid">{filteredPresets.map(p => <article key={p.id} className="workspace-preset-card">
                <div className="workspace-card-content">
                  <span className="workspace-card-title"><strong>{p.name}</strong><span className="workspace-card-meta">{w('memberCount', { count: p.members.length })} · {w('toolCount', { count: new Set(p.members.flatMap(m => m.tools)).size })}</span></span>
                  {p.description ? <span className="workspace-card-description">{p.description}</span> : null}
                  <span className="workspace-card-preview">{p.members.slice(0, 3).map(m => skills.find(skill => skill.id === m.skillId)?.name ?? w('missing')).join(' · ')}{p.members.length > 3 ? ` · +${p.members.length - 3}` : ''}</span>
                  <span className="workspace-card-tools">{toolStack([...new Set(p.members.flatMap(m => m.tools))])}</span>
                </div>
                <div className="workspace-card-footer"><button className="btn btn-secondary" aria-label={w('editNamed', { name: p.name })} disabled={busy} onClick={() => startEdit(p)}><Pencil size={14} aria-hidden="true" />{w('editAction')}</button><button className="btn btn-secondary" disabled={copying} aria-label={w('applyNamed', { name: p.name })} onClick={event => { dialogReturnFocus.current = event.currentTarget; event.currentTarget.focus(); startApply(p) }}>{w('apply')}<ArrowRight size={14} /></button><button className="btn workspace-icon-action workspace-delete-action" aria-label={w('deleteNamed', { name: p.name })} title={w('delete')} disabled={busy} onClick={event => { deleteButton.current = event.currentTarget; setDeleteTarget(p); requestConfirmation('delete') }}><Trash2 size={16} aria-hidden="true" /></button><PresetMoreActions name={p.name} busy={busy} onCopy={() => copyFromList(p)} t={t} /></div>
              </article>)}</div> : <div className="workspace-empty"><span className="workspace-empty-symbol" aria-hidden="true"><Layers2 size={20} /></span><p className="workspace-empty-title">{w(presets.length ? 'noMatches' : 'empty')}</p>{!presets.length ? <><p>{w('emptyHelp')}</p><button className="btn btn-primary" disabled={busy} onClick={() => startEdit(emptyPreset())}><Plus size={16} />{w('new')}</button></> : null}</div>}
            </>}
          </> : <><p className="workspace-note">{w('historyRetention')}</p>{historyFeedback}{historyLoaded ? filteredHistory.length ? <div className="workspace-list workspace-history-list">{filteredHistory.map(historyCard)}</div> : <div className="workspace-empty"><p>{w(history.length ? 'noMatches' : 'noHistory')}</p></div> : null}</>}
        </> : null}
        {view === 'edit' ? <fieldset disabled={busy} className="workspace-form workspace-editor"><section className="workspace-section workspace-editor-info"><div className="workspace-definition"><div className="workspace-name-field"><label className="workspace-field">{w('name')}<input value={preset.name} aria-invalid={Boolean(nameError)} aria-describedby={nameError ? 'workspace-name-error' : undefined} onChange={e => { setPreset(p => ({ ...p, name: e.target.value })); setNameError('') }} /></label>{nameError ? <p id="workspace-name-error" className="workspace-error" role="alert">{nameError}</p> : null}</div><label className="workspace-field">{w('description')}<textarea rows={2} value={preset.description} onChange={e => setPreset(p => ({ ...p, description: e.target.value }))} /></label></div><p className="workspace-definition-hint">{w('definitionOnly')}</p></section>{memberEditor}</fieldset> : null}
        {pageView === 'result' && record ? <>{undoFeedback?.id === record.id ? <p className="workspace-note" role="status">{w('undoSummary', { removed: undoFeedback.removed, retained: undoFeedback.retained })}</p> : null}{table(record)}</> : null}
      </div>
      {view === 'edit' ? <footer className="workspace-footer"><div className="workspace-footer-summary"><span>{w('memberCount', { count: members.length })}</span><p id="workspace-prerequisite">{prerequisite || w('saveHint')}</p></div><button className="btn btn-primary" aria-describedby="workspace-prerequisite" disabled={busy || Boolean(prerequisite) || saveConflict} onClick={() => void run(() => save(preset))}>{w('save')}</button></footer> : null}
      {pageView === 'result' && record ? <footer className="workspace-footer">{!record.remove && record.undoTargetCount > 0 ? <button className="btn btn-secondary" disabled={busy} onClick={event => { dialogReturnFocus.current = event.currentTarget; event.currentTarget.focus(); void run(async () => { setUndo(await invokeTauri<WorkspaceUndoItem[]>('workspace_preview_undo', { id: record.id })); setView('undo') }) }}>{w('undo')}</button> : null}{record.items.some(i => !['done', 'undone'].includes(i.status)) ? <button className="btn btn-secondary" disabled={busy} onClick={event => { dialogReturnFocus.current = event.currentTarget; event.currentTarget.focus(); void run(async () => { setRecord(await invokeTauri<ApplicationRecord>('workspace_recheck', { id: record.id })); setPreviewOrigin('result'); setView('preview') }) }}>{w('recheck')}</button> : null}<button className="btn btn-primary" disabled={busy} onClick={back}>{w('done')}</button></footer> : null}
    </section> : null}
    {overlayOnly && view === 'result' && record ? <WorkspaceDialog returnFocusRef={dialogReturnFocus} title={w(record.remove ? 'removeResult' : 'result')} closeLabel={w('close')} busy={busy} onClose={onBack} footer={<>{record.items.some(item => !['done', 'undone'].includes(item.status)) ? <button className="btn btn-secondary" disabled={busy} onClick={() => void run(async () => { setRecord(await invokeTauri<ApplicationRecord>('workspace_recheck', { id: record.id })); setPreviewOrigin('result'); setView('preview') })}>{w('recheck')}</button> : null}<button className="btn btn-primary" disabled={busy} onClick={onBack}>{w('done')}</button></>}>{alert}{completion}{table(record)}</WorkspaceDialog> : null}
    {view === 'select' || view === 'preview' ? <WorkspaceDialog returnFocusRef={dialogReturnFocus} title={w(view === 'preview' ? record?.remove ? previewBlocked ? 'nothingRemovable' : 'confirmRemove' : previewBlocked ? 'nothingApplicable' : 'reviewConflicts' : selectionTitle)} closeLabel={w('close')} busy={busy} onClose={closeDialog} footer={<>
      {view === 'select' && !scopedConfirmation ? <div className="workspace-footer-summary"><p id="workspace-apply-prerequisite">{prerequisite || w('applySafeHint')}</p></div> : null}
      <button className="btn btn-secondary" disabled={busy} onClick={closeDialog}>{w(view === 'preview' && previewOrigin === 'result' ? 'backToResult' : 'cancel')}</button>
      {view === 'preview' && previewOrigin === 'select' && !directRemoval ? <button className={`btn ${previewBlocked ? 'btn-primary' : 'btn-secondary'}`} disabled={busy} onClick={back}>{w('backToSelection')}</button> : null}
      {view === 'select' ? <button className={`btn ${remove ? 'btn-danger' : 'btn-primary'}`} aria-describedby={scopedConfirmation ? 'workspace-target-hint' : 'workspace-apply-prerequisite'} disabled={busy || Boolean(prerequisite)} onClick={() => void run(preview)}>{w(busy ? 'working' : selectionConfirm)}</button> : record && availableCount > 0 ? <button className={`btn ${record.remove ? 'btn-danger' : 'btn-primary'}`} disabled={busy} onClick={() => void run(() => execute(record))}>{w(record.remove ? 'confirmRemove' : 'applyAvailable')}</button> : null}
    </>}>
      {alert}
      {view === 'select' && scopedConfirmation ? <section className="workspace-target-confirmation">
        <p id="workspace-target-hint" className="workspace-note">{w(remove ? 'removeScopeHint' : 'syncToolHint')}</p>
        <div className="workspace-target-heading"><Folder size={17} aria-hidden="true" /><strong>{scope === 'project' ? selectedProjects.map(pathName).join(', ') : w('global')}</strong></div>
        {targetMemberList(repairing ? repairMembers : members)}
        {prerequisite ? <p className="workspace-note">{prerequisite}</p> : null}
      </section> : view === 'select' ? <fieldset disabled={busy} className="workspace-form">
        <div className="workspace-apply-summary">{overlayOnly ? <Folder size={20} /> : <Layers2 size={20} />}<div><strong>{overlayOnly && members.length === 1 ? skills.find(skill => skill.id === members[0].skillId)?.name ?? members[0].skillId : preset.name || w('manual')}</strong><p>{w('memberCount', { count: members.length })} · {w('toolCount', { count: appliedTools.length })}</p></div></div>
        <div className="workspace-scope-options" role="radiogroup" aria-label={w('scope')}>{(['global', 'project'] as const).map(value => <label key={value}><input type="radio" name="workspace-scope" aria-label={w(value)} checked={scope === value} onChange={() => setScope(value)} /><span><strong>{w(value)}</strong><small>{w(value === 'global' ? 'globalHint' : 'projectHint')}</small></span></label>)}</div>
        {scope === 'project' ? <section className="workspace-section"><div className="workspace-toolbar">{chosenProjects.length > 5 || projectQuery ? <label className="workspace-search"><Search size={16} /><input aria-label={w('searchProjects')} placeholder={w('searchProjects')} value={projectQuery} onChange={e => setProjectQuery(e.target.value)} /></label> : <span className="workspace-count">{w('projectCount', { count: selectedProjects.length })}</span>}<button className="btn btn-secondary" onClick={() => void run(async () => { const path = await open({ directory: true, multiple: false }); if (typeof path === 'string') { setExtraProjects(p => [...p, path]); setSelectedProjects(p => [...new Set([...p, path])]) } })}><Folder size={15} />{w('chooseProject')}</button></div>{chosenProjects.length ? <div className="workspace-projects">{chosenProjects.map(path => <label key={path}><input type="checkbox" aria-label={path} checked={selectedProjects.includes(path)} onChange={() => setSelectedProjects(p => p.includes(path) ? p.filter(v => v !== path) : repairing ? [path] : [...p, path])} /><span className="workspace-project-identity"><strong>{pathName(path)}</strong><span className="workspace-path" title={path}>{path}</span></span></label>)}</div> : <p className="workspace-note">{w('noProjectResults')}</p>}</section> : null}
        <p className="workspace-note">{w(repairing ? 'repairScopeHint' : overlayOnly && intent?.operation === 'targets' ? 'targetManagementHint' : 'sharedScopeHint')}</p>
        {overlayOnly ? intent?.operation === 'tool' || repairing ? targetMemberList(repairing && scope ? repairMembers : members) : memberEditor : <details className="workspace-disclosure workspace-adjustments" open={intent && !preset.id ? true : undefined}><summary>{w('adjustApplication')}</summary>{memberEditor}</details>}
      </fieldset> : record ? <><p className="workspace-note">{previewHint}</p><div className="workspace-members workspace-conflicts">{record.items.filter(i => record.remove || i.status === 'blocked').map((item, i) => <div className="workspace-member" key={`${item.skillId}-${i}`}><strong>{item.name} · {item.scope.kind === 'global' ? w('global') : pathName(item.scope.path)}</strong><p className="workspace-error">{itemReason(item, `actions.${item.action}`)}</p>{targetDetails(item)}</div>)}</div>{!record.remove && availableCount > 0 ? <p className="workspace-note">{w('availableCount', { count: availableCount })}</p> : null}</> : null}
    </WorkspaceDialog> : null}
    {view === 'undo' && record ? <WorkspaceDialog returnFocusRef={dialogReturnFocus} title={w('undo')} closeLabel={w('close')} busy={busy} onClose={back} footer={<><button className="btn btn-secondary" disabled={busy} onClick={back}>{w('cancel')}</button><button className="btn btn-danger" disabled={busy || !undo.some(i => i.canRemove)} onClick={() => void run(async () => { const result = await invokeTauri<WorkspaceUndoItem[]>('workspace_undo', { id: record.id }); setUndo(result); setUndoFeedback({ id: record.id, removed: result.filter(item => item.canRemove).length, retained: result.filter(item => !item.canRemove && item.reason !== 'alreadyUndone').length }); setRecord({ ...record, undoTargetCount: result.filter(item => !item.canRemove && item.reason !== 'alreadyUndone').length }); setView('result'); const latest = (await invokeTauri<ApplicationRecord[]>('workspace_history')).find(r => r.id === record.id); if (latest) setRecord(latest); await onChanged(); await reload() })}>{w('confirmUndo')}</button></>}>
      {alert}<p className="workspace-note">{w('undoHint')}</p>
      {undo.length ? <p className="workspace-undo-overview">{w('undoPreviewSummary', { skills: undoSkillCount, removable: removableUndo.length })}{retainedUndo.length ? <span> · {w('undoRetainedCount', { count: retainedUndo.length })}</span> : null}</p> : null}
      {removableUndo.length ? <section className="workspace-undo-section" aria-label={w('undoTargets')}><h2>{w('undoTargets')}</h2>{undoList(removableUndo)}</section> : <p className="workspace-note">{w('nothingToUndo')}</p>}
      {retainedUndo.length ? <section className="workspace-undo-section" aria-label={w('protectedTargets')}><h2>{w('protectedTargets')}</h2>{undoList(retainedUndo)}</section> : null}
      {completedUndo.length ? <details className="workspace-disclosure workspace-undo-completed"><summary>{w('alreadyUndoneCount', { count: completedUndo.length })}</summary>{undoList(completedUndo)}</details> : null}
    </WorkspaceDialog> : null}
    <ConfirmActionModal open={confirm !== null} loading={busy} returnFocusRef={confirm === 'delete' ? deleteButton : confirm === 'latest' ? latestButton : backButton} title={w(confirm === 'delete' ? 'delete' : confirm === 'latest' ? 'loadLatest' : 'discard')} body={<>{confirm === 'delete' && deleteTarget ? <strong className="workspace-delete-name">{deleteTarget.name}</strong> : null}{w(confirm === 'delete' ? 'deleteHint' : confirm === 'latest' ? 'loadLatestHint' : 'discardHint')}{confirm === 'delete' && error ? <p className="workspace-error" role="alert">{error}</p> : null}</>} cancelLabel={w('cancel')} confirmLabel={w('confirm')} onRequestClose={() => setConfirm(null)} onConfirm={() => { if (confirm === 'latest') void run(loadLatest); else if (confirm === 'delete' && deleteTarget) void run(async () => { await invokeTauri('workspace_delete_preset', { id: deleteTarget.id, revision: deleteTarget.revision }); await reload(); deleteCompleted.current = true; setConfirm(null); setDeleteTarget(null) }); else { setConfirm(null); if (savedPreset) setPreset(savedPreset); if (returnToSource) onBack(); else setView('list') } }} />
  </>
}
