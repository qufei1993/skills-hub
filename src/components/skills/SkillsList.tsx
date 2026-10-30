import { memo } from 'react'
import type { TFunction } from 'i18next'
import type { ManagedSkill, ToolOption } from './types'
import SkillCard from './SkillCard'

type GithubInfo = {
  label: string
  href: string
}

type SkillsListProps = {
  onRepairSource?: (skill: ManagedSkill) => void
  projectContext?: boolean
  onRemoveProject?: (skill: ManagedSkill) => void
  hasManagedSkills: boolean
  hasFilters: boolean
  onClearFilters: () => void
  visibleSkills: ManagedSkill[]
  installedTools: ToolOption[]
  loading: boolean
  bulkMode: boolean
  selectedSkillIds: string[]
  viewMode: 'list' | 'cards'
  getGithubInfo: (url: string | null | undefined) => GithubInfo | null
  getSkillSourceLabel: (skill: ManagedSkill) => string
  formatRelative: (ms: number | null | undefined) => string
  onUpdateSkill: (skill: ManagedSkill) => void
  onDeleteSkill: (skillId: string) => void
  onToggleSkillEnabled: (skill: ManagedSkill) => void
  onToggleTool: (skill: ManagedSkill, toolId: string) => void
  onOpenScope: (skill: ManagedSkill) => void
  onOpenDetail: (skill: ManagedSkill) => void
  onEditTags: (skill: ManagedSkill) => void
  onToggleBulkSelection: (skillId: string) => void
  getSkillScope: (skill: ManagedSkill) => 'global' | 'project'
  getSkillProjects: (skill: ManagedSkill) => string[]
  t: TFunction
}

const SkillsList = ({
  onRepairSource,
  projectContext = false,
  onRemoveProject,
  hasManagedSkills,
  hasFilters,
  onClearFilters,
  visibleSkills,
  installedTools,
  loading,
  bulkMode,
  selectedSkillIds,
  viewMode,
  getGithubInfo,
  getSkillSourceLabel,
  formatRelative,
  onUpdateSkill,
  onDeleteSkill,
  onToggleSkillEnabled,
  onToggleTool,
  onOpenScope,
  onOpenDetail,
  onEditTags,
  onToggleBulkSelection,
  getSkillScope,
  getSkillProjects,
  t,
}: SkillsListProps) => {
  const selectedSkillSet = new Set(selectedSkillIds)

  return (
    <div
      className="skills-list"
      role="region"
      tabIndex={0}
      aria-label={t('navMySkills')}
      onWheel={(event) => {
        if (event.deltaY === 0) return
        const list = event.currentTarget
        const previousScrollTop = list.scrollTop
        list.scrollTop += event.deltaY
        if (list.scrollTop !== previousScrollTop) event.preventDefault()
      }}
    >
      {visibleSkills.length === 0 ? (
        <div className="empty skills-empty-state">
          <span>{t(hasManagedSkills ? 'filters.noResults' : 'skillsEmpty')}</span>
          {hasManagedSkills && hasFilters ? (
            <button className="btn btn-secondary" type="button" onClick={onClearFilters}>
              {t('filters.clear')}
            </button>
          ) : null}
        </div>
      ) : (
        <div className={`skills-table ${viewMode}-view`}>
          {visibleSkills.map((skill) => (
            <SkillCard
              onRepairSource={onRepairSource}
              projectContext={projectContext}
              onRemoveProject={onRemoveProject}
              key={skill.id}
              skill={skill}
              installedTools={installedTools}
              loading={loading}
              bulkMode={bulkMode}
              bulkSelected={selectedSkillSet.has(skill.id)}
              getGithubInfo={getGithubInfo}
              getSkillSourceLabel={getSkillSourceLabel}
              formatRelative={formatRelative}
              onUpdate={onUpdateSkill}
              onDelete={onDeleteSkill}
              onToggleEnabled={onToggleSkillEnabled}
              onToggleTool={onToggleTool}
              onOpenScope={onOpenScope}
              onOpenDetail={onOpenDetail}
              onEditTags={onEditTags}
              onToggleBulkSelection={onToggleBulkSelection}
              getSkillScope={getSkillScope}
              getSkillProjects={getSkillProjects}
              t={t}
            />
          ))}
        </div>
      )}
    </div>
  )
}

export default memo(SkillsList)
