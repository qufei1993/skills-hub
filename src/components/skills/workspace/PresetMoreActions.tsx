import { useEffect, useId, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import { Copy, Ellipsis } from 'lucide-react'
import type { TFunction } from 'i18next'

type Props = { name: string; busy: boolean; onCopy: () => Promise<void>; t: TFunction }

export default function PresetMoreActions({ name, busy, onCopy, t }: Props) {
  const [position, setPosition] = useState<{ left: number; top: number; width: number } | null>(null)
  const trigger = useRef<HTMLButtonElement>(null)
  const popup = useRef<HTMLDivElement>(null)
  const action = useRef<HTMLButtonElement>(null)
  const restoreAfterCopy = useRef(false)
  const id = useId()
  const label = t('workspace110.moreNamed', { name })
  const close = () => { setPosition(null); trigger.current?.focus({ preventScroll: true }) }
  const open = () => {
    const rect = trigger.current!.getBoundingClientRect()
    const width = Math.min(192, window.innerWidth - 24)
    const top = rect.bottom + 56 > window.innerHeight - 12 ? Math.max(12, rect.top - 56) : rect.bottom + 6
    setPosition({ left: Math.max(12, Math.min(rect.right - width, window.innerWidth - width - 12)), top, width })
  }
  useEffect(() => {
    if (!busy && restoreAfterCopy.current) {
      restoreAfterCopy.current = false
      trigger.current?.focus({ preventScroll: true })
    }
  }, [busy])
  useEffect(() => {
    if (!position) return
    action.current?.focus()
    const dismiss = (event: PointerEvent | FocusEvent) => {
      if (event.target instanceof Node && !popup.current?.contains(event.target) && !trigger.current?.contains(event.target)) setPosition(null)
    }
    const reposition = (event: Event) => {
      if (event.target instanceof Node && popup.current?.contains(event.target)) return
      if (popup.current?.contains(document.activeElement)) trigger.current?.focus({ preventScroll: true })
      setPosition(null)
    }
    document.addEventListener('pointerdown', dismiss)
    document.addEventListener('focusin', dismiss)
    window.addEventListener('resize', reposition)
    window.addEventListener('scroll', reposition, true)
    return () => {
      document.removeEventListener('pointerdown', dismiss)
      document.removeEventListener('focusin', dismiss)
      window.removeEventListener('resize', reposition)
      window.removeEventListener('scroll', reposition, true)
    }
  }, [position])
  return <>
    <button ref={trigger} type="button" className="btn workspace-icon-action" disabled={busy} aria-label={label} title={t('workspace110.more')}
      aria-haspopup="menu" aria-expanded={Boolean(position)} aria-controls={position ? id : undefined}
      onClick={() => position ? close() : open()}
      onKeyDown={event => { if (event.key === 'ArrowDown' || event.key === 'ArrowUp') { event.preventDefault(); open() } }}>
      <Ellipsis size={16} aria-hidden="true" />
    </button>
    {position ? createPortal(<div ref={popup} id={id} className="workspace-preset-menu" role="menu" aria-label={label} style={position}
      onKeyDown={event => {
        if (event.key === 'Escape') { event.preventDefault(); event.stopPropagation(); close() }
        if (event.key === 'Tab') close()
        if (['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key)) { event.preventDefault(); action.current?.focus() }
      }}>
      <button ref={action} type="button" role="menuitem" disabled={busy} onClick={() => {
        restoreAfterCopy.current = true
        setPosition(null)
        void onCopy()
      }}><Copy size={15} aria-hidden="true" />{t('workspace110.copy')}</button>
    </div>, document.body) : null}
  </>
}
