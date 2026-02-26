import type { ManagedSkill, ToolOption } from './types'

export type ToolSkillGroup = {
  toolId: string
  label: string
  skillCount: number
  installed: boolean
}

export const getSkillsForTool = (
  managedSkills: ManagedSkill[],
  toolId: string,
) => {
  return managedSkills
    .filter((skill) => skill.targets.some((target) => target.tool === toolId))
    .sort((a, b) => (b.updated_at ?? 0) - (a.updated_at ?? 0))
}

export const buildToolSkillGroups = (
  managedSkills: ManagedSkill[],
  tools: ToolOption[],
  installedToolIds: string[],
): ToolSkillGroup[] => {
  const installed = new Set(installedToolIds)
  return tools.map((tool) => ({
    toolId: tool.id,
    label: tool.label,
    skillCount: managedSkills.filter((skill) =>
      skill.targets.some((target) => target.tool === tool.id),
    ).length,
    installed: installed.has(tool.id),
  }))
}
