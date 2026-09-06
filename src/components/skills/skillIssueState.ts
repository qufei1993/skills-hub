import type { ManagedSkill } from './types'
import { getSkillSyncState } from './skillSyncStatus'

const kinds = ['sourceMissing', 'repoPathMissing', 'modified', 'unsafeTarget', 'disk', 'permission', 'auth', 'network', 'recheck']
export function issueReasonKey(code?: string | null) {
  const kind = code?.replace(/^SKILL_ISSUE\|/, '')
  return `deviceSync.issueReason.${kind && kinds.includes(kind) ? kind : 'unknown'}`
}

export function historicalIssueState(skill: ManagedSkill | undefined, finishedAt: number) {
  if (!skill) return 'removed'
  if (!skill.enabled) return 'disabled'
  if (['source-error', 'partial', 'failed'].includes(getSkillSyncState(skill))) return 'pending'
  if (finishedAt > 0 && (skill.source_checked_at ?? 0) >= finishedAt) return 'recovered'
  return 'unverified'
}
