import { memo, useEffect, useId, useState } from 'react'
import {
  AlertTriangle,
  CalendarClock,
  Clock3,
  ChevronDown,
  Info,
  RefreshCw,
  Download,
  Copy,
} from 'lucide-react'
import type { TFunction } from 'i18next'
import { toast } from 'sonner'
import type { AutoUpdateConfigDto, ManagedSkill } from './types'
import './updates-content.css'
import UpdateCandidates from './UpdateCandidates'
import type { useUpdateCheck } from './workspace/useUpdateCheck'
import type { UpdateCandidate } from './workspace/contentCheck'
import {
  getAutoUpdateTaskStatusKey,
  isAutoUpdatePossiblyStalled,
  parseAutoUpdateFailureItems,
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
  recoverableIds?: string[]
  updateCheck: ReturnType<typeof useUpdateCheck>
  onReview: (plans: UpdateCandidate[]) => void
  recoveryError?: boolean
  onRetryRecoveries?: () => void
  onRestore?: (skill: ManagedSkill) => void
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

const UpdatesPage = ({
  skills = [],
  recoverableIds = [],
  updateCheck,
  onReview,
  recoveryError,
  onRetryRecoveries,
  onRestore,
  autoUpdateConfig,
  onAutoUpdateConfigChange,
  onRunAutoUpdateNow,
  autoUpdateTriggering,
  t,
}: UpdatesPageProps) => {
  const [currentTime, setCurrentTime] = useState(0)
  const [selected, setSelected] = useState<string[]>([])
  const updateHintId = useId()
  const scheduleHeadingId = useId()
  const resultHeadingId = useId()

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
  const selectedPlans = updateCheck.state.plans.filter(plan => selected.includes(plan.id))
  const updatePlans = selectedPlans.length ? selectedPlans : updateCheck.state.plans
  const checkReady = updateCheck.state.checkedAt !== null && !updateCheck.state.fromCache &&
    !updateCheck.state.cancelled && !updateCheck.state.stale && !updateCheck.state.lastApply?.reloadError
  const updateBlocked = autoUpdateButtonBusy || updateCheck.state.phase !== 'idle' || !checkReady || !updatePlans.length
  const updateHint = t(`updatesWorkflow.${updateCheck.state.eligibleCount === 0 ? 'noSkillsHint' : !checkReady ? 'updateCheckRequired' : !updatePlans.length ? 'updateNoChanges' : 'updateScopeHint'}`)
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
  const autoUpdateHasRun = Boolean(autoUpdateConfig?.last_status || autoUpdateConfig?.last_run_at || autoUpdateHasRuntime)
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
      ? 'is-error'
      : autoUpdateConfig?.last_status === 'ok'
        ? 'is-success'
        : autoUpdateRunning
          ? 'is-running'
          : 'is-idle'
  const autoUpdateScheduleHint =
    autoUpdateScheduleType === 'daily'
      ? t('autoUpdateDailyShort', { time: autoUpdateDailyTime })
      : t('autoUpdateIntervalShort', {
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

  const handleCopyRunLog = async () => {
    if (!autoUpdateConfig || !autoUpdateHasRun) return
    try {
      await navigator.clipboard.writeText(JSON.stringify({
        lastRunAt: autoUpdateConfig.last_run_at,
        startedAt: autoUpdateConfig.last_started_at,
        finishedAt: autoUpdateConfig.last_finished_at,
        status: autoUpdateConfig.last_status,
        checked: autoUpdateConfig.last_checked,
        unchanged: autoUpdateConfig.last_unchanged,
        updated: autoUpdateConfig.last_updated,
        failed: autoUpdateConfig.last_failed,
        progress: autoUpdateProgressForDisplay,
        lastError: autoUpdateConfig.last_error,
      }, null, 2))
      toast.success(t('copied'))
    } catch {
      toast.error(t('copyFailed'))
    }
  }

  return (
    <div className="updates-page">
      <header className="updates-page-header">
        <div>
          <h1>{t('manageTabs.updates')}</h1>
          <p>{t('updatesLayout.description')}</p>
        </div>
        <div className="updates-header-controls">
          <div className="updates-header-actions">
            <button className="btn btn-secondary updates-run-btn" type="button"
              disabled={autoUpdateButtonBusy || updateCheck.state.phase !== 'idle' || updateCheck.state.eligibleCount === 0}
              aria-busy={updateCheck.state.phase === 'checking'}
              onClick={() => { void updateCheck.check() }}>
              <RefreshCw size={15} aria-hidden="true" />
              {t('updatesWorkflow.check')}
            </button>
            <button className="btn btn-primary updates-run-btn" type="button" disabled={updateBlocked}
              aria-busy={updateCheck.state.phase === 'applying'} aria-describedby={updateHintId}
              onClick={() => { void updateCheck.update(updatePlans) }}>
              <Download size={15} aria-hidden="true" />
              {updateCheck.state.phase === 'applying' ? t('updatesWorkflow.applying') : t(`updatesWorkflow.${selectedPlans.length ? 'updateSelected' : 'updateAll'}`, { count: updatePlans.length })}
            </button>
          </div>
          <p className="updates-action-hint" id={updateHintId}>{updateHint}</p>
        </div>
      </header>
      {autoUpdateNotice ? (
        <div className="updates-notice" role="status">
          <AlertTriangle size={16} aria-hidden="true" />
          <span>{autoUpdateNotice}</span>
        </div>
      ) : null}
      {autoUpdateProgressForDisplay.running ? (
        <div className="updates-current-item" role="status">
          <RefreshCw size={16} aria-hidden="true" />
          <span>{t('autoUpdateProgressRunning')}</span>
          <strong>{autoUpdateProgressForDisplay.running.name || autoUpdateProgressForDisplay.running.skill_id}</strong>
        </div>
      ) : null}
      <div className="updates-layout">

        <div className="updates-content">
          <UpdateCandidates check={updateCheck} selectedIds={selected} onSelectionChange={setSelected} skills={skills} recoverableIds={recoverableIds} onReview={onReview} onRestore={onRestore} blocked={autoUpdateButtonBusy} recoveryError={recoveryError} onRetryRecoveries={onRetryRecoveries} t={t} />
          {autoUpdateHasLocalSkills ? <details className="updates-permission-note"><summary><Info size={14} aria-hidden="true" />{t('updatesLayout.sourcePermissions')}</summary><p>{t(autoUpdateHasProtectedLocalSkills ? 'autoUpdateLocalPermissionProtectedHint' : 'autoUpdateLocalPermissionHint')}</p></details> : null}
        </div>
        <div className="updates-rail">
          <section className="updates-control-card" aria-labelledby={scheduleHeadingId}>
            <div className="updates-card-head">
              <h2 id={scheduleHeadingId}><CalendarClock size={18} aria-hidden="true" />{t('autoUpdateSkills')}</h2>
              <button
                type="button"
                className={`settings-toggle updates-toggle${autoUpdateEnabled ? ' checked' : ''}`}
                aria-label={t('autoUpdateSkills')}
                aria-pressed={autoUpdateEnabled}
                disabled={updateCheck.state.phase !== 'idle'}
                onClick={() => onAutoUpdateConfigChange(!autoUpdateEnabled, autoUpdateSchedule)}
              >
                <span className="settings-toggle-knob" />
              </button>
            </div>
            <p className="updates-automation-description">{t('updatesLayout.automationDescription')}</p>
            <div className={`updates-task-line${taskStatusKey === 'autoUpdateTaskNeedsAttention' ? ' attention' : ''}`}>
              <span className={`updates-task-dot${taskStatusKey === 'autoUpdateTaskReady' ? ' ready' : ''}`} aria-hidden="true" />
              {t('autoUpdateTaskStatus', { status: t(taskStatusKey) })}
            </div>

            <details className="updates-schedule-panel">
              <summary>
                <span>{t('autoUpdateScheduleTitle')}</span>
                <span className="updates-schedule-value">{autoUpdateScheduleHint}</span>
                <ChevronDown size={14} aria-hidden="true" />
              </summary>
              <div className="settings-segmented updates-segmented" role="group" aria-label={t('autoUpdateScheduleMode')}>
                <button
                  type="button"
                  className={autoUpdateScheduleType === 'interval' ? 'active' : ''}
                  aria-pressed={autoUpdateScheduleType === 'interval'}
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
                  aria-pressed={autoUpdateScheduleType === 'daily'}
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
                      aria-label={t('updatesLayout.intervalValue')}
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
                      aria-label={t('updatesLayout.intervalUnit')}
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
                    aria-label={t('updatesLayout.dailyTime')}
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

              </div>
              <div className="updates-run-plan"><p>{t('updatesWorkflow.runPlanHint')}</p><button className="btn btn-secondary btn-sm" type="button" disabled={autoUpdateButtonBusy || updateCheck.state.phase !== 'idle'} onClick={onRunAutoUpdateNow}>{t(autoUpdateButtonBusy ? 'autoUpdateRunningButton' : autoUpdateStalled ? 'autoUpdateRetryUpdate' : 'updatesWorkflow.runPlan')}</button></div>
            </details>
          </section>

          <section className={`updates-summary-card ${autoUpdateStatusClass}`} aria-labelledby={resultHeadingId}>
            <div className="updates-summary-head">
              <h2 id={resultHeadingId}><Clock3 size={18} aria-hidden="true" />{t(autoUpdateRunning ? 'updatesLayout.currentRun' : 'autoUpdateRecentTitle')}</h2>
              <strong>{autoUpdateStatus}</strong>
            </div>
            {autoUpdateHasRun ? <>
              {!autoUpdateHasRuntime ? <div className="updates-summary-time">
                <span>{t('autoUpdateLastRunTitle')}</span>
                <strong>{autoUpdateLastRun}</strong>
              </div> : null}
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
                  <small>{t('updatesLayout.runFailedHint')}</small>
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
              {!autoUpdateRunning ? <p className="updates-history-note">{t('updatesLayout.historyHint')}</p> : null}
            </> : <p className="updates-result-empty">{t('updatesLayout.noRunHint')}</p>}
            {autoUpdateHasRun ? <div className="updates-run-log"><button type="button" className="btn btn-secondary btn-sm" onClick={() => { void handleCopyRunLog() }}><Copy size={14} aria-hidden="true" />{t('updatesLayout.copyRunLog')}</button></div> : null}
          </section>
        </div>

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
