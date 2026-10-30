// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, within } from '@testing-library/react'
import { afterEach, expect, it, vi } from 'vitest'
import i18n from '../../i18n'
import UpdatesPage from './UpdatesPage'
import type { AutoUpdateConfigDto, ManagedSkill } from './types'
import type { UpdateCheckState } from './workspace/useUpdateCheck'
import type { UpdateCandidate } from './workspace/contentCheck'

afterEach(() => {
  cleanup()
  vi.useRealTimers()
  vi.clearAllMocks()
})

const skill = (id: string, extra: Partial<ManagedSkill> = {}): ManagedSkill => ({
  id, name: id, source_type: 'local', source_ref: `/sources/${id}`,
  central_path: `/central/${id}`, created_at: 1, updated_at: 1,
  enabled: true, status: 'ok', tags: [], targets: [], ...extra,
})

const checkState = (overrides: Partial<UpdateCheckState> = {}): UpdateCheckState => ({
  phase: 'idle', plans: [], failures: [], currentSkill: null, processed: 0, total: 0,
  checkedAt: null, cancelled: false, fromCache: false, stale: false, complete: false, allUpToDate: false,
  checkedCount: 0, eligibleCount: 0, lastApply: null, lastCheck: null, ...overrides,
})
const updateCheck = (overrides: Partial<UpdateCheckState> = {}) => ({
  state: checkState(overrides), check: vi.fn(async () => {}), retryFailures: vi.fn(async () => {}),
  cancel: vi.fn(), apply: vi.fn(async () => null), update: vi.fn(async () => null),
})
const baseProps = {
  updateCheck: updateCheck(),
  onReview: vi.fn(),
  autoUpdateConfig: null,
  onAutoUpdateConfigChange: vi.fn(),
  onRunAutoUpdateNow: vi.fn(),
  autoUpdateTriggering: false,
  t: i18n.getFixedT('en'),
}

const config = (overrides: Partial<AutoUpdateConfigDto> = {}): AutoUpdateConfigDto => ({
  enabled: true, interval_hours: 6, schedule_type: 'interval', interval_value: 6,
  interval_unit: 'hours', daily_time: '05:20', local_skill_count: 0,
  protected_local_skill_count: 0, task_registered: true, task_status_detail: '',
  last_run_at: null, last_started_at: null, last_finished_at: null, last_status: null,
  last_error: null, last_checked: 0, last_unchanged: 0, last_updated: 0, last_failed: 0,
  progress: { total: 0, succeeded: [], failed: [], running: null, pending: [] },
  ...overrides,
})

const pending = (id: string): UpdateCandidate => ({ id: `checked-${id}`, checkId: 'check', skillId: id, name: id,
  signature: `source-${id}`, checkedAt: 1000, status: 'changed', targetCount: 1 })

it.each(['en', 'zh', 'ko'])('offers one scoped direct-update action beside checking in %s', language => {
  const t = i18n.getFixedT(language)
  const alpha = pending('Alpha')
  const beta = pending('Beta')
  const check = updateCheck({ eligibleCount: 3, checkedCount: 3, checkedAt: 1000, complete: true, plans: [alpha, beta] })
  render(<UpdatesPage {...baseProps} t={t} updateCheck={check} skills={[skill('Alpha'), skill('Beta'), skill('Current')]} />)
  const header = screen.getByRole('heading', { name: t('manageTabs.updates') }).closest('header')!
  const all = screen.getByRole('button', { name: t('updatesWorkflow.updateAll', { count: 2 }) }) as HTMLButtonElement
  expect(t('updatesWorkflow.updateAll', { count: 2 })).not.toBe('updatesWorkflow.updateAll')
  expect(all.disabled).toBe(false)
  expect(header.contains(all)).toBe(true)
  const checking = within(header).getByRole('button', { name: t('updatesWorkflow.check') })
  expect(checking.classList.contains('btn-secondary')).toBe(true)
  expect(all.classList.contains('btn-primary')).toBe(true)
  expect(header.querySelectorAll('.btn-primary')).toHaveLength(1)
  const list = screen.getByRole('region', { name: t('updatesWorkflow.title') })
  expect(within(list).queryByRole('button', { name: t('updatesWorkflow.reviewSelected', { count: 0 }) })).toBeNull()
  expect(within(list).getAllByRole('button', { name: t('updatesWorkflow.review') })).toHaveLength(2)
  fireEvent.click(all)
  expect(check.update).toHaveBeenCalledExactlyOnceWith([alpha, beta])
  expect(baseProps.onReview).not.toHaveBeenCalled()
  expect(baseProps.onRunAutoUpdateNow).not.toHaveBeenCalled()
  fireEvent.click(screen.getByRole('checkbox', { name: t('updatesWorkflow.selectSkill', { name: 'Beta' }) }))
  const selected = screen.getByRole('button', { name: t('updatesWorkflow.updateSelected', { count: 1 }) })
  expect(selected).toBe(all)
  fireEvent.click(selected)
  expect(check.update).toHaveBeenLastCalledWith([beta])
  fireEvent.click(screen.getByRole('checkbox', { name: t('updatesWorkflow.selectSkill', { name: 'Beta' }) }))
  expect(screen.getByRole('button', { name: t('updatesWorkflow.updateAll', { count: 2 }) })).toBe(all)
})

it.each([
  { label: 'unchecked', state: { checkedAt: null, complete: false } },
  { label: 'historical', state: { fromCache: true, complete: false } },
  { label: 'stale', state: { stale: true, complete: false } },
  { label: 'cancelled', state: { cancelled: true, complete: false } },
  { label: 'checking', state: { phase: 'checking' as const, complete: false } },
  { label: 'empty', state: { plans: [] } },
  { label: 'reload failed', state: { lastApply: { succeeded: [], failed: [], cancelled: false, reloadError: 'reload failed' } } },
])('prevents direct updates for $label results', ({ state }) => {
  const check = updateCheck({ eligibleCount: 1, checkedCount: 1, checkedAt: 1000, complete: true, plans: [pending('Alpha')], ...state })
  render(<UpdatesPage {...baseProps} updateCheck={check} />)
  const button = screen.getByRole('button', { name: baseProps.t('updatesWorkflow.updateAll', { count: check.state.plans.length }) }) as HTMLButtonElement
  expect(button.disabled).toBe(true)
  fireEvent.click(button)
  expect(check.update).not.toHaveBeenCalled()
})

it('keeps healthy confirmed candidates updateable when another source failed', () => {
  const alpha = pending('Alpha')
  const check = updateCheck({ eligibleCount: 2, checkedCount: 2, checkedAt: 1000, complete: true, plans: [alpha], failures: [{ skillId: 'Failed', name: 'Failed', error: 'INVALID_SOURCE' }] })
  render(<UpdatesPage {...baseProps} updateCheck={check} />)
  const button = screen.getByRole('button', { name: baseProps.t('updatesWorkflow.updateAll', { count: 1 }) }) as HTMLButtonElement
  expect(button.disabled).toBe(false)
  fireEvent.click(button)
  expect(check.update).toHaveBeenCalledExactlyOnceWith([alpha])
})

it('shows a busy primary action and blocks every new update while applying', () => {
  const check = updateCheck({ eligibleCount: 1, checkedCount: 1, checkedAt: 1000, plans: [pending('Alpha')], phase: 'applying', total: 1 })
  render(<UpdatesPage {...baseProps} updateCheck={check} />)
  const header = screen.getByRole('heading', { name: baseProps.t('manageTabs.updates') }).closest('header')!
  const button = within(header).getByRole('button', { name: baseProps.t('updatesWorkflow.applying') }) as HTMLButtonElement
  expect(button.disabled).toBe(true)
  expect(button.getAttribute('aria-busy')).toBe('true')
  expect((within(header).getByRole('button', { name: baseProps.t('updatesWorkflow.check') }) as HTMLButtonElement).disabled).toBe(true)
  fireEvent.click(button)
  expect(check.update).not.toHaveBeenCalled()
})

it('shows an explicit check action without listing the whole library as available updates', () => {
  const check = updateCheck({ eligibleCount: 2 })
  render(<UpdatesPage {...baseProps} skills={[skill('Alpha'), skill('Beta')]} updateCheck={check} />)
  const section = within(screen.getByRole('region', { name: baseProps.t('updatesWorkflow.title') }))
  expect(section.getByText(baseProps.t('updatesWorkflow.ready'))).toBeTruthy()
  expect(section.queryByRole('list')).toBeNull()
  expect(screen.queryByText('Alpha')).toBeNull()
  expect(screen.queryByText('Beta')).toBeNull()
  expect(check.check).not.toHaveBeenCalled()
  fireEvent.click(screen.getByRole('button', { name: baseProps.t('updatesWorkflow.check') }))
  expect(check.check).toHaveBeenCalledOnce()
})

it.each(['en', 'zh', 'ko'])('shows an explicit empty state in %s without hiding automatic updates', language => {
  const t = i18n.getFixedT(language)
  expect(t('updatesWorkflow.noSkills')).not.toBe('updatesWorkflow.noSkills')
  expect(t('updatesWorkflow.noSkillsHint')).not.toBe('updatesWorkflow.noSkillsHint')
  const run = vi.fn()
  render(<UpdatesPage {...baseProps} t={t} skills={[skill('Disabled', { enabled: false })]} onRunAutoUpdateNow={run} />)
  const section = within(screen.getByRole('region', { name: t('updatesWorkflow.title') }))
  expect(section.getByText(t('updatesWorkflow.noSkills'))).toBeTruthy()
  expect(section.queryByRole('list')).toBeNull()
  expect((screen.getByRole('button', { name: t('updatesWorkflow.check') }) as HTMLButtonElement).disabled).toBe(true)
  fireEvent.click(screen.getByText(t('autoUpdateScheduleTitle')))
  fireEvent.click(screen.getByRole('button', { name: t('updatesWorkflow.runPlan') }))
  expect(run).toHaveBeenCalledOnce()
})

it('blocks another check and automatic updates while the explicit check is running', () => {
  const check = updateCheck({ phase: 'checking', eligibleCount: 1, total: 1, currentSkill: { id: 'Alpha', name: 'Alpha' } })
  render(<UpdatesPage {...baseProps} updateCheck={check} autoUpdateConfig={config()} />)
  expect((screen.getByRole('button', { name: baseProps.t('updatesWorkflow.check') }) as HTMLButtonElement).disabled).toBe(true)
  expect((screen.getByRole('button', { name: baseProps.t('autoUpdateSkills') }) as HTMLButtonElement).disabled).toBe(true)
  fireEvent.click(screen.getByText(baseProps.t('autoUpdateScheduleTitle')))
  expect((screen.getByRole('button', { name: baseProps.t('updatesWorkflow.runPlan') }) as HTMLButtonElement).disabled).toBe(true)
  fireEvent.click(screen.getByRole('button', { name: baseProps.t('updatesWorkflow.stopCheck') }))
  expect(check.cancel).toHaveBeenCalledOnce()
})

it.each(['en', 'zh', 'ko'])('keeps the check action and first-check empty content mounted in %s', language => {
  const t = i18n.getFixedT(language)
  const idle = updateCheck({ eligibleCount: 1 })
  const page = render(<UpdatesPage {...baseProps} t={t} updateCheck={idle} />)
  const section = screen.getByRole('region', { name: t('updatesWorkflow.title') })
  const checkButton = screen.getByRole('button', { name: t('updatesWorkflow.check') })
  const empty = within(section).getByRole('heading', { name: t('updatesWorkflow.ready') }).parentElement
  const statusLine = within(section).getByRole('status').parentElement
  const children = section.childElementCount
  expect(within(section).queryByRole('progressbar')).toBeNull()
  expect(within(section).queryByRole('button', { name: t('updatesWorkflow.stopCheck') })).toBeNull()
  const active = updateCheck({ phase: 'checking', eligibleCount: 1, total: 1, currentSkill: { id: 'Alpha', name: 'Alpha' } })
  page.rerender(<UpdatesPage {...baseProps} t={t} updateCheck={active} />)
  expect(screen.getByRole('button', { name: t('updatesWorkflow.check') })).toBe(checkButton)
  expect(checkButton.getAttribute('aria-busy')).toBe('true')
  expect(within(section).getByRole('heading', { name: t('updatesWorkflow.ready') }).parentElement).toBe(empty)
  expect(empty?.getAttribute('aria-busy')).toBe('true')
  expect(within(section).getByRole('progressbar').parentElement).toBe(statusLine)
  expect(section.childElementCount).toBe(children)
  fireEvent.click(within(section).getByRole('button', { name: t('updatesWorkflow.stopCheck') }))
  expect(active.cancel).toHaveBeenCalledOnce()
  page.rerender(<UpdatesPage {...baseProps} t={t} updateCheck={idle} />)
  expect(screen.getByRole('button', { name: t('updatesWorkflow.check') })).toBe(checkButton)
  expect(checkButton.getAttribute('aria-busy')).toBe('false')
  expect(within(section).getByRole('heading', { name: t('updatesWorkflow.ready') }).parentElement).toBe(empty)
  expect(within(section).queryByRole('progressbar')).toBeNull()
  expect(section.childElementCount).toBe(children)
})

it('preserves the completed empty result during rechecking instead of flashing the first-check prompt', () => {
  const previous = { eligibleCount: 1, checkedCount: 1, checkedAt: 1000, complete: true, allUpToDate: true }
  const page = render(<UpdatesPage {...baseProps} updateCheck={updateCheck(previous)} />)
  const empty = screen.getByRole('heading', { name: baseProps.t('updatesWorkflow.upToDate') }).parentElement
  page.rerender(<UpdatesPage {...baseProps} updateCheck={updateCheck({ ...previous, phase: 'checking', complete: false, allUpToDate: false, total: 1 })} />)
  expect(screen.getByRole('heading', { name: baseProps.t('updatesWorkflow.upToDate') }).parentElement).toBe(empty)
  expect(screen.queryByRole('heading', { name: baseProps.t('updatesWorkflow.ready') })).toBeNull()
  expect(screen.getByText(baseProps.t('updatesWorkflow.checking'))).toBeTruthy()
})

it.each([
  { elapsedMs: 7, duration: 'durationBrief', seconds: undefined },
  { elapsedMs: 42, duration: 'durationSeconds', seconds: '0.04' },
  { elapsedMs: 1234, duration: 'durationSeconds', seconds: '1.2' },
])('shows the measured $elapsedMs ms duration in the persistent completion status', ({ elapsedMs, duration, seconds }) => {
  const finishedAt = new Date('2026-10-26T12:34:05Z').getTime()
  const check = updateCheck({ eligibleCount: 1, checkedCount: 1, checkedAt: finishedAt, complete: true, allUpToDate: true,
    lastCheck: { checkId: 'completed-run', finishedAt, elapsedMs, total: 1, completed: 1, changed: 0, unchanged: 1, failed: 0, cancelled: false } })
  render(<UpdatesPage {...baseProps} updateCheck={check} />)
  const section = within(screen.getByRole('region', { name: baseProps.t('updatesWorkflow.title') }))
  const status = section.getByRole('status')
  expect(status.getAttribute('aria-live')).toBe('polite')
  expect(status.textContent).toContain(baseProps.t('updatesWorkflow.checkCompleted'))
  expect(status.textContent).toContain(baseProps.t(`updatesWorkflow.${duration}`, { seconds }))
  expect(status.textContent).toContain(new Date(finishedAt).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' }))
  expect(section.queryByRole('progressbar')).toBeNull()
  expect(section.queryByRole('button', { name: baseProps.t('updatesWorkflow.stopCheck') })).toBeNull()
})

it('keeps pending rows and their selection controls in place while rechecking', () => {
  const pending: UpdateCandidate = { id: 'check-alpha', checkId: 'check', skillId: 'Alpha', name: 'Alpha', signature: 'source', checkedAt: 1000, status: 'changed', targetCount: 2 }
  const previous = { eligibleCount: 1, checkedCount: 1, checkedAt: 1000, plans: [pending] }
  const page = render(<UpdatesPage {...baseProps} updateCheck={updateCheck(previous)} />)
  const list = screen.getByRole('list')
  const row = screen.getByRole('listitem')
  const selected = screen.getByRole('checkbox', { name: baseProps.t('updatesWorkflow.selectSkill', { name: 'Alpha' }) }) as HTMLInputElement
  fireEvent.click(selected)
  page.rerender(<UpdatesPage {...baseProps} updateCheck={updateCheck({ ...previous, phase: 'checking', total: 1 })} />)
  expect(screen.getByRole('list')).toBe(list)
  expect(screen.getByRole('listitem')).toBe(row)
  expect(selected.checked).toBe(true)
  expect(selected.disabled).toBe(true)
  expect(screen.getByRole('progressbar').closest('.updates-check-meta')).toBeTruthy()
})

it('keeps recovery separate and available for a disabled Skill without re-enabling it', () => {
  const disabled = skill('Disabled', { enabled: false })
  const onRestore = vi.fn()
  render(<UpdatesPage {...baseProps} skills={[disabled]} recoverableIds={[disabled.id]} onRestore={onRestore} />)
  const section = within(screen.getByRole('region', { name: baseProps.t('updatesWorkflow.title') }))
  expect(section.queryByText('Disabled', { selector: 'strong' })).toBeNull()
  fireEvent.click(section.getByRole('button', { name: /Restore updates/ }))
  expect(section.getByText('Disabled', { selector: 'strong' })).toBeTruthy()
  expect(section.queryByRole('button', { name: baseProps.t('updatesWorkflow.review') })).toBeNull()
  fireEvent.click(section.getByRole('button', { name: baseProps.t('updatesWorkflow.reviewRecovery') }))
  expect(onRestore).toHaveBeenCalledExactlyOnceWith(disabled)
  expect(baseProps.onReview).not.toHaveBeenCalled()
  expect(disabled.enabled).toBe(false)
})

it.each(['en', 'zh', 'ko'])('names schedule controls and exposes the selected mode in %s', language => {
  const t = i18n.getFixedT(language)
  const current = config()
  const page = render(<UpdatesPage {...baseProps} t={t} autoUpdateConfig={current} />)
  fireEvent.click(screen.getByText(t('autoUpdateScheduleTitle')))
  for (const key of ['updatesLayout.intervalValue', 'updatesLayout.intervalUnit', 'updatesLayout.dailyTime']) {
    expect(t(key)).not.toBe(key)
  }
  expect(screen.getByRole('button', { name: t('autoUpdateSkills') }).getAttribute('aria-pressed')).toBe('true')
  expect((screen.getByRole('spinbutton', { name: t('updatesLayout.intervalValue') }) as HTMLInputElement).value).toBe('6')
  expect((screen.getByRole('combobox', { name: t('updatesLayout.intervalUnit') }) as HTMLSelectElement).value).toBe('hours')
  expect(screen.getByRole('button', { name: t('autoUpdateScheduleInterval') }).getAttribute('aria-pressed')).toBe('true')
  expect(screen.getByRole('button', { name: t('autoUpdateScheduleDaily') }).getAttribute('aria-pressed')).toBe('false')
  page.rerender(<UpdatesPage {...baseProps} t={t} autoUpdateConfig={{ ...current, schedule_type: 'daily' }} />)
  expect((screen.getByLabelText(t('updatesLayout.dailyTime')) as HTMLInputElement).value).toBe('05:20')
  expect(screen.queryByRole('spinbutton')).toBeNull()
  expect(screen.getByRole('button', { name: t('autoUpdateScheduleInterval') }).getAttribute('aria-pressed')).toBe('false')
  expect(screen.getByRole('button', { name: t('autoUpdateScheduleDaily') }).getAttribute('aria-pressed')).toBe('true')
})

it('preserves the other schedule values when toggling or editing one setting', () => {
  const current = config()
  const change = vi.fn()
  const page = render(<UpdatesPage {...baseProps} autoUpdateConfig={current} onAutoUpdateConfigChange={change} />)
  const t = baseProps.t
  const schedule = { scheduleType: 'interval', intervalValue: 6, intervalUnit: 'hours', dailyTime: '05:20' }
  fireEvent.click(screen.getByRole('button', { name: t('autoUpdateSkills') }))
  expect(change).toHaveBeenLastCalledWith(false, schedule)
  fireEvent.click(screen.getByText(t('autoUpdateScheduleTitle')))
  const interval = screen.getByRole('spinbutton', { name: t('updatesLayout.intervalValue') })
  fireEvent.change(interval, { target: { value: '12' } })
  fireEvent.blur(interval)
  expect(change).toHaveBeenLastCalledWith(true, { ...schedule, intervalValue: 12 })
  fireEvent.change(screen.getByRole('combobox', { name: t('updatesLayout.intervalUnit') }), { target: { value: 'minutes' } })
  expect(change).toHaveBeenLastCalledWith(true, { ...schedule, intervalUnit: 'minutes' })
  fireEvent.click(screen.getByRole('button', { name: t('autoUpdateScheduleDaily') }))
  expect(change).toHaveBeenLastCalledWith(true, { ...schedule, scheduleType: 'daily' })
  page.rerender(<UpdatesPage {...baseProps} autoUpdateConfig={{ ...current, schedule_type: 'daily' }} onAutoUpdateConfigChange={change} />)
  const time = screen.getByLabelText(t('updatesLayout.dailyTime'))
  fireEvent.change(time, { target: { value: '09:45' } })
  fireEvent.blur(time)
  expect(change).toHaveBeenLastCalledWith(true, { ...schedule, scheduleType: 'daily', dailyTime: '09:45' })
  fireEvent.click(screen.getByRole('button', { name: t('autoUpdateScheduleInterval') }))
  expect(change).toHaveBeenLastCalledWith(true, schedule)
  expect(change).toHaveBeenCalledTimes(6)
})

it.each(['running', 'triggering'] as const)('prevents another update while %s', state => {
  const run = vi.fn()
  const check = updateCheck({ eligibleCount: 1, checkedCount: 1, checkedAt: 1000, complete: true, plans: [pending('Alpha')] })
  render(<UpdatesPage {...baseProps} updateCheck={check} autoUpdateConfig={config(state === 'running' ? { last_status: 'running', last_run_at: Date.now() } : {})} autoUpdateTriggering={state === 'triggering'} onRunAutoUpdateNow={run} />)
  const direct = screen.getByRole('button', { name: baseProps.t('updatesWorkflow.updateAll', { count: 1 }) }) as HTMLButtonElement
  expect(direct.disabled).toBe(true)
  fireEvent.click(direct)
  expect(check.update).not.toHaveBeenCalled()
  fireEvent.click(screen.getByText(baseProps.t('autoUpdateScheduleTitle')))
  const button = screen.getByRole('button', { name: baseProps.t('autoUpdateRunningButton') }) as HTMLButtonElement
  expect(button.disabled).toBe(true)
  button.click()
  expect(run).not.toHaveBeenCalled()
})

it('offers a retry when a running task has stalled without reporting any progress', () => {
  vi.useFakeTimers()
  vi.setSystemTime(new Date('2026-10-26T12:00:00Z'))
  const run = vi.fn()
  render(<UpdatesPage {...baseProps} autoUpdateConfig={config({ last_status: 'running', last_run_at: Date.now() - 11 * 60 * 1000 })} onRunAutoUpdateNow={run} />)
  act(() => { vi.advanceTimersByTime(1000) })
  fireEvent.click(screen.getByText(baseProps.t('autoUpdateScheduleTitle')))
  const retry = screen.getByRole('button', { name: baseProps.t('autoUpdateRetryUpdate') }) as HTMLButtonElement
  expect(retry.disabled).toBe(false)
  expect(screen.getByText(baseProps.t('autoUpdateStalledHint'))).toBeTruthy()
  fireEvent.click(retry)
  expect(run).toHaveBeenCalledOnce()
})

it('keeps update available before the first run without presenting zero-valued result statistics', () => {
  const run = vi.fn()
  render(<UpdatesPage {...baseProps} autoUpdateConfig={config()} onRunAutoUpdateNow={run} />)
  for (const key of ['autoUpdateCheckedShort', 'autoUpdateUnchangedShort', 'autoUpdateUpdatedShort', 'autoUpdateFailedShort']) {
    expect(screen.queryByText(baseProps.t(key))).toBeNull()
  }
  fireEvent.click(screen.getByText(baseProps.t('autoUpdateScheduleTitle')))
  fireEvent.click(screen.getByRole('button', { name: baseProps.t('updatesWorkflow.runPlan') }))
  expect(run).toHaveBeenCalledOnce()
})

it('retains completed-run statistics and readable failure details', () => {
  const reason = 'TARGET_MODIFIED|/targets/Beta'
  render(<UpdatesPage {...baseProps} skills={[skill('Beta')]} autoUpdateConfig={config({
    last_status: 'partial', last_run_at: 1000, last_started_at: 1000, last_finished_at: 2000,
    last_checked: 11, last_unchanged: 7, last_updated: 3, last_failed: 1, last_error: reason,
    progress: { total: 11, succeeded: [], failed: [{ skill_id: 'Beta', name: 'Beta', reason }], running: null, pending: [] },
  })} />)
  for (const key of ['autoUpdateCheckedShort', 'autoUpdateUnchangedShort', 'autoUpdateUpdatedShort', 'autoUpdateFailedShort']) {
    expect(screen.getByText(baseProps.t(key))).toBeTruthy()
  }
  for (const count of ['11', '7', '3', '1']) expect(screen.getByText(count)).toBeTruthy()
  expect(screen.getByText('Beta')).toBeTruthy()
  expect(screen.getByText(baseProps.t('errors.targetModified', { path: '/targets/Beta' }))).toBeTruthy()
  expect(screen.getByRole('button', { name: baseProps.t('copyDetails') })).toBeTruthy()
})

it('shows the saved schedule in a collapsed disclosure without changing settings when opened', () => {
  const change = vi.fn()
  render(<UpdatesPage {...baseProps} autoUpdateConfig={config()} onAutoUpdateConfigChange={change} />)
  expect(screen.getByText(baseProps.t('autoUpdateIntervalShort', { value: 6, unit: baseProps.t('autoUpdateIntervalUnitHours') }))).toBeTruthy()
  const schedule = screen.getByText(baseProps.t('autoUpdateScheduleTitle')).closest('details') as HTMLDetailsElement
  expect(schedule.open).toBe(false)
  fireEvent.click(screen.getByText(baseProps.t('autoUpdateScheduleTitle')))
  expect(schedule.open).toBe(true)
  expect(screen.getByRole('spinbutton', { name: baseProps.t('updatesLayout.intervalValue') })).toBeTruthy()
  fireEvent.click(screen.getByText(baseProps.t('autoUpdateScheduleTitle')))
  expect(schedule.open).toBe(false)
  expect(change).not.toHaveBeenCalled()
})
