import { memo, useId, useLayoutEffect, useRef, useState } from 'react'
import { Search } from 'lucide-react'
import type { TFunction } from 'i18next'

const SkillDescription = memo(({ text, t }: { text: string; t: TFunction }) => {
  const contentRef = useRef<HTMLDivElement>(null)
  const contentId = useId()
  const [expanded, setExpanded] = useState(false)
  const [overflowing, setOverflowing] = useState(false)

  useLayoutEffect(() => {
    const content = contentRef.current
    if (!content) return
    const measure = () => {
      const lineHeight = Number.parseFloat(getComputedStyle(content).lineHeight)
      const collapsedHeight = Number.isFinite(lineHeight) ? lineHeight * 2 : content.clientHeight
      setOverflowing(content.scrollHeight > collapsedHeight + 1)
    }
    measure()
    const observer = typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(measure)
    observer?.observe(content)
    window.addEventListener('resize', measure)
    return () => {
      observer?.disconnect()
      window.removeEventListener('resize', measure)
    }
  }, [text])

  return (
    <div className="git-pick-description">
      <div id={contentId} ref={contentRef}
        className={`pick-item-desc git-pick-description-text${expanded ? ' is-expanded' : ''}`}>
        {text}
      </div>
      {overflowing || expanded ? (
        <button type="button" className="git-pick-description-toggle"
          aria-expanded={expanded} aria-controls={contentId}
          onClick={() => setExpanded((value) => !value)}>
          {t(expanded ? 'gitInstall.collapseDescription' : 'gitInstall.expandDescription')}
        </button>
      ) : null}
    </div>
  )
})

export type SkillSelectionItem = {
  id: string
  name: string
  path: string
  description?: string | null
  selected: boolean
  selectable: boolean
  selectionLabel?: string
  status?: string
  tone?: 'install' | 'update' | 'conflict' | 'error' | 'neutral'
  note?: string
}

type Props = {
  items: SkillSelectionItem[]
  query: string
  onQueryChange: (query: string) => void
  onToggle: (id: string, checked: boolean) => void
  onToggleAll: (checked: boolean) => void
  disabled: boolean
  searchLabel?: string
  searchType?: 'text' | 'search'
  t: TFunction
}

export default memo(function SkillSelectionList({ items, query, onQueryChange, onToggle, onToggleAll, disabled, searchLabel, searchType = 'text', t }: Props) {
  const selectable = items.filter(item => item.selectable)
  const selected = selectable.filter(item => item.selected).length
  return <div className="skill-selection-list">
    <label className="skill-selection-search"><Search size={16} aria-hidden="true" />
      <input type={searchType} value={query} onChange={event => onQueryChange(event.target.value)} placeholder={searchLabel ?? t('pickSearchPlaceholder')} aria-label={searchLabel ?? t('pickSearchPlaceholder')} />
    </label>
    <div className="skill-selection-toolbar">
      <label className="inline-checkbox"><input type="checkbox" checked={selectable.length > 0 && selected === selectable.length}
        ref={element => { if (element) element.indeterminate = selected > 0 && selected < selectable.length }}
        disabled={disabled || !selectable.length} onChange={event => onToggleAll(event.target.checked)} />{t('selectAll')}</label>
      <span>{t('selectedCount', { selected, total: selectable.length })}</span>
    </div>
    <ul className="skill-selection-items">
      {items.map(item => <li className={`skill-selection-item${!item.selectable ? ' is-unavailable' : ''}`} key={item.id}>
        <div className="skill-selection-copy">
          <div className="skill-selection-heading">
        <input type="checkbox" aria-label={item.selectionLabel ?? item.name} checked={item.selectable && item.selected} disabled={disabled || !item.selectable} onChange={event => onToggle(item.id, event.target.checked)} />
            <div className="skill-selection-identity"><strong>{item.name}</strong><small className="skill-selection-path" title={item.path}>{item.path}</small></div>{item.status && <span className={`git-pick-status git-pick-status-${item.tone ?? 'neutral'}`}>{item.status}</span>}</div>
          {item.description && <SkillDescription text={item.description} t={t} />}
          {item.note && <p className={`skill-selection-note${item.tone === 'error' ? ' is-error' : ''}`}>{item.note}</p>}
        </div>
      </li>)}
      {items.length === 0 && <li className="empty">{t('pickSearchEmpty')}</li>}
    </ul>
  </div>
})
