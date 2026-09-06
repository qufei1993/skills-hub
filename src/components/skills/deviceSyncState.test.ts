import { describe, expect, it, vi } from 'vitest'
import {
  buildDeviceSyncForm,
  getSyncFailureKind,
  selectSyncRepository,
  changeSyncRepositoryUrl,
  isDeviceSyncScheduleValid,
  classifyRepositoryLoadFailure,
  getDeviceSyncExperience,
  getDeviceSyncRunOutcome,
  reduceRepositoryPicker,
  shouldLoadRepositoryPicker,
  getRepositoryDisplayName,
  getDeviceSyncControls,
  getDeviceConnectionState,
  getOtherDeviceSummary,
  getDeviceSyncSetupProgress,
  filterRepositories,
  withTimeout,
} from './deviceSyncState'

it('recognizes protected tool edits without exposing raw paths', () => {
  expect(getSyncFailureKind('DEVICE_SYNC_FAILURE_targetModified')).toBe('targetModified')
  expect(getSyncFailureKind('TARGET_MODIFIED|/private/path')).toBe('unknown')
})

describe('device sync UI state', () => {
  it('binds provider visibility to the selected repository and forgets it after URL edits', () => {
    const form = buildDeviceSyncForm();
    const selected = selectSyncRepository(form, {
      name: 'sync', clone_url: 'https://github.com/example/sync.git', web_url: '',
      private: false, visibility: 'public',
    })
    expect(selected.visibility).toBe('public')
    expect(selected.publicUploadConfirmed).toBe(false)
    const changed = changeSyncRepositoryUrl({ ...selected, publicUploadConfirmed: true }, 'https://github.com/example/other.git')
    expect(changed.visibility).toBe('unknown')
    expect(changed.publicUploadConfirmed).toBe(false)
    expect(selectSyncRepository(form, { name: 'sync', clone_url: 'https://example/sync.git', web_url: '', private: false, visibility: 'internal' }).visibility).toBe('internal')
  })
  it('rejects invalid intervals and times before saving', () => {
    for (const minutes of [0, 4, 5.5, NaN, 43201]) {
      expect(isDeviceSyncScheduleValid({ mode: 'interval', minutes })).toBe(false)
    }
    expect(isDeviceSyncScheduleValid({ mode: 'interval', minutes: 5 })).toBe(true)
    for (const time of ['', '9:00', '24:00', '12:60']) {
      expect(isDeviceSyncScheduleValid({ mode: 'daily', time })).toBe(false)
    }
    expect(isDeviceSyncScheduleValid({ mode: 'daily', time: '09:00' })).toBe(true)
  })
  it('defaults to GitHub with all startup credential access disabled', () => {
    expect(buildDeviceSyncForm()).toEqual({
      visibility: 'unknown',
      publicUploadConfirmed: false,
      provider: 'github',
      remoteUrl: '',
      branch: 'main',
      username: '',
      token: '',
      oauthCredentialKey: '',
      accountLogin: '',
      autoCheck: false,
      autoSync: false,
      schedule: { mode: 'interval', minutes: 15 },
    })
  })

  it('loads saved configuration without exposing the stored credential', () => {
    expect(
      buildDeviceSyncForm({
        provider: 'gitee',
        remote_url: 'https://gitee.com/user/sync.git',
        branch: 'skills',
        username: 'user',
        auto_check: false,
        auto_sync: true,
        has_credential: true,
      }),
    ).toMatchObject({
      provider: 'gitee',
      remoteUrl: 'https://gitee.com/user/sync.git',
      token: '',
      autoSync: true,
    })
  })

  it('disables conflicting actions while a request is running and validates inputs', () => {
    expect(
      getDeviceSyncControls({
        configured: true,
        busy: true,
        remoteUrl: 'https://example/sync.git',
        token: 'token',
        hasCredential: false,
      }),
    ).toEqual({
      canCheck: false,
      canSync: false,
      canSave: false,
      canCreateRepository: false,
    })
    expect(
      getDeviceSyncControls({
        configured: false,
        busy: false,
        remoteUrl: '',
        token: '',
        hasCredential: false,
      }),
    ).toEqual({
      canCheck: false,
      canSync: false,
      canSave: false,
      canCreateRepository: false,
    })
  })

  it('times out a repository request that never settles', async () => {
    vi.useFakeTimers()
    const result = withTimeout(new Promise<string>(() => undefined), 100)
    const assertion = expect(result).rejects.toThrow('DEVICE_SYNC_REPOSITORY_LOAD_TIMEOUT')

    await vi.advanceTimersByTimeAsync(100)
    await assertion
    vi.useRealTimers()
  })

  it('classifies timeout and system credential failures', () => {
    expect(classifyRepositoryLoadFailure(new Error('request timed out'))).toBe('timeout')
    expect(
      classifyRepositoryLoadFailure(
        new Error('Platform secure storage failure: User canceled the operation'),
      ),
    ).toBe('credential')
    expect(classifyRepositoryLoadFailure(new Error('server error'))).toBe('generic')
  })

  it('derives a readable repository name from HTTPS and SSH URLs', () => {
    expect(getRepositoryDisplayName('https://github.com/user/skills-hub-sync.git')).toBe(
      'skills-hub-sync',
    )
    expect(getRepositoryDisplayName('git@gitlab.com:user/private-skills.git')).toBe(
      'private-skills',
    )
  })

  it('routes an unauthorized user to account authorization', () => {
    expect(
      getDeviceSyncExperience({
        configured: false,
        authorized: false,
        hasRepository: false,
        conflictCount: 0,
        busyAction: null,
      }),
    ).toEqual({
      mode: 'setup',
      setupStep: 'authorization',
      syncState: 'disconnected',
      defaultActivity: 'devices',
    })
  })

  it('routes an authorized user without a repository to repository selection', () => {
    expect(
      getDeviceSyncExperience({
        configured: false,
        authorized: true,
        hasRepository: false,
        conflictCount: 0,
        busyAction: null,
      }),
    ).toEqual({
      mode: 'setup',
      setupStep: 'repository',
      syncState: 'disconnected',
      defaultActivity: 'devices',
    })
  })

  it('advances setup only after authorization and repository selection', () => {
    expect(getDeviceSyncSetupProgress(false, false)).toEqual({
      currentStep: 1,
      completedSteps: [],
    })
    expect(getDeviceSyncSetupProgress(true, false)).toEqual({
      currentStep: 2,
      completedSteps: [1],
    })
    expect(getDeviceSyncSetupProgress(true, true)).toEqual({
      currentStep: 3,
      completedSteps: [1, 2],
    })
  })

  it('opens conflicts first when connected sync needs user attention', () => {
    expect(
      getDeviceSyncExperience({
        configured: true,
        authorized: true,
        hasRepository: true,
        conflictCount: 2,
        busyAction: null,
      }),
    ).toEqual({
      mode: 'dashboard',
      setupStep: 'connected',
      syncState: 'conflicts',
      defaultActivity: 'conflicts',
    })
  })

  it('shows the running state while a connected sync is in progress', () => {
    expect(
      getDeviceSyncExperience({
        configured: true,
        authorized: true,
        hasRepository: true,
        conflictCount: 0,
        busyAction: 'sync',
      }),
    ).toEqual({
      mode: 'dashboard',
      setupStep: 'connected',
      syncState: 'syncing',
      defaultActivity: 'devices',
    })
  })

  it('shows the running state reported by a background sync', () => {
    expect(
      getDeviceSyncExperience({
        configured: true,
        authorized: true,
        hasRepository: true,
        conflictCount: 0,
        busyAction: null,
        isRunning: true,
      }).syncState,
    ).toBe('syncing')
  })

  it('shows the running state during the first repository sync', () => {
    expect(
      getDeviceSyncExperience({
        configured: true,
        authorized: true,
        hasRepository: true,
        conflictCount: 0,
        busyAction: 'initial-sync',
      }).syncState,
    ).toBe('syncing')
  })

  it('does not report a failed or pending repository as synced', () => {
    expect(
      getDeviceSyncExperience({
        configured: true,
        authorized: true,
        hasRepository: true,
        conflictCount: 0,
        busyAction: null,
        lastRunStatus: 'failed',
        pendingChanges: 0,
      }).syncState,
    ).toBe('failed')
    expect(
      getDeviceSyncExperience({
        configured: true,
        authorized: true,
        hasRepository: true,
        conflictCount: 0,
        busyAction: null,
        lastRunStatus: 'success',
        pendingChanges: 2,
      }).syncState,
    ).toBe('changes')
  })

  it('distinguishes a completed sync from one that needs conflict resolution', () => {
    expect(getDeviceSyncRunOutcome('ok')).toBe('complete')
    expect(getDeviceSyncRunOutcome('conflicts')).toBe('conflicts')
  })

  it('counts only other devices in the route summary', () => {
    expect(
      getOtherDeviceSummary([
        { is_current: true, state: 'synced' },
        { is_current: false, state: 'synced' },
        { is_current: false, state: 'pending' },
      ]),
    ).toEqual({ count: 2, pendingCount: 1 })
  })

  it('lets the repository picker expand and collapse independently of loaded data', () => {
    expect(reduceRepositoryPicker(false, 'toggle')).toBe(true)
    expect(reduceRepositoryPicker(true, 'toggle')).toBe(false)
    expect(reduceRepositoryPicker(true, 'close')).toBe(false)
  })

  it('closes the repository picker after selecting a repository', () => {
    expect(reduceRepositoryPicker(true, 'select')).toBe(false)
  })

  it('filters repositories by name without changing their order', () => {
    const repositories = [
      { name: 'skills-hub-sync', clone_url: 'https://example/sync.git', web_url: 'https://example/sync', private: true },
      { name: 'article-reviewer', clone_url: 'https://example/reviewer.git', web_url: 'https://example/reviewer', private: true },
      { name: 'AI-Article', clone_url: 'https://example/article.git', web_url: 'https://example/article', private: true },
    ]

    expect(filterRepositories(repositories, 'article').map(({ name }) => name)).toEqual([
      'article-reviewer',
      'AI-Article',
    ])
    expect(filterRepositories(repositories, '  ')).toBe(repositories)
  })

  it('does not start a duplicate repository request when reopening during loading', () => {
    expect(shouldLoadRepositoryPicker(true, 'idle')).toBe(true)
    expect(shouldLoadRepositoryPicker(true, 'error')).toBe(true)
    expect(shouldLoadRepositoryPicker(true, 'loading')).toBe(false)
    expect(shouldLoadRepositoryPicker(true, 'loaded')).toBe(false)
    expect(shouldLoadRepositoryPicker(false, 'idle')).toBe(false)
  })

  it('classifies device freshness using repository version and last activity', () => {
    const now = Date.UTC(2026, 8, 3)
    expect(getDeviceConnectionState({ last_commit: 'latest', last_seen_at: now }, 'latest', now)).toBe('synced')
    expect(getDeviceConnectionState({ last_commit: 'older', last_seen_at: now }, 'latest', now)).toBe('pending')
    expect(getDeviceConnectionState({ last_commit: 'latest', last_seen_at: now - 8 * 86_400_000 }, 'latest', now)).toBe('stale')
  })
})
