import { memo, useState } from 'react'
import { SlidersHorizontal } from 'lucide-react'
import type { TFunction } from 'i18next'
import {
  buildDiscoverySourceEnabledMap,
  collectDisabledDiscoverySourceKeys,
} from '../discoveryScanSettings'
import type { DiscoveryScanSettingsDto } from '../types'

type DiscoveryScanModalProps = {
  open: boolean
  saving: boolean
  settings: DiscoveryScanSettingsDto | null
  onRequestClose: () => void
  onSave: (disabledSourceKeys: string[], extraSourcePaths: string[]) => void
  onAddExtraPath?: () => Promise<string | null>
  t: TFunction
}

const DiscoveryScanModalContent = ({
  saving,
  settings,
  onRequestClose,
  onSave,
  onAddExtraPath,
  t,
}: Omit<DiscoveryScanModalProps, 'open'>) => {
  const [enabledByKey, setEnabledByKey] = useState<Record<string, boolean>>(() =>
    buildDiscoverySourceEnabledMap(settings),
  )
  const [extraPaths, setExtraPaths] = useState<string[]>(() => settings?.extra_source_paths ?? [])

  const enabledCount = settings?.sources.filter((source) => enabledByKey[source.key]).length ?? 0
  const totalCount = settings?.sources.length ?? 0

  return (
    <div className="modal-backdrop" onClick={onRequestClose}>
      <div
        className="modal discovery-scan-modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby="discovery-scan-title"
        onClick={(event) => event.stopPropagation()}
      >
        <div className="modal-header">
          <div>
            <div className="modal-title" id="discovery-scan-title">
              {t('discoveryScan.title')}
            </div>
            <div className="discovery-scan-description">
              {t('discoveryScan.description')}
            </div>
          </div>
          <button
            className="modal-close"
            type="button"
            onClick={onRequestClose}
            aria-label={t('close')}
            disabled={saving}
          >
            ✕
          </button>
        </div>
        <div className="modal-body discovery-scan-body">
          <div className="discovery-scan-summary">
            <span>{t('discoveryScan.sources')}</span>
            <strong>{t('discoveryScan.enabledCount', { enabled: enabledCount, total: totalCount })}</strong>
          </div>
          <div className="discovery-scan-list">
            {settings?.sources.length ? (
              settings.sources.map((source) => {
                const enabled = Boolean(enabledByKey[source.key])
                const sourceLabel = source.key === 'claude_plugins'
                  ? t('discoveryScan.claudePlugins')
                  : source.label
                return (
                  <div className="discovery-scan-source" key={source.key}>
                    <div className="discovery-scan-source-icon">
                      <SlidersHorizontal size={16} />
                    </div>
                    <div className="discovery-scan-source-copy">
                      <strong>{sourceLabel}</strong>
                      <span className="mono" title={source.path}>{source.path}</span>
                    </div>
                    <button
                      className={`settings-toggle${enabled ? ' checked' : ''}`}
                      type="button"
                      role="switch"
                      aria-checked={enabled}
                      aria-label={t('discoveryScan.toggleSource', { source: sourceLabel })}
                      onClick={() => {
                        setEnabledByKey((current) => ({
                          ...current,
                          [source.key]: !enabled,
                        }))
                      }}
                      disabled={saving}
                    >
                      <span className="settings-toggle-knob" />
                    </button>
                  </div>
                )
              })
            ) : (
              <div className="empty">{t('discoveryScan.empty')}</div>
            )}
          </div>
          <div className="discovery-scan-helper">{t('discoveryScan.validOnly')}</div>
          <div className="discovery-scan-helper">{t('discoveryScan.extraHint')}</div>
          <div className="discovery-scan-extra">
            <button
              className="btn btn-secondary"
              type="button"
              disabled={saving || !onAddExtraPath}
              onClick={async () => {
                if (!onAddExtraPath) return
                const picked = await onAddExtraPath()
                if (picked && !extraPaths.includes(picked)) {
                  setExtraPaths((current) => [...current, picked])
                }
              }}
            >
              {t('discoveryScan.addFolder')}
            </button>
            {extraPaths.length ? (
              <ul className="discovery-scan-extra-list">
                {extraPaths.map((path) => (
                  <li key={path}>
                    <span className="mono" title={path}>{path}</span>
                    <button
                      type="button"
                      className="btn btn-secondary"
                      disabled={saving}
                      onClick={() => setExtraPaths((current) => current.filter((item) => item !== path))}
                    >
                      {t('remove')}
                    </button>
                  </li>
                ))}
              </ul>
            ) : null}
          </div>
        </div>
        <div className="modal-footer">
          <button className="btn btn-secondary" type="button" onClick={onRequestClose} disabled={saving}>
            {t('cancel')}
          </button>
          <button
            className="btn btn-primary"
            type="button"
            disabled={saving || !settings}
            onClick={() => {
              if (settings) {
                onSave(collectDisabledDiscoverySourceKeys(settings, enabledByKey), extraPaths)
              }
            }}
          >
            {saving ? t('discoveryScan.saving') : t('discoveryScan.saveAndRescan')}
          </button>
        </div>
      </div>
    </div>
  )
}

const DiscoveryScanModal = ({ open, ...props }: DiscoveryScanModalProps) => {
  if (!open) return null

  const settingsKey =
    props.settings?.sources.map((source) => `${source.key}:${source.enabled}`).join('|') ?? 'loading'

  return <DiscoveryScanModalContent key={settingsKey} {...props} />
}

export default memo(DiscoveryScanModal)
