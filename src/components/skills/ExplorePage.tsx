import { memo, useEffect, useMemo, useRef, useState } from 'react'
import { ArrowLeft, ArrowRight, Check, ChevronRight, Code2, Download, Film, Layers, Megaphone, Palette, Search, X } from 'lucide-react'
import type { TFunction } from 'i18next'
import { useTranslation } from 'react-i18next'
import type { CollectionManifest, ManagedSkill, OnlineSkillDto } from './types'
import type { CollectionInvoke } from './collectionInstall'
import type { Collection, CollectionIndexItem } from './collections/types'
import { validateDetail, validateIndex } from './collections/validation'
import { loadWebsiteCollection, type CollectionSnapshot } from './collections/load'
import { localizeCollection, localizeCollectionIndex } from './collections/localization'
import { createInstallPlan } from './collections/install-plan'
import { matchesCollection } from './collections/search'
import DiscoveryHeader from './DiscoveryHeader'

type ExplorePageProps = {
  invoke: CollectionInvoke
  onInstallCollection: (manifest: CollectionManifest) => void
  exploreFilter: string
  searchResults: OnlineSkillDto[]
  searchLoading: boolean
  managedSkills: ManagedSkill[]
  loading: boolean
  onExploreFilterChange: (value: string) => void
  onInstallSkill: (sourceUrl: string, skillName?: string) => void
  onOpenManualAdd: (tab?: 'git' | 'local') => void
  t: TFunction
}

const collectionIcon = (category: string) => category === 'design' ? Palette : category === 'development' ? Code2 : category === 'media' ? Film : category === 'marketing' ? Megaphone : Layers
const collectionBatchSize = 12
const suggestions = ['frontend', 'testing', 'video', 'marketing'] as const
const sourceKey = (source: string) => source.replace(/^https?:\/\/github.com\//, '').split('/tree/')[0].replace(/\.git$/, '').toLowerCase()

const ExplorePage = ({ invoke, onInstallCollection, exploreFilter, searchResults, searchLoading, managedSkills, loading, onExploreFilterChange, onInstallSkill, onOpenManualAdd, t }: ExplorePageProps) => {
  const { i18n } = useTranslation()
  const language = (i18n.resolvedLanguage ?? i18n.language ?? 'en').startsWith('zh') ? 'zh' : 'en'
  const [items, setItems] = useState<CollectionIndexItem[]>([])
  const [detail, setDetail] = useState<Collection | null>(null)
  const [pending, setPending] = useState(false)
  const [error, setError] = useState(false)
  const [installError, setInstallError] = useState(false)
  const [openingInstall, setOpeningInstall] = useState(false)
  const [attempt, setAttempt] = useState(0)
  const [requested, setRequested] = useState<CollectionIndexItem | null>(null)
  const indexLoaded = useRef(false)
  const scrollBody = useRef<HTMLDivElement>(null)
  const loadTrigger = useRef<HTMLDivElement>(null)
  const snapshot = useRef<CollectionSnapshot>({})
  const backButton = useRef<HTMLButtonElement>(null)
  const collectionButtons = useRef(new Map<string, HTMLButtonElement>())
  const returnFocusId = useRef<string | null>(null)
  useEffect(() => {
    if (requested) backButton.current?.focus()
  }, [requested])
  useEffect(() => {
    if (!requested && !pending && returnFocusId.current) {
      const button = collectionButtons.current.get(returnFocusId.current)
      if (button) {
        button.focus({ preventScroll: true })
        returnFocusId.current = null
      }
    }
  }, [requested, pending])
  useEffect(() => {
    if (!requested && indexLoaded.current) {
      setPending(false)
      setError(false)
      return
    }
    let cancelled = false
    setPending(true)
    setError(false)
    void (async () => {
      try {
        if (requested) {
          const result = await loadWebsiteCollection(invoke, value => {
            const checked = validateDetail(value)
            if (checked.id !== requested.id || checked.revision !== requested.revision) throw new Error('Collection revision changed')
            return checked
          }, requested.id, snapshot.current)
          if (!cancelled) setDetail(result)
        } else {
          const result = await loadWebsiteCollection(invoke, validateIndex, undefined, snapshot.current)
          if (!cancelled) { indexLoaded.current = true; setItems(result); setDetail(null) }
        }
      } catch { if (!cancelled) setError(true) }
      finally { if (!cancelled) setPending(false) }
    })()
    return () => { cancelled = true }
  }, [invoke, requested, attempt])

  const query = exploreFilter.trim().toLowerCase()
  const [collectionPage, setCollectionPage] = useState({ query, limit: collectionBatchSize })
  if (collectionPage.query !== query) setCollectionPage({ query, limit: collectionBatchSize })
  const visibleLimit = collectionPage.query === query ? collectionPage.limit : collectionBatchSize
  const searching = query.length >= 2
  const collections = useMemo(() => items.filter(item => matchesCollection(item, query)).map(item => localizeCollectionIndex(item, language, t)), [items, query, language, t])
  useEffect(() => {
    const target = loadTrigger.current
    if (requested || pending || error || visibleLimit >= collections.length || !target || typeof IntersectionObserver === 'undefined') return
    let active = true
    const observer = new IntersectionObserver(entries => {
      if (!active || !entries.some(entry => entry.isIntersecting)) return
      active = false
      observer.disconnect()
      setCollectionPage(page => page.query === query
        ? { query, limit: Math.min(page.limit + collectionBatchSize, collections.length) }
        : page)
    }, { root: scrollBody.current, rootMargin: '0px 0px 160px 0px' })
    observer.observe(target)
    return () => { active = false; observer.disconnect() }
  }, [requested, pending, error, query, visibleLimit, collections.length])
  const current = detail ? localizeCollection(detail, language) : null
  const plan = current ? createInstallPlan(current, current.skills.map(skill => skill.id)) : null
  const DetailIcon = collectionIcon(requested?.category ?? 'other')
  const detailRepositories = [...new Set(current?.skills.map(skill => skill.entry.repo) ?? [])]
  const install = async () => {
    if (!plan || plan.conflicts.length || openingInstall) return
    setOpeningInstall(true)
    setInstallError(false)
    try {
      const manifest = await invoke<CollectionManifest>('validate_collection_manifest', { manifest: plan.manifest })
      onInstallCollection(manifest)
    } catch { setInstallError(true) }
    finally { setOpeningInstall(false) }
  }

  return <div className="discovery-page">
    <DiscoveryHeader onImport={onOpenManualAdd} t={t} />
    {requested ? <div className="discovery-body">
      <button type="button" ref={backButton} className="discovery-return" onClick={() => { setPending(false); setError(false); setRequested(null); setDetail(null); setInstallError(false) }}><ArrowLeft size={15} />{t('discovery.back')}</button>
      {pending ? <div className="discovery-feedback" role="status">{t('exploreLoading')}</div> : error ? <div className="discovery-feedback" role="alert"><p>{t('catalog.loadError')}</p><button className="btn btn-secondary" onClick={() => setAttempt(value => value + 1)}>{t('catalog.retry')}</button></div> : current && plan && <article className="discovery-detail" data-category={requested.category}>
        <header className="discovery-detail-header">
          <span className="discovery-detail-symbol" aria-hidden="true"><DetailIcon size={32} strokeWidth={1.5} /></span>
          <div className="discovery-detail-identity"><div className="discovery-byline">{requested.authorLabel}<span>{t('catalog.count', { count: plan.items.length })}</span></div><h2>{current.title}</h2><p>{current.description}</p><div className="discovery-detail-tags">{requested.tags.slice(0, 3).map(tag => <span key={tag}>{tag}</span>)}</div></div>
          <button type="button" className="btn btn-primary" disabled={loading || openingInstall || plan.conflicts.length > 0} onClick={() => void install()}>{t('catalog.chooseInstall', { count: plan.items.length })}<ArrowRight size={15} /></button>
        </header>
        {plan.conflicts.length > 0 && <p role="alert">{t('catalog.conflict')}</p>}
        {installError && <p className="discovery-error" role="alert">{t('discovery.installError')}</p>}
        <p className="discovery-overview">{current.overview}</p>
        <div className="discovery-section-heading"><h2>{t('catalog.included')}</h2><span>{detailRepositories.length === 1 ? <span className="discovery-detail-source"><Code2 size={13} aria-hidden="true" />{detailRepositories[0]}</span> : current.skills.length}</span></div>
        <ul className="discovery-detail-skills">{current.skills.map(skill => <li key={skill.id}><strong>{skill.name}</strong><p>{skill.summary}</p>{detailRepositories.length > 1 && <small>{skill.entry.repo}</small>}</li>)}</ul>
      </article>}
    </div> : <>
      <div className="discovery-search-area">
        <div className="discovery-search"><Search size={20} aria-hidden="true" /><input type="search" aria-label={t('discovery.search')} placeholder={t('discovery.search')} value={exploreFilter} onChange={event => onExploreFilterChange(event.target.value)} />{exploreFilter && <button type="button" onClick={() => onExploreFilterChange('')} aria-label={t('discovery.clear')}><X size={17} /></button>}<span className="discovery-search-provider">skills.sh</span></div>
        <div className="discovery-suggestions"><span>{t('discovery.try')}</span>{suggestions.map(value => <button type="button" key={value} onClick={() => onExploreFilterChange(value)}>{t(`discovery.topics.${value}`)}</button>)}</div>
      </div>
      <div className="discovery-body" ref={scrollBody} key={searching ? 'results' : 'browse'}>
        <section aria-label={t('catalog.collections')}>
          <div className="discovery-section-heading"><div><h2>{t(query ? 'discovery.matchingCollections' : 'discovery.collections')}</h2>{!query && <p>{t('discovery.collectionIntro')}</p>}</div><span>{collections.length}</span></div>
          {pending ? <div className="discovery-skeletons" role="status" aria-label={t('exploreLoading')}>{[0, 1, 2, 3].map(value => <div key={value} />)}</div> : error ? <div className="discovery-feedback" role="alert"><p>{t('catalog.loadError')}</p><button className="btn btn-secondary" onClick={() => setAttempt(value => value + 1)}>{t('catalog.retry')}</button></div> : collections.length ? <div className={`discovery-collections${searching ? ' is-searching' : ''}`}>{collections.slice(0, visibleLimit).map(item => {
            const Icon = collectionIcon(item.category)
            return <article className="discovery-collection" data-category={item.category} key={item.id}>
              <button type="button" className="discovery-collection-open" ref={node => { if (node) collectionButtons.current.set(item.id, node); else collectionButtons.current.delete(item.id) }} aria-label={t('discovery.view', { name: item.title })} onClick={() => { returnFocusId.current = item.id; setPending(true); setDetail(null); setInstallError(false); setRequested(item) }}>
                <span className="discovery-collection-symbol" aria-hidden="true"><Icon size={27} strokeWidth={1.5} /></span>
                <span className="discovery-collection-content"><span className="discovery-collection-meta">{item.authorLabel}<span>{t('catalog.count', { count: item.installationCount })}</span></span><span className="discovery-collection-title">{item.title}</span><span className="discovery-collection-description">{item.description}</span><span className="discovery-collection-tags">{item.tags.slice(0, 3).join(' / ')}</span></span>
                <ChevronRight className="discovery-collection-arrow" size={17} aria-hidden="true" />
              </button>
            </article>
          })}</div> : <p className="discovery-no-match">{t('catalog.empty')}</p>}
          {!pending && !error && collections.length > collectionBatchSize && <div className="discovery-pagination" ref={loadTrigger}><span role="status">{t('discovery.showingCollections', { shown: Math.min(visibleLimit, collections.length), total: collections.length })}</span>{visibleLimit < collections.length && typeof IntersectionObserver === 'undefined' && <button type="button" className="btn btn-secondary" onClick={() => setCollectionPage({ query, limit: visibleLimit + collectionBatchSize })}>{t('discovery.loadMoreCollections')}</button>}</div>}
        </section>
        {searching ? <section className="discovery-results" aria-label={t('discovery.results')}>
          <div className="discovery-section-heading"><h2>{t('discovery.results')}</h2><span>skills.sh</span></div>
          {searchLoading ? <div className="discovery-feedback" role="status">{t('searchLoading')}</div> : searchResults.length ? <ul className="discovery-skill-results">{searchResults.map(skill => {
            const installed = managedSkills.some(item => item.name.toLowerCase() === skill.name.toLowerCase() && sourceKey(item.source_ref ?? '') === sourceKey(skill.source_url))
            return <li key={`${skill.source}/${skill.name}`}><span className="discovery-skill-symbol" aria-hidden="true"><Code2 size={18} /></span><div className="discovery-result-identity"><strong>{skill.name}</strong><small>{skill.source}</small></div><span className="discovery-installs"><Download size={13} />{t('catalog.installs', { count: skill.installs })}</span>{installed ? <span className="discovery-installed"><Check size={14} />{t('status.installed')}</span> : <button className="btn btn-secondary" type="button" disabled={loading} onClick={() => onInstallSkill(skill.source_url, skill.name)}>{t('install')}</button>}</li>
          })}</ul> : <div className="discovery-feedback"><p>{t('searchEmpty')}</p><span>{t('discovery.searchHelp')}</span></div>}
        </section> : <p className="discovery-bottom-note"><Search size={14} />{t(query ? 'catalog.searchHint' : 'discovery.searchMore')}</p>}
      </div>
    </>}
  </div>
}
export default memo(ExplorePage)
