import { describe, expect, it } from 'vitest'
import { getSkillsView } from './skillsView'
import type { ManagedSkill, TagWithCountDto } from './types'

const tags: TagWithCountDto[] = [
  { id: 1, name: 'writing', skill_count: 3, updated_at: 0 },
  { id: 2, name: 'code', skill_count: 1, updated_at: 0 },
]
const skill = (id: string, tagIds: number[]): ManagedSkill => ({
  id, name: id, source_type: 'local', central_path: `/skills/${id}`,
  created_at: 0, updated_at: 0, enabled: true, status: 'ok', targets: [],
  tags: tags.filter((tag) => tagIds.includes(tag.id)),
})
const defaults = {
  managedSkills: [skill('global-write', [1]), skill('project-write', [1]), skill('project-both', [1, 2]), skill('global-none', [])],
  tags, scopeFilter: 'all' as const, searchQuery: '', selectedTagIds: [] as number[],
  includeUntagged: false, sortBy: 'name' as const, bulkSelectedIds: [] as string[],
  getSkillScope: (item: ManagedSkill): 'global' | 'project' => item.id.startsWith('global') ? 'global' : 'project',
}

describe('scope-aware Skills view', () => {
  it('counts tags and untagged Skills within scope and search, independently of selected tags', () => {
    const view = getSkillsView({ ...defaults, scopeFilter: 'project', selectedTagIds: [2] })
    expect(view.visibleSkills.map((item) => item.id)).toEqual(['project-both'])
    expect(view.filterTags.map((tag) => tag.skill_count)).toEqual([2, 1])
    expect(view.filterUntaggedCount).toBe(0)
    const search = getSkillsView({ ...defaults, searchQuery: ' GLOBAL-WRITE ' })
    expect(search.filterTags.map((tag) => tag.skill_count)).toEqual([1, 0])
    expect(search.filterUntaggedCount).toBe(0)
    expect(tags[0].skill_count).toBe(3)
  })

  it('excludes hidden selections from all bulk-operation inputs and restores them when visible again', () => {
    const selection = ['global-write', 'project-write', 'project-both', 'missing']
    const view = getSkillsView({ ...defaults, bulkSelectedIds: selection, scopeFilter: 'project', selectedTagIds: [2] })
    expect(view.bulkSelectedSkills.map((item) => item.id)).toEqual(['project-both'])
    const empty = getSkillsView({ ...defaults, bulkSelectedIds: selection, searchQuery: 'no match' })
    expect(empty.bulkSelectedSkills).toEqual([])
    const restored = getSkillsView({ ...defaults, bulkSelectedIds: selection })
    expect(restored.bulkSelectedSkills.map((item) => item.id)).toEqual(['global-write', 'project-both', 'project-write'])
  })

  it('keeps any-match tag filtering, untagged selection, and scope constraints together', () => {
    const view = getSkillsView({ ...defaults, scopeFilter: 'global', selectedTagIds: [2], includeUntagged: true })
    expect(view.visibleSkills.map((item) => item.id)).toEqual(['global-none'])
    expect(view.filterTags.map((tag) => tag.skill_count)).toEqual([1, 0])
    expect(view.filterUntaggedCount).toBe(1)
  })
})
