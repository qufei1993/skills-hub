import { memo } from 'react'
import { MessageCircle, SlidersHorizontal } from 'lucide-react'
import type { TFunction } from 'i18next'
import type { ManagedSkill, OnboardingPlan, ToolOption } from './types'
import SkillCard from './SkillCard'

type GithubInfo = {
  label: string
  href: string
}

type SkillsListProps = {
  plan: OnboardingPlan | null
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
  onReviewImport: () => void
  onOpenScanSettings: () => void
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
  hasManagedSkills,
  hasFilters,
  onClearFilters,
  plan,
  visibleSkills,
  installedTools,
  loading,
  bulkMode,
  selectedSkillIds,
  viewMode,
  getGithubInfo,
  getSkillSourceLabel,
  formatRelative,
  onReviewImport,
  onOpenScanSettings,
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
      {plan && plan.total_skills_found > 0 ? (
        <div className="discovered-banner">
          <div className="banner-left">
            <div className="banner-icon">
              <MessageCircle size={18} />
            </div>
            <div className="banner-content">
              <div className="banner-title">{t('discoveredTitle')}</div>
              <div className="banner-subtitle">
                {t('discoveredCount', { count: plan.total_skills_found })}
              </div>
            </div>
          </div>
          <div className="discovered-banner-actions">
            <button
              className="btn btn-secondary"
              type="button"
              onClick={onOpenScanSettings}
              disabled={loading}
            >
              <SlidersHorizontal size={15} />
              {t('discoveryScan.action')}
            </button>
            <button
              className="btn btn-warning"
              type="button"
              onClick={onReviewImport}
              disabled={loading}
            >
              {t('reviewImport')}
            </button>
          </div>
        </div>
      ) : null}

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
