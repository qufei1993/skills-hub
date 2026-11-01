import { describe, expect, it } from 'vitest'
import i18n from '../../../i18n'
import { formatUpdateCheckError } from './updateCheckError'

describe.each(['en', 'zh', 'ko'])('update check errors in %s', language => {
  const t = i18n.getFixedT(language)

  it('localizes real command categories while preserving actionable paths', () => {
    const path = '/tmp/项目/skill | copy'
    for (const [prefix, key] of [
      ['TARGET_MODIFIED', 'errors.targetModified'],
      ['TARGET_EXISTS', 'errors.targetExistsDetail'],
      ['CENTRAL_MODIFIED', 'errors.centralModified'],
    ]) {
      const result = formatUpdateCheckError(`${prefix}|${path}`, t)
      expect(result).toBe(t(key, { path }))
      expect(result).toContain(path)
      expect(result).not.toContain(`${prefix}|`)
    }
    expect(formatUpdateCheckError('PLAN_STALE|state changed; preview the operation again', t))
      .toBe(t('updatesWorkflow.stalePlan'))
  })

  it('localizes source and busy messages actually returned by the workspace service', () => {
    const source = formatUpdateCheckError('source or recovery content is unavailable; check the source and try again', t)
    expect(source).toBe(t('updatesWorkflow.sourceUnavailable'))
    expect(source).not.toBe('updatesWorkflow.sourceUnavailable')
    expect(formatUpdateCheckError('INVALID_SOURCE|/tmp/missing-source', t)).toContain('/tmp/missing-source')
    expect(formatUpdateCheckError('another Skills Hub operation is already in progress', t))
      .toBe(t('updatesWorkflow.operationBusy'))
    expect(formatUpdateCheckError('UPDATE_IN_PROGRESS|lock is held', t))
      .toBe(t('errors.updateInProgress'))
  })

  it('keeps both rollback paths and handles malformed details without throwing', () => {
    const detail = { target: '/tmp/original', recovery: '/tmp/recovered-write' }
    const result = formatUpdateCheckError(`ROLLBACK_CONFLICT|${JSON.stringify(detail)}`, t)
    expect(result).toBe(t('errors.rollbackConflict', detail))
    expect(result).toContain(detail.target)
    expect(result).toContain(detail.recovery)
    expect(formatUpdateCheckError('ROLLBACK_CONFLICT|not-json', t))
      .toBe(t('errors.rollbackConflictUnknown'))
    expect(formatUpdateCheckError(`ROLLBACK_CONFLICT|${JSON.stringify({ recovery: detail.recovery })}`, t))
      .toContain(detail.recovery)
  })

  it('keeps unfamiliar diagnostics and does not classify codes inside a path', () => {
    const raw = 'cannot read /tmp/PLAN_STALE/notes.md'
    const result = formatUpdateCheckError(raw, t)
    expect(result).toContain(raw)
    expect(result).not.toBe(t('updatesWorkflow.stalePlan'))
    expect(formatUpdateCheckError('', t)).toBe(t('updatesWorkflow.checkFailed'))
  })
})
