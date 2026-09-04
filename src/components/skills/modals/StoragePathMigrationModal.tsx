import { memo } from 'react'
import { FolderSync, TriangleAlert } from 'lucide-react'
import type { TFunction } from 'i18next'
import type { StoragePathChangePreview } from '../types'

type StoragePathMigrationModalProps = {
  preview: StoragePathChangePreview | null
  loading: boolean
  onRequestClose: () => void
  onConfirm: () => void
  t: TFunction
}

const StoragePathMigrationModal = ({
  preview,
  loading,
  onRequestClose,
  onConfirm,
  t,
}: StoragePathMigrationModalProps) => {
  if (!preview) return null

  return (
    <div className="modal-backdrop" onClick={loading ? undefined : onRequestClose}>
      <div
        className="modal storage-migration-modal"
        onClick={(event) => event.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-labelledby="storage-migration-title"
      >
        <div className="modal-header">
          <div className="modal-title" id="storage-migration-title">
            <FolderSync size={20} />
            {t('storageMigration.title')}
          </div>
        </div>
        <div className="modal-body storage-migration-body">
          <p>{t('storageMigration.description', { count: preview.skill_count })}</p>
          <div className="storage-migration-paths">
            <div>
              <span>{t('storageMigration.currentPath')}</span>
              <code>{preview.current_path}</code>
            </div>
            <div>
              <span>{t('storageMigration.newPath')}</span>
              <code>{preview.new_path}</code>
            </div>
          </div>
          <div className="storage-migration-notice">
            <TriangleAlert size={18} />
            <div>
              <strong>{t('storageMigration.noticeTitle')}</strong>
              <p>{t('storageMigration.noticeBody')}</p>
            </div>
          </div>
        </div>
        <div className="modal-footer space-between">
          <button
            className="btn btn-secondary"
            type="button"
            onClick={onRequestClose}
            disabled={loading}
          >
            {t('cancel')}
          </button>
          <button
            className="btn btn-primary"
            type="button"
            onClick={onConfirm}
            disabled={loading}
          >
            {loading ? t('storageMigration.migrating') : t('storageMigration.confirm')}
          </button>
        </div>
      </div>
    </div>
  )
}

export default memo(StoragePathMigrationModal)
