import { memo, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import ConfirmActionModal from './ConfirmActionModal'
import { installCollectionEntry, type CollectionInvoke } from '../collectionInstall'
import type { CollectionManifest, CollectionResult, InstallResultDto, ToolOption } from '../types'

type CollectionInstallModalProps = {
  open?: boolean
  manifest: CollectionManifest
  tools: ToolOption[]
  invoke: CollectionInvoke
  onComplete: () => Promise<void>
  onClose: () => void
}

export default memo(function CollectionInstallModal({ open = true, manifest, tools, invoke, onComplete, onClose }: CollectionInstallModalProps) {
  const { t } = useTranslation()
  const [selectedTools, setSelectedTools] = useState<string[]>([])
  const [results, setResults] = useState<Record<number, CollectionResult>>({})
  const [running, setRunning] = useState(false)
  const [active, setActive] = useState<number | null>(null)
  const busy = useRef(false)
  const installed = useRef<Record<number, InstallResultDto>>({})
  const close = () => { if (!busy.current) onClose() }
  const run = async () => {
    if (busy.current) return
    busy.current = true
    setRunning(true)
    try {
      for (let index = 0; index < manifest.skills.length; index++) {
        if (results[index] && results[index].state !== 'failed') continue
        setActive(index)
        try {
          const result = await installCollectionEntry(invoke, manifest, index, selectedTools, installed.current[index], created => { installed.current[index] = created })
          setResults(previous => ({ ...previous, [index]: result }))
        } catch (error) {
          const raw = error instanceof Error ? error.message : String(error)
          const message = raw.startsWith('collectionInstall.') ? t(raw) : raw
          setResults(previous => ({ ...previous, [index]: { state: 'failed', message } }))
        }
      }
      await onComplete()
    } catch { toast.error(t('collectionInstall.refreshFailed')) }
    finally { setActive(null); setRunning(false); busy.current = false }
  }
  const completed = Object.keys(results).length === manifest.skills.length
  const failed = Object.values(results).some(result => result.state === 'failed')
  return <ConfirmActionModal open={open} loading={running} intent="default" title={t('collectionInstall.title')}
    cancelLabel={t('collectionInstall.close')}
    confirmLabel={t(completed ? (failed ? 'collectionInstall.retry' : 'collectionInstall.done') : 'collectionInstall.install')}
    onRequestClose={close} onConfirm={() => { if (completed && !failed) close(); else void run() }}
    body={<div className="collection-install-body">
      <h3>{manifest.title}</h3>
      <p>{t('collectionInstall.intro', { count: manifest.skills.length })}</p>
      <ul aria-label={t('collectionInstall.listLabel')}>{manifest.skills.map((skill, index) => <li key={`${skill.source}/${skill.path}`}>
        <div><strong>{skill.name}</strong><small>{manifest.sources[skill.source].repo} · {skill.path}</small></div>
        <span role="status">{active === index ? t('collectionInstall.working') : results[index] ? t(`collectionInstall.${results[index].state}`) : t('collectionInstall.pending')}</span>
        {results[index]?.message && <p className="collection-install-error">{results[index].message}</p>}
      </li>)}</ul>
      <fieldset disabled={running || Object.keys(results).length > 0}>
        <legend>{t('collectionInstall.tools')}</legend><p>{t('collectionInstall.scope')}</p>
        <div className="collection-install-tools">{tools.map(tool => <label key={tool.id}>
          <input type="checkbox" checked={selectedTools.includes(tool.id)} onChange={event => setSelectedTools(current => event.target.checked ? [...current, tool.id] : current.filter(id => id !== tool.id))} />{tool.label}
        </label>)}</div>
      </fieldset>
      <p className="collection-install-note">{t('collectionInstall.existingNote')}</p>
    </div>} />
})
