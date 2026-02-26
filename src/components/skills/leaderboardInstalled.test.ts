import { describe, expect, it } from 'vitest'
import type { ManagedSkill } from './types'
import { buildInstalledSkillNameSet, isLeaderboardEntryInstalled } from './leaderboardInstalled'

const managedSkill = (name: string): ManagedSkill => ({
  id: name,
  name,
  source_type: 'git',
  source_ref: null,
  central_path: `/hub/${name}`,
  created_at: 1,
  updated_at: 1,
  status: 'active',
  targets: [],
})

describe('leaderboardInstalled helpers', () => {
  it('matches installed leaderboard skill by normalized name', () => {
    const installedNames = buildInstalledSkillNameSet([
      managedSkill('Find Skills'),
      managedSkill('code-reviewer'),
    ])

    expect(
      isLeaderboardEntryInstalled({ name: '  find skills ' }, installedNames),
    ).toBe(true)
    expect(
      isLeaderboardEntryInstalled({ name: 'Code-Reviewer' }, installedNames),
    ).toBe(true)
    expect(
      isLeaderboardEntryInstalled({ name: 'not-installed' }, installedNames),
    ).toBe(false)
  })
})
