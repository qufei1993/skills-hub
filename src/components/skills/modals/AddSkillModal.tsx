import { memo, useId } from 'react'
import { FolderOpen, GitBranch, Info, X } from 'lucide-react'
import type { TFunction } from 'i18next'
import { useInstallDialog } from './useInstallDialog'

type AddSkillModalProps = {
  open: boolean
  loading: boolean
  canClose: boolean
  addModalTab: 'local' | 'git'
  localPath: string
  gitUrl: string
  onRequestClose: () => void
  onTabChange: (tab: 'local' | 'git') => void
  onLocalPathChange: (value: string) => void
  onPickLocalPath: () => void
  onGitUrlChange: (value: string) => void
  onCancelOperation?: () => void
  onSubmit: () => void
  t: TFunction
}

const AddSkillModal = ({ open, loading, canClose, addModalTab, localPath, gitUrl,
  onRequestClose, onTabChange, onLocalPathChange, onPickLocalPath, onGitUrlChange, onCancelOperation, onSubmit, t }: AddSkillModalProps) => {
  const sourceId = useId()
  const dialogRef = useInstallDialog(open, !canClose, onRequestClose)
  if (!open) return null
  const local = addModalTab === 'local'
  return <div className="modal-backdrop install-flow-backdrop" onClick={canClose ? onRequestClose : undefined}>
    <div ref={dialogRef} className="modal install-flow-dialog install-source-dialog" role="dialog" aria-modal="true" aria-label={t('addSkillTitle')} tabIndex={-1} onClick={event => event.stopPropagation()}>
      <header className="modal-header">
        <div className="install-source-heading"><span className="install-source-icon" aria-hidden="true">{local ? <FolderOpen size={22} /> : <GitBranch size={22} />}</span><div><h2>{t('addSkillTitle')}</h2><p>{t('installFlow.sourceStep')}</p></div></div>
        <button type="button" className="modal-close" aria-label={t('close')} disabled={!canClose} onClick={onRequestClose}><X size={18} /></button>
      </header>
      <form className="install-source-form" onSubmit={event => { event.preventDefault(); if (!loading && (local ? localPath : gitUrl).trim()) onSubmit() }}>
        <div className="install-source-tabs" aria-label={t('installFlow.source')}>
          <button type="button" aria-pressed={!local} disabled={loading} onClick={() => onTabChange('git')}><GitBranch size={17} />{t('discovery.git')}</button>
          <button type="button" aria-pressed={local} disabled={loading} onClick={() => onTabChange('local')}><FolderOpen size={17} />{t('discovery.local')}</button>
        </div>
        <div className="install-source-field">
          <label htmlFor={sourceId}>{t(local ? 'localFolder' : 'repositoryUrl')}</label>
          <span className="input-row">
            <input id={sourceId} className="input" disabled={loading} placeholder={t(local ? 'localPathPlaceholder' : 'gitUrlPlaceholder')} value={local ? localPath : gitUrl} onChange={event => local ? onLocalPathChange(event.target.value) : onGitUrlChange(event.target.value)} />
            {local && <button className="btn btn-secondary" type="button" disabled={loading} onClick={onPickLocalPath}>{t('browse')}</button>}
          </span>
        </div>
        <p className="install-source-help"><Info size={15} aria-hidden="true" /><span>{t(loading ? 'installFlow.detecting' : 'installFlow.detectHint')}</span></p>
        <footer className="modal-footer">
          <button type="button" className="btn btn-secondary" disabled={loading && !onCancelOperation} onClick={loading ? onCancelOperation : onRequestClose}>{t('cancel')}</button>
          <button type="submit" className="btn btn-primary" disabled={loading || !(local ? localPath : gitUrl).trim()}>{t('installFlow.detect')}</button>
        </footer>
      </form>
    </div>
  </div>
}
export default memo(AddSkillModal)
