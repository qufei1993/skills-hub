import { memo, useMemo, useState } from 'react'
import { Bot, CircleCheck, CircleX } from 'lucide-react'
import type { TFunction } from 'i18next'
import type { ManagedSkill, ToolOption } from './types'
import { buildToolSkillGroups, getSkillsForTool } from './toolSkills'

type ToolSkillsTabProps = {
  managedSkills: ManagedSkill[]
  tools: ToolOption[]
  installedToolIds: string[]
  loading: boolean
  formatRelative: (ms: number | null | undefined) => string
  getSkillSourceLabel: (skill: ManagedSkill) => string
  onToggleTool: (skill: ManagedSkill, toolId: string) => void
  t: TFunction
}

const ToolSkillsTab = ({
  managedSkills,
  tools,
  installedToolIds,
  loading,
  formatRelative,
  getSkillSourceLabel,
  onToggleTool,
  t,
}: ToolSkillsTabProps) => {
  const toolGroups = useMemo(
    () => buildToolSkillGroups(managedSkills, tools, installedToolIds),
    [installedToolIds, managedSkills, tools],
  )

  const [selectedToolId, setSelectedToolId] = useState<string | null>(null)

  const effectiveSelectedToolId = useMemo(() => {
    if (toolGroups.length === 0) return null
    if (
      selectedToolId &&
      toolGroups.some((group) => group.toolId === selectedToolId)
    ) {
      return selectedToolId
    }
    return toolGroups[0].toolId
  }, [selectedToolId, toolGroups])

  const selectedTool = useMemo(
    () =>
      toolGroups.find((group) => group.toolId === effectiveSelectedToolId) ??
      null,
    [effectiveSelectedToolId, toolGroups],
  )

  const selectedSkills = useMemo(
    () =>
      effectiveSelectedToolId
        ? getSkillsForTool(managedSkills, effectiveSelectedToolId)
        : [],
    [effectiveSelectedToolId, managedSkills],
  )

  return (
    <div className="tool-skills-tab">
      <aside className="tool-skills-sidebar">
        <div className="tool-skills-sidebar-title">{t('toolSkills.toolsTitle')}</div>
        {toolGroups.length === 0 ? (
          <div className="tool-skills-sidebar-empty">{t('toolSkills.emptyTools')}</div>
        ) : (
          <div className="tool-skills-sidebar-list">
            {toolGroups.map((group) => (
              <button
                key={group.toolId}
                type="button"
                className={`tool-skills-tool-item ${effectiveSelectedToolId === group.toolId ? 'active' : ''}`}
                onClick={() => setSelectedToolId(group.toolId)}
              >
                <div className="tool-skills-tool-head">
                  <span className="tool-skills-tool-name">{group.label}</span>
                  <span className="tool-skills-tool-count">
                    {t('toolSkills.count', { count: group.skillCount })}
                  </span>
                </div>
                <div className={`tool-skills-tool-status ${group.installed ? 'installed' : 'not-installed'}`}>
                  {group.installed ? (
                    <>
                      <CircleCheck size={14} />
                      {t('toolSkills.installed')}
                    </>
                  ) : (
                    <>
                      <CircleX size={14} />
                      {t('toolSkills.notInstalled')}
                    </>
                  )}
                </div>
              </button>
            ))}
          </div>
        )}
      </aside>

      <section className="tool-skills-content">
        <div className="tool-skills-content-header">
          <div>
            <div className="tool-skills-title">{selectedTool?.label ?? t('toolSkills.title')}</div>
            {selectedTool ? (
              <div className="tool-skills-subtitle">
                {t('toolSkills.selectedSummary', {
                  tool: selectedTool.label,
                  count: selectedTool.skillCount,
                })}
              </div>
            ) : (
              <div className="tool-skills-subtitle">{t('toolSkills.emptyTools')}</div>
            )}
          </div>
        </div>

        {!selectedTool ? (
          <div className="tool-skills-empty">
            <Bot size={18} />
            <span>{t('toolSkills.emptyTools')}</span>
          </div>
        ) : selectedSkills.length === 0 ? (
          <div className="tool-skills-empty">
            <Bot size={18} />
            <span>{t('toolSkills.emptyForTool', { tool: selectedTool.label })}</span>
          </div>
        ) : (
          <div className="tool-skills-list">
            {selectedSkills.map((skill) => (
              <article
                key={`${effectiveSelectedToolId}-${skill.id}`}
                className="tool-skill-card"
              >
                <div className="tool-skill-main">
                  <div className="tool-skill-name">{skill.name}</div>
                  <div className="tool-skill-meta">
                    <span>{t('sourceLabel')}: {getSkillSourceLabel(skill)}</span>
                    <span>{t('updatedLabel')}: {formatRelative(skill.updated_at)}</span>
                  </div>
                </div>
                {effectiveSelectedToolId ? (
                  <button
                    type="button"
                    className="btn btn-secondary"
                    onClick={() => onToggleTool(skill, effectiveSelectedToolId)}
                    disabled={loading}
                  >
                    {t('remove')}
                  </button>
                ) : null}
              </article>
            ))}
          </div>
        )}
      </section>
    </div>
  )
}

export default memo(ToolSkillsTab)
