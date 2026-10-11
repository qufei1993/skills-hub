import { useEffect, useRef } from 'react'

export function useInstallDialog(open: boolean, busy: boolean, close: () => void) {
  const ref = useRef<HTMLDivElement>(null)
  const state = useRef({ busy, close })
  useEffect(() => { state.current = { busy, close } }, [busy, close])
  useEffect(() => {
    if (!open) return
    const previous = document.activeElement
    const elements = () => Array.from(ref.current?.querySelectorAll<HTMLElement>(
      'button:not(:disabled), input:not(:disabled), [tabindex="0"]',
    ) ?? []).filter(element => !element.closest('[hidden]'))
    const input = ref.current?.querySelector<HTMLInputElement>('input:not(:disabled)')
    ;(input ?? elements()[0] ?? ref.current)?.focus()
    const keydown = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && !state.current.busy) { event.preventDefault(); state.current.close() }
      if (event.key !== 'Tab') return
      const list = elements()
      const first = list[0]
      const last = list[list.length - 1]
      if (!first) { event.preventDefault(); ref.current?.focus(); return }
      if (event.shiftKey && (document.activeElement === first || !list.includes(document.activeElement as HTMLElement))) { event.preventDefault(); last.focus() }
      if (!event.shiftKey && (document.activeElement === last || !list.includes(document.activeElement as HTMLElement))) { event.preventDefault(); first.focus() }
    }
    document.addEventListener('keydown', keydown)
    return () => {
      document.removeEventListener('keydown', keydown)
      if (previous instanceof HTMLElement && previous.isConnected) previous.focus()
    }
  }, [open])
  return ref
}
