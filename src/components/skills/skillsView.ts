import type { ManagedSkill, TagWithCountDto } from './types'

type SkillsViewOptions = {
  managedSkills: ManagedSkill[]
  tags: TagWithCountDto[]
  scopeFilter: 'all' | 'global' | 'project'
  searchQuery: string
  selectedTagIds: number[]
  includeUntagged: boolean
  sortBy: 'updated' | 'name'
  bulkSelectedIds: string[]
  getSkillScope: (skill: ManagedSkill) => 'global' | 'project'
}

export function getSkillsView({
  managedSkills, tags, scopeFilter, searchQuery, selectedTagIds,
  includeUntagged, sortBy, bulkSelectedIds, getSkillScope,
}: SkillsViewOptions) {
  const query = searchQuery.trim().toLowerCase()
  const scopedSkills = managedSkills.filter((skill) => {
    if (scopeFilter !== 'all' && getSkillScope(skill) !== scopeFilter) return false
    return !query || skill.name.toLowerCase().includes(query) ||
      skill.central_path.toLowerCase().includes(query) ||
      skill.source_type.toLowerCase().includes(query) ||
      skill.tags.some((tag) => tag.name.toLowerCase().includes(query))
  })
  const counts = new Map<number, number>()
  let filterUntaggedCount = 0
  for (const skill of scopedSkills) {
    if (skill.tags.length === 0) filterUntaggedCount++
    for (const id of new Set(skill.tags.map((tag) => tag.id))) {
      counts.set(id, (counts.get(id) ?? 0) + 1)
    }
  }
  const selectedTags = new Set(selectedTagIds)
  const hasTagFilter = selectedTags.size > 0 || includeUntagged
  const visibleSkills = scopedSkills.filter((skill) => !hasTagFilter ||
    skill.tags.some((tag) => selectedTags.has(tag.id)) ||
    (includeUntagged && skill.tags.length === 0),
  ).sort((a, b) => sortBy === 'name'
    ? a.name.localeCompare(b.name)
    : (b.updated_at ?? 0) - (a.updated_at ?? 0),
  )
  const selectedIds = new Set(bulkSelectedIds)
  return {
    visibleSkills,
    filterTags: tags.map((tag) => ({ ...tag, skill_count: counts.get(tag.id) ?? 0 })),
    filterUntaggedCount,
    bulkSelectedSkills: visibleSkills.filter((skill) => selectedIds.has(skill.id)),
  }
}
