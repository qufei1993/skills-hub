import { useCallback, useEffect, useRef, useState } from 'react'
import { getCurrent, onOpenUrl } from '@tauri-apps/plugin-deep-link'
import { toast } from 'sonner'
import { useTranslation } from 'react-i18next'
import type { CollectionManifest } from './types'
import type { CollectionInvoke } from './collectionInstall'

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
        } catch {
          occupied.current = false
          if (!disposed) toast.error(translate.current('collectionInstall.invalid'))
        }
      }
    }
    void (async () => {
      const unlisten = await onOpenUrl(urls => { void receive(urls) })
      if (disposed) { unlisten(); return }
      stop = unlisten
      const urls = await getCurrent()
      if (urls && !disposed) await receive(urls)
    })().catch(() => { if (!disposed) toast.error(translate.current('collectionInstall.unavailable')) })
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
