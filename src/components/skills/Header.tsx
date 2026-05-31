import { memo } from 'react'
import { ExternalLink, Layers, Search, Settings, Tag } from 'lucide-react'
import type { TFunction } from 'i18next'
import { EXPLORE_ENABLED } from '../../features'

type HeaderProps = {
  language: string
  loading: boolean
  activeView: 'myskills' | 'explore' | 'moreskills' | 'detail' | 'settings' | 'tags'
  onToggleLanguage: () => void
  onOpenSettings: () => void
  onViewChange: (view: 'myskills' | 'explore' | 'moreskills' | 'tags') => void
  t: TFunction
}

const Header = ({
  language,
  activeView,
  onToggleLanguage,
  onOpenSettings,
  onViewChange,
  t,
}: HeaderProps) => {
  return (
    <header className="skills-header">
      <div className="header-left">
        <div className="brand-area">
          <img className="logo-icon" src="/logo.png" alt="" />
          <div className="brand-text-wrap">
            <div className="brand-text">{t('appName')}</div>
          </div>
        </div>
        <nav className="nav-tabs">
          <button
            className={`nav-tab${activeView === 'myskills' || activeView === 'detail' ? ' active' : ''}`}
            type="button"
            onClick={() => onViewChange('myskills')}
          >
            <Layers size={16} />
            {t('navMySkills')}
          </button>
          {EXPLORE_ENABLED && (
            <button
              className={`nav-tab${activeView === 'explore' ? ' active' : ''}`}
              type="button"
              onClick={() => onViewChange('explore')}
            >
              <Search size={16} />
              {t('navExplore')}
            </button>
          )}
          <button
            className={`nav-tab${activeView === 'tags' ? ' active' : ''}`}
            type="button"
            onClick={() => onViewChange('tags')}
          >
            <Tag size={16} />
            {t('navTags')}
          </button>
          <button
            className={`nav-tab${activeView === 'moreskills' ? ' active' : ''}`}
            type="button"
            onClick={() => onViewChange('moreskills')}
          >
            <ExternalLink size={16} />
            {t('navMoreSkills')}
          </button>
        </nav>
      </div>
      <div className="header-actions">
        <button className="lang-btn" type="button" onClick={onToggleLanguage}>
          {language === 'en' ? t('languageShort.en') : t('languageShort.zh')}
        </button>
        <button className={`icon-btn${activeView === 'settings' ? ' active' : ''}`} type="button" onClick={onOpenSettings}>
          <Settings size={18} />
        </button>
      </div>
    </header>
  )
}

export default memo(Header)
