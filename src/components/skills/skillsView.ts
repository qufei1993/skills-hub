import type { ManagedSkill, TagWithCountDto } from './types'
import { getSkillSyncState, isActiveSkillTarget, isSuccessfulSkillTarget } from './skillSyncStatus'

export type SkillStatusFilter = 'all' | 'enabled' | 'disabled'

type SkillsViewOptions = {
  projectPath?: string
  issuesOnly?: boolean
  managedSkills: ManagedSkill[]
  tags: TagWithCountDto[]
  scopeFilter: 'all' | 'global' | 'project'
  searchQuery: string
  selectedTagIds: number[]
  includeUntagged: boolean
  statusFilter?: SkillStatusFilter
  selectedToolIds?: string[]
  sortBy: 'updated' | 'name'
  bulkSelectedIds: string[]
  getSkillScope: (skill: ManagedSkill) => 'global' | 'project'
}

export function getSkillsView({
  managedSkills, tags, scopeFilter, searchQuery, selectedTagIds,
  includeUntagged, sortBy, bulkSelectedIds, getSkillScope, issuesOnly = false, projectPath = '',
  statusFilter = 'all', selectedToolIds = [],
}: SkillsViewOptions) {
  const query = searchQuery.trim().toLowerCase()
  const matchesTargetScope = (target: ManagedSkill['targets'][number]) =>
    scopeFilter === 'all' || ((target.scope || 'global') === scopeFilter &&
      (!projectPath || target.project_path === projectPath))
  const isSyncedToTool = (skill: ManagedSkill, toolId: string) =>
    skill.targets.some((target) =>
      target.tool === toolId && matchesTargetScope(target) &&
      isActiveSkillTarget(target) && isSuccessfulSkillTarget(target))
  const scopedSkills = managedSkills.filter((skill) => {
    if (issuesOnly && !['source-error', 'partial', 'failed'].includes(getSkillSyncState(skill))) return false
    if (scopeFilter !== 'all' && !(skill.targets.length ? skill.targets.some(matchesTargetScope) : !projectPath && getSkillScope(skill) === scopeFilter)) return false
    return !query || skill.name.toLowerCase().includes(query) ||
      skill.central_path.toLowerCase().includes(query) ||
      skill.source_type.toLowerCase().includes(query) ||
      skill.tags.some((tag) => tag.name.toLowerCase().includes(query))
  })
  const selectedTags = new Set(selectedTagIds)
  const hasTagFilter = selectedTags.size > 0 || includeUntagged
  const matchesTags = (skill: ManagedSkill) => !hasTagFilter ||
    skill.tags.some((tag) => selectedTags.has(tag.id)) ||
    (includeUntagged && skill.tags.length === 0)
  const matchesStatus = (skill: ManagedSkill) =>
    statusFilter === 'all' || skill.enabled === (statusFilter === 'enabled')
  const matchesTools = (skill: ManagedSkill) =>
    selectedToolIds.length === 0 || selectedToolIds.some((toolId) => isSyncedToTool(skill, toolId))

  const counts = new Map<number, number>()
  let filterUntaggedCount = 0
  for (const skill of scopedSkills) {
    if (!matchesStatus(skill) || !matchesTools(skill)) continue
    if (skill.tags.length === 0) filterUntaggedCount++
    for (const id of new Set(skill.tags.map((tag) => tag.id))) {
      counts.set(id, (counts.get(id) ?? 0) + 1)
    }
  }
  const statusCounts = { all: 0, enabled: 0, disabled: 0 }
  const toolCounts = new Map<string, number>()
  for (const skill of scopedSkills) {
    if (!matchesTags(skill)) continue
    if (matchesTools(skill)) {
      statusCounts.all++
      statusCounts[skill.enabled ? 'enabled' : 'disabled']++
    }
    if (!matchesStatus(skill)) continue
    for (const toolId of new Set(skill.targets.map((target) => target.tool))) {
      if (isSyncedToTool(skill, toolId)) toolCounts.set(toolId, (toolCounts.get(toolId) ?? 0) + 1)
    }
  }

  const visibleSkills = scopedSkills.filter((skill) =>
    matchesTags(skill) && matchesStatus(skill) && matchesTools(skill),
  ).sort((a, b) => sortBy === 'name'
    ? a.name.localeCompare(b.name)
    : (b.updated_at ?? 0) - (a.updated_at ?? 0),
  )
  const selectedIds = new Set(bulkSelectedIds)
  return {
    visibleSkills,
    filterTags: tags.map((tag) => ({ ...tag, skill_count: counts.get(tag.id) ?? 0 })),
    filterUntaggedCount,
    statusCounts,
    toolCounts,
    bulkSelectedSkills: visibleSkills.filter((skill) => selectedIds.has(skill.id)),
  }
}
