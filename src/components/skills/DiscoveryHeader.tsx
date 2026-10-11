import { memo } from 'react'
import { ArrowLeft, FolderOpen, GitBranch } from 'lucide-react'
import type { TFunction } from 'i18next'

type DiscoveryHeaderProps = {
  active?: 'git' | 'local'
  disabled?: boolean
  onBack?: () => void
  onImport: (tab: 'git' | 'local') => void
  t: TFunction
}

const DiscoveryHeader = ({ active, disabled, onBack, onImport, t }: DiscoveryHeaderProps) => (
  <header className="discovery-header">
    <div className="discovery-heading">
      {onBack && <button type="button" className="discovery-back" onClick={onBack} disabled={disabled} aria-label={t('discovery.back')}><ArrowLeft size={18} /></button>}
      <div><h1>{t('addSkills')}</h1><p>{t(active ? 'discovery.importIntro' : 'discovery.intro')}</p></div>
    </div>
    <div className="discovery-import-actions">
      <button type="button" className={`btn btn-secondary${active === 'git' ? ' selected' : ''}`} aria-pressed={active === 'git'} disabled={disabled} onClick={() => onImport('git')}><GitBranch size={15} />{t('discovery.git')}</button>
      <button type="button" className={`btn btn-secondary${active === 'local' ? ' selected' : ''}`} aria-pressed={active === 'local'} disabled={disabled} onClick={() => onImport('local')}><FolderOpen size={15} />{t('discovery.local')}</button>
    </div>
  </header>
)

export default memo(DiscoveryHeader)
