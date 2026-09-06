import { expect, it } from 'vitest'
import type { ManagedSkill } from './types'
import { historicalIssueState, issueReasonKey } from './skillIssueState'

const skill: ManagedSkill = { id: 'one', name: 'one', enabled: true, status: 'ok', source_type: 'local', central_path: '/central', created_at: 0, updated_at: 0, tags: [], targets: [] }
it('does not interpret cloud or tool success as source recovery', () => {
  expect(historicalIssueState({ ...skill, targets: [{ tool: 'one', scope: 'global', target_path: '/tool', status: 'ok', mode: 'copy', synced_at: 300 }] }, 100)).toBe('unverified')
  expect(historicalIssueState({ ...skill, status: 'error', source_checked_at: 300 }, 100)).toBe('pending')
  expect(historicalIssueState({ ...skill, source_checked_at: 300 }, 100)).toBe('recovered')
  expect(historicalIssueState({ ...skill, source_checked_at: 50 }, 100)).toBe('unverified')
})
it('only renders known diagnostic categories', () => {
  expect(issueReasonKey('SKILL_ISSUE|modified')).toBe('deviceSync.issueReason.modified')
  expect(issueReasonKey('https://token:secret@example.com')).toBe('deviceSync.issueReason.unknown')
})
