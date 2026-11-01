import { useEffect, useId, useRef, type ReactNode, type RefObject } from 'react'
import { createPortal } from 'react-dom'
import { X } from 'lucide-react'

type Props = { title: string; closeLabel: string; busy: boolean; onClose: () => void; children: ReactNode; footer: ReactNode; returnFocusRef?: RefObject<HTMLElement | null> }

export default function WorkspaceDialog({ title, closeLabel, busy, onClose, children, footer, returnFocusRef }: Props) {
  const titleId = useId()
  const ref = useRef<HTMLDivElement>(null)
  const state = useRef({ busy, onClose })
  useEffect(() => { state.current = { busy, onClose } }, [busy, onClose])
  useEffect(() => {
    // WebKit can blur the trigger when its page becomes inert before this effect runs.
    const previous = returnFocusRef?.current ?? document.activeElement
    const dialog = ref.current
    const elements = () => Array.from(dialog?.querySelectorAll<HTMLElement>('button, input, select, textarea, summary, [tabindex="0"]') ?? []).filter(element => {
      if (element.matches(':disabled') || element.closest('[hidden]')) return false
      // Closed disclosures must not leak hidden tool choices into the focus loop.
      let parent = element.parentElement
      while (parent && parent !== dialog) {
        if (parent.tagName === 'DETAILS' && !parent.hasAttribute('open') && element !== parent.querySelector(':scope > summary')) return false
        parent = parent.parentElement
      }
      return true
    })
    dialog?.focus({ preventScroll: true })
    const keydown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') { event.preventDefault(); if (!state.current.busy) state.current.onClose(); return }
      if (event.key !== 'Tab') return
      const list = elements(), first = list[0], last = list.at(-1)
      if (!first || !last) { event.preventDefault(); dialog?.focus(); return }
      if (event.shiftKey && (document.activeElement === first || !list.includes(document.activeElement as HTMLElement))) { event.preventDefault(); last.focus() }
      else if (!event.shiftKey && (document.activeElement === last || !list.includes(document.activeElement as HTMLElement))) { event.preventDefault(); first.focus() }
    }
    document.addEventListener('keydown', keydown)
    return () => {
      document.removeEventListener('keydown', keydown)
      if (previous instanceof HTMLElement && previous.isConnected) previous.focus({ preventScroll: true })
    }
  }, [returnFocusRef])
  return createPortal(<div className="modal-backdrop workspace-overlay" onClick={() => { if (!busy) onClose() }}>
    <div ref={ref} className="workspace-page workspace-dialog" role="dialog" aria-modal="true" aria-labelledby={titleId} aria-busy={busy} tabIndex={-1} onClick={event => event.stopPropagation()}>
      <header className="workspace-dialog-heading"><h2 id={titleId}>{title}</h2><button className="btn btn-secondary workspace-icon-button" aria-label={closeLabel} disabled={busy} onClick={onClose}><X size={18} /></button></header>
      <div className="workspace-dialog-body">{children}</div>
      <footer className="workspace-footer">{footer}</footer>
    </div>
  </div>, document.body)
}
