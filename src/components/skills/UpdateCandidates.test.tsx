// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, expect, it, vi } from 'vitest'
import { useState } from 'react'
import i18n from '../../i18n'
import UpdatesPage from './UpdatesPage'
import type { ManagedSkill } from './types'
import ContentReview, { type ContentPlan } from './workspace/ContentReview'
import type { CheckSources, ContentCheckRunResult, ContentCheckSummary, UpdateCandidate } from './workspace/contentCheck'
import { useUpdateCheck } from './workspace/useUpdateCheck'

afterEach(() => {
  cleanup()
  vi.useRealTimers()
  vi.restoreAllMocks()
})
const t = i18n.getFixedT('en')
const reload = vi.fn(async () => {})
const skill = (id: string, extra: Partial<ManagedSkill> = {}): ManagedSkill => ({
  id, name: id, source_type: 'local', source_ref: `/sources/${id}`, central_path: `/central/${id}`,
  created_at: 1, updated_at: 1, enabled: true, status: 'ok', tags: [], targets: [], ...extra,
})
const plan = (id: string): ContentPlan => ({
  id: `plan-${id}`, skillId: id, name: id, restore: false, targets: [],
  changes: [{ path: 'SKILL.md', kind: 'modified', before: 'old', after: 'new' }],
})
const summary = (id: string, status: ContentCheckSummary['status'] = 'changed', extra: Partial<ContentCheckSummary> = {}): ContentCheckSummary => ({
  skillId: id, name: id, signature: `source-${id}`, checkedAt: 1000, targetCount: 2, status, ...extra,
})
const checkResult = (checkId: string, summaries: ContentCheckSummary[], extra: Partial<ContentCheckRunResult> = {}): ContentCheckRunResult => ({
  checkId, summaries, total: summaries.length, completed: summaries.length, checkedAt: 1000, cancelled: false, ...extra,
})
const checkFor = (...summaries: ContentCheckSummary[]) => vi.fn<CheckSources>(async checkId => checkResult(checkId, summaries))
type Invoke = Parameters<typeof useUpdateCheck>[0]['invokeTauri']
const desktopInvoke = (execute: (id: string) => Promise<unknown> = async () => ({ changed: true }), cached: ContentCheckRunResult | null = null, preview: (id: string) => Promise<ContentPlan> = async id => plan(id)) => {
  const mocked = vi.fn<Invoke>(async <T,>(command: string, args?: Record<string, unknown>): Promise<T> => {
  if (command === 'workspace_content_check_cached') return cached as T
  if (command === 'workspace_content_preview') return await preview(String(args?.skillId)) as T
  if (command === 'workspace_content_execute') return await execute(String(args?.id)) as T
    return null as T
  })
  // Vitest preserves the implementation but erases generic call signatures.
  return mocked as typeof mocked & Invoke
}
const selectedIds = (items: UpdateCandidate[]) => Object.fromEntries(items.map(item => [item.skillId, item.id]))
type HarnessProps = {
  skills: ManagedSkill[]
  invoke: Parameters<typeof useUpdateCheck>[0]['invokeTauri']
  checkSources: CheckSources
  onReview?: (plans: UpdateCandidate[]) => void
  recoverableIds?: string[]
  onRestore?: (skill: ManagedSkill) => void
  recoveryError?: boolean
  onRetryRecoveries?: () => void
  reviewModal?: boolean
  onReload?: () => Promise<unknown>
  t?: typeof t
}
function Harness({ skills, invoke, checkSources, onReview = () => {}, reviewModal, onReload = reload, t: translate = t, ...props }: HarnessProps) {
  const updateCheck = useUpdateCheck({ skills, invokeTauri: invoke, checkSources, onReload })
  const [review, setReview] = useState<UpdateCandidate[] | null>(null)
  return <><UpdatesPage {...props} skills={skills} updateCheck={updateCheck} onReview={items => {
    onReview(items)
    if (reviewModal) setReview(items)
  }}
    autoUpdateConfig={null} onAutoUpdateConfigChange={() => {}} onRunAutoUpdateNow={() => {}}
    autoUpdateTriggering={false} t={translate} />
    {review ? <ContentReview skills={skills.filter(skill => review.some(item => item.skillId === skill.id))}
      restore={false} invokeTauri={invoke} onClose={() => setReview(null)} t={translate} onConfirm={async plans => {
        const expected = selectedIds(review)
        setReview(null)
        await updateCheck.apply(plans, expected)
      }} /> : null}
  </>
}
function checkForUpdates(translate = t) {
  fireEvent.click(screen.getByRole('button', { name: translate('updatesWorkflow.check') }))
}
const updateAction = (count: number, selected = false, translate = t) => screen.getByRole('button', { name: translate(`updatesWorkflow.${selected ? 'updateSelected' : 'updateAll'}`, { count }) }) as HTMLButtonElement
const candidates = (translate = t) => within(screen.getByRole('region', { name: translate('updatesWorkflow.title') }))
async function waitUntilChecked(translate = t) {
  await waitFor(() => expect((screen.getByRole('button', { name: translate('updatesWorkflow.check') }) as HTMLButtonElement).disabled).toBe(false))
}
async function confirmReview() {
  const dialog = await screen.findByRole('dialog', { name: t('workspace110.reviewUpdate') })
  const button = within(dialog).getByRole('button', { name: t('workspace110.confirmUpdate') }) as HTMLButtonElement
  await waitFor(() => expect(button.disabled).toBe(false))
  fireEvent.click(button)
}

it.each(['en', 'zh', 'ko'])('keeps a clear completed result after an immediate check and distinguishes same-minute rechecks in %s', async language => {
  vi.useFakeTimers()
  vi.spyOn(performance, 'now').mockReturnValue(100)
  const translate = i18n.getFixedT(language)
  const finishedAt = new Date('2026-10-26T12:34:01Z').getTime()
  const invoke = desktopInvoke()
  const checkSources = vi.fn<CheckSources>(async checkId => checkResult(checkId, [summary('Current', 'unchanged')], { checkedAt: finishedAt }))
  render(<Harness skills={[skill('Current')]} invoke={invoke} checkSources={checkSources} t={translate} />)
  const section = candidates(translate)
  const status = section.getByRole('status')
  const button = screen.getByRole('button', { name: translate('updatesWorkflow.check') }) as HTMLButtonElement
  expect(translate('updatesWorkflow.checkCompleted')).not.toBe('updatesWorkflow.checkCompleted')
  await act(async () => { fireEvent.click(button) })
  expect(button.disabled).toBe(false)
  expect(section.getByRole('status')).toBe(status)
  expect(status.textContent).toContain(translate('updatesWorkflow.checkCompleted'))
  expect(status.textContent).toContain(translate('updatesWorkflow.durationBrief'))
  expect(status.textContent).toContain(translate('updatesWorkflow.checkBreakdown', { completed: 1, total: 1, changed: 0, unchanged: 1 }))
  const firstTime = new Date(finishedAt).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' })
  expect(status.textContent).toContain(firstTime)
  expect(status.closest('.updates-check-meta')?.classList.contains('is-complete')).toBe(true)
  await act(async () => { await vi.advanceTimersByTimeAsync(10_000) })
  expect(status.textContent).toContain(translate('updatesWorkflow.checkCompleted'))
  expect(status.textContent).toContain(firstTime)
  checkSources.mockImplementationOnce(async checkId => checkResult(checkId, [summary('Current', 'unchanged')], { checkedAt: finishedAt + 1000 }))
  await act(async () => { fireEvent.click(button) })
  expect(button.disabled).toBe(false)
  expect(checkSources).toHaveBeenCalledTimes(2)
  expect(section.getByRole('status')).toBe(status)
  expect(status.textContent).not.toContain(firstTime)
  expect(status.textContent).toContain(new Date(finishedAt + 1000).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' }))
  expect(status.textContent).toContain(translate('updatesWorkflow.checkCompleted'))
})

it('checks lightweight sources only on request, lists changes only, and loads full content when review opens', async () => {
  const invoke = desktopInvoke()
  const checkSources = checkFor(summary('Changed'), summary('Unchanged', 'unchanged'))
  const onReview = vi.fn()
  render(<Harness skills={[skill('Changed'), skill('Unchanged'), skill('Disabled', { enabled: false })]} invoke={invoke} checkSources={checkSources} onReview={onReview} reviewModal />)
  expect(invoke.mock.calls).toEqual([['workspace_content_check_cached']])
  expect(checkSources).not.toHaveBeenCalled()
  expect(candidates().getByRole('heading', { name: t('updatesWorkflow.ready') })).toBeTruthy()
  expect(candidates().queryByRole('list')).toBeNull()
  checkForUpdates()
  await waitUntilChecked()
  expect(checkSources.mock.calls[0][1]).toEqual(['Changed', 'Unchanged'])
  expect(candidates().getAllByRole('listitem')).toHaveLength(1)
  expect(candidates().getByText('Changed')).toBeTruthy()
  expect(candidates().queryByText('Unchanged')).toBeNull()
  expect(candidates().queryByText('Disabled')).toBeNull()
  expect(invoke).toHaveBeenCalledTimes(1)
  fireEvent.click(candidates().getByRole('button', { name: t('updatesWorkflow.review') }))
  const dialog = await screen.findByRole('dialog', { name: t('workspace110.reviewUpdate') })
  await waitFor(() => expect(within(dialog).getByText('SKILL.md')).toBeTruthy())
  expect(onReview.mock.calls[0][0][0].skillId).toBe('Changed')
  expect(invoke.mock.calls.filter(([command]) => command === 'workspace_content_preview')).toEqual([
    ['workspace_content_preview', { skillId: 'Changed', restore: false }],
  ])
  expect(invoke.mock.calls.some(([command]) => command === 'workspace_content_execute')).toBe(false)
})

it('directly updates all confirmed changes without opening review or touching current, failed or disabled Skills', async () => {
  const invoke = desktopInvoke()
  const checkSources = checkFor(summary('Alpha'), summary('Beta'), summary('Current', 'unchanged'), summary('Failed', 'failed', { error: 'INVALID_SOURCE' }))
  const onReview = vi.fn()
  const onReload = vi.fn(async () => {})
  render(<Harness skills={[skill('Alpha'), skill('Beta'), skill('Current'), skill('Failed'), skill('Disabled', { enabled: false })]}
    invoke={invoke} checkSources={checkSources} onReview={onReview} onReload={onReload} reviewModal />)
  expect(updateAction(0).disabled).toBe(true)
  checkForUpdates()
  await waitUntilChecked()
  expect(updateAction(2).disabled).toBe(false)
  fireEvent.click(updateAction(2))
  await waitFor(() => expect(invoke.mock.calls.filter(([command]) => command === 'workspace_content_execute')).toHaveLength(2))
  await waitUntilChecked()
  expect(screen.queryByRole('dialog')).toBeNull()
  expect(onReview).not.toHaveBeenCalled()
  expect(onReload).toHaveBeenCalledOnce()
  expect(invoke.mock.calls.filter(([command]) => command === 'workspace_content_preview')).toEqual([
    ['workspace_content_preview', { skillId: 'Alpha', restore: false }],
    ['workspace_content_preview', { skillId: 'Beta', restore: false }],
  ])
  expect(invoke.mock.calls.filter(([command]) => command === 'workspace_content_execute')).toEqual([
    ['workspace_content_execute', { id: 'plan-Alpha' }], ['workspace_content_execute', { id: 'plan-Beta' }],
  ])
  expect(candidates().getByText(t('updatesWorkflow.applyResult', { succeeded: 2, failed: 0 }))).toBeTruthy()
  expect(candidates().getByText('Failed')).toBeTruthy()
})

it('directly updates only the selected Skill and keeps the other pending row', async () => {
  const invoke = desktopInvoke()
  const checkSources = checkFor(summary('Alpha'), summary('Beta'))
  const onReview = vi.fn()
  render(<Harness skills={[skill('Alpha'), skill('Beta')]} invoke={invoke} checkSources={checkSources} onReview={onReview} reviewModal />)
  checkForUpdates()
  await waitUntilChecked()
  fireEvent.click(candidates().getByRole('checkbox', { name: t('updatesWorkflow.selectSkill', { name: 'Beta' }) }))
  fireEvent.click(updateAction(1, true))
  await waitFor(() => expect(invoke).toHaveBeenCalledWith('workspace_content_execute', { id: 'plan-Beta' }))
  await waitUntilChecked()
  expect(invoke.mock.calls.filter(([command]) => command === 'workspace_content_preview')).toEqual([
    ['workspace_content_preview', { skillId: 'Beta', restore: false }],
  ])
  expect(invoke.mock.calls.filter(([command]) => command === 'workspace_content_execute')).toHaveLength(1)
  expect(onReview).not.toHaveBeenCalled()
  expect(screen.queryByRole('dialog')).toBeNull()
  expect(candidates().getByText('Alpha')).toBeTruthy()
  expect(candidates().queryByText('Beta')).toBeNull()
  expect(updateAction(1).disabled).toBe(false)
})

it('retains an actionable preparation failure while completing other requested updates', async () => {
  const invoke = desktopInvoke(undefined, null, async id => {
    if (id === 'Alpha') throw new Error('TARGET_MODIFIED|/targets/Alpha')
    return plan(id)
  })
  const checkSources = checkFor(summary('Alpha'), summary('Beta'))
  render(<Harness skills={[skill('Alpha'), skill('Beta')]} invoke={invoke} checkSources={checkSources} />)
  checkForUpdates()
  await waitUntilChecked()
  fireEvent.click(updateAction(2))
  await waitFor(() => expect(invoke).toHaveBeenCalledWith('workspace_content_execute', { id: 'plan-Beta' }))
  await waitUntilChecked()
  expect(invoke.mock.calls.filter(([command]) => command === 'workspace_content_execute')).toEqual([
    ['workspace_content_execute', { id: 'plan-Beta' }],
  ])
  expect(candidates().getByText(t('updatesWorkflow.applyResult', { succeeded: 1, failed: 1 }))).toBeTruthy()
  expect(candidates().getAllByText(/\/targets\/Alpha/).length).toBeGreaterThan(0)
  expect(candidates().getByRole('button', { name: t('updatesWorkflow.retryFailed') })).toBeTruthy()
  expect(screen.queryByRole('dialog')).toBeNull()
  expect(candidates().queryByText(t('updatesWorkflow.upToDate'))).toBeNull()
})

it('requires a fresh check before directly applying historical pending candidates', async () => {
  const invoke = desktopInvoke(undefined, checkResult('yesterday', [summary('Alpha')]))
  const checkSources = checkFor(summary('Alpha'))
  render(<Harness skills={[skill('Alpha')]} invoke={invoke} checkSources={checkSources} />)
  await waitFor(() => expect(candidates().getByText('Alpha')).toBeTruthy())
  expect(updateAction(1).disabled).toBe(true)
  fireEvent.click(updateAction(1))
  expect(invoke.mock.calls).toEqual([['workspace_content_check_cached']])
  expect(checkSources).not.toHaveBeenCalled()
  checkForUpdates()
  await waitUntilChecked()
  expect(updateAction(1).disabled).toBe(false)
  fireEvent.click(updateAction(1))
  await waitFor(() => expect(invoke).toHaveBeenCalledWith('workspace_content_execute', { id: 'plan-Alpha' }))
  await waitUntilChecked()
})

it.each(['en', 'zh', 'ko'])('localizes failed sources in %s without counting them as current and retries only failures', async language => {
  const translate = i18n.getFixedT(language)
  const invoke = desktopInvoke()
  const checkSources = checkFor(summary('Current', 'unchanged'), summary('Failed', 'failed', { error: 'INVALID_SOURCE' }))
  render(<Harness skills={[skill('Current'), skill('Failed')]} invoke={invoke} checkSources={checkSources} t={translate} />)
  checkForUpdates(translate)
  await waitUntilChecked(translate)
  const section = candidates(translate)
  const localizedError = translate('updatesWorkflow.sourceUnavailable')
  expect(localizedError).not.toBe('updatesWorkflow.sourceUnavailable')
  expect(section.getByRole('heading', { name: translate('updatesWorkflow.noConfirmedChanges') })).toBeTruthy()
  expect(section.getByText(localizedError)).toBeTruthy()
  expect(section.queryByText('INVALID_SOURCE')).toBeNull()
  expect(section.queryByText(translate('updatesWorkflow.upToDate'))).toBeNull()
  expect(section.queryByRole('checkbox')).toBeNull()
  const status = section.getByRole('status')
  expect(status.textContent).toContain(translate('updatesWorkflow.checkPartial'))
  expect(status.textContent).toContain(translate('updatesWorkflow.checkBreakdown', { completed: 2, total: 2, changed: 0, unchanged: 1 }))
  expect(status.textContent).toContain(translate('updatesWorkflow.checkFailureCount', { count: 1 }))
  expect(status.closest('.updates-check-meta')?.classList.contains('has-warning')).toBe(true)
  expect(status.closest('.updates-check-meta')?.classList.contains('is-complete')).toBe(false)
  checkSources.mockImplementationOnce(async checkId => checkResult(checkId, [summary('Failed', 'unchanged')]))
  fireEvent.click(section.getByRole('button', { name: translate('updatesWorkflow.retryFailed') }))
  await waitUntilChecked(translate)
  expect(checkSources.mock.calls.map(([, ids]) => ids)).toEqual([['Current', 'Failed'], ['Failed']])
  expect(section.queryByText(localizedError)).toBeNull()
  expect(section.getByRole('heading', { name: translate('updatesWorkflow.upToDate') })).toBeTruthy()
  expect(status.textContent).toContain(translate('updatesWorkflow.checkCompleted'))
  expect(status.textContent).toContain(translate('updatesWorkflow.checkBreakdown', { completed: 1, total: 1, changed: 0, unchanged: 1 }))
})

it('clearly reports a wholly failed check without a success treatment', async () => {
  const invoke = desktopInvoke()
  const checkSources = vi.fn<CheckSources>(async () => { throw new Error('CHECK_FAILED') })
  render(<Harness skills={[skill('Alpha'), skill('Beta')]} invoke={invoke} checkSources={checkSources} />)
  checkForUpdates()
  await waitUntilChecked()
  const status = candidates().getByRole('status')
  expect(t('updatesWorkflow.checkAllFailed')).not.toBe('updatesWorkflow.checkAllFailed')
  expect(status.textContent).toContain(t('updatesWorkflow.checkAllFailed'))
  expect(status.textContent).toContain(t('updatesWorkflow.checkBreakdown', { completed: 2, total: 2, changed: 0, unchanged: 0 }))
  expect(status.textContent).toContain(t('updatesWorkflow.checkFailureCount', { count: 2 }))
  expect(status.textContent).not.toContain(t('updatesWorkflow.checkCompleted'))
  expect(status.closest('.updates-check-meta')?.classList.contains('has-warning')).toBe(true)
  expect(status.closest('.updates-check-meta')?.classList.contains('is-complete')).toBe(false)
})

it('shows a new retry outcome alongside cached library observations without claiming all sources are current', async () => {
  const cached = checkResult('yesterday', [summary('Current', 'unchanged'), summary('Failed', 'failed', { error: 'INVALID_SOURCE' })])
  const invoke = desktopInvoke(undefined, cached)
  const checkSources = checkFor(summary('Failed', 'unchanged'))
  render(<Harness skills={[skill('Current'), skill('Failed')]} invoke={invoke} checkSources={checkSources} />)
  await waitFor(() => expect(candidates().getByText(t('updatesWorkflow.cachedHint'))).toBeTruthy())
  expect(checkSources).not.toHaveBeenCalled()
  fireEvent.click(candidates().getByRole('button', { name: t('updatesWorkflow.retryFailed') }))
  await waitUntilChecked()
  expect(checkSources.mock.calls[0][1]).toEqual(['Failed'])
  const status = candidates().getByRole('status')
  expect(status.textContent).toContain(t('updatesWorkflow.checkCompleted'))
  expect(status.textContent).toContain(t('updatesWorkflow.checkBreakdown', { completed: 1, total: 1, changed: 0, unchanged: 1 }))
  expect(candidates().queryByRole('heading', { name: t('updatesWorkflow.upToDate') })).toBeNull()
})

it('keeps selected rows stable during rechecking and never carries consent to the new check result', async () => {
  let finish!: (value: ContentCheckRunResult) => void
  const checking = new Promise<ContentCheckRunResult>(resolve => { finish = resolve })
  const invoke = desktopInvoke()
  const checkSources = checkFor(summary('Alpha'), summary('Beta'))
  const onReview = vi.fn()
  render(<Harness skills={[skill('Alpha'), skill('Beta')]} invoke={invoke} checkSources={checkSources} onReview={onReview} />)
  checkForUpdates()
  await waitUntilChecked()
  fireEvent.click(candidates().getByRole('checkbox', { name: t('updatesWorkflow.selectSkill', { name: 'Beta' }) }))
  expect(updateAction(1, true).disabled).toBe(false)
  fireEvent.click(within(candidates().getByText('Beta').closest('li')!).getByRole('button', { name: t('updatesWorkflow.review') }))
  const oldSelection = onReview.mock.calls[0][0] as UpdateCandidate[]
  expect(oldSelection.map(item => item.skillId)).toEqual(['Beta'])
  const list = candidates().getByRole('list')
  const row = candidates().getByText('Beta').closest('li')
  checkSources.mockImplementationOnce(() => checking)
  checkForUpdates()
  const [checkId, , progress] = checkSources.mock.calls[1]
  act(() => { progress({ checkId, phase: 'checking', completed: 1, total: 2, currentSkillId: 'Alpha', summary: summary('Alpha') }) })
  expect(candidates().getByRole('list')).toBe(list)
  expect(candidates().getByText('Beta').closest('li')).toBe(row)
  expect((candidates().getByRole('checkbox', { name: t('updatesWorkflow.selectSkill', { name: 'Beta' }) }) as HTMLInputElement).checked).toBe(true)
  await act(async () => { finish(checkResult(checkId, [summary('Alpha'), summary('Beta')])); await checking })
  await waitUntilChecked()
  expect((candidates().getByRole('checkbox', { name: t('updatesWorkflow.selectSkill', { name: 'Beta' }) }) as HTMLInputElement).checked).toBe(false)
  expect(updateAction(2).disabled).toBe(false)
  expect(screen.queryByRole('button', { name: t('updatesWorkflow.updateSelected', { count: 1 }) })).toBeNull()
  fireEvent.click(candidates().getByRole('checkbox', { name: t('updatesWorkflow.selectAll') }))
  expect(updateAction(2, true).disabled).toBe(false)
  fireEvent.click(within(candidates().getByText('Beta').closest('li')!).getByRole('button', { name: t('updatesWorkflow.review') }))
  const nextSelection = onReview.mock.calls[1][0] as UpdateCandidate[]
  expect(nextSelection.map(item => item.skillId)).toEqual(['Beta'])
  expect(nextSelection[0].id).not.toBe(oldSelection[0].id)
  expect(invoke.mock.calls.some(([command]) => command === 'workspace_content_preview')).toBe(false)
})

it('shows a distinct stop-check action and waits for scoped cancellation without overlapping checks', async () => {
  let finish!: (value: ContentCheckRunResult) => void
  const checking = new Promise<ContentCheckRunResult>(resolve => { finish = resolve })
  const invoke = desktopInvoke()
  const checkSources = vi.fn<CheckSources>(() => checking)
  render(<Harness skills={[skill('Alpha'), skill('Beta')]} invoke={invoke} checkSources={checkSources} />)
  const empty = candidates().getByRole('heading', { name: t('updatesWorkflow.ready') }).parentElement
  checkForUpdates()
  expect(candidates().getByRole('heading', { name: t('updatesWorkflow.ready') }).parentElement).toBe(empty)
  expect(candidates().getByRole('progressbar', { name: t('updatesWorkflow.progress') })).toBeTruthy()
  fireEvent.click(candidates().getByRole('button', { name: t('updatesWorkflow.stopCheck') }))
  const checkId = checkSources.mock.calls[0][0]
  expect(invoke).toHaveBeenLastCalledWith('workspace_content_check_cancel', { checkId })
  expect((screen.getByRole('button', { name: t('updatesWorkflow.check') }) as HTMLButtonElement).disabled).toBe(true)
  await act(async () => { finish(checkResult(checkId, [summary('Alpha', 'unchanged')], { cancelled: true, total: 2, completed: 1 })); await checking })
  await waitUntilChecked()
  expect(checkSources).toHaveBeenCalledTimes(1)
  expect(candidates().getByRole('heading', { name: t('updatesWorkflow.incomplete') })).toBeTruthy()
  expect(candidates().queryByText(t('updatesWorkflow.upToDate'))).toBeNull()
  const status = candidates().getByRole('status')
  expect(status.textContent).toContain(t('updatesWorkflow.checkStopped'))
  expect(status.textContent).toContain(t('updatesWorkflow.checkBreakdown', { completed: 1, total: 2, changed: 0, unchanged: 1 }))
  expect(status.closest('.updates-check-meta')?.classList.contains('has-warning')).toBe(true)
  expect(status.closest('.updates-check-meta')?.classList.contains('is-complete')).toBe(false)
})

it('labels cached results as historical, hydrates without source access, and requires a live check to report current content', async () => {
  const invoke = desktopInvoke(undefined, checkResult('yesterday', [summary('Alpha', 'unchanged')]))
  const checkSources = checkFor(summary('Alpha', 'unchanged'))
  render(<Harness skills={[skill('Alpha')]} invoke={invoke} checkSources={checkSources} />)
  await waitFor(() => expect(candidates().getByText(t('updatesWorkflow.cachedHint'))).toBeTruthy())
  expect(checkSources).not.toHaveBeenCalled()
  expect(candidates().queryByText(t('updatesWorkflow.upToDate'))).toBeNull()
  expect(candidates().getByRole('status').textContent).not.toContain(t('updatesWorkflow.checkCompleted'))
  expect(candidates().getByRole('status').closest('.updates-check-meta')?.classList.contains('is-complete')).toBe(false)
  expect(invoke.mock.calls).toEqual([['workspace_content_check_cached']])
  checkForUpdates()
  await waitUntilChecked()
  expect(candidates().queryByText(t('updatesWorkflow.cachedHint'))).toBeNull()
  expect(candidates().getByRole('heading', { name: t('updatesWorkflow.upToDate') })).toBeTruthy()
})

it('separates recovery records from pending updates and preserves pending selection when switching sections', async () => {
  const alpha = skill('Alpha')
  const disabled = skill('Disabled', { enabled: false })
  const invoke = desktopInvoke()
  const checkSources = checkFor(summary('Alpha'))
  const onRestore = vi.fn()
  render(<Harness skills={[alpha, disabled]} invoke={invoke} checkSources={checkSources} recoverableIds={['Disabled', 'Missing']} onRestore={onRestore} />)
  fireEvent.click(candidates().getByRole('button', { name: /Restore updates/ }))
  expect(candidates().getAllByRole('listitem')).toHaveLength(1)
  expect(candidates().queryByText('Alpha')).toBeNull()
  fireEvent.click(candidates().getByRole('button', { name: t('updatesWorkflow.reviewRecovery') }))
  expect(onRestore).toHaveBeenCalledExactlyOnceWith(disabled)
  expect(checkSources).not.toHaveBeenCalled()
  checkForUpdates()
  await waitUntilChecked()
  fireEvent.click(candidates().getByRole('checkbox', { name: t('updatesWorkflow.selectSkill', { name: 'Alpha' }) }))
  fireEvent.click(candidates().getByRole('button', { name: /Restore updates/ }))
  fireEvent.click(candidates().getByRole('button', { name: /Available updates/ }))
  expect((candidates().getByRole('checkbox', { name: t('updatesWorkflow.selectSkill', { name: 'Alpha' }) }) as HTMLInputElement).checked).toBe(true)
  expect(checkSources).toHaveBeenCalledTimes(1)
})

it('reports recovery loading failures with a retry instead of claiming no snapshots exist', () => {
  const invoke = desktopInvoke()
  const checkSources = checkFor(summary('Alpha'))
  const retry = vi.fn()
  render(<Harness skills={[skill('Alpha')]} invoke={invoke} checkSources={checkSources} recoveryError onRetryRecoveries={retry} />)
  fireEvent.click(candidates().getByRole('button', { name: /Restore updates/ }))
  expect(candidates().getByRole('alert').textContent).toContain(t('updatesWorkflow.recoveryFailed'))
  expect(candidates().queryByText(t('updatesWorkflow.noRecovery'))).toBeNull()
  fireEvent.click(candidates().getByRole('button', { name: t('updatesWorkflow.retryFailed') }))
  expect(retry).toHaveBeenCalledOnce()
  expect(checkSources).not.toHaveBeenCalled()
})

it('updates selected rows directly and allows stopping the remaining items', async () => {
  let finish!: (value: { changed: boolean }) => void
  const executing = new Promise<{ changed: boolean }>(resolve => { finish = resolve })
  const invoke = desktopInvoke(() => executing)
  const checkSources = checkFor(summary('Alpha'), summary('Beta'))
  render(<Harness skills={[skill('Alpha'), skill('Beta')]} invoke={invoke} checkSources={checkSources} reviewModal />)
  checkForUpdates()
  await waitUntilChecked()
  fireEvent.click(candidates().getByRole('checkbox', { name: t('updatesWorkflow.selectAll') }))
  fireEvent.click(updateAction(2, true))
  await waitFor(() => expect(invoke).toHaveBeenCalledWith('workspace_content_execute', { id: 'plan-Alpha' }))
  expect(screen.queryByRole('dialog')).toBeNull()
  expect(candidates().getByText(t('updatesWorkflow.applying'))).toBeTruthy()
  fireEvent.click(candidates().getByRole('button', { name: t('updatesWorkflow.stop') }))
  expect(invoke).toHaveBeenLastCalledWith('workspace_content_execute', { id: 'plan-Alpha' })
  await act(async () => { finish({ changed: true }); await executing })
  await waitUntilChecked()
  expect(invoke.mock.calls.filter(([command]) => command === 'workspace_content_execute')).toHaveLength(1)
  expect(candidates().getByText(t('updatesWorkflow.applyResult', { succeeded: 1, failed: 0 }))).toBeTruthy()
  expect(candidates().getAllByRole('listitem')).toHaveLength(1)
  expect(candidates().getByText('Beta')).toBeTruthy()
  expect(candidates().queryByText(t('updatesWorkflow.upToDate'))).toBeNull()
})

it('shows a stopped warning after cancelling direct preparation without executing a write', async () => {
  let finish!: (value: ContentPlan) => void
  const preparing = new Promise<ContentPlan>(resolve => { finish = resolve })
  const invoke = desktopInvoke(undefined, null, () => preparing)
  const checkSources = checkFor(summary('Alpha'))
  render(<Harness skills={[skill('Alpha')]} invoke={invoke} checkSources={checkSources} />)
  checkForUpdates()
  await waitUntilChecked()
  fireEvent.click(updateAction(1))
  await waitFor(() => expect(invoke).toHaveBeenCalledWith('workspace_content_preview', { skillId: 'Alpha', restore: false }))
  fireEvent.click(candidates().getByRole('button', { name: t('updatesWorkflow.stop') }))
  await act(async () => { finish(plan('Alpha')); await preparing })
  await waitUntilChecked()
  expect(invoke.mock.calls.some(([command]) => command === 'workspace_content_execute')).toBe(false)
  const outcome = candidates().getByText(t('updatesWorkflow.applyResult', { succeeded: 0, failed: 0 })).closest('[role="status"]')!
  expect(outcome.classList.contains('has-errors')).toBe(true)
  expect(within(outcome as HTMLElement).getByText(t('updatesWorkflow.cancelled'))).toBeTruthy()
  expect(candidates().getByText('Alpha')).toBeTruthy()
  expect(updateAction(1).disabled).toBe(true)
})

it('shows pending deployment repairs as an incomplete update outcome', async () => {
  const invoke = desktopInvoke(async () => ({ changed: true, pending_targets: ['/targets/Alpha'] }))
  const checkSources = checkFor(summary('Alpha'))
  render(<Harness skills={[skill('Alpha')]} invoke={invoke} checkSources={checkSources} />)
  checkForUpdates()
  await waitUntilChecked()
  fireEvent.click(updateAction(1))
  await waitFor(() => expect(invoke).toHaveBeenCalledWith('workspace_content_execute', { id: 'plan-Alpha' }))
  await waitUntilChecked()
  const outcome = candidates().getByText(t('updatesWorkflow.applyResult', { succeeded: 1, failed: 0 })).closest('[role="status"]')!
  expect(outcome.classList.contains('has-errors')).toBe(true)
  expect(within(outcome as HTMLElement).getByText(t('updatesWorkflow.targetsPending', { name: 'Alpha', count: 1 }))).toBeTruthy()
})

it('keeps optional per-row review scoped to that row and closes it before the write finishes', async () => {
  let finish!: (value: { changed: boolean }) => void
  const executing = new Promise<{ changed: boolean }>(resolve => { finish = resolve })
  const invoke = desktopInvoke(() => executing)
  const checkSources = checkFor(summary('Alpha'), summary('Beta'))
  const onReview = vi.fn()
  render(<Harness skills={[skill('Alpha'), skill('Beta')]} invoke={invoke} checkSources={checkSources} onReview={onReview} reviewModal />)
  checkForUpdates()
  await waitUntilChecked()
  fireEvent.click(candidates().getByRole('checkbox', { name: t('updatesWorkflow.selectSkill', { name: 'Alpha' }) }))
  fireEvent.click(within(candidates().getByText('Beta').closest('li')!).getByRole('button', { name: t('updatesWorkflow.review') }))
  const dialog = await screen.findByRole('dialog', { name: t('workspace110.reviewUpdate') })
  expect(onReview.mock.calls[0][0].map((item: UpdateCandidate) => item.skillId)).toEqual(['Beta'])
  await waitFor(() => expect(within(dialog).getByText('SKILL.md')).toBeTruthy())
  expect(invoke.mock.calls.some(([command]) => command === 'workspace_content_execute')).toBe(false)
  await confirmReview()
  await waitFor(() => expect(invoke).toHaveBeenCalledWith('workspace_content_execute', { id: 'plan-Beta' }))
  expect(screen.queryByRole('dialog')).toBeNull()
  expect(candidates().getByText(t('updatesWorkflow.applying'))).toBeTruthy()
  await act(async () => { finish({ changed: true }); await executing })
  await waitUntilChecked()
  expect(invoke.mock.calls.filter(([command]) => command === 'workspace_content_execute')).toHaveLength(1)
  expect(candidates().getByText('Alpha')).toBeTruthy()
  expect(updateAction(1, true).disabled).toBe(false)
})

it('shows completed updates after reloading changed Skill metadata without claiming sources are current', async () => {
  const original = skill('Alpha', { content_hash: 'before' })
  const invoke = desktopInvoke()
  const checkSources = checkFor(summary('Alpha'))
  const reloaded = vi.fn()
  function ReloadingHarness() {
    const [skills, setSkills] = useState([original])
    return <Harness skills={skills} invoke={invoke} checkSources={checkSources} reviewModal onReload={async () => {
      reloaded()
      setSkills([{ ...original, updated_at: 2, content_hash: 'after' }])
    }} />
  }
  render(<ReloadingHarness />)
  checkForUpdates()
  await waitUntilChecked()
  fireEvent.click(candidates().getByRole('button', { name: t('updatesWorkflow.review') }))
  await confirmReview()
  await waitUntilChecked()
  expect(reloaded).toHaveBeenCalledOnce()
  expect(candidates().getByRole('heading', { name: t('updatesWorkflow.applied') })).toBeTruthy()
  expect(candidates().getByText(t('updatesWorkflow.appliedHint'))).toBeTruthy()
  expect(candidates().queryByText(t('updatesWorkflow.incomplete'))).toBeNull()
  expect(candidates().queryByText(t('updatesWorkflow.stale'))).toBeNull()
  expect(candidates().queryByText(t('updatesWorkflow.upToDate'))).toBeNull()
  expect(candidates().queryByRole('list')).toBeNull()
})

it('still shows expired results when Skill metadata changes outside an update', async () => {
  const original = skill('Alpha', { content_hash: 'before' })
  const invoke = desktopInvoke()
  const checkSources = checkFor(summary('Alpha', 'unchanged'))
  const page = render(<Harness skills={[original]} invoke={invoke} checkSources={checkSources} />)
  checkForUpdates()
  await waitUntilChecked()
  expect(candidates().getByRole('heading', { name: t('updatesWorkflow.upToDate') })).toBeTruthy()
  page.rerender(<Harness skills={[{ ...original, content_hash: 'external-change' }]} invoke={invoke} checkSources={checkSources} />)
  expect(candidates().getByRole('heading', { name: t('updatesWorkflow.incomplete') })).toBeTruthy()
  expect(candidates().getByText(t('updatesWorkflow.stale'))).toBeTruthy()
  expect(candidates().queryByText(t('updatesWorkflow.upToDate'))).toBeNull()
})

it.each(['partial', 'reload'] as const)('does not hide incomplete results after a %s failure followed by changed metadata', async failure => {
  const original = [skill('Alpha'), skill('Beta')]
  const invoke = desktopInvoke(async id => {
    if (failure === 'partial' && id === 'plan-Beta') throw new Error('PLAN_STALE')
    return { changed: true }
  })
  const checkSources = checkFor(summary('Alpha'), summary('Beta'))
  const reloaded = vi.fn()
  function ReloadingHarness() {
    const [skills, setSkills] = useState(original)
    return <Harness skills={skills} invoke={invoke} checkSources={checkSources} reviewModal onReload={async () => {
      reloaded()
      setSkills(original.map(skill => ({ ...skill, updated_at: 2, content_hash: 'refreshed' })))
      if (failure === 'reload') throw new Error('reload failed')
    }} />
  }
  render(<ReloadingHarness />)
  checkForUpdates()
  await waitUntilChecked()
  fireEvent.click(candidates().getByRole('checkbox', { name: t('updatesWorkflow.selectAll') }))
  fireEvent.click(updateAction(2, true))
  await waitFor(() => expect(invoke).toHaveBeenCalledWith('workspace_content_execute', { id: 'plan-Alpha' }))
  await waitUntilChecked()
  expect(reloaded).toHaveBeenCalledOnce()
  expect(candidates().getByRole('heading', { name: t('updatesWorkflow.incomplete') })).toBeTruthy()
  expect(candidates().getByText(t('updatesWorkflow.stale'))).toBeTruthy()
  expect(candidates().queryByText(t('updatesWorkflow.applied'))).toBeNull()
  expect(candidates().queryByText(t('updatesWorkflow.upToDate'))).toBeNull()
  expect(candidates().getByText(t('updatesWorkflow.applyResult', { succeeded: failure === 'partial' ? 1 : 2, failed: failure === 'partial' ? 1 : 0 }))).toBeTruthy()
})
