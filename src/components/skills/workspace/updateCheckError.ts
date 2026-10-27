import type { TFunction } from 'i18next'

export function formatUpdateCheckError(error: string, t: TFunction): string {
  const separator = error.indexOf('|')
  const code = separator < 0 ? error : error.slice(0, separator)
  const detail = separator < 0 ? '' : error.slice(separator + 1)
  switch (code) {
    case 'PLAN_STALE': return t('updatesWorkflow.stalePlan')
    case 'TARGET_MODIFIED': return t('errors.targetModified', { path: detail })
    case 'TARGET_EXISTS': return t('errors.targetExistsDetail', { path: detail })
    case 'CENTRAL_MODIFIED': return t('errors.centralModified', { path: detail })
    case 'UPDATE_IN_PROGRESS': return t('errors.updateInProgress')
    case 'OPERATION_BUSY': return t('updatesWorkflow.operationBusy')
    case 'INVALID_SOURCE': return detail ? `${t('updatesWorkflow.sourceUnavailable')}\n${detail}` : t('updatesWorkflow.sourceUnavailable')
    case 'ROLLBACK_CONFLICT': {
      try {
        const paths = JSON.parse(detail) as { target?: unknown; recovery?: unknown }
        if (typeof paths.target === 'string' && typeof paths.recovery === 'string') {
          return t('errors.rollbackConflict', { target: paths.target, recovery: paths.recovery })
        }
        const available = [paths.target, paths.recovery].filter((path): path is string => typeof path === 'string')
        if (available.length) return `${t('errors.rollbackConflictUnknown')}\n${available.join('\n')}`
      } catch { /* Malformed details still need a recoverable user-facing message. */ }
      return t('errors.rollbackConflictUnknown')
    }
  }
  // Older desktop commands return these safe service messages without a code.
  if (error === 'state changed; preview the operation again') return t('updatesWorkflow.stalePlan')
  if (error === 'source or recovery content is unavailable; check the source and try again') return t('updatesWorkflow.sourceUnavailable')
  if (error === 'another Skills Hub operation is already in progress') return t('updatesWorkflow.operationBusy')
  const fallback = t('updatesWorkflow.checkFailed')
  return error.trim() ? `${fallback}\n${error}` : fallback
}
