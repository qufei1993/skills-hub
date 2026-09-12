export type TemporaryActivationScope = 'global' | 'project'

export type TemporaryActivationEntry = {
  toolId: string
  scope: TemporaryActivationScope
  projectPath?: string
  tagId: number | null
  tagIds?: Array<number | null>
  toolIds?: string[]
  snapshot: Record<string, boolean>
  updatedAt: number
}

export const temporaryActivationKey = (
  toolId: string,
  scope: TemporaryActivationScope,
  projectPath?: string,
) => `${toolId}|${scope}|${projectPath ?? ''}`

export const hasTag = (skillTags: { id: number }[], tagId: number | null) =>
  tagId === null ? skillTags.length === 0 : skillTags.some((tag) => tag.id === tagId)

export const hasAnyTag = (skillTags: { id: number }[], tagIds: Array<number | null>) => {
  if (tagIds.length === 0) return true
  return tagIds.some((tagId) => hasTag(skillTags, tagId))
}

export const buildTemporarySnapshot = (
  skills: { id: string; targets: { tool: string; scope: string; project_path?: string | null; status: string }[] }[],
  toolId: string,
  scope: TemporaryActivationScope,
  projectPath?: string,
) => {
  const snapshot: Record<string, boolean> = {}
  for (const skill of skills) {
    snapshot[skill.id] = skill.targets.some(
      (target) =>
        target.tool === toolId &&
        (target.scope ?? 'global') === scope &&
        (scope !== 'project' || target.project_path === projectPath) &&
        target.status !== 'disabled',
    )
  }
  return snapshot
}
