import { describe, expect, it } from 'vitest'
import { getSkillSyncState, getToolSyncState } from './skillSyncStatus'
import type { ManagedSkill } from './types'

type Target = ManagedSkill['targets'][number]

const target = (
  status: string,
  tool = 'codex',
  scope: 'global' | 'project' = 'global',
): Target => ({
  tool,
  scope,
  mode: 'copy',
  status,
  target_path: `/tmp/${tool}`,
})

describe('getSkillSyncState', () => {
  it('distinguishes disabled, idle, healthy, partial, and failed skills', () => {
    expect(getSkillSyncState({ enabled: false, targets: [target('ok')] })).toBe('disabled')
    expect(getSkillSyncState({ enabled: true, targets: [] })).toBe('idle')
    expect(getSkillSyncState({ enabled: true, targets: [target('ok')] })).toBe('healthy')
    expect(
      getSkillSyncState({ enabled: true, targets: [target('ok'), target('error', 'cursor')] }),
    ).toBe('partial')
    expect(getSkillSyncState({ enabled: true, targets: [target('error')] })).toBe('failed')
  })

  it('ignores disabled targets when calculating health', () => {
    expect(
      getSkillSyncState({ enabled: true, targets: [target('ok'), target('disabled', 'cursor')] }),
    ).toBe('healthy')
  })
})

describe('getToolSyncState', () => {
  it('uses only matching active targets in the selected scope', () => {
    const skill = {
      targets: [
        target('error'),
        target('ok', 'codex', 'project'),
        target('disabled', 'cursor'),
      ],
    }
    expect(getToolSyncState(skill, 'codex', 'global')).toBe('failed')
    expect(getToolSyncState(skill, 'codex', 'project')).toBe('synced')
    expect(getToolSyncState(skill, 'cursor', 'global')).toBe('not-synced')
  })

  it('reports partial when project targets have mixed results', () => {
    const skill = {
      targets: [target('ok', 'codex', 'project'), target('error', 'codex', 'project')],
    }
    expect(getToolSyncState(skill, 'codex', 'project')).toBe('partial')
  })
})
