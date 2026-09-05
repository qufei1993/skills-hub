import type {
  AutoUpdateConfigDto,
  AutoUpdateProgressSnapshotDto,
  AutoUpdateSkillProgressDto,
} from './types'

export type AutoUpdateToastKey =
  | 'autoUpdateEnabled'
  | 'autoUpdateDisabled'
  | 'autoUpdateConfigSaved'

export type AutoUpdateTaskStatusKey =
  | 'autoUpdateTaskOff'
  | 'autoUpdateTaskReady'
  | 'autoUpdateTaskNeedsAttention'

export function getAutoUpdateRunningReset() {
  return {
    last_checked: 0,
    last_unchanged: 0,
    last_updated: 0,
    last_failed: 0,
  }
}

export function getAutoUpdateToastKey(
  previousEnabled: boolean | null | undefined,
  nextEnabled: boolean,
): AutoUpdateToastKey {
  if (previousEnabled === true && !nextEnabled) {
    return 'autoUpdateDisabled'
  }
  if (previousEnabled === false && nextEnabled) {
    return 'autoUpdateEnabled'
  }
  return 'autoUpdateConfigSaved'
}

export function getAutoUpdateTaskStatusKey(
  enabled: boolean,
  taskRegistered: boolean,
): AutoUpdateTaskStatusKey {
  if (!enabled) {
    return 'autoUpdateTaskOff'
  }
  if (taskRegistered) {
    return 'autoUpdateTaskReady'
  }
  return 'autoUpdateTaskNeedsAttention'
}

export type AutoUpdateErrorSummary = {
  key: string
  count: number
}

export function summarizeAutoUpdateErrors(rawError: string | null | undefined) {
  const lines = (rawError ?? '')
    .split('\n')
    .map((line) => line.trim())
    .filter(Boolean)

  const summaries = new Map<string, number>()
  for (const line of lines) {
    const key = classifyAutoUpdateError(line)
    summaries.set(key, (summaries.get(key) ?? 0) + 1)
  }

  return {
    total: lines.length,
    summaries: Array.from(summaries.entries()).map(([key, count]) => ({
      key,
      count,
    })),
  }
}

export function getAutoUpdateProgressCounts(
  progress: AutoUpdateProgressSnapshotDto | null | undefined,
) {
  const succeeded = progress?.succeeded.length ?? 0
  const failed = progress?.failed.length ?? 0
  const total = progress?.total ?? succeeded + failed
  const running = progress?.running ? 1 : 0
  const pending = progress?.pending.length ?? Math.max(0, total - succeeded - failed - running)

  return {
    total,
    completed: succeeded + failed,
    succeeded,
    failed,
    active: running + pending,
  }
}

export function parseAutoUpdateFailureItems(
  rawError: string | null | undefined,
): AutoUpdateSkillProgressDto[] {
  return (rawError ?? '')
    .split('\n')
    .map((line) => line.trim())
    .filter(Boolean)
    .map((line) => {
      const separatorIndex = line.indexOf(':')
      if (separatorIndex <= 0) {
        return {
          skill_id: line,
          name: line,
          reason: line,
        }
      }
      const skillId = line.slice(0, separatorIndex).trim()
      const reason = line.slice(separatorIndex + 1).trim()
      return {
        skill_id: skillId,
        name: skillId,
        reason,
      }
    })
}

export function shouldKeepWaitingForTriggeredAutoUpdate(
  config: Pick<AutoUpdateConfigDto, 'last_run_at' | 'last_status'>,
  triggeredAtMs: number,
  sawRunning: boolean,
) {
  if (config.last_status === 'running') {
    return true
  }
  if (sawRunning) {
    return false
  }
  if (typeof config.last_run_at === 'number' && config.last_run_at >= triggeredAtMs) {
    return false
  }
  return true
}

export function isAutoUpdatePossiblyStalled(
  config: (Pick<
    AutoUpdateConfigDto,
    'last_run_at' | 'last_status' | 'last_updated' | 'last_failed' | 'progress'
  > & { last_unchanged?: number }) | null | undefined,
  nowMs: number,
  staleAfterMs = 10 * 60 * 1000,
) {
  if (!config?.last_run_at || config.last_status !== 'running') {
    return false
  }
  const completed =
    (config.last_unchanged ?? 0) + config.last_updated + config.last_failed
  return (
    nowMs - config.last_run_at > staleAfterMs &&
    completed === 0 &&
    !config.progress?.running
  )
}

function classifyAutoUpdateError(line: string) {
  const lower = line.toLowerCase()
  if (
    lower.includes('source path not found') ||
    lower.includes('central path not found')
  ) {
    return 'autoUpdateErrorSourceMissing'
  }
  if (
    lower.includes('failed to connect to github.com') ||
    lower.includes('unable to access') ||
    lower.includes('operation too slow') ||
    lower.includes('unexpected disconnect') ||
    lower.includes('early eof') ||
    lower.includes('git operation timed out') ||
    lower.includes('git 命令执行失败')
  ) {
    return 'autoUpdateErrorNetwork'
  }
  if (lower.includes('rate limit')) {
    return 'autoUpdateErrorRateLimited'
  }
  return 'autoUpdateErrorOther'
}
