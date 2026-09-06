import { useEffect } from 'react'
import type { AutoUpdateConfigDto } from './types'

export function useSkillStatusRefresh(enabled: boolean, read: () => Promise<AutoUpdateConfigDto>, receive: (config: AutoUpdateConfigDto) => void, refresh: () => Promise<void>) {
  useEffect(() => {
    if (!enabled) return
    let cancelled = false
    let inFlight = false
    let lastSignature = ''
    const poll = async (force = false) => {
      if (cancelled || inFlight) return
      inFlight = true
      try {
        const config = await read()
        if (cancelled) return
        receive(config)
        const signature = JSON.stringify([config.last_run_at, config.last_status, config.progress])
        if (force || signature !== lastSignature) {
          await refresh()
          lastSignature = signature
        }
      } catch { /* Keep the previous state on a transient read failure. */ }
      finally { inFlight = false }
    }
    const focus = () => { void poll(true) }
    void poll()
    const timer = window.setInterval(() => { void poll() }, 5000)
    window.addEventListener('focus', focus)
    return () => { cancelled = true; window.clearInterval(timer); window.removeEventListener('focus', focus) }
  }, [enabled, read, receive, refresh])
}
