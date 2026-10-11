import { memo, useId, type ReactNode } from 'react'
import type { TFunction } from 'i18next'

type Props = {
  tags: ReactNode
  tools: ReactNode
  scope: ReactNode
  toolSummary: string
  disabled: boolean
  t: TFunction
}

export default memo(function InstallSettingsSections({ tags, tools, scope, toolSummary, disabled, t }: Props) {
  const id = useId()
  return <div className="install-settings-sections install-settings-visible">
    <section className="install-settings-row install-settings-tools" aria-labelledby={`${id}-tools`}>
      <div className="install-settings-label"><strong id={`${id}-tools`}>{t('installToTools')}</strong></div>
      <div className="install-settings-content">
        <fieldset disabled={disabled}>{tools}</fieldset>
        <p className="install-settings-summary" aria-live="polite">{toolSummary}</p>
      </div>
    </section>
    <section className="install-settings-row install-settings-scope" aria-label={t('installScope.title')}>
      <div className="install-settings-content"><fieldset disabled={disabled}>{scope}</fieldset></div>
    </section>
    <section className="install-settings-row install-settings-tags" aria-labelledby={`${id}-tags`}>
      <strong className="install-settings-label" id={`${id}-tags`}>{t('addTags')}</strong>
      <div className="install-settings-content">
        <div className="install-tag-options"><fieldset disabled={disabled}>{tags}</fieldset></div>
      </div>
    </section>
  </div>
})
