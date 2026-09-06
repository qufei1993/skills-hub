import { memo, useEffect, useId, useRef, type ReactNode } from 'react'
import { TriangleAlert } from 'lucide-react'

type ConfirmActionModalProps = {
  open: boolean
  loading: boolean
  title: string
  body: ReactNode
  cancelLabel: string
  confirmLabel: string
  onRequestClose: () => void
  onConfirm: () => void
}

const ConfirmActionModal = ({
  open,
  loading,
  title,
  body,
  cancelLabel,
  confirmLabel,
  onRequestClose,
  onConfirm,
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
    const dialog = dialogRef.current
    const focusableSelector =
      'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])'
    const focusableElements = () =>
      Array.from(dialog?.querySelectorAll<HTMLElement>(focusableSelector) ?? [])

    focusableElements()[0]?.focus()
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
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault()
        last.focus()
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault()
        first.focus()
      }
    }
    document.addEventListener('keydown', handleKeyDown)
    return () => {
      document.removeEventListener('keydown', handleKeyDown)
      if (previouslyFocused instanceof HTMLElement) previouslyFocused.focus()
    }
  }, [open])

  if (!open) return null

  return (
    <div
      className="modal-backdrop"
      onClick={loading ? undefined : onRequestClose}
    >
      <div
        ref={dialogRef}
        className="modal modal-delete"
        tabIndex={-1}
        onClick={(event) => event.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        aria-describedby={descriptionId}
      >
        <div className="modal-body delete-body">
          <div className="delete-title" id={titleId}>
            <TriangleAlert size={20} />
            {title}
          </div>
          <div className="delete-desc" id={descriptionId}>
            {body}
          </div>
        </div>
        <div className="modal-footer space-between">
          <button
            className="btn btn-secondary"
            type="button"
            onClick={onRequestClose}
            disabled={loading}
          >
            {cancelLabel}
          </button>
          <button
            className="btn btn-danger-solid"
            type="button"
            onClick={onConfirm}
            disabled={loading}
          >
            {confirmLabel}
          </button>
        </div>
      </div>
    </div>
  )
}

export default memo(ConfirmActionModal)
