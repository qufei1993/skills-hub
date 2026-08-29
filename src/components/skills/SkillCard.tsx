import { memo } from 'react'
import { Copy, Folder, Github, RefreshCw, Tag, Trash2 } from 'lucide-react'
import type { TFunction } from 'i18next'
import { toast } from 'sonner'
import { getFullySyncedTools, getToolSyncState } from './skillSyncStatus'
import type { ManagedSkill, ToolOption } from './types'
import ToolIcon from './ToolIcon'

type GithubInfo = { label: string; href: string }

type SkillCardProps = {
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
  const github = getGithubInfo(skill.source_ref)
  const isGit = skill.source_type.toLowerCase().includes('git')
  const sourceLabel = github?.label ?? getSkillSourceLabel(skill)
  const copyValue = (github?.href ?? skill.source_ref ?? skill.central_path).trim()
  const description = skill.description?.trim() || t('skillDescriptionEmpty')
  const scope = getSkillScope(skill)
  const projectCount = getSkillProjects(skill).length
  const enabled = skill.enabled !== false
  const syncedToolCount = getFullySyncedTools(skill, installedTools, scope).length
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
            </div>
            <div className={`skill-description${skill.description?.trim() ? '' : ' empty'}`} title={description}>
              {description}
            </div>
          </div>
        </div>

        <button
          className={`skill-switch${enabled ? ' enabled' : ''}`}
          type="button"
          onClick={() => onToggleEnabled(skill)}
          disabled={loading}
          aria-label={enabled ? t('disableSkill') : t('enableSkill')}
        ><span /></button>
      </div>

      <div className="skill-card-meta">
        <div className="skill-tags-inline">
          {skill.tags.length > 0 ? skill.tags.slice(0, 2).map((tag) => (
            <button key={tag.id} className="skill-tag-pill" type="button" onClick={() => onEditTags(skill)}>
              {tag.name}
            </button>
          )) : <span className="table-empty">{t('untagged')}</span>}
        </div>

        <button className={`scope-badge ${scope}`} type="button" onClick={() => onOpenScope(skill)}>
          {scope === 'project' ? t('scope.projectCount', { count: projectCount }) : t('scope.globalBadge')}
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
              const syncState = getToolSyncState(skill, tool.id, scope)
              const synced = syncState === 'synced'
              const stateLabel =
                syncState === 'failed'
                  ? t('toolManagement.syncFailed')
                  : syncState === 'partial'
                    ? t('toolManagement.syncPartialFailed')
                    : synced
                      ? t('toolManagement.synced')
                      : t('toolManagement.notSynced')
              return (
                <button
                  key={tool.id}
                  className={syncState}
                  type="button"
                  title={`${tool.label} · ${stateLabel}`}
                  aria-label={`${tool.label} · ${stateLabel}`}
                  aria-pressed={synced}
                  onClick={() => enabled && onToggleTool(skill, tool.id)}
                  disabled={!enabled}
                >
                  <ToolIcon
                    toolKey={tool.id}
                    label={tool.label}
                    avatar={tool.avatar}
                  />
                </button>
              )
            })}
          </div>
        </div>

        <div className="skill-actions-col">
          <button type="button" onClick={() => onEditTags(skill)} disabled={loading} aria-label={t('editTags')}><Tag size={16} /></button>
          <button type="button" onClick={() => onUpdate(skill)} disabled={loading || !enabled} aria-label={t('update')}><RefreshCw size={16} /></button>
          <button type="button" onClick={() => onDelete(skill.id)} disabled={loading} aria-label={t('remove')}><Trash2 size={16} /></button>
        </div>
      </div>
    </div>
  )
}

export default memo(SkillCard)
