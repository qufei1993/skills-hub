import { memo, useEffect, useState } from 'react'
import { ArrowRight, Bot, X } from 'lucide-react'
import type { TFunction } from 'i18next'
import type { AgentAccessStatusDto, ManagedSkill } from './types'

const dismissalKey = 'skills-ai-management-notice-dismissed-v1'

type Props = {
  isTauri: boolean
  invokeTauri: (command: string, args?: Record<string, unknown>) => Promise<unknown>
  skills: readonly ManagedSkill[]
  onStatusChanged?: (status: AgentAccessStatusDto) => void
  onOpen: () => void
  t: TFunction
}

const AiManagementNotice = ({ isTauri, invokeTauri, skills, onStatusChanged, onOpen, t }: Props) => {
  const [dismissed, setDismissed] = useState(() => {
    try { return localStorage.getItem(dismissalKey) === 'true' } catch { return false }
  })
  const [status, setStatus] = useState<AgentAccessStatusDto | null>(null)
  useEffect(() => {
    if (!isTauri || dismissed) return
    let generation = 0
    const read = async () => {
      const request = ++generation
      try {
        const result = await invokeTauri('get_agent_access_status') as AgentAccessStatusDto
        if (request !== generation) return
        setStatus(result)
        onStatusChanged?.(result)
      } catch {
        if (request === generation) setStatus(null)
      }
    }
    void read()
    window.addEventListener('focus', read)
    return () => {
      generation++
      window.removeEventListener('focus', read)
    }
  }, [isTauri, dismissed, invokeTauri, skills, onStatusChanged])

  if (!isTauri || dismissed || !status || status.installed || status.officialState !== 'missing') return null
  return <aside className="ai-management-notice" aria-label={t('aiManagement.notice.title')}>
    <span className="ai-management-notice-icon"><Bot size={22} aria-hidden="true" /></span>
    <div className="ai-management-notice-copy">
      <strong>{t('aiManagement.notice.title')}</strong>
      <p>{t('aiManagement.notice.description')}</p>
    </div>
    <button className="btn btn-primary" type="button" onClick={onOpen}>
      {t('aiManagement.notice.action')}<ArrowRight size={16} aria-hidden="true" />
    </button>
    <button className="ai-management-notice-close" type="button" aria-label={t('aiManagement.notice.dismiss')} onClick={() => {
      setDismissed(true)
      try { localStorage.setItem(dismissalKey, 'true') } catch { /* Keep dismissal for this mounted page when storage is unavailable. */ }
    }}><X size={18} aria-hidden="true" /></button>
  </aside>
}
export default memo(AiManagementNotice)
