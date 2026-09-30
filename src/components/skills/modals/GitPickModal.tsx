import { memo, useMemo, useState } from 'react'
import { Search } from 'lucide-react'
import type { TFunction } from 'i18next'
import type { GitSkillCandidate } from '../types'

type GitPickModalProps = {
  open: boolean
  loading: boolean
  gitCandidates: GitSkillCandidate[]
  gitCandidateSelected: Record<string, boolean>
  onRequestClose: () => void
  onCancel: () => void
  onToggleCandidate: (subpath: string, checked: boolean) => void
  onInstall: (subpaths: string[]) => void
  t: TFunction
}

const GitPickModal = ({
  open,
  loading,
  gitCandidates,
  gitCandidateSelected,
  onRequestClose,
  onCancel,
  onToggleCandidate,
  onInstall,
  t,
}: GitPickModalProps) => {
  const [query, setQuery] = useState('')
  const normalizedQuery = query.trim().toLowerCase()
  const filteredCandidates = useMemo(() => {
    if (!normalizedQuery) return gitCandidates
    return gitCandidates.filter((c) =>
      [c.name, c.description ?? '', c.subpath].some((value) =>
        value.toLowerCase().includes(normalizedQuery),
      ),
    )
  }, [gitCandidates, normalizedQuery])
  const selectableCandidates = filteredCandidates.filter((c) => c.status !== 'conflict')
  const selectedCandidates = selectableCandidates.filter(
    (c) => gitCandidateSelected[c.subpath],
  )
  const selectedCount = selectedCandidates.length
  const allVisibleSelected =
    selectableCandidates.length > 0 &&
    selectableCandidates.every((c) => gitCandidateSelected[c.subpath])

  const toggleVisibleCandidates = (checked: boolean) => {
    selectableCandidates.forEach((c) => onToggleCandidate(c.subpath, checked))
  }

  const updateCount = selectedCandidates.filter((c) => c.status === 'update').length
  const conflictCount = filteredCandidates.length - selectableCandidates.length

  if (!open) return null

  return (
    <div className="modal-backdrop" onClick={onRequestClose}>
      <div className="modal pick-skill-modal" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <div className="modal-title">{t('gitPickTitle')}</div>
          <button
            className="modal-close"
            type="button"
            onClick={onRequestClose}
            aria-label={t('close')}
          >
            ✕
          </button>
        </div>
        <div className="modal-body">
          <p className="label">{t('gitPickBody')}</p>
          {updateCount > 0 || conflictCount > 0 ? (
            <p className="label">{t('gitInstall.preview', {
              install: selectedCount - updateCount, update: updateCount, conflict: conflictCount,
            })}</p>
          ) : null}
          <div className="pick-search">
            <Search size={16} className="search-icon-abs" />
            <input
              className="search-input"
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder={t('pickSearchPlaceholder')}
            />
          </div>
          <div className="pick-toolbar">
            <label className="inline-checkbox">
              <input
                type="checkbox"
                checked={allVisibleSelected}
                onChange={(e) => toggleVisibleCandidates(e.target.checked)}
                disabled={loading || selectableCandidates.length === 0}
              />
              {t('selectAll')}
            </label>
            <span className="pick-toolbar-count">
              {t('selectedCount', {
                selected: selectedCount,
                total: selectableCandidates.length,
              })}
            </span>
          </div>
          <div className="pick-list">
            {filteredCandidates.length === 0 ? (
              <div className="empty">{t('pickSearchEmpty')}</div>
            ) : null}
            {filteredCandidates.map((c) => (
              <div className="pick-item" key={c.subpath}>
                <label className="pick-item-checkbox">
                  <input
                    type="checkbox"
                    checked={c.status !== 'conflict' && Boolean(gitCandidateSelected[c.subpath])}
                    disabled={loading || c.status === 'conflict'}
                    aria-label={c.name}
                    onChange={(e) => onToggleCandidate(c.subpath, e.target.checked)}
                  />
                </label>
                <div className="pick-item-main">
                  <div className="git-pick-item-heading">
                    <div className="pick-item-title">{c.name}</div>
                    {c.status ? (
                      <span className={`git-pick-status git-pick-status-${c.status}`}>
                        {t(c.status === 'conflict' ? 'gitInstall.conflictLabel' : `gitInstall.${c.status}`)}
                      </span>
                    ) : null}
                  </div>
                  {c.status === 'conflict' ? (
                    <div className="pick-item-desc">{t('gitInstall.conflict')}</div>
                  ) : null}
                  {c.description ? (
                    <div className="pick-item-desc">{c.description}</div>
                  ) : null}
                  <div className="pick-item-path">{c.subpath}</div>
                </div>
              </div>
            ))}
          </div>
        </div>
        <div className="modal-footer">
          <button className="btn btn-secondary" onClick={onCancel} disabled={loading}>
            {t('cancel')}
          </button>
          <button
            className="btn btn-primary"
            onClick={() => onInstall(selectedCandidates.map((c) => c.subpath))}
            disabled={loading || selectedCount === 0}
          >
            {t(updateCount > 0 ? 'gitInstall.submit' : 'installSelected')}
          </button>
        </div>
      </div>
    </div>
  )
}

export default memo(GitPickModal)
