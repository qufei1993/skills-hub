import { memo, useEffect, useId, useRef, type ReactNode, type RefObject } from 'react'
import { PackagePlus, Trash2, TriangleAlert } from 'lucide-react'

type ConfirmActionModalProps = {
  intent?: 'default' | 'danger'
  recoverable?: boolean
  open: boolean
  loading: boolean
  title: string
  body: ReactNode
  footerSummary?: ReactNode
  cancelLabel: string
  confirmLabel: string
  confirmDisabled?: boolean
  returnFocusRef?: RefObject<HTMLElement | null>
  onRequestClose: () => void
  onCancel?: () => void
  onConfirm: () => void
}

const ConfirmActionModal = ({
  open,
  intent = 'danger',
  recoverable = false,
  loading,
  title,
  body,
  footerSummary,
  cancelLabel,
  confirmLabel,
  confirmDisabled = false,
  returnFocusRef,
  onRequestClose,
  onConfirm,
  onCancel,
}: ConfirmActionModalProps) => {
  const titleId = useId()
  const descriptionId = useId()
  const dialogRef = useRef<HTMLDivElement>(null)
  const closeRef = useRef(onRequestClose)
  const loadingRef = useRef(loading)

  useEffect(() => {
    closeRef.current = onRequestClose
    loadingRef.current = loading
  }, [loading, onRequestClose])

  useEffect(() => {
    if (!open) return
    const previouslyFocused = document.activeElement
    const fallbackFocus = returnFocusRef?.current
    const dialog = dialogRef.current
    const focusableSelector =
      'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])'
    const focusableElements = () =>
      Array.from(dialog?.querySelectorAll<HTMLElement>(focusableSelector) ?? []).filter(element => !element.closest('[hidden]') && !element.matches(':disabled'))

    focusableElements()[0]?.focus({ preventScroll: true })
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && !loadingRef.current) {
        event.preventDefault()
        closeRef.current()
        return
      }
      if (event.key !== 'Tab') return
      const elements = focusableElements()
      if (elements.length === 0) {
        event.preventDefault()
        dialog?.focus()
        return
      }
      const first = elements[0]
      const last = elements[elements.length - 1]
      if (event.shiftKey && (document.activeElement === first || !elements.includes(document.activeElement as HTMLElement))) {
        event.preventDefault()
        last.focus()
      } else if (!event.shiftKey && (document.activeElement === last || !elements.includes(document.activeElement as HTMLElement))) {
        event.preventDefault()
        first.focus()
      }
    }
    document.addEventListener('keydown', handleKeyDown)
    return () => {
      document.removeEventListener('keydown', handleKeyDown)
      if (previouslyFocused instanceof HTMLElement && previouslyFocused.isConnected && !previouslyFocused.matches(':disabled')) {
        previouslyFocused.focus()
      } else {
        fallbackFocus?.focus()
      }
    }
  }, [open, returnFocusRef])

  if (!open) return null

  return (
    <div
      className="modal-backdrop"
      onClick={loading ? undefined : onRequestClose}
    >
      <div
        ref={dialogRef}
        className={`modal modal-delete${recoverable ? ' modal-recoverable-delete' : ''}${intent === 'default' ? ' modal-collection-install' : ''}`}
        tabIndex={-1}
        onClick={(event) => event.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        aria-describedby={descriptionId}
      >
        <div className="modal-body delete-body">
          <div className="delete-title" id={titleId}>
            {intent === 'default' ? <PackagePlus size={20} /> : recoverable ? <Trash2 size={20} /> : <TriangleAlert size={20} />}
            {title}
          </div>
          <div className="delete-desc" id={descriptionId}>
            {body}
          </div>
        </div>
        <div className="modal-footer">
          {footerSummary}
          <button
            className="btn btn-secondary"
            type="button"
            onClick={onCancel ?? onRequestClose}
            disabled={loading}
          >
            {cancelLabel}
          </button>
          <button
            className={`btn ${intent === 'default' ? 'btn-primary' : recoverable ? 'btn-danger' : 'btn-danger-solid'}`}
            type="button"
            onClick={onConfirm}
            disabled={loading || confirmDisabled}
          >
            {confirmLabel}
          </button>
        </div>
      </div>
    </div>
  )
}

export default memo(ConfirmActionModal)
