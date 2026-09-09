import { memo } from 'react'
import { Download } from 'lucide-react'
import type { TFunction } from 'i18next'
import type { PresentationElements } from '../skillProfile'
import PresentationElementsPicker from '../PresentationElementsPicker'

type ExportSkillsModalProps = {
  open: boolean
  loading: boolean
  selectedCount: number
  skillLabel?: string
  presentation: PresentationElements
  onPresentationChange: (next: PresentationElements) => void
  onRequestClose: () => void
  onConfirm: () => void
  t: TFunction
}

const ExportSkillsModal = ({
  open,
  loading,
  selectedCount,
  skillLabel,
  presentation,
  onPresentationChange,
  onRequestClose,
  onConfirm,
  t,
}: ExportSkillsModalProps) => {
  if (!open) return null

  return (
    <div className="modal-backdrop" onClick={loading ? undefined : onRequestClose}>
      <div
        className="modal bulk-sync-modal"
        onClick={(event) => event.stopPropagation()}
        role="dialog"
        aria-modal="true"
      >
        <div className="modal-header">
          <div>
            <div className="modal-title">{t('bulk.exportTitle')}</div>
            <div className="bulk-modal-subtitle">
              {skillLabel
                ? t('bulk.exportSubtitleSingle', { name: skillLabel })
                : t('bulk.exportSubtitle', { count: selectedCount })}
            </div>
          </div>
        </div>
        <div className="modal-body">
          <div className="scope-help">{t('bulk.exportHelp')}</div>
        </div>
        <PresentationElementsPicker
          value={presentation}
          onChange={onPresentationChange}
          disabled={loading}
          title="导出包内要露出的元素"
        />
        <div className="modal-footer">
          <button className="btn btn-ghost" type="button" onClick={onRequestClose} disabled={loading}>
            {t('cancel')}
          </button>
          <button
            className="btn btn-primary"
            type="button"
            onClick={onConfirm}
            disabled={loading || selectedCount === 0}
          >
            <Download size={14} />
            {t('bulk.exportConfirm')}
          </button>
        </div>
      </div>
    </div>
  )
}

export default memo(ExportSkillsModal)
