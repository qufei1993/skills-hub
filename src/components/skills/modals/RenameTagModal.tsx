import { memo, useState } from 'react'
import type { TFunction } from 'i18next'
import type { TagWithCountDto } from '../types'

type RenameTagModalProps = {
  open: boolean
  loading: boolean
  tag: TagWithCountDto | null
  onRequestClose: () => void
  onSave: (tagId: number, name: string) => void
  t: TFunction
}

const RenameTagModal = ({
  open,
  loading,
  tag,
  onRequestClose,
  onSave,
  t,
}: RenameTagModalProps) => {
  const [name, setName] = useState(() => tag?.name ?? '')

  if (!open || !tag) return null

  const normalizedName = name.trim()
  const canSave = Boolean(normalizedName && normalizedName !== tag.name)

  return (
    <div
      className="modal-backdrop"
      onClick={loading ? undefined : onRequestClose}
      onKeyDown={(event) => {
        if (event.key === 'Escape' && !loading) onRequestClose()
      }}
    >
      <form
        className="modal tag-rename-modal"
        onClick={(event) => event.stopPropagation()}
        onSubmit={(event) => {
          event.preventDefault()
          if (canSave && !loading) onSave(tag.id, normalizedName)
        }}
        role="dialog"
        aria-modal="true"
        aria-labelledby="rename-tag-title"
      >
        <div className="modal-header">
          <div className="modal-title" id="rename-tag-title">
            {t('renameTagTitle')}
          </div>
          <button
            className="modal-close"
            type="button"
            onClick={onRequestClose}
            disabled={loading}
            aria-label={t('cancel')}
          >
            ×
          </button>
        </div>
        <div className="modal-body tag-rename-body">
          <label htmlFor="rename-tag-name">{t('tagName')}</label>
          <input
            id="rename-tag-name"
            className="search-input"
            value={name}
            onChange={(event) => setName(event.target.value)}
            disabled={loading}
            autoFocus
          />
        </div>
        <div className="modal-footer">
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
            type="submit"
            disabled={loading || !canSave}
          >
            {t('rename')}
          </button>
        </div>
      </form>
    </div>
  )
}

export default memo(RenameTagModal)
