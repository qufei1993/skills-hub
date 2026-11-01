import { memo } from 'react'
import ConfirmActionModal from './ConfirmActionModal'
import type { TFunction } from 'i18next'

type BulkDeleteModalProps = {
  open: boolean
  loading: boolean
  skillNames: string[]
  missingCentralCount?: number
  onRequestClose: () => void
  onConfirm: () => void
  t: TFunction
}

const BulkDeleteModal = ({
  open,
  loading,
  skillNames,
  missingCentralCount = 0,
  onRequestClose,
  onConfirm,
  t,
}: BulkDeleteModalProps) => {
  return <ConfirmActionModal
    open={open} loading={loading} recoverable={missingCentralCount < skillNames.length}
    title={t('bulk.deleteTitle', { count: skillNames.length })} cancelLabel={t('cancel')} confirmLabel={t('bulk.deleteConfirm', { count: skillNames.length })}
    onRequestClose={onRequestClose} onConfirm={onConfirm}
    body={<>
      <p>{t(missingCentralCount === skillNames.length ? 'delete.warningMissingContent' : 'bulk.deleteBody')}</p><div className="bulk-delete-list">{skillNames.slice(0, 6).map((name, index) => <span key={`${name}-${index}`}>{name}</span>)}{skillNames.length > 6 ? <span>{t('bulk.moreSelected', { count: skillNames.length - 6 })}</span> : null}</div>
      {missingCentralCount < skillNames.length ? <div className="delete-warning"><ul>
        {(missingCentralCount === skillNames.length ? [] : ['warningRecycle', 'warningRemoveFromTools', 'warningKeepSource']).map((key) => <li key={key}>{t(`delete.${key}`)}</li>)}
      {missingCentralCount > 0 && missingCentralCount < skillNames.length ? <li>{t('delete.warningMissingContent')}</li> : null}
      </ul></div> : null}
    </>}
  />
}

export default memo(BulkDeleteModal)
