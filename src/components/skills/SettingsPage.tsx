import { memo, useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { ArrowLeft, Database, ExternalLink, Github, Palette, Radar, RefreshCw } from 'lucide-react'
import { openUrl } from '@tauri-apps/plugin-opener'
import type { TFunction } from 'i18next'
import type { DownloadOptions, Update } from '@tauri-apps/plugin-updater'
import { toast } from 'sonner'
import type { GithubProxyConfigDto } from './types'
import ConfirmActionModal from './modals/ConfirmActionModal'

const PROJECT_REPOSITORY_URL = 'https://github.com/qufei1993/skills-hub'

type UpdateStatus = 'idle' | 'checking' | 'up-to-date' | 'available' | 'downloading' | 'done' | 'error'
type UpdaterProxyOptions = { proxy?: string }
type UpdaterDownloadOptions = DownloadOptions & UpdaterProxyOptions

const buildUpdaterProxyOptions = (
  enabled: boolean,
  url: string,
): UpdaterProxyOptions | undefined => {
  const proxy = enabled ? url.trim() : ''
  return proxy ? { proxy } : undefined
}

type SettingsPageProps = {
  isTauri: boolean
  language: string
  storagePath: string
  gitCacheCleanupDays: number
  gitCacheTtlSecs: number
  themePreference: 'system' | 'light' | 'dark'
  githubTokenDraft: string
  githubTokenConfigured: boolean
  githubProxyConfig: GithubProxyConfigDto
  discoveryScanEnabledCount: number
  discoveryScanSourceCount: number
  onPickStoragePath: () => void
  onLanguageChange: (nextLanguage: string) => void
  onThemeChange: (nextTheme: 'system' | 'light' | 'dark') => void
  onGitCacheCleanupDaysChange: (nextDays: number) => void
  onGitCacheTtlSecsChange: (nextSecs: number) => void
  onClearGitCacheNow: () => Promise<boolean>
  onGithubTokenDraftChange: (token: string) => void
  onGithubTokenSave: () => void
  onGithubTokenRemove: () => void
  onGithubProxyConfigChange: (enabled: boolean, port: number) => void
  onOpenDiscoveryScanSettings: () => void
  onBack: () => void
  t: TFunction
}

const SettingsPage = ({
  isTauri,
  language,
  storagePath,
  gitCacheCleanupDays,
  gitCacheTtlSecs,
  themePreference,
  onPickStoragePath,
  onLanguageChange,
  onThemeChange,
  onGitCacheCleanupDaysChange,
  onGitCacheTtlSecsChange,
  onClearGitCacheNow,
  githubTokenDraft,
  githubTokenConfigured,
  onGithubTokenDraftChange,
  onGithubTokenSave,
  onGithubTokenRemove,
  githubProxyConfig,
  onGithubProxyConfigChange,
  discoveryScanEnabledCount,
  discoveryScanSourceCount,
  onOpenDiscoveryScanSettings,
  onBack,
  t,
}: SettingsPageProps) => {
  const [confirmingClearGitCache, setConfirmingClearGitCache] = useState(false)
  const [clearingGitCache, setClearingGitCache] = useState(false)
  const [localGithubProxyPort, setLocalGithubProxyPort] = useState(
    String(githubProxyConfig.port),
  )
  useEffect(() => {
    setLocalGithubProxyPort(String(githubProxyConfig.port))
  }, [githubProxyConfig.port])

  const [updateStatus, setUpdateStatus] = useState<UpdateStatus>('idle')
  const [updateVersion, setUpdateVersion] = useState<string | null>(null)
  const [updateError, setUpdateError] = useState<string | null>(null)
  const updateRef = useRef<Update | null>(null)
  const updaterProxyOptions = useMemo(
    () => buildUpdaterProxyOptions(githubProxyConfig.enabled, githubProxyConfig.url),
    [githubProxyConfig.enabled, githubProxyConfig.url],
  )

  const handleConfirmClearGitCache = async () => {
    if (clearingGitCache) return
    setClearingGitCache(true)
    try {
      const cleared = await onClearGitCacheNow()
      if (cleared) setConfirmingClearGitCache(false)
    } finally {
      setClearingGitCache(false)
    }
  }

  const handleCheckUpdate = useCallback(async () => {
    if (!isTauri) return
    setUpdateStatus('checking')
    setUpdateError(null)
    try {
      const { check } = await import('@tauri-apps/plugin-updater')
      const update = await check(updaterProxyOptions)
      if (update) {
        updateRef.current = update
        setUpdateVersion(update.version)
        setUpdateStatus('available')
      } else {
        setUpdateStatus('up-to-date')
      }
    } catch (err) {
      setUpdateError(err instanceof Error ? err.message : String(err))
      setUpdateStatus('error')
    }
  }, [isTauri, updaterProxyOptions])

  const handleInstallUpdate = useCallback(async () => {
    const update = updateRef.current
    if (!update) return
    setUpdateStatus('downloading')
    setUpdateError(null)
    try {
      await update.downloadAndInstall(
        undefined,
        updaterProxyOptions as UpdaterDownloadOptions | undefined,
      )
      setUpdateStatus('done')
    } catch (err) {
      setUpdateError(err instanceof Error ? err.message : String(err))
      setUpdateStatus('error')
    }
  }, [updaterProxyOptions])

  const [appVersion, setAppVersion] = useState<string | null>(null)
  const versionText = useMemo(() => {
    if (!isTauri) return t('notAvailable')
    if (!appVersion) return t('unknown')
    return `v${appVersion}`
  }, [appVersion, isTauri, t])

  const loadAppVersion = useCallback(async () => {
    if (!isTauri) {
      setAppVersion(null)
      return
    }
    try {
      const { getVersion } = await import('@tauri-apps/api/app')
      const v = await getVersion()
      setAppVersion(v)
    } catch {
      setAppVersion(null)
    }
  }, [isTauri])

  useEffect(() => {
    void loadAppVersion()
    return () => { updateRef.current = null }
  }, [loadAppVersion])

  const handleOpenProject = useCallback(async () => {
    try {
      if (isTauri) {
        await openUrl(PROJECT_REPOSITORY_URL)
      } else {
        window.open(PROJECT_REPOSITORY_URL, '_blank', 'noopener,noreferrer')
      }
    } catch {
      toast.error(t('projectLink.openFailed'))
    }
  }, [isTauri, t])

  return (
    <div className="settings-page">
      <div className="settings-shell">
        <div className="settings-hero">
          <div className="settings-hero-main">
            <div className="settings-title-row">
              <button className="detail-back-btn settings-back" type="button" onClick={onBack}>
                <ArrowLeft size={16} />
                {t('detail.back')}
              </button>
              <div className="settings-title-copy">
                <h1>{t('settings')}</h1>
                <p>{t('settingsPageSubtitle')}</p>
              </div>
            </div>
          </div>
          <div className="settings-hero-summary" aria-label={t('settingsSummary')}>
            <div className="settings-summary-item">
              <span>{t('interfaceLanguage')}</span>
              <strong>{t(`languageOptions.${language}`)}</strong>
            </div>
            <div className="settings-summary-item">
              <span>{t('themeMode')}</span>
              <strong>{t(`themeOptions.${themePreference}`)}</strong>
            </div>
            <div className="settings-summary-item">
              <span>{t('appVersion')}</span>
              <strong>{versionText}</strong>
            </div>
          </div>
        </div>

        <div className="settings-grid">
          <div className="settings-column">
            <section className="settings-card">
              <div className="settings-card-head">
                <span className="settings-card-icon">
                  <Palette size={18} />
                </span>
                <div>
                  <h2>{t('settingsSectionAppearance')}</h2>
                  <p>{t('settingsSectionAppearanceDesc')}</p>
                </div>
              </div>
              <div className="settings-card-body">
                <div className="settings-field">
                  <label className="settings-label" htmlFor="settings-language">
                    {t('interfaceLanguage')}
                  </label>
                  <div className="settings-select-wrap">
                    <select
                      id="settings-language"
                      className="settings-select"
                      value={language}
                      onChange={(event) => {
                        if (event.target.value !== language) {
                          onLanguageChange(event.target.value)
                        }
                      }}
                    >
                      <option value="en">{t('languageOptions.en')}</option>
                      <option value="zh">{t('languageOptions.zh')}</option>
                      <option value="ko">{t('languageOptions.ko')}</option>
                    </select>
                    <svg
                      className="settings-select-caret"
                      viewBox="0 0 24 24"
                      fill="none"
                      stroke="currentColor"
                      strokeWidth="2"
                      aria-hidden="true"
                    >
                      <path d="M6 9l6 6 6-6" />
                    </svg>
                  </div>
                </div>

                <div className="settings-field">
                  <label className="settings-label" id="settings-theme-label">
                    {t('themeMode')}
                  </label>
                  <div className="settings-theme-options" role="group" aria-labelledby="settings-theme-label">
                    <button
                      type="button"
                      className={`settings-theme-btn ${
                        themePreference === 'system' ? 'active' : ''
                      }`}
                      aria-pressed={themePreference === 'system'}
                      onClick={() => onThemeChange('system')}
                    >
                      {t('themeOptions.system')}
                    </button>
                    <button
                      type="button"
                      className={`settings-theme-btn ${
                        themePreference === 'light' ? 'active' : ''
                      }`}
                      aria-pressed={themePreference === 'light'}
                      onClick={() => onThemeChange('light')}
                    >
                      {t('themeOptions.light')}
                    </button>
                    <button
                      type="button"
                      className={`settings-theme-btn ${
                        themePreference === 'dark' ? 'active' : ''
                      }`}
                      aria-pressed={themePreference === 'dark'}
                      onClick={() => onThemeChange('dark')}
                    >
                      {t('themeOptions.dark')}
                    </button>
                  </div>
                </div>
              </div>
            </section>

            <section className="settings-card">
              <div className="settings-card-head">
                <span className="settings-card-icon">
                  <Radar size={18} />
                </span>
                <div>
                  <h2>{t('discoveryScan.settingsTitle')}</h2>
                  <p>{t('discoveryScan.settingsDescription')}</p>
                </div>
              </div>
              <div className="settings-card-body">
                <div className="settings-project-row discovery-settings-row">
                  <div className="settings-item-info">
                    <div className="settings-item-title">{t('discoveryScan.sources')}</div>
                    <div className="settings-item-desc">
                      {t('discoveryScan.enabledCount', {
                        enabled: discoveryScanEnabledCount,
                        total: discoveryScanSourceCount,
                      })}
                    </div>
                  </div>
                  <button
                    className="btn btn-secondary btn-sm"
                    type="button"
                    onClick={onOpenDiscoveryScanSettings}
                  >
                    {t('discoveryScan.manage')}
                  </button>
                </div>
              </div>
            </section>

            <section className="settings-card">
            <div className="settings-card-head">
              <span className="settings-card-icon">
                <Database size={18} />
              </span>
              <div>
                <h2>{t('settingsSectionStorage')}</h2>
                <p>{t('settingsSectionStorageDesc')}</p>
              </div>
            </div>
            <div className="settings-card-body">
              <div className="settings-field">
                <label className="settings-label" htmlFor="settings-storage">
                  {t('skillsStoragePath')}
                </label>
                <div className="settings-input-row">
                  <input
                    id="settings-storage"
                    className="settings-input mono"
                    value={storagePath}
                    readOnly
                  />
                  <button
                    className="btn btn-secondary settings-browse"
                    type="button"
                    onClick={onPickStoragePath}
                  >
                    {t('browse')}
                  </button>
                </div>
                <div className="settings-helper">{t('skillsStorageHint')}</div>
              </div>

              <div className="settings-field">
                <label className="settings-label" htmlFor="settings-git-cache-days">
                  {t('gitCacheCleanupDays')}
                </label>
                <div className="settings-input-row">
                  <input
                    id="settings-git-cache-days"
                    className="settings-input"
                    type="number"
                    min={0}
                    max={3650}
                    step={1}
                    value={gitCacheCleanupDays}
                    onChange={(event) => {
                      const next = Number(event.target.value)
                      if (!Number.isNaN(next)) {
                        onGitCacheCleanupDaysChange(next)
                      }
                    }}
                  />
                  <button
                    className="btn btn-secondary settings-browse"
                    type="button"
                    onClick={() => setConfirmingClearGitCache(true)}
                  >
                    {t('cleanNow')}
                  </button>
                </div>
                <div className="settings-helper">{t('gitCacheCleanupHint')}</div>
              </div>

              <div className="settings-field">
                <label className="settings-label" htmlFor="settings-git-cache-ttl">
                  {t('gitCacheTtlSecs')}
                </label>
                <div className="settings-input-row">
                  <input
                    id="settings-git-cache-ttl"
                    className="settings-input"
                    type="number"
                    min={0}
                    max={3600}
                    step={1}
                    value={gitCacheTtlSecs}
                    onChange={(event) => {
                      const next = Number(event.target.value)
                      if (!Number.isNaN(next)) {
                        onGitCacheTtlSecsChange(next)
                      }
                    }}
                  />
                </div>
                <div className="settings-helper">{t('gitCacheTtlHint')}</div>
              </div>
            </div>
            </section>
          </div>

          <div className="settings-column">
            <section className="settings-card">
            <div className="settings-card-head">
              <span className="settings-card-icon">
                <Github size={18} />
              </span>
              <div>
                <h2>{t('settingsSectionNetwork')}</h2>
                <p>{t('settingsSectionNetworkDesc')}</p>
              </div>
            </div>
            <div className="settings-card-body">
              <div className="settings-project-row">
                <div className="settings-item-info">
                  <div className="settings-item-title">{t('projectLink.title')}</div>
                  <div className="settings-item-desc">{t('projectLink.description')}</div>
                </div>
                <button
                  className="btn btn-secondary btn-sm settings-project-link"
                  type="button"
                  onClick={() => void handleOpenProject()}
                  aria-label={t('projectLink.open')}
                >
                  {t('projectLink.view')}
                  <ExternalLink size={14} />
                </button>
              </div>
              <div className="settings-field">
                <label className="settings-label" htmlFor="settings-github-token">
                  {t('githubToken')}
                </label>
                <div className="settings-input-row">
                  <input
                    id="settings-github-token"
                    className="settings-input mono"
                    type="password"
                    placeholder={
                      githubTokenConfigured
                        ? t('githubTokenReplacementPlaceholder')
                        : t('githubTokenPlaceholder')
                    }
                    value={githubTokenDraft}
                    onChange={(e) => onGithubTokenDraftChange(e.target.value)}
                  />
                  <button
                    className="btn btn-secondary btn-sm"
                    type="button"
                    disabled={!githubTokenDraft.trim()}
                    onClick={onGithubTokenSave}
                  >
                    {t('githubTokenSave')}
                  </button>
                  {githubTokenConfigured ? (
                    <button
                      className="btn btn-danger btn-sm"
                      type="button"
                      onClick={() => {
                        if (window.confirm(t('githubTokenRemoveConfirm'))) {
                          onGithubTokenRemove()
                        }
                      }}
                    >
                      {t('githubTokenRemove')}
                    </button>
                  ) : null}
                </div>
                <div className="settings-helper" role="status">
                  {githubTokenConfigured
                    ? t('githubTokenConfigured')
                    : t('githubTokenNotConfigured')}
                </div>
                <div className="settings-helper">{t('githubTokenHint')}</div>
              </div>

              <div className="settings-field">
                <div className="settings-item">
                  <div className="settings-item-info">
                    <div className="settings-item-title">{t('networkProxy')}</div>
                    <div className="settings-item-desc">{t('networkProxyHint')}</div>
                  </div>
                  <button
                    type="button"
                    className={`settings-toggle${githubProxyConfig.enabled ? ' checked' : ''}`}
                    aria-pressed={githubProxyConfig.enabled}
                    onClick={() => {
                      const nextPort = Number(localGithubProxyPort)
                      onGithubProxyConfigChange(
                        !githubProxyConfig.enabled,
                        Number.isNaN(nextPort) ? githubProxyConfig.port : nextPort,
                      )
                    }}
                  >
                    <span className="settings-toggle-knob" />
                  </button>
                </div>
                <label className="settings-label" htmlFor="settings-github-proxy-port">
                  {t('networkProxyPort')}
                </label>
                <div className="settings-input-row">
                  <input
                    id="settings-github-proxy-port"
                    className="settings-input mono"
                    type="number"
                    min={1}
                    max={65535}
                    step={1}
                    value={localGithubProxyPort}
                    onChange={(e) => setLocalGithubProxyPort(e.target.value)}
                    onBlur={() => {
                      const nextPort = Number(localGithubProxyPort)
                      if (
                        githubProxyConfig.enabled &&
                        !Number.isNaN(nextPort) &&
                        nextPort !== githubProxyConfig.port
                      ) {
                        onGithubProxyConfigChange(true, nextPort)
                      }
                    }}
                  />
                </div>
                <div className="settings-helper">
                  {githubProxyConfig.auto_detected
                    ? t('networkProxyAutoDetected')
                    : t('networkProxyPortHint')}
                </div>
              </div>
            </div>
            </section>

            <section className="settings-card">
            <div className="settings-card-head">
              <span className="settings-card-icon">
                <RefreshCw size={18} />
              </span>
              <div>
                <h2>{t('settingsSectionUpdates')}</h2>
                <p>{t('settingsSectionUpdatesDesc')}</p>
              </div>
            </div>
            <div className="settings-card-body">
              <div className="settings-version-row">
                <div>
                  <span className="settings-version-label">{t('appVersion')}</span>
                  <span className="settings-version-text">{versionText}</span>
                </div>
                {isTauri && updateStatus === 'idle' && (
                  <button
                    className="btn btn-secondary btn-sm"
                    type="button"
                    onClick={handleCheckUpdate}
                  >
                    {t('checkForUpdates')}
                  </button>
                )}
                {updateStatus === 'checking' && (
                  <span className="settings-update-status">{t('checkingUpdates')}</span>
                )}
                {updateStatus === 'up-to-date' && (
                  <span className="settings-update-status settings-update-ok">{t('updateNotAvailable')}</span>
                )}
              </div>
              {updateStatus === 'available' && (
                <div className="settings-update-available">
                  <span>{t('updateAvailableWithVersion', { version: updateVersion })}</span>
                  <button
                    className="btn btn-primary btn-sm"
                    type="button"
                    onClick={handleInstallUpdate}
                  >
                    {t('downloadAndInstall')}
                  </button>
                </div>
              )}
              {updateStatus === 'downloading' && (
                <div className="settings-update-status">{t('installingUpdate')}</div>
              )}
              {updateStatus === 'done' && (
                <div className="settings-update-ok">{t('updateInstalledRestart')}</div>
              )}
              {updateStatus === 'error' && (
                <div className="settings-update-error">
                  <span>{updateError}</span>
                  <button
                    className="btn btn-secondary btn-sm"
                    type="button"
                    onClick={handleCheckUpdate}
                  >
                    {t('checkForUpdates')}
                  </button>
                </div>
              )}
              <div className="settings-helper">{t('updateHint')}</div>
            </div>
            </section>
          </div>
        </div>
      </div>
      <ConfirmActionModal
        open={confirmingClearGitCache}
        loading={clearingGitCache}
        title={t('gitCacheConfirm.title')}
        body={t('gitCacheConfirm.body')}
        cancelLabel={t('cancel')}
        confirmLabel={t('gitCacheConfirm.confirm')}
        onRequestClose={() => setConfirmingClearGitCache(false)}
        onConfirm={() => void handleConfirmClearGitCache()}
      />
    </div>
  )
}

export default memo(SettingsPage)
