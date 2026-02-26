import { describe, expect, it } from 'vitest'
import type { GitSkillCandidate } from './types'
import { pickLeaderboardCandidateSubpath } from './leaderboardInstallResolver'

describe('leaderboardInstallResolver', () => {
  const candidates: GitSkillCandidate[] = [
    {
      name: 'Find Skills',
      subpath: 'skills/find-skills',
    },
    {
      name: 'Requesting Code Review',
      subpath: 'skills/requesting-code-review',
    },
  ]

  it('matches by leaderboard skill slug against subpath segment', () => {
    expect(
      pickLeaderboardCandidateSubpath(candidates, 'requesting-code-review', 'whatever'),
    ).toBe('skills/requesting-code-review')
  })

  it('falls back to matching by normalized skill name', () => {
    expect(
      pickLeaderboardCandidateSubpath(candidates, 'unknown-slug', 'find skills'),
    ).toBe('skills/find-skills')
  })

  it('returns null when no candidate matches', () => {
    expect(
      pickLeaderboardCandidateSubpath(candidates, 'missing', 'also-missing'),
    ).toBeNull()
  })
})
