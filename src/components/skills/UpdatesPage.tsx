import { memo, useEffect, useState } from 'react'
import {
  AlertTriangle,
  CalendarClock,
  RefreshCw,
} from 'lucide-react'
import type { TFunction } from 'i18next'
import type { AutoUpdateConfigDto, ManagedSkill } from './types'
import { historicalIssueState } from './skillIssueState'
import {
  getAutoUpdateTaskStatusKey,
  isAutoUpdatePossiblyStalled,
  parseAutoUpdateFailureItems,
  parseToolSyncPathChangedError,
  parseToolSyncTargetConflictError,
} from './autoUpdateSettings'

type AutoUpdateScheduleType = 'interval' | 'daily'
type AutoUpdateIntervalUnit = 'minutes' | 'hours'

type AutoUpdateScheduleInput = {
  scheduleType: AutoUpdateScheduleType
  intervalValue: number
  intervalUnit: AutoUpdateIntervalUnit
  dailyTime: string
}

type UpdatesPageProps = {
  skills?: ManagedSkill[]
  autoUpdateConfig: AutoUpdateConfigDto | null
  onAutoUpdateConfigChange: (
    enabled: boolean,
    schedule: AutoUpdateScheduleInput,
  ) => void
  onRunAutoUpdateNow: () => void
  autoUpdateTriggering: boolean
  t: TFunction
}

function formatUpdateFailure(reason: string, t: TFunction): string {
  if (reason.startsWith('TARGET_MODIFIED|')) {
    return t('errors.targetModified', {
      path: reason.slice('TARGET_MODIFIED|'.length),
    })
  }
  if (reason.startsWith('UPDATE_IN_PROGRESS|')) {
    return t('errors.updateInProgress')
  }
  if (reason.startsWith('CENTRAL_MODIFIED|')) {
    return t('errors.centralModified', {
      path: reason.slice('CENTRAL_MODIFIED|'.length),
    })
  }
  if (reason.startsWith('ROLLBACK_CONFLICT|')) {
    try {
      const detail = JSON.parse(reason.slice('ROLLBACK_CONFLICT|'.length)) as {
        target?: string
        recovery?: string
      }
      return t('errors.rollbackConflict', {
        target: detail.target ?? '',
        recovery: detail.recovery ?? '',
      })
    } catch {
      return t('errors.rollbackConflictUnknown')
    }
  }
  return reason
}

const UpdatesPage = ({
  skills = [],
  autoUpdateConfig,
  onAutoUpdateConfigChange,
  onRunAutoUpdateNow,
  autoUpdateTriggering,
  t,
}: UpdatesPageProps) => {
  const [currentTime, setCurrentTime] = useState(0)

  const autoUpdateEnabled = autoUpdateConfig?.enabled ?? false
  const autoUpdateInterval = autoUpdateConfig?.interval_hours ?? 24
  const autoUpdateScheduleType = autoUpdateConfig?.schedule_type ?? 'interval'
  const autoUpdateIntervalValue =
    autoUpdateConfig?.interval_value ?? autoUpdateInterval
  const autoUpdateIntervalUnit = autoUpdateConfig?.interval_unit ?? 'hours'
  const autoUpdateDailyTime = autoUpdateConfig?.daily_time ?? '03:00'
  const autoUpdateSchedule = {
    scheduleType: autoUpdateScheduleType,
    intervalValue: autoUpdateIntervalValue,
    intervalUnit: autoUpdateIntervalUnit,
    dailyTime: autoUpdateDailyTime,
  }
  const autoUpdateHasLocalSkills = (autoUpdateConfig?.local_skill_count ?? 0) > 0
  const autoUpdateHasProtectedLocalSkills =
    (autoUpdateConfig?.protected_local_skill_count ?? 0) > 0
  const autoUpdateRunning = autoUpdateConfig?.last_status === 'running'

  useEffect(() => {
    if (!autoUpdateRunning) return undefined
    const timer = window.setInterval(() => setCurrentTime(Date.now()), 1000)
    return () => window.clearInterval(timer)
  }, [autoUpdateRunning])

  const progressTime = currentTime || autoUpdateConfig?.last_run_at || 0
  const autoUpdateRunningLong =
    autoUpdateRunning &&
    Boolean(autoUpdateConfig?.last_run_at) &&
    progressTime - (autoUpdateConfig?.last_run_at ?? 0) > 10 * 60 * 1000
  const autoUpdateStalled = isAutoUpdatePossiblyStalled(autoUpdateConfig, progressTime)
  const autoUpdateButtonBusy =
    (autoUpdateRunning && !autoUpdateStalled) || autoUpdateTriggering
  const autoUpdateLastRun = autoUpdateConfig?.last_run_at
    ? new Date(autoUpdateConfig.last_run_at).toLocaleString()
    : t('autoUpdateNeverRun')
  const autoUpdateStartedAt = autoUpdateConfig?.last_started_at
    ? new Date(autoUpdateConfig.last_started_at).toLocaleString()
    : t('autoUpdateNeverRun')
  const autoUpdateFinishedAt = autoUpdateConfig?.last_finished_at
    ? new Date(autoUpdateConfig.last_finished_at).toLocaleString()
    : autoUpdateRunning
      ? t('autoUpdateStatus.running')
      : t('autoUpdateNeverRun')
  const autoUpdateDuration =
    autoUpdateConfig?.last_started_at && autoUpdateConfig?.last_finished_at
      ? formatDuration(autoUpdateConfig.last_finished_at - autoUpdateConfig.last_started_at)
      : autoUpdateRunning && autoUpdateConfig?.last_started_at
        ? formatDuration(progressTime - autoUpdateConfig.last_started_at)
        : t('autoUpdateNeverRun')
  const autoUpdateHasRuntime =
    autoUpdateRunning ||
    Boolean(autoUpdateConfig?.last_started_at) ||
    Boolean(autoUpdateConfig?.last_finished_at)
  const autoUpdateStatus = autoUpdateConfig?.last_status
    ? t(`autoUpdateStatus.${autoUpdateConfig.last_status}`)
    : t('autoUpdateStatus.none')
  const taskStatusKey = getAutoUpdateTaskStatusKey(
    autoUpdateEnabled,
    autoUpdateConfig?.task_registered ?? false,
  )
  const autoUpdateProgress = autoUpdateConfig?.progress
  const hasStructuredProgress =
    Boolean(autoUpdateProgress?.total) ||
    Boolean(autoUpdateProgress?.succeeded.length) ||
    Boolean(autoUpdateProgress?.failed.length) ||
    Boolean(autoUpdateProgress?.running) ||
    Boolean(autoUpdateProgress?.pending.length)
  const autoUpdateProgressForDisplay =
    autoUpdateProgress && hasStructuredProgress
      ? autoUpdateProgress
      : {
          total: autoUpdateConfig?.last_checked ?? 0,
          succeeded: [],
          failed: parseAutoUpdateFailureItems(autoUpdateConfig?.last_error),
          running: null,
          pending: [],
        }
  const autoUpdateStatusClass =
    autoUpdateConfig?.last_status === 'error' || autoUpdateConfig?.last_status === 'partial'
      ? 'error'
      : autoUpdateConfig?.last_status === 'ok'
        ? 'success'
        : autoUpdateRunning
          ? 'running'
          : 'idle'
  const autoUpdateScheduleHint =
    autoUpdateScheduleType === 'daily'
      ? t('autoUpdateDailyHint', { time: autoUpdateDailyTime })
      : t('autoUpdateIntervalHint', {
          value: autoUpdateIntervalValue,
          unit: t(
            autoUpdateIntervalUnit === 'minutes'
              ? 'autoUpdateIntervalUnitMinutes'
              : 'autoUpdateIntervalUnitHours',
          ),
        })
  const autoUpdateNotice = autoUpdateStalled
    ? t('autoUpdateStalledHint')
    : autoUpdateRunningLong
      ? t('autoUpdateLongRunningHint')
      : taskStatusKey === 'autoUpdateTaskNeedsAttention'
        ? t('autoUpdateTaskNeedsAttentionHint')
        : ''

  const handleCopyAutoUpdateError = async () => {
    if (!autoUpdateConfig?.last_error) return
    await navigator.clipboard.writeText(autoUpdateConfig.last_error)
  }

  return (
    <div className="updates-page">
      <div className="updates-dashboard">
        <section className={`updates-control-card${autoUpdateEnabled ? ' enabled' : ''}`}>
          <div className="updates-card-head">
            <div className="updates-card-title">
              <span className="updates-icon" aria-hidden="true">
                <RefreshCw size={16} />
              </span>
              <div>
                <span>{t('autoUpdateSkills')}</span>
                <strong>{t('autoUpdateSystemTask')}</strong>
              </div>
            </div>
            <span
              className={`updates-state-pill${autoUpdateEnabled ? ' enabled' : ''}`}
            >
              <span />
              {t(autoUpdateEnabled ? 'autoUpdateStateOn' : 'autoUpdateStateOff')}
            </span>
          </div>

          <div className="updates-control-main">
            <div>
              <p>{t('autoUpdateSystemTaskDesc')}</p>
              <div className="updates-task-line">
                {t('autoUpdateTaskStatus', { status: t(taskStatusKey) })}
              </div>
            </div>
            <button
              type="button"
              className={`settings-toggle updates-toggle${autoUpdateEnabled ? ' checked' : ''}`}
              aria-pressed={autoUpdateEnabled}
              onClick={() => {
                onAutoUpdateConfigChange(
                  !autoUpdateEnabled,
                  autoUpdateSchedule,
                )
              }}
            >
              <span className="settings-toggle-knob" />
            </button>
          </div>

          <div className="updates-schedule-panel">
            <div className="updates-section-label">
              <CalendarClock size={15} />
              {t('autoUpdateScheduleTitle')}
            </div>
            <div className="settings-segmented updates-segmented" role="group" aria-label={t('autoUpdateScheduleMode')}>
              <button
                type="button"
                className={autoUpdateScheduleType === 'interval' ? 'active' : ''}
                onClick={() => {
                  onAutoUpdateConfigChange(autoUpdateEnabled, {
                    ...autoUpdateSchedule,
                    scheduleType: 'interval',
                  })
                }}
              >
                {t('autoUpdateScheduleInterval')}
              </button>
              <button
                type="button"
                className={autoUpdateScheduleType === 'daily' ? 'active' : ''}
                onClick={() => {
                  onAutoUpdateConfigChange(autoUpdateEnabled, {
                    ...autoUpdateSchedule,
                    scheduleType: 'daily',
                  })
                }}
              >
                {t('autoUpdateScheduleDaily')}
              </button>
            </div>
            <div className="updates-schedule-row">
              {autoUpdateScheduleType === 'interval' ? (
                <>
                  <input
                    id="settings-auto-update-interval"
                    key={`interval-${autoUpdateIntervalUnit}-${autoUpdateIntervalValue}`}
                    className="settings-input"
                    type="number"
                    min={autoUpdateIntervalUnit === 'minutes' ? 15 : 1}
                    max={autoUpdateIntervalUnit === 'minutes' ? 43200 : 720}
                    step={1}
                    defaultValue={autoUpdateIntervalValue}
                    onBlur={(event) => {
                      const next = Number(event.currentTarget.value)
                      if (!Number.isNaN(next)) {
                        onAutoUpdateConfigChange(autoUpdateEnabled, {
                          ...autoUpdateSchedule,
                          intervalValue: next,
                        })
                      }
                    }}
                  />
                  <select
                    className="settings-select settings-unit-select"
                    value={autoUpdateIntervalUnit}
                    onChange={(event) => {
                      const nextUnit = event.target.value as AutoUpdateIntervalUnit
                      onAutoUpdateConfigChange(autoUpdateEnabled, {
                        ...autoUpdateSchedule,
                        intervalUnit: nextUnit,
                      })
                    }}
                  >
                    <option value="minutes">{t('autoUpdateIntervalUnitMinutes')}</option>
                    <option value="hours">{t('autoUpdateIntervalUnitHours')}</option>
                  </select>
                </>
              ) : (
                <input
                  id="settings-auto-update-daily-time"
                  key={`daily-${autoUpdateDailyTime}`}
                  className="settings-input"
                  type="time"
                  defaultValue={autoUpdateDailyTime}
                  onBlur={(event) => {
                    onAutoUpdateConfigChange(autoUpdateEnabled, {
                      ...autoUpdateSchedule,
                      dailyTime: event.currentTarget.value,
                    })
                  }}
                />
              )}
              <button
                className="btn btn-primary updates-run-btn"
                type="button"
                disabled={autoUpdateButtonBusy}
                onClick={onRunAutoUpdateNow}
              >
                <RefreshCw size={15} />
                {autoUpdateButtonBusy
                  ? t('autoUpdateRunningButton')
                  : autoUpdateStalled
                    ? t('autoUpdateRetryUpdate')
                    : t('autoUpdateRunNow')}
              </button>
            </div>
            <div className="updates-schedule-hint">{autoUpdateScheduleHint}</div>
          </div>
        </section>

        <aside className={`updates-summary-card ${autoUpdateStatusClass}`}>
          <p className="device-sync-no-change-note">{t('deviceSync.historicalRunNote')}</p>
          <div className="updates-summary-head">
            <span>{t('autoUpdateRunResultTitle')}</span>
            <strong>{autoUpdateStatus}</strong>
          </div>
          <div className="updates-summary-time">
            <span>{t('autoUpdateLastRunTitle')}</span>
            <strong>{autoUpdateLastRun}</strong>
          </div>
          <div className="updates-stat-grid">
            <div>
              <span>{t('autoUpdateCheckedShort')}</span>
              <strong>{autoUpdateConfig?.last_checked ?? 0}</strong>
              <small>{t('autoUpdateCheckedDescription')}</small>
            </div>
            <div>
              <span>{t('autoUpdateUnchangedShort')}</span>
              <strong>{autoUpdateConfig?.last_unchanged ?? 0}</strong>
              <small>{t('autoUpdateUnchangedDescription')}</small>
            </div>
            <div>
              <span>{t('autoUpdateUpdatedShort')}</span>
              <strong>{autoUpdateConfig?.last_updated ?? 0}</strong>
              <small>{t('autoUpdateUpdatedDescription')}</small>
            </div>
            <div className={(autoUpdateConfig?.last_failed ?? 0) > 0 ? 'danger' : ''}>
              <span>{t('autoUpdateFailedShort')}</span>
              <strong>{autoUpdateConfig?.last_failed ?? 0}</strong>
              <small>{t('autoUpdateFailedDescription')}</small>
            </div>
          </div>
          {autoUpdateHasRuntime ? (
            <div className="updates-runtime-grid">
              <div>
                <span>{t('autoUpdateStartedShort')}</span>
                <strong>{autoUpdateStartedAt}</strong>
              </div>
              <div>
                <span>{t('autoUpdateFinishedShort')}</span>
                <strong>{autoUpdateFinishedAt}</strong>
              </div>
              <div>
                <span>{t('autoUpdateProgressDuration')}</span>
                <strong>{autoUpdateDuration}</strong>
              </div>
            </div>
          ) : null}
        </aside>
      </div>

      <div className="updates-followups">
        {autoUpdateNotice ? (
          <div className="updates-notice">
            <AlertTriangle size={15} />
            <span>{autoUpdateNotice}</span>
          </div>
        ) : null}
        {autoUpdateProgressForDisplay.running ? (
          <div className="updates-current-item">
            <span>{t('autoUpdateProgressRunning')}</span>
            <strong>
              {autoUpdateProgressForDisplay.running.name ||
                autoUpdateProgressForDisplay.running.skill_id}
            </strong>
          </div>
        ) : null}
        {autoUpdateProgressForDisplay.succeeded.filter(item => item.reason?.startsWith('TOOLS_PENDING|')).map(item => <div className="updates-issue-item" key={`pending-${item.skill_id}`}><strong>{item.name || item.skill_id}</strong><p>{t('deviceSync.runToolsPending', { count: Number(item.reason?.split('|')[1]) || 0 })}</p></div>)}
        {autoUpdateProgressForDisplay.failed.length > 0 ? (
          <div className="updates-issue-block">
            <div className="updates-section-label">
              <AlertTriangle size={15} />
              {t('autoUpdateIssuesTitle')}
            </div>
            <div className="updates-issue-list">
              {autoUpdateProgressForDisplay.failed.map((item) => {
                const pathChanged = parseToolSyncPathChangedError(item.reason)
                const targetConflict = parseToolSyncTargetConflictError(item.reason)
                const reason = targetConflict
                  ? t('autoUpdateFailureToolTargetConflict', {
                      tool: targetConflict.tool,
                      path: targetConflict.expectedPath,
                    })
                  : pathChanged
                    ? t('autoUpdateFailureToolPathChanged', {
                        tool: pathChanged.tool,
                        path: pathChanged.expectedPath,
                      })
                    : item.reason
                      ? formatUpdateFailure(item.reason, t)
                      : null
                return (
                  <div className="updates-issue-item" key={item.skill_id}>
                    <strong>{item.name || item.skill_id}</strong>
                    <span>{t(`deviceSync.issueHistory.${historicalIssueState(skills.find(skill => skill.id === item.skill_id), autoUpdateConfig?.last_finished_at ?? autoUpdateConfig?.last_run_at ?? 0)}`)}</span>
                    {reason ? <code>{reason}</code> : null}
                  </div>
                )
              })}
            </div>
            {autoUpdateConfig?.last_error ? (
              <button
                className="btn btn-secondary btn-sm"
                type="button"
                onClick={() => {
                  void handleCopyAutoUpdateError()
                }}
              >
                {t('copyDetails')}
              </button>
            ) : null}
          </div>
        ) : null}
        {autoUpdateHasLocalSkills ? (
          <div className="updates-permission-note">
            {t(
              autoUpdateHasProtectedLocalSkills
                ? 'autoUpdateLocalPermissionProtectedHint'
                : 'autoUpdateLocalPermissionHint',
            )}
          </div>
        ) : null}
      </div>
    </div>
  )
}

function formatDuration(durationMs: number) {
  const totalSeconds = Math.max(0, Math.round(durationMs / 1000))
  const hours = Math.floor(totalSeconds / 3600)
  const minutes = Math.floor((totalSeconds % 3600) / 60)
  const seconds = totalSeconds % 60
  if (hours > 0) {
    return `${hours}h ${minutes}m ${seconds}s`
  }
  if (minutes > 0) {
    return `${minutes}m ${seconds}s`
  }
  return `${seconds}s`
}

export default memo(UpdatesPage)
