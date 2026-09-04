import { memo } from 'react'
import { TriangleAlert } from 'lucide-react'
import type { TFunction } from 'i18next'

type DeleteModalProps = {
  open: boolean
  loading: boolean
  skillName: string | null
  onRequestClose: () => void
  onConfirm: () => void
  t: TFunction
}

const DeleteModal = ({
  open,
  loading,
  skillName,
  onRequestClose,
  onConfirm,
  t,
}: DeleteModalProps) => {
  if (!open) return null

  return (
    <div className="modal-backdrop" onClick={onRequestClose}>
      <div
        className="modal modal-delete"
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
      >
        <div className="modal-body delete-body">
          <div className="delete-title">
            <TriangleAlert size={20} />
            {t('deleteTitle')}
          </div>
          <div className="delete-desc">
            {skillName ? (
              <>
                {t('delete.confirmPrefix')}
                <strong>{skillName}</strong>
                {t('delete.confirmSuffix')}
              </>
            ) : (
              t('deleteBody')
            )}
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
            {t('delete.confirmButton')}
          </button>
        </div>
      </div>
    </div>
  )
}

export default memo(DeleteModal)
