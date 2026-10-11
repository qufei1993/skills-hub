import InstallFlowSteps from './InstallFlowSteps'
import InstallSettingsSections from './InstallSettingsSections'
import { memo, useEffect, useId, useRef, useState } from 'react'
import { Check } from 'lucide-react'
import SkillSelectionList from './SkillSelectionList'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import ConfirmActionModal from './ConfirmActionModal'
import ScopeSelector from '../ScopeSelector'
import ToolIcon from '../ToolIcon'
import { isToolUnsupportedForScope, normalizeProjectPaths, normalizeProjectSharedTargets, type InstallScope } from '../installScope'
import { installCollectionEntry, type CollectionInvoke } from '../collectionInstall'
import type { CollectionManifest, CollectionResult, GitSkillCandidate, InstallResultDto, TagDto, ToolOption } from '../types'

type CollectionInstallModalProps = {
  open?: boolean
  manifest: CollectionManifest
  tools: ToolOption[]
  tags?: TagDto[]
  recentProjects?: string[]
  onPickProject?: () => Promise<string | undefined>
  sharedTools?: Record<string, string[]>
  sharedProjectTools?: Record<string, string[]>
  invoke: CollectionInvoke
  onComplete: () => Promise<void>
  onClose: () => void
}

export default memo(function CollectionInstallModal({ open = true, manifest, tools, tags = [], recentProjects = [], onPickProject = async () => undefined, sharedTools = {}, sharedProjectTools = {}, invoke, onComplete, onClose }: CollectionInstallModalProps) {
  const { t } = useTranslation()
  const tagInputId = useId()
  const [query, setQuery] = useState('')
  const [step, setStep] = useState<1 | 2>(1)
  const [selectedSkills, setSelectedSkills] = useState(() => manifest.skills.map((_, index) => index))
  const [selectedTools, setSelectedTools] = useState<string[]>([])
  const [selectedTags, setSelectedTags] = useState<number[]>([])
  const [newTag, setNewTag] = useState('')
  const [scope, setScope] = useState<InstallScope>('global')
  const [projects, setProjects] = useState<string[]>([])
  const [results, setResults] = useState<Record<number, CollectionResult>>({})
  const [running, setRunning] = useState(false)
  const [started, setStarted] = useState(false)
  const [active, setActive] = useState<number | null>(null)
  const [error, setError] = useState('')
  const [preview, setPreview] = useState<Record<number, GitSkillCandidate>>({})
  const eligibleSkills = manifest.skills.map((_, index) => index).filter(index => preview[index]?.status !== 'conflict')
  const installSelection = selectedSkills.filter(index => eligibleSkills.includes(index))
  const selectedCount = installSelection.length
  const [checking, setChecking] = useState(true)
  const [previewError, setPreviewError] = useState(false)
  const [previewAttempt, setPreviewAttempt] = useState(0)
  useEffect(() => {
    let cancelled = false
    setChecking(true)
    setPreviewError(false)
    const inspect = async () => {
      try {
        const next: Record<number, GitSkillCandidate> = {}
        for (const [sourceIndex, source] of manifest.sources.entries()) {
          const entries = manifest.skills.map((skill, index) => ({ skill, index })).filter(({ skill }) => skill.source === sourceIndex)
          const candidates = await invoke<GitSkillCandidate[]>('preview_git_skills_local', {
            repoUrl: `https://github.com/${source.repo}/tree/${source.ref}`,
            candidates: entries.map(({ skill }) => ({ name: skill.name, subpath: skill.path, description: null })),
          })
          for (const { skill, index } of entries) {
            const candidate = candidates.find(item => item.name === skill.name && item.subpath === skill.path)
            if (candidate) next[index] = candidate
          }
        }
        if (!cancelled) {
          setPreview(next)
          setSelectedSkills(current => current.filter(index => next[index]?.status !== 'conflict'))
        }
      } catch {
        if (!cancelled) setPreviewError(true)
      } finally {
        if (!cancelled) setChecking(false)
      }
    }
    void inspect()
    return () => { cancelled = true }
  }, [manifest, invoke, previewAttempt])
  const busy = useRef(false)
  const createdTag = useRef<TagDto | null>(null)
  const installed = useRef<Record<number, InstallResultDto>>({})
  const close = () => { if (!busy.current) onClose() }
  const missingProject = selectedTools.length > 0 && scope === 'project' && normalizeProjectPaths(projects).length === 0
  const locked = running || started
  const run = async () => {
    if (busy.current || checking || previewError || missingProject || selectedCount === 0) return
    busy.current = true
    setRunning(true)
    setError('')
    let succeeded = false
    let hasFailures = false
    try {
      const tagIds = [...selectedTags]
      if (newTag.trim()) {
        const tag = createdTag.current ?? tags.find(item => item.name.toLowerCase() === newTag.trim().toLowerCase()) ?? await invoke<TagDto>('create_tag', { name: newTag.trim() })
        createdTag.current = tag
        tagIds.push(tag.id)
      }
      setQuery('')
      setStarted(true)
      const candidates = new Map<string, GitSkillCandidate[]>()
      for (const index of installSelection) {
        if (results[index] && results[index].state !== 'failed') continue
        setActive(index)
        try {
          const result = await installCollectionEntry(invoke, manifest, index, selectedTools, installed.current[index], created => { installed.current[index] = created }, { tagIds, scope, projects, candidates })
          setResults(previous => ({ ...previous, [index]: result }))
        } catch (failure) {
          hasFailures = true
          const raw = failure instanceof Error ? failure.message : String(failure)
          const message = raw.startsWith('collectionInstall.') || raw.startsWith('projectSync.') ? t(raw) : raw
          setResults(previous => ({ ...previous, [index]: { state: 'failed', message } }))
        }
      }
      succeeded = !hasFailures
    } catch (failure) {
      setError(failure instanceof Error ? failure.message : String(failure))
    } finally {
      try { await onComplete() } catch { toast.error(t('collectionInstall.refreshFailed')) }
      setActive(null)
      setRunning(false)
      busy.current = false
      if (succeeded) {
        onClose()
        toast.success(t('collectionInstall.success', { count: selectedCount }))
      }
    }
  }
  const completed = selectedCount > 0 && installSelection.every(index => results[index])
  const failed = Object.values(results).filter(result => result.state === 'failed').length
  const finished = Object.values(results).filter(result => result.state !== 'failed').length
  const search = query.trim().toLowerCase()
  const visible = manifest.skills.map((skill, index) => ({ skill, index })).filter(({ skill }) => `${skill.name} ${skill.path} ${manifest.sources[skill.source].repo}`.toLowerCase().includes(search))
  const changeTool = (id: string, checked: boolean) => {
    const shared = (scope === 'project' ? sharedProjectTools : sharedTools)[id] ?? [id]
    const affected = tools.filter(tool => shared.includes(tool.id) && !isToolUnsupportedForScope(tool, scope)).map(tool => tool.id)
    setSelectedTools(current => checked ? [...new Set([...current, ...affected])] : current.filter(tool => !affected.includes(tool)))
  }
  return <ConfirmActionModal open={open} loading={running} intent="default" title={t('collectionInstall.title')}
    cancelLabel={t(step === 2 && !started ? 'installFlow.back' : 'collectionInstall.close')}
    onCancel={step === 2 && !started ? () => setStep(1) : close}
    confirmLabel={step === 1 && !started ? t('installFlow.next') : running ? t('collectionInstall.working') : completed ? t(failed ? 'collectionInstall.retry' : 'collectionInstall.done') : t('collectionInstall.installCount', { count: selectedCount })}
    confirmDisabled={checking || previewError || (step === 2 && missingProject) || selectedCount === 0}
    footerSummary={<div className="collection-install-progress" aria-live="polite">
      <strong>{started ? t('collectionInstall.progress', { count: finished, total: selectedCount }) : t('collectionInstall.selected', { count: selectedCount, total: eligibleSkills.length })}</strong>
      <span>{failed ? t('collectionInstall.failures', { count: failed }) : selectedTools.length ? t('collectionInstall.toolCount', { count: selectedTools.length }) : t('collectionInstall.libraryOnly')}</span>
      {started && <progress aria-label={t('collectionInstall.progressLabel')} value={finished} max={selectedCount} />}
    </div>}
    onRequestClose={close} onConfirm={() => { if (step === 1 && !started) { setStep(2); return }; if (completed && !failed) close(); else void run() }}
    body={<div className="collection-install-body install-wizard">
      <header className="collection-install-heading">
        <h3>{manifest.title}</h3>
        <div className="collection-install-sources">{manifest.sources.map(source => <span key={`${source.repo}/${source.ref}`} title={source.ref}>{source.repo}<code>{source.ref.slice(0, 7)}</code></span>)}</div>
      </header>
      <InstallFlowSteps step={step} t={t} />
      <div className="collection-install-workspace">
        <section hidden={step === 2 && !started} className="collection-install-catalog" aria-label={t('collectionInstall.listLabel')}>
          <SkillSelectionList query={query} onQueryChange={setQuery} searchType="search" searchLabel={t('collectionInstall.search')} disabled={locked} t={t}
            onToggle={(id, checked) => { const index = Number(id); if (!eligibleSkills.includes(index)) return; setSelectedSkills(current => checked ? [...new Set([...current, index])].sort((a, b) => a - b) : current.filter(item => item !== index)) }}
            onToggleAll={checked => { const ids = visible.filter(({ index }) => eligibleSkills.includes(index)).map(({ index }) => index); setSelectedSkills(current => checked ? [...new Set([...current, ...ids])].sort((a, b) => a - b) : current.filter(index => !ids.includes(index))) }}
            items={visible.map(({ skill, index }) => {
              const status = preview[index]?.status
              const state: string = active === index ? 'working' : results[index]?.state ?? (checking ? 'checking' : status === 'update' ? 'existing' : 'pending')
              return { id: String(index), name: skill.name, path: skill.path, description: preview[index]?.description,
                selected: installSelection.includes(index), selectable: status !== 'conflict', selectionLabel: t('collectionInstall.selectSkill', { name: skill.name }),
                status: status === 'conflict' ? t('gitInstall.conflictLabel') : state === 'pending' ? t('gitInstall.install') : t(`collectionInstall.${state}`),
                tone: status === 'conflict' ? 'conflict' : state === 'failed' ? 'error' : state === 'existing' ? 'update' : state === 'installed' || state === 'pending' ? 'install' : 'neutral',
                note: status === 'conflict' ? t('gitInstall.conflict') : results[index]?.message,
              }
            })} />
          {previewError && <p role="alert" className="collection-install-error">{t('collectionInstall.previewFailed')} <button type="button" className="btn btn-secondary" onClick={() => setPreviewAttempt(value => value + 1)}>{t('collectionInstall.previewRetry')}</button></p>}
        </section>
        <aside hidden={step === 1 || started} className="collection-install-settings" aria-label={t('collectionInstall.settings')}>
          <InstallSettingsSections disabled={locked} t={t}
            toolSummary={selectedTools.length ? t('collectionInstall.toolCount', { count: selectedTools.length }) : t('collectionInstall.libraryOnly')}

            tags={<>          <fieldset disabled={locked}>
            <legend>{t('addTags')}</legend>
            {tags.length > 0 && <div className="add-tags-list">{tags.map(tag => <button key={tag.id} type="button" aria-pressed={selectedTags.includes(tag.id)} className={`add-tag-pill${selectedTags.includes(tag.id) ? ' selected' : ''}`} onClick={() => setSelectedTags(current => current.includes(tag.id) ? current.filter(id => id !== tag.id) : [...current, tag.id])}><span className="add-tag-check">{selectedTags.includes(tag.id) && <Check size={12} />}</span>#{tag.name}</button>)}</div>}
            <label className="collection-install-label" htmlFor={tagInputId}>{t('collectionInstall.newTag')}</label>
            <input id={tagInputId} className="input" value={newTag} onChange={event => setNewTag(event.target.value)} placeholder={manifest.title} />
            <p className="collection-install-note">{t('collectionInstall.tagHint')}</p>
          </fieldset></>}
            tools={<><fieldset disabled={locked}>
            <legend>{t('collectionInstall.tools')}</legend>
            <div className="collection-install-tools">{tools.map(tool => {
              const unsupported = isToolUnsupportedForScope(tool, scope)
              const selected = selectedTools.includes(tool.id)
              return <label key={tool.id} className={`tool-pill-toggle${selected ? ' active' : ''}${unsupported ? ' disabled' : ''}`} title={unsupported ? t('installScope.unsupportedTool', { tool: tool.label }) : undefined}>
                <input type="checkbox" aria-label={tool.label} checked={selected} disabled={unsupported} onChange={event => changeTool(tool.id, event.target.checked)} />
                <ToolIcon toolKey={tool.id} label={tool.label} avatar={tool.avatar} className="add-tool-logo" /><span className="add-tool-label">{tool.label}</span>
              </label>
            })}</div>
            {tools.length === 0 && <p className="collection-install-note">{t('collectionInstall.noTools')}</p>}
            {selectedTools.some(id => ((scope === 'project' ? sharedProjectTools : sharedTools)[id]?.length ?? 0) > 1) && <p className="collection-install-note">{t('collectionInstall.sharedTools')}</p>}
          </fieldset></>}
            scope={<><ScopeSelector compact showRequired={selectedTools.length > 0} scope={scope} projects={projects} recentProjects={recentProjects} disabled={locked} title={t('installScope.title')}
            onScopeChange={next => {
              setScope(next)
              setSelectedTools(current => {
                const targets = Object.fromEntries(current.map(id => [id, true]))
                if (next === 'project') {
                  const normalized = normalizeProjectSharedTargets(targets, tools, sharedProjectTools)
                  return tools.filter(tool => normalized[tool.id] && !isToolUnsupportedForScope(tool, next)).map(tool => tool.id)
                }
                const expanded = new Set(current.flatMap(id => sharedTools[id] ?? [id]))
                return tools.filter(tool => expanded.has(tool.id)).map(tool => tool.id)
              })
            }}
            onProjectsChange={setProjects} onPickProject={onPickProject} t={t} /></>}
          />
          <p className="collection-install-note collection-install-preserve">{t('collectionInstall.existingNote')}</p>
          {error && <p role="alert" className="collection-install-error">{error}</p>}
        </aside>
      </div>
    </div>} />
})
