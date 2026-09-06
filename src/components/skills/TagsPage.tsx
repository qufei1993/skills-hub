import { memo, useMemo, useState } from 'react'
import { ArrowLeft, Pencil, Plus, Search, Tag, Trash2 } from 'lucide-react'
import type { TFunction } from 'i18next'
import type { TagWithCountDto } from './types'

type TagsPageProps = {
  tags: TagWithCountDto[]
  untaggedCount: number
  loading: boolean
  formatRelative: (ms: number | null | undefined) => string
  embedded?: boolean
  onBack?: () => void
  onReviewUntagged: () => void
  onViewTag: (tagId: number) => void
  onCreateTag: (name: string) => void
  onRenameTag: (tag: TagWithCountDto) => void
  onDeleteTag: (tag: TagWithCountDto) => void
  t: TFunction
}

const TagsPage = ({
  tags,
  untaggedCount,
  loading,
  formatRelative,
  embedded = false,
  onBack,
  onReviewUntagged,
  onViewTag,
  onCreateTag,
  onRenameTag,
  onDeleteTag,
  t,
}: TagsPageProps) => {
  const [query, setQuery] = useState('')
  const [newTagName, setNewTagName] = useState('')
  const filteredTags = useMemo(() => {
    const normalized = query.trim().toLowerCase()
    if (!normalized) return tags
    return tags.filter((tag) => tag.name.toLowerCase().includes(normalized))
  }, [query, tags])

  const submitNewTag = () => {
    const name = newTagName.trim()
    if (!name) return
    onCreateTag(name)
    setNewTagName('')
  }

  return (
    <div className={`tags-page${embedded ? ' embedded' : ''}`}>
      {!embedded ? (
        <div className="detail-header">
          <button className="btn btn-secondary" type="button" onClick={onBack}>
            <ArrowLeft size={16} />
            {t('back')}
          </button>
          <div>
            <div className="detail-skill-name">{t('tags')}</div>
            <div className="tags-page-subtitle">{t('tagsHelp')}</div>
          </div>
        </div>
      ) : null}

      <div className="tags-layout">
        <section className="tags-list-panel">
          <div className="tags-panel-head">
            <strong>{t('tagListTitle')}</strong>
            <span>{t('tagCount', { count: tags.length })}</span>
          </div>
          <div className="search-container tags-search">
            <Search size={16} className="search-icon-abs" />
            <input
              className="search-input"
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder={t('searchTags')}
            />
          </div>
          <div className="tags-table">
            <div className="tags-table-row tags-table-head">
              <span>{t('tagName')}</span>
              <span>{t('skills')}</span>
              <span>{t('lastUsed')}</span>
              <span>{t('actionsLabel')}</span>
            </div>
            {filteredTags.length === 0 ? (
              <div className="empty">{t('tagsEmpty')}</div>
            ) : (
              filteredTags.map((tag) => (
                <div className="tags-table-row" key={tag.id}>
                  <button className="tags-table-name" type="button" onClick={() => onViewTag(tag.id)}>{tag.name}</button>
                  <span>{tag.skill_count}</span>
                  <span>{formatRelative(tag.updated_at)}</span>
                  <span className="tags-table-actions">
                    <button type="button" aria-label={t('rename')} title={t('rename')} onClick={() => onRenameTag(tag)}><Pencil size={15} /></button>
                    <button type="button" aria-label={t('deleteAction')} title={t('deleteAction')} onClick={() => onDeleteTag(tag)}><Trash2 size={15} /></button>
                  </span>
                </div>
              ))
            )}
          </div>
        </section>

        <aside className="tags-side-panel">
          <section className="tags-create-card">
            <div className="tags-panel-head"><strong>{t('newTag')}</strong></div>
            <label>{t('tagName')}</label>
            <input
              className="search-input"
              value={newTagName}
              onChange={(event) => setNewTagName(event.target.value)}
              onKeyDown={(event) => { if (event.key === 'Enter') submitNewTag() }}
              placeholder={t('newTagPlaceholder')}
            />
            <button className="btn btn-primary" type="button" onClick={submitNewTag} disabled={loading || !newTagName.trim()}>
              <Plus size={14} />{t('newTag')}
            </button>
          </section>
          <div className="tags-review-row">
            <div className="tags-review-left">
              <Tag size={16} />
              <span>{t('untaggedSkillsCount', { count: untaggedCount })}</span>
            </div>
            <button className="btn btn-secondary" type="button" onClick={onReviewUntagged} disabled={untaggedCount === 0}>{t('review')}</button>
          </div>
        </aside>
      </div>
    </div>
  )
}

export default memo(TagsPage)
