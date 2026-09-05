import { describe, expect, it } from 'vitest'
import {
  getAutoUpdateProgressCounts,
  getAutoUpdateRunningReset,
  getAutoUpdateTaskStatusKey,
  getAutoUpdateToastKey,
  isAutoUpdatePossiblyStalled,
  parseAutoUpdateFailureItems,
  parseToolSyncPathChangedError,
  parseToolSyncTargetConflictError,
  shouldKeepWaitingForTriggeredAutoUpdate,
  summarizeAutoUpdateErrors,
} from './autoUpdateSettings'

describe('getAutoUpdateRunningReset', () => {
  it('clears every outcome count before polling the new run', () => {
    expect(getAutoUpdateRunningReset()).toEqual({
      last_checked: 0,
      last_unchanged: 0,
      last_updated: 0,
      last_failed: 0,
    })
  })
})

describe('getAutoUpdateToastKey', () => {
  it('uses enable and disable messages only when the toggle changes', () => {
    expect(getAutoUpdateToastKey(false, true)).toBe('autoUpdateEnabled')
    expect(getAutoUpdateToastKey(true, false)).toBe('autoUpdateDisabled')
  })

  it('uses a neutral saved message when only the interval changes', () => {
    expect(getAutoUpdateToastKey(false, false)).toBe('autoUpdateConfigSaved')
    expect(getAutoUpdateToastKey(true, true)).toBe('autoUpdateConfigSaved')
  })
})

describe('summarizeAutoUpdateErrors', () => {
  it('groups noisy raw update errors into user-facing categories', () => {
    const result = summarizeAutoUpdateErrors([
      's1: source path not found: "/Users/may/Downloads/demo"',
      's2: git 命令执行失败。git 操作超时 (300s)。error: RPC failed; curl 28 Operation too slow',
      's3: fatal: unable to access github.com port 443',
    ].join('\n'))

    expect(result.total).toBe(3)
    expect(result.summaries).toEqual([
      { key: 'autoUpdateErrorSourceMissing', count: 1 },
      { key: 'autoUpdateErrorNetwork', count: 2 },
    ])
  })

  it('returns no summaries for empty errors', () => {
    expect(summarizeAutoUpdateErrors('').summaries).toEqual([])
  })

  it('classifies a stale tool sync path separately', () => {
    const result = summarizeAutoUpdateErrors(
      'skill-a: TOOL_SYNC_PATH_CHANGED|Kimi Code CLI|/Users/may/.kimi-code/skills/skill-a',
    )

    expect(result.summaries).toEqual([
      { key: 'autoUpdateErrorToolPathChanged', count: 1 },
    ])
  })

  it('classifies a conflicting current tool target separately', () => {
    const result = summarizeAutoUpdateErrors(
      'skill-a: TOOL_SYNC_TARGET_CONFLICT|Kimi Code CLI|/Users/may/.kimi-code/skills/skill-a',
    )

    expect(result.summaries).toEqual([
      { key: 'autoUpdateErrorToolPathChanged', count: 1 },
    ])
  })
})

describe('parseToolSyncPathChangedError', () => {
  it('extracts the tool and current target path for an actionable message', () => {
    expect(
      parseToolSyncPathChangedError(
        'TOOL_SYNC_PATH_CHANGED|Kimi Code CLI|/Users/may/.kimi-code/skills/demo',
      ),
    ).toEqual({
      tool: 'Kimi Code CLI',
      expectedPath: '/Users/may/.kimi-code/skills/demo',
    })
  })

  it('leaves unrelated failures untouched', () => {
    expect(parseToolSyncPathChangedError('network timeout')).toBeNull()
  })
})

describe('parseToolSyncTargetConflictError', () => {
  it('extracts a conflicting target without exposing the protocol code', () => {
    expect(
      parseToolSyncTargetConflictError(
        'TOOL_SYNC_TARGET_CONFLICT|Kimi Code CLI|/Users/may/.kimi-code/skills/demo',
      ),
    ).toEqual({
      tool: 'Kimi Code CLI',
      expectedPath: '/Users/may/.kimi-code/skills/demo',
    })
  })
})

describe('getAutoUpdateTaskStatusKey', () => {
  it('hides registration details when auto-update is off', () => {
    expect(getAutoUpdateTaskStatusKey(false, false)).toBe('autoUpdateTaskOff')
    expect(getAutoUpdateTaskStatusKey(false, true)).toBe('autoUpdateTaskOff')
  })

  it('shows user-level readiness instead of platform implementation details', () => {
    expect(getAutoUpdateTaskStatusKey(true, true)).toBe('autoUpdateTaskReady')
    expect(getAutoUpdateTaskStatusKey(true, false)).toBe(
      'autoUpdateTaskNeedsAttention',
    )
  })
})

describe('getAutoUpdateProgressCounts', () => {
  it('counts completed and remaining structured progress items', () => {
    expect(getAutoUpdateProgressCounts({
      total: 4,
      succeeded: [{ skill_id: 'a', name: 'A' }],
      failed: [{ skill_id: 'b', name: 'B', reason: 'network timeout' }],
      running: { skill_id: 'c', name: 'C' },
      pending: [{ skill_id: 'd', name: 'D' }],
    })).toEqual({
      total: 4,
      completed: 2,
      succeeded: 1,
      failed: 1,
      active: 2,
    })
  })
})

describe('parseAutoUpdateFailureItems', () => {
  it('extracts failed skill ids and reasons from legacy raw error text', () => {
    expect(parseAutoUpdateFailureItems([
      'skill-a: source path not found: "/tmp/a"',
      'skill-b: git clone failed',
    ].join('\n'))).toEqual([
      {
        skill_id: 'skill-a',
        name: 'skill-a',
        reason: 'source path not found: "/tmp/a"',
      },
      {
        skill_id: 'skill-b',
        name: 'skill-b',
        reason: 'git clone failed',
      },
    ])
  })
})

describe('shouldKeepWaitingForTriggeredAutoUpdate', () => {
  it('keeps waiting when polling still sees an old completed run', () => {
    expect(shouldKeepWaitingForTriggeredAutoUpdate(
      { last_status: 'error', last_run_at: 1_000 },
      2_000,
      false,
    )).toBe(true)
  })

  it('keeps waiting while the triggered run is running', () => {
    expect(shouldKeepWaitingForTriggeredAutoUpdate(
      { last_status: 'running', last_run_at: 2_000 },
      2_000,
      false,
    )).toBe(true)
  })

  it('stops after a run has been observed running and then completes', () => {
    expect(shouldKeepWaitingForTriggeredAutoUpdate(
      { last_status: 'ok', last_run_at: 3_000 },
      2_000,
      true,
    )).toBe(false)
  })

  it('stops when the latest completed run started after the trigger', () => {
    expect(shouldKeepWaitingForTriggeredAutoUpdate(
      { last_status: 'error', last_run_at: 2_500 },
      2_000,
      false,
    )).toBe(false)
  })
})

describe('isAutoUpdatePossiblyStalled', () => {
  it('detects a long-running update with no current item and no progress', () => {
    expect(isAutoUpdatePossiblyStalled({
      last_status: 'running',
      last_run_at: 1_000,
      last_updated: 0,
      last_failed: 0,
      progress: {
        total: 60,
        succeeded: [],
        failed: [],
        running: null,
        pending: [],
      },
    }, 1_000 + 11 * 60 * 1000)).toBe(true)
  })

  it('does not mark a run with an active current item as stalled', () => {
    expect(isAutoUpdatePossiblyStalled({
      last_status: 'running',
      last_run_at: 1_000,
      last_updated: 0,
      last_failed: 0,
      progress: {
        total: 60,
        succeeded: [],
        failed: [],
        running: { skill_id: 'a', name: 'A' },
        pending: [],
      },
    }, 1_000 + 11 * 60 * 1000)).toBe(false)
  })

  it('does not mark a run with an unchanged completed item as stalled', () => {
    expect(isAutoUpdatePossiblyStalled({
      last_status: 'running',
      last_run_at: 1_000,
      last_unchanged: 1,
      last_updated: 0,
      last_failed: 0,
      progress: {
        total: 60,
        succeeded: [{ skill_id: 'a', name: 'A' }],
        failed: [],
        running: null,
        pending: [],
      },
    }, 1_000 + 11 * 60 * 1000)).toBe(false)
  })
})
