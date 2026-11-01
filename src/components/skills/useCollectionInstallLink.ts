import { useCallback, useEffect, useRef, useState } from 'react'
import { getCurrent, onOpenUrl } from '@tauri-apps/plugin-deep-link'
import { message } from '@tauri-apps/plugin-dialog'
import { toast } from 'sonner'
import { useTranslation } from 'react-i18next'
import type { CollectionManifest } from './types'
import type { CollectionInvoke } from './collectionInstall'

function linkErrorReason(error: unknown) {
  const code = error instanceof Error ? error.message : typeof error === 'string' ? error : ''
  switch (code) {
    case 'COLLECTION_LINK_TOO_LARGE': return 'tooLarge'
    case 'COLLECTION_LINK_TOO_MANY_SKILLS': return 'tooManySkills'
    case 'COLLECTION_LINK_TOO_MANY_SOURCES': return 'tooManySources'
    case 'COLLECTION_LINK_UNSUPPORTED_VERSION': return 'unsupported'
    case 'COLLECTION_LINK_EMPTY': return 'empty'
    case 'COLLECTION_LINK_DUPLICATE': return 'duplicate'
    case 'COLLECTION_LINK_UNSAFE': return 'unsafe'
    case 'COLLECTION_LINK_INVALID': return 'invalid'
    case 'COLLECTION_LINK_UNAVAILABLE': return 'unavailable'
    default: return 'failed'
  }
}

export function useCollectionInstallLink(enabled: boolean, invoke: CollectionInvoke, onReceive?: () => void) {
  const { t } = useTranslation()
  const [manifest, setManifest] = useState<CollectionManifest | null>(null)
  const occupied = useRef(false)
  const lastLink = useRef('')
  const received = useRef(onReceive)
  useEffect(() => { received.current = onReceive }, [onReceive])
  const translate = useRef(t)
  useEffect(() => { translate.current = t }, [t])
  useEffect(() => {
    if (!enabled) return
    let disposed = false
    let stop: (() => void) | undefined
    const explainError = async (error: unknown) => {
      const key = `collectionInstall.linkErrors.${linkErrorReason(error)}`
      const title = translate.current(`${key}.title`)
      const help = translate.current(`${key}.help`)
      try {
        await message(help, { title, kind: 'error', buttons: { ok: translate.current('collectionInstall.close') } })
      } catch {
        toast.error(title, { description: help, duration: Infinity, closeButton: true })
      }
    }
    const receive = async (urls: string[]) => {
      for (const link of urls) {
        if (disposed || lastLink.current === link) continue
        if (occupied.current) { toast.info(translate.current('collectionInstall.busy')); continue }
        occupied.current = true
        try {
          const parsed = await invoke<CollectionManifest>('parse_collection_link', { link })
          if (disposed) { occupied.current = false; return }
          lastLink.current = link
          received.current?.()
          setManifest(parsed)
        } catch (error) {
          try {
            if (!disposed) await explainError(error)
          } finally {
            occupied.current = false
          }
        }
      }
    }
    void (async () => {
      const unlisten = await onOpenUrl(urls => { void receive(urls) })
      if (disposed) { unlisten(); return }
      stop = unlisten
      const urls = await getCurrent()
      if (urls && !disposed) await receive(urls)
    })().catch(async () => { if (!disposed) await explainError('COLLECTION_LINK_UNAVAILABLE') })
    return () => { disposed = true; stop?.() }
  }, [enabled, invoke])
  const dismiss = useCallback(() => { setManifest(null); occupied.current = false; lastLink.current = '' }, [])
  const openManifest = useCallback((value: CollectionManifest) => {
    if (occupied.current) { toast.info(translate.current('collectionInstall.busy')); return }
    occupied.current = true
    received.current?.()
    setManifest(value)
  }, [])
  return { manifest, dismiss, openManifest }
}
