import { memo, useCallback, useEffect, useId, useRef, useState } from 'react'
import { CheckCircle2, CircleMinus, Copy, LoaderCircle, RefreshCw, TriangleAlert } from 'lucide-react'
import type { TFunction } from 'i18next'
import ConfirmActionModal from './modals/ConfirmActionModal'
import type { AgentAccessAgentDto, AgentAccessStatusDto } from './types'

type Action = 'install' | 'repair' | 'remove'
type Props = {
  isTauri: boolean
  invokeTauri: (command: string, args?: Record<string, unknown>) => Promise<unknown>
  t: TFunction
}
const npmCommand = 'npm install -g skillshub-cli'

const AgentAccessPage = ({ isTauri, invokeTauri, t }: Props) => {
  const [status, setStatus] = useState<AgentAccessStatusDto | null>(null)
  const [reading, setReading] = useState(false)
  const [pending, setPending] = useState<{ key: string; action: Action } | null>(null)
  const [removing, setRemoving] = useState<AgentAccessAgentDto | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [success, setSuccess] = useState<{ action: Action; agent: string } | null>(null)
  const [copyState, setCopyState] = useState<'idle' | 'copying' | 'copied' | 'failed'>('idle')
  const busy = useRef(false)
  const refreshRef = useRef<HTMLButtonElement>(null)
  const mounted = useRef(false)
  const conflictHelpId = useId()
  const nameConflict = status?.officialState === 'name_conflict'

  const refresh = useCallback(async () => {
    if (!isTauri || busy.current) return
    busy.current = true
    setReading(true)
    setError(null)
    setSuccess(null)
    try {
      const result = await invokeTauri('get_agent_access_status') as AgentAccessStatusDto
      if (mounted.current) setStatus(result)
    } catch {
      if (mounted.current) setError('read')
    } finally {
      busy.current = false
      if (mounted.current) setReading(false)
    }
  }, [isTauri, invokeTauri])

  useEffect(() => {
    mounted.current = true
    void refresh()
    return () => { mounted.current = false }
  }, [refresh])

  const changeAccess = async (agent: AgentAccessAgentDto, action: Action) => {
    if (!isTauri || busy.current || nameConflict) return
    busy.current = true
    setPending({ key: agent.key, action })
    setError(null)
    setSuccess(null)
    try {
      const result = await invokeTauri('set_agent_access', {
        agent: agent.key, action, ...(action === 'remove' ? { confirmed: true } : {}),
      }) as AgentAccessStatusDto
      if (mounted.current) {
        setStatus(result)
        setSuccess({ action, agent: agent.label })
        setRemoving(null)
      }
    } catch (cause) {
      if (mounted.current) {
        const message = String(cause)
        setError(message.includes('TARGET_MODIFIED') ? 'targetModified'
          : message.includes('TARGET_CONFLICT') || message.includes('TARGET_EXISTS') ? 'conflict'
            : message.includes('OPERATION_BUSY') ? 'busy' : 'action')
        setRemoving(null)
      }
    } finally {
      busy.current = false
      if (mounted.current) setPending(null)
    }
  }

  const copyCommand = async () => {
    if (copyState === 'copying') return
    setCopyState('copying')
    try {
      await navigator.clipboard.writeText(npmCommand)
      if (mounted.current) setCopyState('copied')
    } catch {
      if (mounted.current) setCopyState('failed')
    }
  }
  const working = reading || pending !== null
  const visibleAgents = status?.agents.filter(agent => agent.detected || agent.deployed) ?? []
  return <section className="agent-access-page" aria-label={t('manageTabs.agents')} aria-busy={working}>
    <div className="agent-access-heading">
      <div><h2>{t('manageTabs.agents')}</h2><p>{t('agentAccess.help')}</p></div>
      <button ref={refreshRef} className="btn btn-secondary" type="button" disabled={!isTauri || working} onClick={() => void refresh()}>
        <RefreshCw size={15} aria-hidden="true" />{t('agentAccess.refresh')}
      </button>
    </div>
    {!isTauri ? <p>{t('agentAccess.desktopOnly')}</p> : null}
    {reading ? <p role="status" className="agent-access-state"><LoaderCircle size={16} aria-hidden="true" />{t('agentAccess.loading')}</p> : null}
    {error ? <p role="alert" className="agent-access-feedback error"><TriangleAlert size={16} aria-hidden="true" />{t(`agentAccess.errors.${error}`)}</p> : null}
    {success ? <p role="status" className="agent-access-feedback success"><CheckCircle2 size={16} aria-hidden="true" />{t(`agentAccess.success.${success.action}`, { agent: success.agent })}</p> : null}
    {status ? <>
      {nameConflict ? <div className="agent-access-feedback warning" role="status">
        <TriangleAlert size={16} aria-hidden="true" />
        <div>
          <strong>{t('agentAccess.nameConflict.title')}</strong>
          <p id={conflictHelpId}>{t('agentAccess.nameConflict.help')}</p>
          {status.conflict ? <dl className="agent-access-conflict-details">
            <div><dt>{t('agentAccess.nameConflict.source')}</dt><dd>{t(`agentAccess.nameConflict.sourceKind.${status.conflict.sourceKind}`)}</dd></div>
            <div><dt>{t('agentAccess.nameConflict.location')}</dt><dd><code>{status.conflict.centralPath}</code></dd></div>
          </dl> : null}
        </div>
      </div> : null}
      <dl className="agent-access-summary">
        <div><dt>{t('agentAccess.cliStatus')}</dt><dd className={`agent-access-state ${status.bridge.status}`}>
          {status.bridge.status === 'valid' ? <CheckCircle2 size={16} aria-hidden="true" /> : <TriangleAlert size={16} aria-hidden="true" />}
          {t(`agentAccess.bridgeState.${status.bridge.status}`)}
        </dd></div>
        <div><dt>{t('agentAccess.cliVersion')}</dt><dd><code>{status.bridge.version ?? t('agentAccess.unknown')}</code></dd></div>
        <div className="agent-access-wide"><dt>{t('agentAccess.cliPath')}</dt><dd><code>{status.bridge.path}</code></dd></div>
        {status.bridge.reason ? <div className="agent-access-wide"><dt>{t('agentAccess.checkReason')}</dt><dd>{t(`agentAccess.reason.${status.bridge.reason}`)}</dd></div> : null}
        <div><dt>{t('agentAccess.officialSkill')}</dt><dd>{t(nameConflict ? 'agentAccess.nameConflict.state' : status.installed ? 'agentAccess.installed' : 'agentAccess.notInstalled')}</dd></div>
        {status.centralReason ? <div className="agent-access-wide"><dt>{t('agentAccess.checkReason')}</dt><dd className="agent-access-state damaged"><TriangleAlert size={16} aria-hidden="true" />{t(`agentAccess.healthReason.${status.centralReason}`)}</dd></div> : null}
        <div><dt>{t('agentAccess.bundledVersion')}</dt><dd><code>{status.bundledVersion}</code></dd></div>
        <div><dt>{t('agentAccess.installedVersion')}</dt><dd><code>{status.installedVersion ?? t('agentAccess.unknown')}</code></dd></div>
      </dl>
      <div className="agent-access-table-scroll" role="region" aria-label={t('agentAccess.agents')} tabIndex={0}>
        <table className="agent-access-table">
          <caption>{t('agentAccess.agents')}</caption>
          <thead><tr>{['agent', 'detection', 'configuration', 'deployment', 'actions'].map(column => <th key={column} scope="col">{t(`agentAccess.column.${column}`)}</th>)}</tr></thead>
          <tbody>{visibleAgents.map(agent => {
            const operation = pending?.key === agent.key ? pending.action : null
            const progress = operation ? t(`agentAccess.${operation === 'install' ? 'installing' : operation === 'repair' ? 'repairing' : 'removing'}`) : null
            return <tr key={agent.key}>
              <th scope="row"><strong>{agent.label}</strong><code>{agent.path}</code></th>
              <td><span className="agent-access-state">{agent.detected ? <CheckCircle2 size={14} aria-hidden="true" /> : <CircleMinus size={14} aria-hidden="true" />}{t(agent.detected ? 'agentAccess.detected' : 'agentAccess.notDetected')}</span></td>
              <td>{t(agent.enabled ? 'agentAccess.enabled' : 'agentAccess.disabled')}</td>
              <td><span className={`agent-access-state${agent.needsRepair ? ' damaged' : ''}`}>
                {agent.needsRepair ? <TriangleAlert size={14} aria-hidden="true" /> : agent.deployed ? <CheckCircle2 size={14} aria-hidden="true" /> : <CircleMinus size={14} aria-hidden="true" />}
                {t(agent.needsRepair ? 'agentAccess.needsRepair' : agent.deployed ? 'agentAccess.deployed' : 'agentAccess.notDeployed')}
              </span>{agent.reason ? <small className="agent-access-health-reason">{t(`agentAccess.healthReason.${agent.reason}`)}</small> : null}</td>
              <td><div className="agent-access-actions">
                <button className={`btn ${agent.deployed ? 'btn-secondary' : 'btn-primary'}`} type="button" disabled={working || nameConflict || !agent.detected || !agent.enabled} aria-describedby={nameConflict ? conflictHelpId : undefined} onClick={() => void changeAccess(agent, agent.deployed ? 'repair' : 'install')}>
                  {operation && operation !== 'remove' ? <LoaderCircle size={14} aria-hidden="true" /> : null}
                  {operation && operation !== 'remove' ? progress : t(agent.deployed ? 'agentAccess.repair' : 'agentAccess.install')}
                </button>
                {agent.deployed && status.installed && !nameConflict ? <button className="btn btn-danger" type="button" disabled={working} onClick={() => setRemoving(agent)}>{operation === 'remove' ? progress : t('agentAccess.remove')}</button> : null}
              </div></td>
            </tr>
          })}</tbody>
        </table>
      </div>
      {!visibleAgents.length ? <p>{t('agentAccess.empty')}</p> : null}
      <p className="agent-access-note">{t('agentAccess.prerequisite')}</p>
    </> : null}
    <div className="agent-access-cli-only">
      <h3>{t('agentAccess.cliOnly')}</h3>
      <div className="agent-access-command"><code tabIndex={0}>{npmCommand}</code><button className="btn btn-secondary" type="button" disabled={copyState === 'copying'} onClick={() => void copyCommand()}><Copy size={15} aria-hidden="true" />{t(copyState === 'copied' ? 'agentAccess.copied' : 'agentAccess.copyCommand')}</button></div>
      {copyState === 'failed' ? <p role="alert">{t('agentAccess.copyFailed')}</p> : null}
      <p className="agent-access-note">{t('agentAccess.desktopResponsibilities')}</p>
    </div>
    <ConfirmActionModal open={removing !== null} loading={pending !== null} title={t('agentAccess.removeTitle')} body={<p>{t('agentAccess.removeBody', { agent: removing?.label })}</p>} cancelLabel={t('cancel')} confirmLabel={t(pending ? 'agentAccess.removing' : 'agentAccess.confirmRemove')} returnFocusRef={refreshRef} onRequestClose={() => setRemoving(null)} onConfirm={() => { if (removing) void changeAccess(removing, 'remove') }} />
  </section>
}
export default memo(AgentAccessPage)
