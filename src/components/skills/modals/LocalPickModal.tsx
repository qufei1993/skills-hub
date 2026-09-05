import { memo, useMemo, useState } from 'react'
import { Search } from 'lucide-react'
import type { TFunction } from 'i18next'
import type { LocalSkillCandidate } from '../types'

type LocalPickModalProps = {
  open: boolean
  loading: boolean
  localCandidates: LocalSkillCandidate[]
  localCandidateSelected: Record<string, boolean>
  onRequestClose: () => void
  onCancel: () => void
  onToggleCandidate: (subpath: string, checked: boolean) => void
  onInstall: (subpaths: string[]) => void
  t: TFunction
}

const LocalPickModal = ({
  open,
  loading,
  localCandidates,
  localCandidateSelected,
  onRequestClose,
  onCancel,
  onToggleCandidate,
  onInstall,
  t,
}: LocalPickModalProps) => {
  const [query, setQuery] = useState('')
  const normalizedQuery = query.trim().toLowerCase()
  const filteredCandidates = useMemo(() => {
    if (!normalizedQuery) return localCandidates
    return localCandidates.filter((c) =>
      [c.name, c.description ?? '', c.subpath].some((value) =>
        value.toLowerCase().includes(normalizedQuery),
      ),
    )
  }, [localCandidates, normalizedQuery])
  const selectableCandidates = filteredCandidates.filter((c) => c.valid)
  const selectedCandidates = selectableCandidates.filter(
    (c) => localCandidateSelected[c.subpath],
  )
  const selectedCount = selectedCandidates.length
  const selectableCount = selectableCandidates.length
  const allVisibleSelected =
    selectableCount > 0 &&
    selectableCandidates.every((c) => localCandidateSelected[c.subpath])

  const toggleVisibleCandidates = (checked: boolean) => {
    selectableCandidates.forEach((c) => onToggleCandidate(c.subpath, checked))
  }

  if (!open) return null

  const mapReason = (code?: string | null) => {
    if (!code) return t('localSkillInvalid.unknown')
    if (code === 'missing_skill_md') return t('localSkillInvalid.missingSkillMd')
    if (code === 'invalid_frontmatter') return t('localSkillInvalid.invalidFrontmatter')
    if (code === 'missing_name') return t('localSkillInvalid.missingName')
    if (code === 'read_failed') return t('localSkillInvalid.readFailed')
    return t('localSkillInvalid.unknown')
  }

  return (
    <div className="modal-backdrop" onClick={onRequestClose}>
      <div className="modal pick-skill-modal" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <div className="modal-title">{t('localPickTitle')}</div>
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
          <p className="label">{t('localPickBody')}</p>
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
                disabled={selectableCount === 0}
              />
              {t('selectAll')}
            </label>
            <span className="pick-toolbar-count">
              {t('selectedCount', {
                selected: selectedCount,
                total: selectableCount,
              })}
            </span>
          </div>
          <div className="pick-list">
            {filteredCandidates.length === 0 ? (
              <div className="empty">{t('pickSearchEmpty')}</div>
            ) : null}
            {filteredCandidates.map((c) => (
              <div
                className={`pick-item${c.valid ? '' : ' disabled'}`}
                key={c.subpath}
              >
                <label className="pick-item-checkbox">
                  <input
                    type="checkbox"
                    checked={Boolean(localCandidateSelected[c.subpath])}
                    onChange={(e) => onToggleCandidate(c.subpath, e.target.checked)}
                    disabled={!c.valid}
                  />
                </label>
                <div className="pick-item-main">
                  <div className="pick-item-title">{c.name}</div>
                  {c.description ? (
                    <div className="pick-item-desc">{c.description}</div>
                  ) : null}
                  <div className="pick-item-path">{c.subpath}</div>
                  {!c.valid ? (
                    <div className="pick-item-reason">
                      {t('localPickInvalidReason', { reason: mapReason(c.reason) })}
                    </div>
                  ) : null}
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
            {t('installSelected')}
          </button>
        </div>
      </div>
    </div>
  )
}

export default memo(LocalPickModal)
