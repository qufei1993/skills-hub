import { memo, useCallback, useEffect, useRef, useState } from 'react'
import { Bot, CheckCircle2 } from 'lucide-react'
import type { TFunction } from 'i18next'
import type { AgentAccessStatusDto } from './types'

type Props = {
  focusOnMount?: boolean
  initialStatus?: AgentAccessStatusDto | null
  onStatusChanged?: (status: AgentAccessStatusDto) => void
  isTauri: boolean
  invokeTauri: (command: string, args?: Record<string, unknown>) => Promise<unknown>
  onChanged: () => void | Promise<void>
  onOpenSkill: (id: string) => void
  t: TFunction
}

const AiManagementSettings = ({ focusOnMount = false, initialStatus = null, onStatusChanged, isTauri, invokeTauri, onChanged, onOpenSkill, t }: Props) => {
  const cardRef = useRef<HTMLElement>(null)
  useEffect(() => {
    if (!focusOnMount) return
    cardRef.current?.scrollIntoView?.({ block: 'center', behavior: 'instant' })
    cardRef.current?.focus({ preventScroll: true })
  }, [focusOnMount])
  const [status, setStatus] = useState<AgentAccessStatusDto | null>(initialStatus)
  const [pending, setPending] = useState(false)
  const [enabling, setEnabling] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const busy = useRef(false)
  const mounted = useRef(false)
  const read = useCallback(async () => {
    if (!isTauri || busy.current) return
    busy.current = true
    setPending(true)
    setError(null)
    try {
      const result = await invokeTauri('get_agent_access_status') as AgentAccessStatusDto
      if (mounted.current) {
        setStatus(result)
        onStatusChanged?.(result)
      }
    } catch {
      if (mounted.current) setError('read')
    } finally {
      busy.current = false
      if (mounted.current) setPending(false)
    }
  }, [isTauri, invokeTauri, onStatusChanged])
  useEffect(() => {
    mounted.current = true
    void read()
    return () => { mounted.current = false }
  }, [read])

  const enable = async () => {
    if (!isTauri || busy.current) return
    busy.current = true
    setEnabling(true)
    setPending(true)
    setError(null)
    try {
      const result = await invokeTauri('enable_ai_management') as AgentAccessStatusDto
      if (mounted.current) {
        setStatus(result)
        onStatusChanged?.(result)
      }
      await onChanged()
    } catch (cause) {
      const message = String(cause)
      if (mounted.current) setError(message.includes('CLI_TERMINAL_UNAVAILABLE') ? 'terminal'
        : message.includes('CLI_UNAVAILABLE') ? 'cli'
        : message.includes('AGENT_NOT_FOUND') ? 'noTools'
          : message.includes('OPERATION_BUSY') ? 'busy'
            : message.includes('SHARED_DIRECTORY_SCOPE_EXPANSION') ? 'sharedDirectory'
              : message.includes('TARGET_CONFLICT') ? 'conflict' : 'action')
      try {
        const latest = await invokeTauri('get_agent_access_status') as AgentAccessStatusDto
        if (mounted.current) {
          setStatus(latest)
          onStatusChanged?.(latest)
        }
        await onChanged()
      } catch {
        return
      }
    } finally {
      busy.current = false
      if (mounted.current) {
        setPending(false)
        setEnabling(false)
      }
    }
  }
  const updatePending = Boolean(status?.installed && (status.bridge.reason === 'VERSION_MISMATCH'
    || (status.installedVersion && status.installedVersion !== status.bundledVersion)))
  const activeDeployment = status?.agents.some(agent => agent.deployed && !agent.needsRepair && agent.enabled && agent.detected)
  const ready = !updatePending && status?.skillEnabled && status.terminalReady && !status.terminalPathConflict && status.bridge.status === 'valid' && status.officialState === 'healthy'
    && activeDeployment
  const conflict = status?.officialState === 'name_conflict'
  const inactive = !updatePending && status?.installed && status.terminalReady && status.officialState === 'healthy' && status.bridge.status === 'valid' && (!status.skillEnabled || !activeDeployment)
  const pathConflictOnly = status?.terminalPathConflict && !updatePending && status.installed && status.bridge.status === 'valid' && status.officialState === 'healthy'
  return <section ref={cardRef} tabIndex={-1} className={`settings-card ai-management-card${focusOnMount ? ' ai-management-card-highlight' : ''}${ready ? ' ai-management-card-ready' : ''}`} aria-label={t('aiManagement.title')} aria-busy={pending}>
    <div className="settings-card-head">
      <span className="settings-card-icon"><Bot size={18} aria-hidden="true" /></span>
      <div className="ai-management-copy">
        <div className="ai-management-title">
          <h2>{t('aiManagement.title')}</h2>
          {ready ? <strong className="ai-management-ready"><CheckCircle2 size={14} aria-hidden="true" />{t('aiManagement.ready')}</strong> : null}
        </div>
        <p>{t('aiManagement.description')}</p>
      </div>
      <div className="ai-management-actions">
        {!ready && !inactive && !pathConflictOnly && status ? <button className="btn btn-primary" type="button" disabled={pending || !isTauri || conflict} onClick={() => void enable()}>{t(enabling ? 'aiManagement.enabling' : updatePending ? 'aiManagement.update' : 'aiManagement.enable')}</button> : null}
        {!status && isTauri ? <button className="btn btn-secondary" type="button" disabled={pending} onClick={() => void read()}>{t(pending ? 'agentAccess.loading' : 'agentAccess.retry')}</button> : null}
        {status?.skillId ? <button className="btn btn-secondary" type="button" disabled={enabling} onClick={() => onOpenSkill(status.skillId!)}>{t('aiManagement.viewSkill')}</button> : null}
      </div>
    </div>
    <div className="ai-management-body">
      {!isTauri ? <p className="settings-helper">{t('agentAccess.desktopOnly')}</p> : null}
      {ready ? <p className="settings-helper">{t('aiManagement.terminalReady')}</p> : null}
      {updatePending ? <p className="settings-helper">{t('aiManagement.updatePending')}</p> : null}
      {status?.terminalPathConflict ? <p className="settings-helper">{t('aiManagement.pathConflict')}</p> : null}
      {inactive ? <p className="settings-helper">{t('aiManagement.inactive')}</p> : null}
      {conflict ? <p role="alert">{t('aiManagement.errors.conflict')}</p> : null}
      {error ? <p role="alert" className="ai-management-error">{t(`aiManagement.errors.${error}`)}</p> : null}
      {status ? <details className="ai-management-details">
        <summary>{t('aiManagement.details')}</summary>
        <dl>
          <dt>{t('agentAccess.cliStatus')}</dt><dd>{t(`agentAccess.bridgeState.${status.bridge.status}`)}</dd>
          <dt>{t('agentAccess.cliVersion')}</dt><dd>{status.bridge.version ?? t('agentAccess.unknown')}</dd>
          <dt>{t('agentAccess.cliPath')}</dt><dd><code>{status.bridge.path}</code></dd>
          {status.bridge.reason ? <><dt>{t('agentAccess.checkReason')}</dt><dd>{t(`agentAccess.reason.${status.bridge.reason}`)}</dd></> : null}
          <dt>{t('agentAccess.officialSkill')}</dt><dd>manage-skills-hub</dd>
          <dt>{t('agentAccess.installedVersion')}</dt><dd>{status.installedVersion ?? t('agentAccess.notInstalled')}</dd>
          {status.centralReason ? <><dt>{t('agentAccess.checkReason')}</dt><dd>{t(`agentAccess.healthReason.${status.centralReason}`)}</dd></> : null}
        </dl>
      </details> : null}
    </div>
  </section>
}
export default memo(AiManagementSettings)
