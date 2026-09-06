import { memo, useId, useState } from 'react'
import { AlertTriangle, ChevronDown, ChevronRight } from 'lucide-react'
import type { TFunction } from 'i18next'

type Props = {
  issues: { skill_name: string; tool: string }[]
  toolLabels?: Record<string, string>
  onOpen: () => void
  t: TFunction
}

const ToolSyncNotice = memo(({ issues, toolLabels, onOpen, t }: Props) => {
  const [expanded, setExpanded] = useState(false)
  const detailsId = useId()
  if (!issues.length) return null

  return <section className="tool-sync-notice" aria-label={t('deviceSync.toolUpdatesPending', { count: issues.length })}>
    <div className="tool-sync-notice-header">
      <AlertTriangle size={18} aria-hidden="true" />
      <div className="tool-sync-notice-copy">
        <strong>{t('deviceSync.toolUpdatesPending', { count: issues.length })}</strong>
        <p>{t('deviceSync.toolUpdatesSummary')}</p>
      </div>
      <div className="tool-sync-notice-actions">
        <button type="button" className="tool-sync-notice-toggle" aria-expanded={expanded} aria-controls={detailsId} onClick={() => setExpanded(!expanded)}>
          {t(expanded ? 'deviceSync.hideToolIssues' : 'deviceSync.showToolIssues')}<ChevronDown size={14} aria-hidden="true" />
        </button>
        <button type="button" className="btn btn-secondary" onClick={onOpen}>{t('deviceSync.openToolIssues')}<ChevronRight size={14} aria-hidden="true" /></button>
      </div>
    </div>
    {expanded ? <div className="tool-sync-notice-details" id={detailsId}>
      <p>{t('deviceSync.toolUpdatesPendingHelp')}</p>
      <div className="tool-sync-notice-list" tabIndex={0} role="region" aria-label={t('deviceSync.toolUpdatesPending', { count: issues.length })}>
        <table>
          <thead><tr><th>{t('deviceSync.toolIssueSkill')}</th><th>{t('deviceSync.toolIssueTool')}</th></tr></thead>
          <tbody>{issues.map((issue) => <tr key={`${issue.skill_name}:${issue.tool}`}>
            <td title={issue.skill_name}>{issue.skill_name}</td>
            <td title={toolLabels?.[issue.tool] || issue.tool}>{toolLabels?.[issue.tool] || issue.tool}</td>
          </tr>)}</tbody>
        </table>
      </div>
    </div> : null}
  </section>
})

export default ToolSyncNotice
