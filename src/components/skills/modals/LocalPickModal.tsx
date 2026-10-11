import InstallFlowSteps from './InstallFlowSteps'
import { useInstallDialog } from './useInstallDialog'
import { type ReactNode, memo, useMemo, useState } from 'react'
import SkillSelectionList from './SkillSelectionList'
import type { TFunction } from 'i18next'
import type { LocalSkillCandidate } from '../types'

type LocalPickModalProps = {
  actionMessage?: string | null
  onCancelOperation?: () => void
  settings?: ReactNode
  source?: string
  installDisabled?: boolean
  installDisabledReason?: string
  open: boolean
  loading: boolean
  localCandidates: LocalSkillCandidate[]
  localCandidateSelected: Record<string, boolean>
  onRequestClose: () => void
  onCancel: () => void
  onToggleCandidate: (subpath: string, checked: boolean) => void
  onInstall: (subpaths: string[]) => void
  t: TFunction
}

const LocalPickModal = ({
  actionMessage,
  onCancelOperation,
  settings,
  source,
  installDisabled = false,
  installDisabledReason,
  open,
  loading,
  localCandidates,
  localCandidateSelected,
  onRequestClose,
  onCancel,
  onToggleCandidate,
  onInstall,
  t,
}: LocalPickModalProps) => {
  const dialogRef = useInstallDialog(open, loading, onRequestClose)
  const [query, setQuery] = useState('')
  const [step, setStep] = useState<1 | 2>(1)
  const normalizedQuery = query.trim().toLowerCase()
  const filteredCandidates = useMemo(() => {
    if (!normalizedQuery) return localCandidates
    return localCandidates.filter((c) =>
      [c.name, c.description ?? '', c.subpath].some((value) =>
        value.toLowerCase().includes(normalizedQuery),
      ),
    )
  }, [localCandidates, normalizedQuery])
  const selectableCandidates = filteredCandidates.filter((c) => c.valid)
  const selectedCandidates = selectableCandidates.filter(
    (c) => localCandidateSelected[c.subpath],
  )
  const selectedCount = selectedCandidates.length
  const toggleVisibleCandidates = (checked: boolean) => {
    selectableCandidates.forEach((c) => onToggleCandidate(c.subpath, checked))
  }

  if (!open) return null

  const mapReason = (code?: string | null) => {
    if (!code) return t('localSkillInvalid.unknown')
    if (code === 'missing_skill_md') return t('localSkillInvalid.missingSkillMd')
    if (code === 'invalid_frontmatter') return t('localSkillInvalid.invalidFrontmatter')
    if (code === 'missing_name') return t('localSkillInvalid.missingName')
    if (code === 'read_failed') return t('localSkillInvalid.readFailed')
    return t('localSkillInvalid.unknown')
  }

  return (
    <div className="modal-backdrop install-flow-backdrop" onClick={onRequestClose}>
      <div ref={dialogRef} role="dialog" aria-modal="true" aria-label={t(settings ? 'installFlow.confirmTitle' : 'localPickTitle')} tabIndex={-1} className="modal pick-skill-modal install-flow-dialog install-wizard" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <div><h2>{t(settings ? 'installFlow.confirmTitle' : 'localPickTitle')}</h2>{source && <small className="install-source-reference" title={source}>{source}</small>}</div>
          <button
            className="modal-close"
            type="button"
            onClick={onRequestClose}
            aria-label={t('close')}
          >
            ✕
          </button>
        </div>
        {settings && <InstallFlowSteps step={step} t={t} />}
        <div className="modal-body">
          <div className="install-selection-page" hidden={Boolean(settings) && step === 2}>
          {!settings && <p className="label">{t('localPickBody')}</p>}
          <SkillSelectionList query={query} onQueryChange={setQuery} disabled={loading} t={t}
            onToggle={onToggleCandidate} onToggleAll={toggleVisibleCandidates}
            items={filteredCandidates.map(c => ({ id: c.subpath, name: c.name, path: c.subpath, description: c.description,
              selected: Boolean(localCandidateSelected[c.subpath]), selectable: c.valid,
              tone: c.valid ? 'install' : 'conflict', status: c.valid ? t('gitInstall.install') : t('skillSelection.unavailable'),
              note: c.valid ? undefined : t('localPickInvalidReason', { reason: mapReason(c.reason) }),
            }))} />
          </div>
          <div className="install-settings-page" hidden={!settings || step === 1}>{settings}</div>
        </div>
        <div className="modal-footer">
          {!loading && step === 2 && <span className="install-selection-summary">{t('selectedCount', { selected: selectedCount, total: selectableCandidates.length })}</span>}
          {loading && <span className="install-inline-progress" role="status">{actionMessage ?? t('processingTipShort')}</span>}
          {!loading && step === 2 && installDisabled && <span className="install-inline-progress" role="status">{installDisabledReason ?? t('projectSync.projectRequired')}</span>}
          <button className="btn btn-secondary" onClick={loading ? onCancelOperation : settings && step === 2 ? () => setStep(1) : onCancel} disabled={loading && !onCancelOperation}>
            {t(loading ? 'cancel' : settings ? 'installFlow.back' : 'cancel')}
          </button>
          <button
            className="btn btn-primary"
            onClick={() => { if (settings && step === 1) setStep(2); else onInstall(selectedCandidates.map((c) => c.subpath)) }}
            disabled={loading || (step === 2 && installDisabled) || selectedCount === 0}
          >
            {t(settings && step === 1 ? 'installFlow.next' : 'installSelected')}
          </button>
        </div>
      </div>
    </div>
  )
}

export default memo(LocalPickModal)
