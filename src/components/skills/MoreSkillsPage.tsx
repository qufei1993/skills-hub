import { memo } from 'react'
import { ExternalLink, Plus } from 'lucide-react'
import type { TFunction } from 'i18next'
import { SKILL_SOURCE_SITES } from '../../skillSourceSites'

type MoreSkillsPageProps = {
  loading: boolean
  onOpenManualAdd: () => void
  onOpenSite: (url: string) => void
  t: TFunction
}

const MoreSkillsPage = ({
  loading,
  onOpenManualAdd,
  onOpenSite,
  t,
}: MoreSkillsPageProps) => {
  return (
    <div className="more-skills-page">
      <div className="more-skills-hero">
        <div className="more-skills-hero-top">
          <div>
            <h1 className="more-skills-title">{t('moreSkillsTitle')}</h1>
            <p className="more-skills-intro">{t('moreSkillsIntro')}</p>
          </div>
          <button
            className="btn btn-secondary more-skills-manual-btn"
            type="button"
            onClick={onOpenManualAdd}
            disabled={loading}
          >
            <Plus size={15} />
            {t('manualAdd')}
          </button>
        </div>
        <p className="more-skills-footer-note">{t('moreSkillsFooter')}</p>
      </div>

      <div className="more-skills-scroll">
        <div className="more-skills-grid">
          {SKILL_SOURCE_SITES.map((site) => (
            <div key={site.id} className="more-skills-card">
              <div className="more-skills-card-top">
                <div className="more-skills-card-info">
                  <div className="more-skills-card-name">{t(site.nameKey)}</div>
                  <div className="more-skills-card-url">{site.url.replace(/^https:\/\//, '')}</div>
                </div>
                <button
                  className="more-skills-visit-btn"
                  type="button"
                  onClick={() => onOpenSite(site.url)}
                >
                  <ExternalLink size={14} />
                  {t('moreSkillsVisit')}
                </button>
              </div>
              <p className="more-skills-card-desc">{t(site.descriptionKey)}</p>
            </div>
          ))}
        </div>
      </div>
    </div>
  )
}

export default memo(MoreSkillsPage)
