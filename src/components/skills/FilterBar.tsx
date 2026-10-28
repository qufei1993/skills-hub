import { memo, useEffect, useMemo, useRef, useState } from 'react'
import { ArrowUpDown, Check, CheckSquare, ChevronDown, LayoutGrid, List, ListFilter, Search, Tags } from 'lucide-react'
import type { TFunction } from 'i18next'
import type { TagWithCountDto, ToolOption } from './types'
import type { SkillStatusFilter } from './skillsView'
import ToolIcon from './ToolIcon'

type FilterBarProps = {
  sortBy: 'updated' | 'name'
  searchQuery: string
  tags: TagWithCountDto[]
  selectedTagIds: number[]
  includeUntagged: boolean
  untaggedCount: number
  statusFilter: SkillStatusFilter
  statusCounts: Record<SkillStatusFilter, number>
  tools: ToolOption[]
  toolCounts: Map<string, number>
  selectedToolIds: string[]
  bulkMode: boolean
  bulkSelectedCount: number
  viewMode: 'list' | 'cards'
  onSortChange: (value: 'updated' | 'name') => void
  onSearchChange: (value: string) => void
  onToggleTag: (tagId: number) => void
  onToggleUntagged: () => void
  onClearTags: () => void
  onStatusFilterChange: (value: SkillStatusFilter) => void
  onToggleTool: (toolId: string) => void
  onClearSkillFilters: () => void
  onManageTags: () => void
  onOpenTags?: () => void
  onToggleBulkMode: () => void
  onViewModeChange: (value: 'list' | 'cards') => void
  t: TFunction
}

const FilterBar = ({
  sortBy,
  searchQuery,
  tags,
  selectedTagIds,
  includeUntagged,
  untaggedCount,
  statusFilter,
  statusCounts,
  tools,
  toolCounts,
  selectedToolIds,
  bulkMode,
  bulkSelectedCount,
  viewMode,
  onSortChange,
  onSearchChange,
  onToggleTag,
  onToggleUntagged,
  onClearTags,
  onStatusFilterChange,
  onToggleTool,
  onClearSkillFilters,
  onManageTags,
  onOpenTags,
  onToggleBulkMode,
  onViewModeChange,
  t,
}: FilterBarProps) => {
  const [tagMenuOpen, setTagMenuOpen] = useState(false)
  const [tagQuery, setTagQuery] = useState('')
  const tagMenuRef = useRef<HTMLDivElement | null>(null)
  const [filterMenuOpen, setFilterMenuOpen] = useState(false)
  const filterMenuRef = useRef<HTMLDivElement | null>(null)
  const selectedToolSet = useMemo(() => new Set(selectedToolIds), [selectedToolIds])
  const activeFilterCount = selectedToolIds.length + (statusFilter === 'all' ? 0 : 1)
  const selectedTagSet = useMemo(() => new Set(selectedTagIds), [selectedTagIds])
  const selectedCount = selectedTagIds.length + (includeUntagged ? 1 : 0)
  const filteredTags = useMemo(() => {
    const query = tagQuery.trim().toLowerCase()
    if (!query) return tags
    return tags.filter((tag) => tag.name.toLowerCase().includes(query))
  }, [tagQuery, tags])

  useEffect(() => {
    if (!tagMenuOpen) return
    const handlePointerDown = (event: MouseEvent) => {
      if (!tagMenuRef.current?.contains(event.target as Node)) {
        setTagMenuOpen(false)
      }
    }
    document.addEventListener('mousedown', handlePointerDown)
    return () => document.removeEventListener('mousedown', handlePointerDown)
  }, [tagMenuOpen])

  useEffect(() => {
    if (!filterMenuOpen) return
    const handlePointerDown = (event: MouseEvent) => {
      if (!filterMenuRef.current?.contains(event.target as Node)) {
        setFilterMenuOpen(false)
      }
    }
    document.addEventListener('mousedown', handlePointerDown)
    return () => document.removeEventListener('mousedown', handlePointerDown)
  }, [filterMenuOpen])

  return (
    <div className="filter-bar">
      <div className="filter-actions">
        <div className="search-container">
          <Search size={16} className="search-icon-abs" />
          <input
            className="search-input"
            aria-label={t('searchPlaceholder')}
            value={searchQuery}
            onChange={(event) => onSearchChange(event.target.value)}
            placeholder={t('searchPlaceholder')}
          />
        </div>
        <button className="btn btn-secondary sort-btn" type="button">
          {sortBy === 'updated' ? t('sortUpdated') : t('sortName')}
          <ArrowUpDown size={12} />
          <select
            aria-label={t('filterSort')}
            value={sortBy}
            onChange={(event) => onSortChange(event.target.value as 'updated' | 'name')}
          >
            <option value="updated">{t('sortUpdated')}</option>
            <option value="name">{t('sortName')}</option>
          </select>
        </button>
        <button
          className={`btn btn-secondary bulk-mode-btn${bulkMode ? ' active' : ''}`}
          type="button"
          onClick={onToggleBulkMode}
        >
          <CheckSquare size={14} />
          {bulkMode
            ? t('bulk.selectedShort', { count: bulkSelectedCount })
            : t('bulk.manage')}
        </button>
        <div className="tag-filter-wrap" ref={filterMenuRef}>
          <button
            className={`btn btn-secondary tag-filter-btn${activeFilterCount > 0 ? ' active' : ''}`}
            type="button"
            aria-expanded={filterMenuOpen}
            onClick={() => setFilterMenuOpen(!filterMenuOpen)}
          >
            <ListFilter size={14} />
            {activeFilterCount > 0
              ? t('filters.menuSelected', { count: activeFilterCount })
              : t('filters.menu')}
            <ChevronDown size={12} />
          </button>
          {filterMenuOpen ? (
            <div className="tag-filter-menu">
              <div className="tag-filter-head">
                <span>{t('filters.status')}</span>
                <span />
              </div>
              <div className="skill-status-filter settings-segmented" role="group" aria-label={t('filters.status')}>
                {(['all', 'enabled', 'disabled'] as const).map((value) => (
                  <button
                    key={value}
                    className={statusFilter === value ? 'active' : ''}
                    type="button"
                    aria-pressed={statusFilter === value}
                    onClick={() => onStatusFilterChange(value)}
                  >
                    {t(`filters.status_${value}`)}
                    <span className="tag-count">{statusCounts[value]}</span>
                  </button>
                ))}
              </div>
              <div className="tag-filter-head">
                <span>{t('filters.tools')}</span>
                <span>{t('matchAny')}</span>
              </div>
              <div className="tag-filter-options">
                {tools.map((tool) => {
                  const selected = selectedToolSet.has(tool.id)
                  const count = toolCounts.get(tool.id) ?? 0
                  return (
                    <button
                      key={tool.id}
                      className={`tag-filter-option${selected ? ' selected' : ''}`}
                      type="button"
                      onClick={() => onToggleTool(tool.id)}
                      disabled={count === 0 && !selected}
                      aria-pressed={selected}
                    >
                      <span className="tag-check">{selected ? <Check size={14} /> : null}</span>
                      <span className="tool-filter-label">
                        <ToolIcon toolKey={tool.id} label={tool.label} avatar={tool.avatar} />
                        {tool.label}
                      </span>
                      <span className="tag-count">{count}</span>
                    </button>
                  )
                })}
              </div>
              <div className="tag-filter-footer">
                <button type="button" onClick={onClearSkillFilters} disabled={activeFilterCount === 0}>
                  {t('clearAll')}
                </button>
              </div>
            </div>
          ) : null}
        </div>
        <div className="tag-filter-wrap" ref={tagMenuRef}>
          <button
            className={`btn btn-secondary tag-filter-btn${selectedCount > 0 ? ' active' : ''}`}
            type="button"
            onClick={() => {
              if (!tagMenuOpen) onOpenTags?.()
              setTagMenuOpen(!tagMenuOpen)
            }}
          >
            <Tags size={14} />
            {selectedCount > 0
              ? t('tagsSelected', { count: selectedCount })
              : t('tags')}
            <ChevronDown size={12} />
          </button>
          {tagMenuOpen ? (
            <div className="tag-filter-menu">
              <div className="tag-filter-head">
                <span>{t('tags')}</span>
                <span>{t('matchAny')}</span>
              </div>
              <div className="tag-filter-search">
                <Search size={15} />
                <input
                  value={tagQuery}
                  onChange={(event) => setTagQuery(event.target.value)}
                  placeholder={t('searchTags')}
                />
              </div>
              <div className="tag-filter-options">
                <button
                  className={`tag-filter-option${includeUntagged ? ' selected' : ''}`}
                  type="button"
                  onClick={onToggleUntagged}
                  disabled={untaggedCount === 0 && !includeUntagged}
                  aria-pressed={includeUntagged}
                >
                  <span className="tag-check">{includeUntagged ? <Check size={14} /> : null}</span>
                  <span>{t('untagged')}</span>
                  <span className="tag-count">{untaggedCount}</span>
                </button>
                {filteredTags.map((tag) => {
                  const selected = selectedTagSet.has(tag.id)
                  return (
                    <button
                      key={tag.id}
                      className={`tag-filter-option${selected ? ' selected' : ''}`}
                      type="button"
                      onClick={() => onToggleTag(tag.id)}
                      disabled={tag.skill_count === 0 && !selected}
                      aria-pressed={selected}
                    >
                      <span className="tag-check">{selected ? <Check size={14} /> : null}</span>
                      <span>{tag.name}</span>
                      <span className="tag-count">{tag.skill_count}</span>
                    </button>
                  )
                })}
              </div>
              <div className="tag-filter-footer">
                <button type="button" onClick={onClearTags} disabled={selectedCount === 0}>
                  {t('clearAll')}
                </button>
                <button type="button" onClick={onManageTags}>
                  {t('manageTags')}
                </button>
              </div>
            </div>
          ) : null}
        </div>
        <div className="view-mode-toggle" role="group" aria-label={t('viewMode.label')}>
          <button
            className={viewMode === 'list' ? 'active' : ''}
            type="button"
            onClick={() => onViewModeChange('list')}
            aria-label={t('viewMode.list')}
            title={t('viewMode.list')}
            aria-pressed={viewMode === 'list'}
          >
            <List size={15} />
          </button>
          <button
            className={viewMode === 'cards' ? 'active' : ''}
            type="button"
            onClick={() => onViewModeChange('cards')}
            aria-label={t('viewMode.cards')}
            title={t('viewMode.cards')}
            aria-pressed={viewMode === 'cards'}
          >
            <LayoutGrid size={15} />
          </button>
        </div>

      </div>
    </div>
  )
}

export default memo(FilterBar)
