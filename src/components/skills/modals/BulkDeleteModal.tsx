import { memo } from 'react'
import { TriangleAlert } from 'lucide-react'
import type { TFunction } from 'i18next'

type BulkDeleteModalProps = {
  open: boolean
  loading: boolean
  skillNames: string[]
  onRequestClose: () => void
  onConfirm: () => void
  t: TFunction
}

const BulkDeleteModal = ({
  open,
  loading,
  skillNames,
  onRequestClose,
  onConfirm,
  t,
}: BulkDeleteModalProps) => {
  if (!open) return null

  return (
    <div className="modal-backdrop" onClick={onRequestClose}>
      <div
        className="modal modal-delete bulk-delete-modal"
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
      >
        <div className="modal-body delete-body">
          <div className="delete-title">
            <TriangleAlert size={20} />
            {t('bulk.deleteTitle', { count: skillNames.length })}
          </div>
          <div className="delete-desc">{t('bulk.deleteBody')}</div>
          <div className="bulk-delete-list">
            {skillNames.slice(0, 6).map((name, index) => (
              <span key={`${name}-${index}`}>{name}</span>
            ))}
            {skillNames.length > 6 ? (
              <span>{t('bulk.moreSelected', { count: skillNames.length - 6 })}</span>
            ) : null}
          </div>
          <div className="delete-warning">
            <ul>
              <li>{t('delete.warningRemoveFromTools')}</li>
              <li>{t('delete.warningDeleteFromHub')}</li>
              <li>{t('delete.warningRecycle')}</li>
              <li>{t('delete.warningKeepSource')}</li>
            </ul>
          </div>
        </div>
        <div className="modal-footer space-between">
          <button
            className="btn btn-secondary"
            onClick={onRequestClose}
            disabled={loading}
          >
            {t('cancel')}
          </button>
          <button
            className="btn btn-danger-solid"
            onClick={onConfirm}
            disabled={loading}
          >
            {t('bulk.deleteConfirm', { count: skillNames.length })}
          </button>
        </div>
      </div>
    </div>
  )
}

export default memo(BulkDeleteModal)
