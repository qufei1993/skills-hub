import { hasUnboundLocalSource } from './skillSourceState'
import { Fragment, memo, useEffect, useState } from 'react'
import { createPortal } from 'react-dom'
import { Copy, Folder, Github, RefreshCw, Tag, Trash2 } from 'lucide-react'
import type { TFunction } from 'i18next'
import { toast } from 'sonner'
import { getFullySyncedTools, getToolSyncState } from './skillSyncStatus'
import type { ManagedSkill, ToolOption } from './types'
import ToolIcon from './ToolIcon'
import SkillIssueNotice from './SkillIssueNotice'

type GithubInfo = { label: string; href: string }

type SkillCardProps = {
  onRepairSource?: (skill: ManagedSkill) => void
  onRepairTargets?: (skill: ManagedSkill) => void
  projectContext?: boolean
  onRemoveProject?: (skill: ManagedSkill) => void
  skill: ManagedSkill
  installedTools: ToolOption[]
  loading: boolean
  bulkMode: boolean
  bulkSelected: boolean
  getGithubInfo: (url: string | null | undefined) => GithubInfo | null
  getSkillSourceLabel: (skill: ManagedSkill) => string
  formatRelative: (ms: number | null | undefined) => string
  onUpdate: (skill: ManagedSkill) => void
  onDelete: (skillId: string) => void
  onToggleEnabled: (skill: ManagedSkill) => void
  onToggleTool: (skill: ManagedSkill, toolId: string) => void
  onOpenScope: (skill: ManagedSkill) => void
  onOpenDetail: (skill: ManagedSkill) => void
  onEditTags: (skill: ManagedSkill) => void
  onToggleBulkSelection: (skillId: string) => void
  getSkillScope: (skill: ManagedSkill) => 'global' | 'project'
  getSkillProjects: (skill: ManagedSkill) => string[]
  t: TFunction
}

const SkillCard = ({
  skill,
  onRepairSource,
  onRepairTargets,
  projectContext = false,
  onRemoveProject,
  installedTools,
  loading,
  bulkMode,
  bulkSelected,
  getGithubInfo,
  getSkillSourceLabel,
  formatRelative,
  onUpdate,
  onDelete,
  onToggleEnabled,
  onToggleTool,
  onOpenScope,
  onOpenDetail,
  onEditTags,
  onToggleBulkSelection,
  getSkillScope,
  getSkillProjects,
  t,
}: SkillCardProps) => {
  const unboundSource = hasUnboundLocalSource(skill)
  const [toolHint, setToolHint] = useState<{ toolId: string; left: number; top: number } | null>(null)
  const showToolHint = (button: HTMLElement, toolId: string) => {
    const rect = button.getBoundingClientRect()
    setToolHint({ toolId, left: rect.left + rect.width / 2, top: rect.top })
  }

  useEffect(() => {
    if (!toolHint) return
    const hide = () => setToolHint(null)
    window.addEventListener('scroll', hide, true)
    window.addEventListener('resize', hide)
    return () => {
      window.removeEventListener('scroll', hide, true)
      window.removeEventListener('resize', hide)
    }
  }, [toolHint])
  const github = getGithubInfo(skill.source_ref)
  const isGit = skill.source_type.toLowerCase().includes('git')
  const sourceLabel = github?.label ?? getSkillSourceLabel(skill)
  const copyValue = unboundSource ? '' : (github?.href ?? skill.source_ref ?? skill.central_path).trim()
  const description = skill.description?.trim() || t('skillDescriptionEmpty')
  const scope = getSkillScope(skill)
  const projectCount = getSkillProjects(skill).length
  const enabled = skill.enabled !== false
  // The list already narrows targets for a selected scope or project.
  const syncedToolCount = getFullySyncedTools(skill, installedTools, 'all').length
  const handleCopySource = async () => {
    if (!copyValue) return
    try {
      await navigator.clipboard.writeText(copyValue)
      toast.success(t('copied'))
    } catch {
      toast.error(t('copyFailed'))
    }
  }

  return (
    <div
      className={`skill-card${bulkMode ? ' bulk-mode' : ''}${bulkSelected ? ' bulk-selected' : ''}${enabled ? '' : ' disabled-skill'}`}
    >
      {bulkMode ? (
        <label className="bulk-skill-check" aria-label={t('bulk.toggleSkill')}>
          <input
            type="checkbox"
            checked={bulkSelected}
            onChange={() => onToggleBulkSelection(skill.id)}
            disabled={loading}
          />
          <span />
        </label>
      ) : null}

      <div className="skill-card-head">
        <div className="skill-identity">
          <div className="skill-icon">{isGit ? <Github size={16} /> : <Folder size={16} />}</div>
          <div className="skill-identity-copy">
            <div className="skill-title-line">
              <button className="skill-name clickable" type="button" onClick={() => onOpenDetail(skill)}>
                {skill.name}
              </button>
              <button
                className="skill-source-copy"
                type="button"
                title={`${t('copy')}：${sourceLabel}`}
                aria-label={`${t('copy')}：${sourceLabel}`}
                onClick={() => void handleCopySource()}
                disabled={!copyValue}
              >
                <Copy size={12} />
              </button>
              <SkillIssueNotice onRepairSource={() => onRepairSource?.(skill)} onRepairTargets={() => onRepairTargets ? onRepairTargets(skill) : onOpenScope(skill)} compact skill={skill} tools={installedTools} t={t} />
            </div>
            <div className={`skill-description${skill.description?.trim() ? '' : ' empty'}`} title={description}>
              {description}
            </div>
            {unboundSource ? <p className="skill-source-unbound" title={t('deviceSync.unboundSourceHelp')}>{t('deviceSync.unboundSource')}</p> : null}
          </div>
        </div>

        {projectContext ? <button className="btn btn-secondary skill-manage-targets" onClick={event => { event.currentTarget.focus(); onOpenScope(skill) }}>{t('workspace110.manageTargets')}</button> : <button
          className={`skill-switch${enabled ? ' enabled' : ''}`}
          type="button"
          onClick={() => onToggleEnabled(skill)}
          disabled={loading}
          aria-label={enabled ? t('disableSkill') : t('enableSkill')}
        ><span /></button>}
      </div>

      <div className="skill-card-meta">
        <div className="skill-tags-inline">
          {skill.tags.length > 0 ? skill.tags.slice(0, 2).map((tag) => (
            <button key={tag.id} className="skill-tag-pill" type="button" onClick={() => onEditTags(skill)}>
              {tag.name}
            </button>
          )) : <span className="table-empty">{t('untagged')}</span>}
        </div>

        <button className={`scope-badge ${scope}`} type="button" onClick={event => { event.currentTarget.focus(); onOpenScope(skill) }}>
          {skill.targets.some(target => target.scope === 'global') && projectCount > 0 ? t('workspace110.mixedScope', { count: projectCount }) : scope === 'project' ? t('scope.projectCount', { count: projectCount }) : t('scope.globalBadge')}
        </button>

        <div className="skill-updated">{formatRelative(skill.updated_at)}</div>
      </div>

      <div className="skill-card-footer">
        <div className="skill-tools-block">
          <span>
            {t('syncTargetsCount', {
              synced: syncedToolCount,
              total: installedTools.length,
            })}
          </span>
          <div className="skill-tool-avatars">
            {installedTools.map((tool) => {
              const syncState = getToolSyncState(skill, tool.id, 'all')
              const synced = syncState === 'synced'
              const stateLabel =
                syncState === 'failed'
                  ? t('toolManagement.syncFailed')
                  : syncState === 'partial'
                    ? t('toolManagement.syncPartialFailed')
                    : synced
                      ? t('toolManagement.synced')
                      : t('toolManagement.notSynced')
              const toolLabel = `${tool.label} · ${stateLabel}`
              return (
                <Fragment key={tool.id}>
                  <button
                    className={syncState}
                    type="button"
                    title={enabled ? undefined : toolLabel}
                    aria-label={toolLabel}
                    aria-pressed={synced}
                    onClick={event => { if (enabled) { event.currentTarget.focus(); onToggleTool(skill, tool.id) } }}
                    onMouseEnter={(event) => showToolHint(event.currentTarget, tool.id)}
                    onFocus={(event) => showToolHint(event.currentTarget, tool.id)}
                    onMouseLeave={() => setToolHint(null)}
                    onBlur={() => setToolHint(null)}
                    disabled={!enabled}
                  >
                    <ToolIcon
                      toolKey={tool.id}
                      label={tool.label}
                      avatar={tool.avatar}
                    />
                  </button>
                  {enabled && toolHint?.toolId === tool.id
                    ? createPortal(
                        <span className="skill-tool-tooltip" role="tooltip" style={{ left: toolHint.left, top: toolHint.top }}>
                          {toolLabel}
                        </span>,
                        document.body,
                      )
                    : null}
                </Fragment>
              )
            })}
          </div>
        </div>

        <div className="skill-actions-col">
          <button type="button" onClick={() => onEditTags(skill)} disabled={loading} aria-label={t('editTags')}><Tag size={16} /></button>
          <button type="button" onClick={() => onUpdate(skill)} disabled={loading || !enabled || unboundSource} title={unboundSource ? t('deviceSync.unboundSourceHelp') : t('update')} aria-label={t('update')}><RefreshCw size={16} /></button>
          <button type="button" onClick={event => { if (projectContext) { event.currentTarget.focus(); onRemoveProject?.(skill) } else onDelete(skill.id) }} disabled={loading} aria-label={t(projectContext ? 'workspace110.removeTargets' : 'remove')}><Trash2 size={16} /></button>
        </div>
      </div>
    </div>
  )
}

export default memo(SkillCard)
