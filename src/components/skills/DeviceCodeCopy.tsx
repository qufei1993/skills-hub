import type { TFunction } from 'i18next'
import { Check, Copy } from 'lucide-react'
import { memo, useEffect, useRef, useState } from 'react'

function DeviceCodeCopy({ code, t }: { code: string; t: TFunction }) {
  const [state, setState] = useState<'idle' | 'pending' | 'copied' | 'failed'>('idle')
  const generation = useRef(0)
  useEffect(() => () => { generation.current += 1 }, [])
  useEffect(() => {
    if (state !== 'copied') return
    const timer = setTimeout(() => setState('idle'), 2000)
    return () => clearTimeout(timer)
  }, [state])
  const copy = async () => {
    const attempt = ++generation.current
    setState('pending')
    try {
      await navigator.clipboard.writeText(code)
      if (generation.current === attempt) setState('copied')
    } catch {
      if (generation.current === attempt) setState('failed')
    }
  }
  return <>
    <button className="device-sync-code" type="button" disabled={state === 'pending'} onClick={() => void copy()}>
      <span>{code}</span>{state === 'copied' ? <Check size={14} /> : <Copy size={14} />}
      <span aria-live="polite">{t(state === 'copied' ? 'copied' : 'deviceSync.copyCode')}</span>
    </button>
    {state === 'failed' ? <span role="alert">{t('copyFailed')}</span> : null}
  </>
}
export default memo(DeviceCodeCopy)
