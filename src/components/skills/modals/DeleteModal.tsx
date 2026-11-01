import { memo } from 'react'
import ConfirmActionModal from './ConfirmActionModal'
import type { TFunction } from 'i18next'

type DeleteModalProps = {
  open: boolean
  loading: boolean
  skillName: string | null
  centralMissing?: boolean
  onRequestClose: () => void
  onConfirm: () => void
  t: TFunction
}

const DeleteModal = ({
  open,
  loading,
  skillName,
  centralMissing = false,
  onRequestClose,
  onConfirm,
  t,
}: DeleteModalProps) => {
  return <ConfirmActionModal
    open={open} loading={loading} recoverable={!centralMissing}
    title={t('deleteTitle')} cancelLabel={t('cancel')} confirmLabel={t('delete.confirmButton')}
    onRequestClose={onRequestClose} onConfirm={onConfirm}
    body={<>
      <p>{skillName ? <>{t('delete.confirmPrefix')}<strong>{skillName}</strong>{t('delete.confirmSuffix')}</> : t('deleteBody')}</p>
      <div className="delete-warning"><ul>
        {(centralMissing ? ['warningMissingContent'] : ['warningRecycle', 'warningRemoveFromTools', 'warningKeepSource']).map((key) => <li key={key}>{t(`delete.${key}`)}</li>)}
      </ul></div>
    </>}
  />
}

export default memo(DeleteModal)
