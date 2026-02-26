import type { ManagedSkill } from './types'

const normalizeSkillName = (name: string) => name.trim().toLowerCase()

export const buildInstalledSkillNameSet = (
  managedSkills: Pick<ManagedSkill, 'name'>[],
): Set<string> => {
  const out = new Set<string>()
  for (const skill of managedSkills) {
    out.add(normalizeSkillName(skill.name))
  }
  return out
}

export const isLeaderboardEntryInstalled = (
  entry: Pick<{ name: string }, 'name'>,
  installedNames: Set<string>,
): boolean => installedNames.has(normalizeSkillName(entry.name))
