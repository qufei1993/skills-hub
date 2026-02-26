import type { GitSkillCandidate } from './types'

const normalizeSkillToken = (value: string) =>
  value
    .trim()
    .toLowerCase()
    .replace(/[_\s]+/g, '-')

const subpathLeaf = (subpath: string) => {
  const parts = subpath.split('/').filter(Boolean)
  return parts.length > 0 ? parts[parts.length - 1] : subpath
}

export const pickLeaderboardCandidateSubpath = (
  candidates: GitSkillCandidate[],
  skillSlug?: string,
  skillName?: string,
): string | null => {
  const normalizedSlug = normalizeSkillToken(skillSlug ?? '')
  const normalizedName = normalizeSkillToken(skillName ?? '')

  if (normalizedSlug) {
    const bySlug = candidates.find((candidate) => {
      const leaf = normalizeSkillToken(subpathLeaf(candidate.subpath))
      const name = normalizeSkillToken(candidate.name)
      return leaf === normalizedSlug || name === normalizedSlug
    })
    if (bySlug) {
      return bySlug.subpath
    }
  }

  if (normalizedName) {
    const byName = candidates.find((candidate) => {
      const leaf = normalizeSkillToken(subpathLeaf(candidate.subpath))
      const name = normalizeSkillToken(candidate.name)
      return leaf === normalizedName || name === normalizedName
    })
    if (byName) {
      return byName.subpath
    }
  }

  return null
}
