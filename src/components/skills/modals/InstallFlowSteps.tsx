import { memo, useEffect, useRef } from 'react'
import type { TFunction } from 'i18next'

export default memo(function InstallFlowSteps({ step, t }: { step: 1 | 2; t: TFunction }) {
  const ref = useRef<HTMLOListElement>(null)
  const previous = useRef(step)
  useEffect(() => {
    if (previous.current !== step) ref.current?.focus()
    previous.current = step
  }, [step])
  return <ol ref={ref} tabIndex={-1} className="install-flow-steps" aria-label={t('installFlow.confirmTitle')}>
    <li aria-current={step === 1 ? 'step' : undefined}><span>1</span>{t('installFlow.selectStep')}</li>
    <li aria-current={step === 2 ? 'step' : undefined}><span>2</span>{t('installFlow.settingsStep')}</li>
  </ol>
})
