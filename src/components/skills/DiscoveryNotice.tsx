import { memo, useState } from 'react'
import { ArrowRight, ScanSearch, SlidersHorizontal, X } from 'lucide-react'
import type { TFunction } from 'i18next'

const dismissalKey = 'skills-discovery-notice-dismissed-v1'

type Props = {
  count: number
  loading: boolean
  onReviewImport: () => void
  onOpenScanSettings: () => void
  t: TFunction
}

const DiscoveryNotice = ({ count, loading, onReviewImport, onOpenScanSettings, t }: Props) => {
  const [dismissed, setDismissed] = useState(() => {
    try { return localStorage.getItem(dismissalKey) === 'true' } catch { return false }
  })
  if (dismissed || count <= 0) return null

  return <aside className="library-notice discovery-notice" aria-label={t('discoveredTitle')}>
    <span className="library-notice-icon"><ScanSearch size={18} aria-hidden="true" /></span>
    <div className="library-notice-copy">
      <strong>{t('discoveredTitle')}</strong>
      <p>{t('discoveredCount', { count })}</p>
    </div>
    <div className="library-notice-actions">
      <button className="library-notice-action library-notice-settings" type="button" onClick={onOpenScanSettings} disabled={loading} aria-label={t('discoveryScan.action')} title={t('discoveryScan.action')}><SlidersHorizontal size={16} aria-hidden="true" /></button>
      <button className="library-notice-action" type="button" onClick={onReviewImport} disabled={loading}>{t('reviewImport')}<ArrowRight size={16} aria-hidden="true" /></button>
    </div>
    <button className="library-notice-close" type="button" aria-label={t('discovery.dismissNotice')} onClick={() => {
      setDismissed(true)
      try { localStorage.setItem(dismissalKey, 'true') } catch { /* Keep this visit dismissible even when storage is unavailable. */ }
    }}><X size={18} aria-hidden="true" /></button>
  </aside>
}

export default memo(DiscoveryNotice)
