import type {
  DeviceSyncConfigDto,
  DeviceSyncProvider,
  DeviceSyncRemoteRepository,
  DeviceSyncSchedule,
} from './types'

export const REPOSITORY_LOAD_TIMEOUT_MS = 30_000
const REPOSITORY_LOAD_TIMEOUT_ERROR = 'DEVICE_SYNC_REPOSITORY_LOAD_TIMEOUT'

export type RepositoryLoadFailure = 'timeout' | 'credential' | 'generic'

export const withTimeout = <T>(promise: Promise<T>, timeoutMs: number): Promise<T> =>
  new Promise((resolve, reject) => {
    const timer = setTimeout(
      () => reject(new Error(REPOSITORY_LOAD_TIMEOUT_ERROR)),
      timeoutMs,
    )
    promise.then(
      (value) => {
        clearTimeout(timer)
        resolve(value)
      },
      (error: unknown) => {
        clearTimeout(timer)
        reject(error)
      },
    )
  })

export const classifyRepositoryLoadFailure = (error: unknown): RepositoryLoadFailure => {
  const message = String(error).toLowerCase()
  if (message.includes(REPOSITORY_LOAD_TIMEOUT_ERROR.toLowerCase()) || message.includes('timed out')) {
    return 'timeout'
  }
  if (
    message.includes('keychain') ||
    message.includes('credential') ||
    message.includes('secure storage') ||
    message.includes('user canceled') ||
    message.includes('user cancelled')
  ) {
    return 'credential'
  }
  return 'generic'
}

export type DeviceSyncFormState = {
  provider: DeviceSyncProvider
  remoteUrl: string
  branch: string
  username: string
  token: string
  oauthCredentialKey: string
  accountLogin: string
  autoCheck: boolean
  autoSync: boolean
  schedule: DeviceSyncSchedule
}

export const buildDeviceSyncForm = (
  config?: DeviceSyncConfigDto | null,
): DeviceSyncFormState => ({
  provider: config?.provider ?? 'github',
  remoteUrl: config?.remote_url ?? '',
  branch: config?.branch ?? 'main',
  username: config?.username ?? '',
  token: '',
  oauthCredentialKey: '',
  accountLogin: '',
  autoCheck: config?.auto_check ?? false,
  autoSync: config?.auto_sync ?? false,
  schedule: config?.auto_sync_schedule ?? { mode: 'interval', minutes: 15 },
})

export const isDeviceSyncScheduleValid = (schedule: DeviceSyncSchedule): boolean =>
  schedule.mode === 'interval'
    ? Number.isInteger(schedule.minutes) && schedule.minutes >= 5 && schedule.minutes <= 43200
    : /^([01]\d|2[0-3]):[0-5]\d$/.test(schedule.time)

export const getRepositoryDisplayName = (remoteUrl: string): string => {
  const normalized = remoteUrl.trim().replace(/\/+$/, '').replace(/\.git$/, '')
  const match = normalized.match(/(?:[:/])([^/:]+)$/)
  return match?.[1] || normalized
}

export type DeviceSyncActivity = 'devices' | 'history' | 'conflicts' | 'trash'
export type DeviceConnectionState = 'synced' | 'pending' | 'stale'

export const getOtherDeviceSummary = (
  devices: { is_current: boolean; state: DeviceConnectionState }[],
) => {
  const otherDevices = devices.filter(({ is_current }) => !is_current)
  return {
    count: otherDevices.length,
    pendingCount: otherDevices.filter(({ state }) => state === 'pending').length,
  }
}
export type DeviceSyncSetupStep = 'authorization' | 'repository' | 'connected'

export const getDeviceSyncSetupProgress = (
  authorized: boolean,
  hasRepository: boolean,
): { currentStep: 1 | 2 | 3; completedSteps: number[] } => {
  if (!authorized) return { currentStep: 1, completedSteps: [] }
  if (!hasRepository) return { currentStep: 2, completedSteps: [1] }
  return { currentStep: 3, completedSteps: [1, 2] }
}
export type DeviceSyncVisualState =
  | 'disconnected'
  | 'healthy'
  | 'changes'
  | 'failed'
  | 'conflicts'
  | 'syncing'
export type DeviceSyncRunOutcome = 'complete' | 'conflicts'
export type RepositoryPickerEvent = 'toggle' | 'close' | 'select'
export type RepositoryLoadState =
  | 'idle'
  | 'loading'
  | 'loaded'
  | 'error'
  | 'timeout'
  | 'credential-error'

export const getDeviceSyncRunOutcome = (status: string): DeviceSyncRunOutcome =>
  status === 'conflicts' ? 'conflicts' : 'complete'

export const reduceRepositoryPicker = (
  current: boolean,
  event: RepositoryPickerEvent,
): boolean => (event === 'toggle' ? !current : false)

export const shouldLoadRepositoryPicker = (
  opening: boolean,
  loadState: RepositoryLoadState,
): boolean => opening && loadState !== 'loading' && loadState !== 'loaded'

export const filterRepositories = (
  repositories: DeviceSyncRemoteRepository[],
  query: string,
): DeviceSyncRemoteRepository[] => {
  const normalizedQuery = query.trim().toLocaleLowerCase()
  if (!normalizedQuery) return repositories
  return repositories.filter(({ name }) =>
    name.toLocaleLowerCase().includes(normalizedQuery),
  )
}

export const getDeviceConnectionState = (
  device: { last_commit?: string | null; last_seen_at: number },
  latestCommit: string | null | undefined,
  now = Date.now(),
): DeviceConnectionState => {
  if (now - device.last_seen_at > 7 * 86_400_000) return 'stale'
  return latestCommit && device.last_commit !== latestCommit ? 'pending' : 'synced'
}

export const getDeviceSyncExperience = ({
  configured,
  authorized,
  hasRepository,
  conflictCount,
  busyAction,
  isRunning,
  lastRunStatus,
  pendingChanges = 0,
}: {
  configured: boolean
  authorized: boolean
  hasRepository: boolean
  conflictCount: number
  busyAction: string | null
  isRunning?: boolean
  lastRunStatus?: string | null
  pendingChanges?: number
}) => {
  const connected = configured && hasRepository
  const syncing =
    busyAction === 'sync' || busyAction === 'initial-sync' || isRunning === true
  return {
    mode: connected ? ('dashboard' as const) : ('setup' as const),
    setupStep: connected
      ? ('connected' as const)
      : authorized
        ? ('repository' as const)
        : ('authorization' as const),
    syncState: connected
      ? syncing
        ? ('syncing' as const)
        : conflictCount > 0
          ? ('conflicts' as const)
          : lastRunStatus === 'failed'
            ? ('failed' as const)
            : pendingChanges > 0
              ? ('changes' as const)
              : ('healthy' as const)
      : ('disconnected' as const),
    defaultActivity: conflictCount > 0
      ? ('conflicts' as DeviceSyncActivity)
      : ('devices' as DeviceSyncActivity),
  }
}

export const getDeviceSyncControls = ({
  configured,
  busy,
  remoteUrl,
  token,
  hasCredential,
}: {
  configured: boolean
  busy: boolean
  remoteUrl: string
  token: string
  hasCredential: boolean
}) => ({
  canCheck: configured && !busy,
  canSync: configured && !busy,
  canSave: !busy && remoteUrl.trim().length > 0,
  canCreateRepository: !busy && (hasCredential || token.trim().length > 0),
})
