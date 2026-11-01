import { memo, useCallback, useEffect, useRef, useState } from 'react'
import { Bot, CheckCircle2 } from 'lucide-react'
import type { TFunction } from 'i18next'
import type { AgentAccessStatusDto, CliPreparationProgress } from './types'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

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
  const [progress, setProgress] = useState<CliPreparationProgress | null>(null)
  const activeOperation = useRef<{ id: string; unlisten?: UnlistenFn } | null>(null)
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
    return () => {
      mounted.current = false
      activeOperation.current?.unlisten?.()
      activeOperation.current = null
    }
  }, [read])

  const enable = async () => {
    if (!isTauri || busy.current) return
    busy.current = true
    setEnabling(true)
    setPending(true)
    setError(null)
    const operationId = crypto.randomUUID()
    const operation: { id: string; unlisten?: UnlistenFn } = { id: operationId }
    activeOperation.current = operation
    setProgress({ operationId, phase: 'preparing', downloadedBytes: 0, totalBytes: null })
    try {
      const unsubscribe = await listen<CliPreparationProgress>('ai-management-progress', event => {
        if (mounted.current && activeOperation.current?.id === event.payload.operationId) setProgress(event.payload)
      })
      if (!mounted.current || activeOperation.current !== operation) { unsubscribe(); return }
      operation.unlisten = unsubscribe
      const result = await invokeTauri('enable_ai_management', { operationId }) as AgentAccessStatusDto
      if (mounted.current) {
        setStatus(result)
        onStatusChanged?.(result)
      }
      try {
        await onChanged()
      } catch {
        if (mounted.current) setError('refresh')
      }
    } catch (cause) {
      const message = String(cause)
      if (mounted.current) setError(message.includes('CLI_TERMINAL_UNAVAILABLE') ? 'terminal'
        : message.includes('CLI_DOWNLOAD_UNAVAILABLE') ? 'unavailable'
        : message.includes('CLI_DOWNLOAD_FAILED') ? 'download'
        : message.includes('CLI_INTEGRITY_FAILED') ? 'integrity'
        : message.includes('CLI_UNAVAILABLE') ? 'cli'
        : message.includes('AGENT_NOT_FOUND') ? 'noTools'
          : message.includes('BUSY') ? 'busy'
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
      if (activeOperation.current === operation) {
        operation.unlisten?.()
        activeOperation.current = null
      }
      busy.current = false
      if (mounted.current) {
        setProgress(null)
        setPending(false)
        setEnabling(false)
      }
    }
  }
  const updatePending = Boolean(status?.installed && (status.skillUpdateAvailable || status.bridge.reason === 'VERSION_MISMATCH'
    || (status.installedVersion && status.installedVersion !== status.bundledVersion)))
  const activeDeployment = status?.agents.some(agent => agent.deployed && !agent.needsRepair && agent.enabled && agent.detected)
  const ready = !updatePending && status?.skillEnabled && status.terminalReady && !status.terminalPathConflict && status.bridge.status === 'valid' && status.officialState === 'healthy'
    && activeDeployment
  const conflict = status?.officialState === 'name_conflict'
  const inactive = !updatePending && status?.installed && status.terminalReady && status.officialState === 'healthy' && status.bridge.status === 'valid' && (!status.skillEnabled || !activeDeployment)
  const pathConflictOnly = status?.terminalReady && status.terminalPathConflict && !updatePending && status.installed && status.bridge.status === 'valid' && status.officialState === 'healthy'
  const cliInstallationFailure = error && ['unavailable', 'download', 'integrity', 'cli'].includes(error)
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
      {enabling && progress ? <div role="status" aria-live="polite" className="ai-management-progress">
        <span>{t(`aiManagement.progress.${progress.phase}`)}</span>
        {progress.phase === 'downloading' ? <>
          <span>{progress.totalBytes ? t('aiManagement.progress.bytesTotal', { downloaded: (progress.downloadedBytes / 1048576).toFixed(1), total: (progress.totalBytes / 1048576).toFixed(1) }) : t('aiManagement.progress.bytes', { downloaded: (progress.downloadedBytes / 1048576).toFixed(1) })}</span>
          {progress.totalBytes ? <progress aria-label={t('aiManagement.progress.downloading')} max={100} value={Math.min(100, Math.floor(progress.downloadedBytes * 100 / progress.totalBytes))} /> : null}
        </> : null}
      </div> : null}
      {error ? <p role="alert" className="ai-management-error">{t(`aiManagement.errors.${error}`)}</p> : null}
      {status ? <details className="ai-management-details">
        <summary>{t('aiManagement.details')}</summary>
        <dl>
          <dt>{t('agentAccess.cliStatus')}</dt><dd>{t(`agentAccess.bridgeState.${status.bridge.status}`)}</dd>
          <dt>{t('agentAccess.cliVersion')}</dt><dd>{status.bridge.version ?? t('agentAccess.unknown')}</dd>
          <dt>{t('agentAccess.cliPath')}</dt><dd><code>{status.bridge.path}</code></dd>
          {status.bridge.reason ? <><dt>{t('agentAccess.checkReason')}</dt><dd>{t(cliInstallationFailure ? 'aiManagement.cliNeedsInstallation' : `agentAccess.reason.${status.bridge.reason}`)}</dd></> : null}
          {cliInstallationFailure ? <><dt>{t('aiManagement.cliInstallationFailure')}</dt><dd>{t(`aiManagement.errors.${error}`)}</dd></> : null}
          <dt>{t('agentAccess.officialSkill')}</dt><dd>manage-skills-hub</dd>
          <dt>{t('agentAccess.installedVersion')}</dt><dd>{status.installedVersion ?? t('agentAccess.notInstalled')}</dd>
          {status.centralReason ? <><dt>{t('agentAccess.checkReason')}</dt><dd>{t(`agentAccess.healthReason.${status.centralReason}`)}</dd></> : null}
        </dl>
      </details> : null}
    </div>
  </section>
}
export default memo(AiManagementSettings)
