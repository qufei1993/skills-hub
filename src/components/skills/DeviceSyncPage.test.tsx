// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import type { TFunction } from 'i18next'
import { toast } from 'sonner'
import { afterEach, describe, expect, it, vi } from 'vitest'
import DeviceSyncPage from './DeviceSyncPage'

const invokeMock = vi.hoisted(() => vi.fn())

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }))
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => undefined) }))
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn() }))

describe('DeviceSyncPage', () => {
  it('explains a no-change run above historical failures', async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === 'get_device_sync_config') return Promise.resolve({ provider: 'github', remote_url: 'https://github.com/example/sync.git', branch: 'main', has_credential: true })
      if (command === 'get_device_sync_status') return Promise.resolve({ configured: true, last_run_status: 'unchanged', last_run_at: 2000, conflict_count: 0 })
      if (command === 'get_device_sync_history') return Promise.resolve([{ id: 'old', started_at: 1000, finished_at: 1000, status: 'failed', error: 'DEVICE_SYNC_FAILURE_auth' }])
      if (command === 'get_device_sync_pending_oauth') return Promise.resolve(null)
      return Promise.resolve([])
    })
    render(<DeviceSyncPage active isTauri onSkillsChanged={vi.fn(async () => undefined)} onConflictCountChange={vi.fn()} onOpenToolIssues={vi.fn()} t={((key: string) => key) as TFunction} />)
    expect(await screen.findByText('deviceSync.confirmedInSync')).toBeTruthy()
    expect(screen.getByText('deviceSync.lastConfirmedValue')).toBeTruthy()
    fireEvent.click(screen.getByRole('tab', { name: 'deviceSync.history' }))
    expect(screen.getByText('deviceSync.noChangeHistoryNote')).toBeTruthy()
    expect(screen.getAllByText('deviceSync.status.failed').length).toBeGreaterThan(0)
    expect(screen.queryByText('deviceSync.visualState.healthy')).toBeNull()
  })
  it('keeps cloud success visible and offers navigation for pending tools', async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === 'get_device_sync_config') return Promise.resolve({ provider: 'github', remote_url: 'https://github.com/example/sync.git', branch: 'main', has_credential: true, visibility: 'private' })
      if (command === 'get_device_sync_status') return Promise.resolve({ configured: true, is_running: false, last_run_status: 'success', last_run_at: 2000, conflict_count: 0, tool_issues: [{ skill_name: 'example', tool: 'hermes' }] })
      if (command === 'get_device_sync_pending_oauth') return Promise.resolve(null)
      return Promise.resolve([])
    })
    const open = vi.fn()
    render(<DeviceSyncPage active isTauri onSkillsChanged={vi.fn(async () => undefined)} onConflictCountChange={vi.fn()} onOpenToolIssues={open} t={((key: string) => key) as TFunction} />)
    expect(await screen.findByText('deviceSync.visualState.healthy')).toBeTruthy()
    expect(screen.queryByText('example')).toBeNull()
    const details = screen.getByRole('button', { name: 'deviceSync.showToolIssues' })
    expect(details.getAttribute('aria-expanded')).toBe('false')
    fireEvent.click(details)
    expect(screen.getByText('example')).toBeTruthy()
    expect(screen.getByText('hermes')).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: 'deviceSync.hideToolIssues' }))
    expect(screen.queryByText('example')).toBeNull()
    fireEvent.click(screen.getByRole('button', { name: 'deviceSync.openToolIssues' }))
    expect(open).toHaveBeenCalledOnce()
    expect(screen.queryByText('deviceSync.failureReasons.targetModified')).toBeNull()
  })
  it('refreshes the saved failure immediately after a failed manual sync', async () => {
    let failed = false
    invokeMock.mockImplementation((command: string) => {
      if (command === 'get_device_sync_config') return Promise.resolve({ provider: 'github', remote_url: 'https://github.com/example/sync.git', branch: 'main', has_credential: true, visibility: 'private' })
      if (command === 'get_device_sync_status') return Promise.resolve({ configured: true, is_running: false, last_run_status: failed ? 'failed' : 'success', last_run_at: 2000, conflict_count: 0 })
      if (command === 'get_device_sync_history') return Promise.resolve(failed ? [{ id: 'failed', started_at: 1000, finished_at: 2000, status: 'failed', added: 0, updated: 0, deleted: 0, conflicted: 0, error: 'DEVICE_SYNC_FAILURE_auth' }] : [])
      if (command === 'run_device_sync') { failed = true; return Promise.reject('DEVICE_SYNC_FAILURE_auth') }
      if (command === 'get_device_sync_pending_oauth') return Promise.resolve(null)
      return Promise.resolve([])
    })
    const t = ((key: string) => key) as TFunction
    render(<DeviceSyncPage onOpenToolIssues={vi.fn()} active isTauri onSkillsChanged={vi.fn(async () => undefined)} onConflictCountChange={vi.fn()} t={t} />)
    await screen.findByText('deviceSync.visualState.healthy')
    fireEvent.click(screen.getByRole('button', { name: 'deviceSync.syncLocalRepository' }))
    expect(await screen.findByText('deviceSync.failureReasons.auth')).toBeTruthy()
  })
  it.each(['DEVICE_SYNC_FAILURE_network', null, 'Authorization: Bearer do-not-display'])('shows safe persistent failure details for %s', async (error) => {
    invokeMock.mockImplementation((command: string) => {
      if (command === 'get_device_sync_config') return Promise.resolve({ provider: 'github', remote_url: 'https://github.com/example/sync.git', branch: 'main', has_credential: true, visibility: 'private' })
      if (command === 'get_device_sync_status') return Promise.resolve({ configured: true, is_running: false, last_run_status: 'failed', last_run_at: 2000, conflict_count: 0 })
      if (command === 'get_device_sync_history') return Promise.resolve([{ id: 'failed', started_at: 1000, finished_at: 2000, status: 'failed', added: 0, updated: 0, deleted: 0, conflicted: 0, error }])
      if (command === 'get_device_sync_pending_oauth') return Promise.resolve(null)
      return Promise.resolve([])
    })
    const t = ((key: string) => key) as TFunction
    render(<DeviceSyncPage onOpenToolIssues={vi.fn()} active isTauri onSkillsChanged={vi.fn(async () => undefined)} onConflictCountChange={vi.fn()} t={t} />)
    const reason = error === 'DEVICE_SYNC_FAILURE_network' ? 'network' : 'unknown'
    expect(await screen.findByText(`deviceSync.failureReasons.${reason}`)).toBeTruthy()
    fireEvent.click(screen.getByRole('tab', { name: 'deviceSync.history' }))
    const details = screen.getByText('deviceSync.failureDetails').closest('details')!
    fireEvent.click(within(details).getByText('deviceSync.failureDetails'))
    expect(details.open).toBe(true)
    expect(within(details).getByText(`deviceSync.failureReasons.${reason}`)).toBeTruthy()
    expect(screen.queryByText('deviceSync.previewSummary')).toBeNull()
    expect(document.body.textContent).not.toContain('do-not-display')
  })
  it('shows the saved schedule and backend deadline, and opens automation without credential calls', async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === 'get_device_sync_config') return Promise.resolve({
        provider: 'github', remote_url: 'https://github.com/example/sync.git', branch: 'main',
        username: 'example', auto_check: false, auto_sync: true,
        auto_sync_schedule: { mode: 'interval', minutes: 15 }, visibility: 'private', public_upload_confirmed: false, has_credential: true,
      })
      if (command === 'get_device_sync_status') return Promise.resolve({
        configured: true, is_running: false, provider: 'github', remote_url: 'https://github.com/example/sync.git',
        auto_check: false, auto_sync: true, pending_local_changes: 0, conflict_count: 0,
        last_run_status: 'success', last_run_at: 1000,
        schedule_status: { state: 'scheduled', next_at: 1780000000000 },
      })
      if (command === 'get_device_sync_pending_oauth') return Promise.resolve(null)
      return Promise.resolve([])
    })
    const t = ((key: string, options?: { count?: number; value?: string }) =>
      `${key}${options?.count !== undefined ? ` ${options.count}` : ''}${options?.value ? ` ${options.value}` : ''}`) as TFunction
    render(<DeviceSyncPage onOpenToolIssues={vi.fn()} active isTauri onSkillsChanged={vi.fn(async () => undefined)} onConflictCountChange={vi.fn()} t={t} />)
    const summary = await screen.findByRole('region', { name: 'deviceSync.scheduleSummary' })
    expect(within(summary).getByText('deviceSync.scheduleInterval 15')).toBeTruthy()
    expect(summary.querySelector('time')?.dateTime).toBe('2026-05-28T20:26:40.000Z')
    const before = invokeMock.mock.calls.length
    fireEvent.click(within(summary).getByRole('button', { name: 'deviceSync.editSchedule' }))
    const automation = screen.getByRole('region', { name: 'deviceSync.automation' })
    expect(document.activeElement).toBe(automation)
    fireEvent.change(screen.getByRole('spinbutton', { name: 'deviceSync.intervalMinutes' }), { target: { value: '30' } })
    expect(within(summary).getByText('deviceSync.scheduleInterval 15')).toBeTruthy()
    expect(invokeMock.mock.calls.length).toBe(before)
    fireEvent.click(screen.getByText('deviceSync.advancedSettings'))
    expect(screen.queryByRole('combobox', { name: 'deviceSync.repositoryVisibility' })).toBeNull()
    expect(screen.getByLabelText('deviceSync.repositoryVisibility').textContent).toBe('deviceSync.visibility.private')
    const errorToast = vi.spyOn(toast, 'error')
    fireEvent.change(screen.getByLabelText('deviceSync.remoteUrl'), { target: { value: 'https://github.com/example/other.git' } })
    expect(screen.getByLabelText('deviceSync.repositoryVisibility').textContent).toBe('deviceSync.visibility.unknown')
    fireEvent.click(screen.getByRole('button', { name: 'deviceSync.saveChanges' }))
    expect(errorToast).toHaveBeenCalledWith('deviceSync.visibilityUnknownHelp')
    expect(invokeMock.mock.calls.length).toBe(before)
  })
  afterEach(() => {
    cleanup()
    vi.restoreAllMocks()
    invokeMock.mockReset()
  })
  it.each([
    { state: 'disabled', enabled: false, next: null, label: 'deviceSync.scheduleHint.disabled' },
    { state: 'paused', enabled: true, next: 1780000000000, label: 'deviceSync.scheduleHint.paused' },
    { state: 'backoff', enabled: true, next: 1780000000000, label: 'deviceSync.scheduleRetryAt' },
    { state: 'waiting', enabled: true, next: null, label: 'deviceSync.scheduleHint.waiting' },
    { state: 'running', enabled: true, next: 1780000000000, label: 'deviceSync.scheduleHint.running' },
  ])('renders daily schedule state $state without suggesting a false deadline', async ({ state, enabled, next, label }) => {
    invokeMock.mockImplementation((command: string) => {
      if (command === 'get_device_sync_config') return Promise.resolve({
        provider: 'github', remote_url: 'https://github.com/example/sync.git', branch: 'main',
        username: 'example', auto_check: false, auto_sync: enabled,
        auto_sync_schedule: { mode: 'daily', time: '09:00' }, visibility: 'private', public_upload_confirmed: false, has_credential: true,
      })
      if (command === 'get_device_sync_status') return Promise.resolve({
        configured: true, is_running: state === 'running', provider: 'github',
        remote_url: 'https://github.com/example/sync.git', auto_check: false, auto_sync: enabled,
        pending_local_changes: 0, conflict_count: state === 'paused' ? 1 : 0,
        last_run_status: 'success', last_run_at: 1000,
        schedule_status: { state, next_at: next },
      })
      if (command === 'get_device_sync_pending_oauth') return Promise.resolve(null)
      return Promise.resolve([])
    })
    const t = ((key: string, options?: { value?: string }) => key === 'deviceSync.scheduleDaily' ? `${key} ${options?.value}` : key) as TFunction
    render(<DeviceSyncPage onOpenToolIssues={vi.fn()} active isTauri onSkillsChanged={vi.fn(async () => undefined)} onConflictCountChange={vi.fn()} t={t} />)
    const summary = await screen.findByRole('region', { name: 'deviceSync.scheduleSummary' })
    expect(within(summary).getByText(label)).toBeTruthy()
    expect(Boolean(summary.querySelector('time'))).toBe(state === 'backoff')
    expect(Boolean(within(summary).queryByText('deviceSync.scheduleDaily 09:00'))).toBe(enabled)
  })
  it('shows a recognizable icon for every supported Git platform', () => {
    const t = ((key: string) => key) as TFunction

    render(
      <DeviceSyncPage onOpenToolIssues={vi.fn()}
        active={false}
        isTauri={false}
        onSkillsChanged={vi.fn(async () => undefined)}
        onConflictCountChange={vi.fn()}
        t={t}
      />,
    )

    for (const provider of ['GitHub', 'GitLab', 'Gitee']) {
      expect(
        screen
          .getByRole('button', { name: provider, hidden: true })
          .querySelector('svg'),
      ).toBeTruthy()
    }
  })

  it('keeps a large repository list collapsible and closes it after selection', async () => {
    const repositories = Array.from({ length: 12 }, (_, index) => ({
      name: `repository-${index}`,
      clone_url: `https://github.com/example/repository-${index}.git`,
      private: true,
    }))
    invokeMock.mockImplementation((command: string) => {
      if (command === 'get_device_sync_config') return Promise.resolve(null)
      if (command === 'get_device_sync_status') return Promise.resolve({ is_running: false })
      if (command === 'get_device_sync_oauth_availability') {
        return Promise.resolve([{ provider: 'github', available: true }])
      }
      if (command === 'get_device_sync_pending_oauth') {
        return Promise.resolve({
          provider: 'github',
          credential_key: 'github:test',
          account: { login: 'example' },
        })
      }
      if (command === 'list_device_sync_repositories') return Promise.resolve(repositories)
      return Promise.resolve([])
    })
    const t = ((key: string) => key) as TFunction

    render(
      <DeviceSyncPage onOpenToolIssues={vi.fn()}
        active
        isTauri
        onSkillsChanged={vi.fn(async () => undefined)}
        onConflictCountChange={vi.fn()}
        t={t}
      />,
    )

    fireEvent.click(await screen.findByRole('button', { name: 'deviceSync.loadRepositories' }))
    expect(await screen.findByRole('searchbox', { name: 'deviceSync.searchRepositories' })).toBeTruthy()
    expect(screen.getByText('repository-11')).toBeTruthy()

    fireEvent.click(screen.getByText('repository-11'))
    await waitFor(() => {
      expect(screen.queryByRole('searchbox', { name: 'deviceSync.searchRepositories' })).toBeNull()
    })
    expect(screen.getByText('repository-11')).toBeTruthy()
  })

  it.each([false, true])('keeps help local and shows exchange motion only when backend running is %s', async (isRunning) => {
    invokeMock.mockImplementation((command: string) => {
      if (command === 'get_device_sync_config') {
        return Promise.resolve({
          provider: 'github',
          remote_url: 'https://github.com/example/skills-hub-sync.git',
          branch: 'main',
          username: 'example',
          auto_check: false,
          auto_sync: false,
          visibility: 'private', public_upload_confirmed: false, has_credential: true,
        })
      }
      if (command === 'get_device_sync_status') {
        return Promise.resolve({
          configured: true,
          is_running: isRunning,
          provider: 'github',
          remote_url: 'https://github.com/example/skills-hub-sync.git',
          auto_check: false,
          auto_sync: false,
          last_synced_commit: 'abc123',
          repository_head_commit: 'abc123',
          pending_local_changes: 0,
          conflict_count: 0,
          last_run_status: 'success',
          last_run_at: 1_780_000_000,
        })
      }
      if (command === 'get_device_sync_devices') {
        return Promise.resolve([
          {
            id: 'current-device',
            name: 'Office Mac',
            last_commit: 'abc123',
            last_seen_at: 1_780_000_000,
            is_current: true,
          },
        ])
      }
      if (command === 'get_device_sync_oauth_availability') {
        return Promise.resolve([{ provider: 'github', available: true }])
      }
      if (command === 'get_device_sync_pending_oauth') return Promise.resolve(null)
      return Promise.resolve([])
    })
    const t = ((key: string) => key) as TFunction

    render(
      <DeviceSyncPage onOpenToolIssues={vi.fn()}
        active
        isTauri
        onSkillsChanged={vi.fn(async () => undefined)}
        onConflictCountChange={vi.fn()}
        t={t}
      />,
    )

    const trigger = await screen.findByRole('button', {
      name: 'deviceSync.syncHelpTrigger',
    })
    const callsBeforeOpening = invokeMock.mock.calls.length

    const exchange = screen.getByRole('group', { name: 'deviceSync.localRepositoryExchange' })
    expect(exchange.getAttribute('aria-busy')).toBe(String(isRunning))
    expect(exchange.classList.contains('is-syncing')).toBe(isRunning)
    expect(screen.getByText('deviceSync.independentSyncNote')).toBeTruthy()

    fireEvent.click(trigger)

    expect(
      screen.getByRole('dialog', { name: 'deviceSync.syncHelpTitle' }),
    ).toBeTruthy()
    expect(invokeMock).toHaveBeenCalledTimes(callsBeforeOpening)

    fireEvent.click(
      screen.getByRole('button', { name: 'deviceSync.syncHelpClose' }),
    )
    expect(
      screen.queryByRole('dialog', { name: 'deviceSync.syncHelpTitle' }),
    ).toBeNull()

    fireEvent.click(trigger)
    fireEvent.keyDown(window, { key: 'Escape' })
    expect(
      screen.queryByRole('dialog', { name: 'deviceSync.syncHelpTitle' }),
    ).toBeNull()
    expect(invokeMock).toHaveBeenCalledTimes(callsBeforeOpening)
    fireEvent.click(screen.getByRole('button', { name: 'deviceSync.syncSettings' }))
    expect(screen.getByRole('dialog', { name: 'deviceSync.syncSettings' })).toBeTruthy()
    fireEvent.click(screen.getByRole('checkbox', { name: /deviceSync.autoSync/ }))
    const minutes = screen.getByRole('spinbutton', { name: 'deviceSync.intervalMinutes' }) as HTMLInputElement
    expect(minutes.value).toBe('15')
    fireEvent.change(minutes, { target: { value: '4' } })
    expect(screen.getByRole('alert').textContent).toBe('deviceSync.invalidSchedule')
    const callsBeforeInvalidSave = invokeMock.mock.calls.length
    fireEvent.click(screen.getByRole('button', { name: 'deviceSync.saveChanges' }))
    expect(invokeMock.mock.calls.length).toBe(callsBeforeInvalidSave)
    fireEvent.click(screen.getByRole('button', { name: 'autoUpdateScheduleDaily' }))
    expect((screen.getByLabelText('deviceSync.dailyTime') as HTMLInputElement).value).toBe('09:00')
    expect(screen.queryByRole('alert')).toBeNull()
    fireEvent.keyDown(window, { key: 'Escape' })
    expect(screen.queryByRole('dialog', { name: 'deviceSync.syncSettings' })).toBeNull()

    if (!isRunning) {
      const infoToast = vi.spyOn(toast, 'info')
      let finishCheck!: (value: unknown) => void
      invokeMock.mockImplementationOnce(() => new Promise((resolve) => { finishCheck = resolve }))
      fireEvent.click(screen.getByRole('button', { name: 'deviceSync.check' }))
      expect(exchange.classList.contains('is-syncing')).toBe(false)
      expect(exchange.getAttribute('aria-busy')).toBe('false')
      finishCheck({ added: 0, updated: 0, deleted: 0, conflicted: 0 })
      await waitFor(() => expect((screen.getByRole('button', { name: 'deviceSync.syncLocalRepository' }) as HTMLButtonElement).disabled).toBe(false))

      let finishSync!: (value: unknown) => void
      invokeMock.mockImplementationOnce(() => new Promise((resolve) => { finishSync = resolve }))
      fireEvent.click(screen.getByRole('button', { name: 'deviceSync.syncLocalRepository' }))
      expect(exchange.classList.contains('is-syncing')).toBe(true)
      finishSync({ status: 'success', changes: { added: 0, updated: 0, deleted: 0, conflicted: 0 }, message: 'device sync completed' })
      await waitFor(() => expect(exchange.classList.contains('is-syncing')).toBe(false))
      expect(infoToast).toHaveBeenCalledWith('deviceSync.alreadyInSync')
    }
  })
})
