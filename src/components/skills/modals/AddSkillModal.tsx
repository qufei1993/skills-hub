import { memo, type SetStateAction } from 'react'
import { Check, FolderOpen, GitBranch, Info } from 'lucide-react'
import type { TFunction } from 'i18next'
import ScopeSelector from '../ScopeSelector'
import ToolIcon from '../ToolIcon'
import {
  getUnsupportedToolsForScope,
  isToolUnsupportedForScope,
  normalizeProjectPaths,
  type InstallScope,
} from '../installScope'
import type { TagWithCountDto, ToolOption, ToolStatusDto } from '../types'

type AddSkillModalProps = {
  open: boolean
  loading: boolean
  canClose: boolean
  addModalTab: 'local' | 'git'
  localPath: string
  gitUrl: string
  tags: TagWithCountDto[]
  selectedTagIds: number[]
  syncTargets: Record<string, boolean>
  installedTools: ToolOption[]
  toolStatus: ToolStatusDto | null
  installScope: InstallScope
  installProjects: string[]
  recentProjects: string[]
  onRequestClose: () => void
  onTabChange: (tab: 'local' | 'git') => void
  onLocalPathChange: (value: string) => void
  onPickLocalPath: () => void
  onGitUrlChange: (value: string) => void
  onToggleTag: (tagId: number) => void
  onSyncTargetChange: (toolId: string, checked: boolean) => void
  onInstallScopeChange: (scope: InstallScope) => void
  onInstallProjectsChange: (projects: SetStateAction<string[]>) => void
  onPickProject: () => Promise<string | undefined>
  onSubmit: () => void
  t: TFunction
}

const AddSkillModal = ({
  open,
  loading,
  canClose,
  addModalTab,
  localPath,
  gitUrl,
  tags,
  selectedTagIds,
  syncTargets,
  installedTools,
  toolStatus,
  installScope,
  installProjects,
  recentProjects,
  onRequestClose,
  onTabChange,
  onLocalPathChange,
  onPickLocalPath,
  onGitUrlChange,
  onToggleTag,
  onSyncTargetChange,
  onInstallScopeChange,
  onInstallProjectsChange,
  onPickProject,
  onSubmit,
  t,
}: AddSkillModalProps) => {
  if (!open) return null

  const projectRequired =
    installScope === 'project' &&
    normalizeProjectPaths(installProjects).length === 0
  const sourceRequired =
    (addModalTab === 'local' ? localPath : gitUrl).trim().length === 0
  const unsupportedTools = getUnsupportedToolsForScope(
    installedTools,
    installScope,
  )
  const selectedToolCount = installedTools.filter(
    (tool) =>
      syncTargets[tool.id] &&
      !isToolUnsupportedForScope(tool, installScope),
  ).length
  const sourceHelp =
    addModalTab === 'local' ? t('localInstallHelp') : t('gitInstallHelp')

  return (
    <div
      className="modal-backdrop"
      onClick={() => (canClose ? onRequestClose() : null)}
    >
      <div className="modal add-skill-modal" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <div className="modal-title">{t('addSkillTitle')}</div>
          <button
            className="modal-close"
            type="button"
            onClick={onRequestClose}
            aria-label={t('close')}
            disabled={!canClose}
          >
            ✕
          </button>
        </div>
        <div className="modal-body">
          <div className="tabs">
            <button className="tab-item" type="button" onClick={onRequestClose}>
              {t('exploreTabs.online')}
            </button>
            <button
              className={`tab-item${addModalTab === 'git' ? ' active' : ''}`}
              type="button"
              onClick={() => onTabChange('git')}
            >
              {t('gitTab')}
            </button>
            <button
              className={`tab-item${addModalTab === 'local' ? ' active' : ''}`}
              type="button"
              onClick={() => onTabChange('local')}
            >
              {t('localTab')}
            </button>
          </div>

          <div className="add-install-workspace">
            <div className="add-install-scroll">
              <section className="add-source-card">
              <div className="add-card-header">
                <span className="add-card-icon" aria-hidden="true">
                  {addModalTab === 'local' ? (
                    <FolderOpen size={18} />
                  ) : (
                    <GitBranch size={18} />
                  )}
                </span>
                <div className="add-form-heading">
                  <strong>
                    {addModalTab === 'local' ? t('localTab') : t('gitTab')}
                  </strong>
                  <span>{sourceHelp}</span>
                </div>
              </div>

              <div className="add-source-body">
                {addModalTab === 'local' ? (
                  <div className="form-field">
                    <label className="label">{t('localFolder')}</label>
                    <div className="input-row">
                      <input
                        className="input"
                        placeholder={t('localPathPlaceholder')}
                        value={localPath}
                        onChange={(event) =>
                          onLocalPathChange(event.target.value)
                        }
                      />
                      <button
                        className="btn btn-secondary input-action"
                        type="button"
                        onClick={onPickLocalPath}
                        disabled={!canClose}
                      >
                        {t('browse')}
                      </button>
                    </div>
                  </div>
                ) : (
                  <div className="form-field">
                    <label className="label">{t('repositoryUrl')}</label>
                    <input
                      className="input"
                      placeholder={t('gitUrlPlaceholder')}
                      value={gitUrl}
                      onChange={(event) => onGitUrlChange(event.target.value)}
                    />
                  </div>
                )}

                <div className="add-source-note">
                  <Info size={15} />
                  <span>{t('installDetectionHint')}</span>
                </div>
              </div>
              </section>

              <aside className="add-config-card">
                <div className="add-config-body add-linear-config">
                  <section className="add-linear-row add-linear-tags">
                    <div className="add-linear-label">{t('addTags')}</div>
                    <div className="add-linear-content">
                      {tags.length > 0 ? (
                        <div className="add-tags-list">
                          {tags.map((tag) => {
                            const selected = selectedTagIds.includes(tag.id)
                            return (
                              <button
                                key={tag.id}
                                className={`add-tag-pill${selected ? ' selected' : ''}`}
                                type="button"
                                onClick={() => onToggleTag(tag.id)}
                              >
                                <span className="add-tag-check">
                                  {selected ? <Check size={12} /> : null}
                                </span>
                                <span>#{tag.name}</span>
                              </button>
                            )
                          })}
                        </div>
                      ) : (
                        <div className="helper-text">{t('noTagsYet')}</div>
                      )}
                    </div>
                  </section>

                  <section className="add-linear-row add-linear-tools">
                    <div className="add-linear-label">{t('installToTools')}</div>
                    <div className="add-linear-content">
                      {toolStatus ? (
                        <div className="tool-matrix add-tool-matrix">
                          {installedTools.map((tool) => {
                            const unsupported = isToolUnsupportedForScope(
                              tool,
                              installScope,
                            )
                            const selected = Boolean(syncTargets[tool.id])
                            return (
                              <label
                                key={tool.id}
                                className={`tool-pill-toggle${selected ? ' active' : ''}${unsupported ? ' disabled' : ''}`}
                                title={
                                  unsupported
                                    ? t('installScope.unsupportedTool', {
                                        tool: tool.label,
                                      })
                                    : undefined
                                }
                              >
                                <input
                                  type="checkbox"
                                  checked={selected}
                                  onChange={(event) =>
                                    onSyncTargetChange(
                                      tool.id,
                                      event.target.checked,
                                    )
                                  }
                                  disabled={unsupported}
                                />
                                <span className="add-tool-state" />
                                <ToolIcon
                                  toolKey={tool.id}
                                  label={tool.label}
                                  avatar={tool.avatar}
                                  className="add-tool-logo"
                                />
                                <span className="add-tool-label">{tool.label}</span>
                              </label>
                            )
                          })}
                        </div>
                      ) : (
                        <div className="helper-text">{t('detectingTools')}</div>
                      )}
                      {unsupportedTools.length > 0 ? (
                        <div className="helper-text" role="status">
                          {t('installScope.unsupportedSelectedHint', {
                            tools: unsupportedTools
                              .map((tool) => tool.label)
                              .join(', '),
                          })}
                        </div>
                      ) : null}
                    </div>
                  </section>

                  <section className="add-linear-row add-linear-scope">
                    <div className="add-linear-label">{t('installScope.title')}</div>
                    <div className="add-linear-content">
                      <ScopeSelector
                        scope={installScope}
                        projects={installProjects}
                        recentProjects={recentProjects}
                        disabled={loading}
                        compact
                        title={t('installScope.title')}
                        onScopeChange={onInstallScopeChange}
                        onProjectsChange={onInstallProjectsChange}
                        onPickProject={onPickProject}
                        t={t}
                      />
                    </div>
                  </section>
                </div>
              </aside>
            </div>

            <footer className="add-install-footer">
              <div className="add-install-summary">
                <span>{t('installSummary')}</span>
                <strong className="add-summary-source">
                  {addModalTab === 'local' ? localPath : gitUrl}
                </strong>
                <strong>
                  {t('installTargetSummary', {
                    scope:
                      installScope === 'global'
                        ? t('scope.global')
                        : t('scope.project'),
                    count: selectedToolCount,
                  })}
                </strong>
                <strong>{t('tagSelectionSummary', { count: selectedTagIds.length })}</strong>
              </div>

              <div className="add-config-actions">
                <button
                  className="btn btn-secondary"
                  onClick={onRequestClose}
                  disabled={!canClose}
                >
                  {t('cancel')}
                </button>
                <button
                  className="btn btn-primary"
                  onClick={onSubmit}
                  disabled={loading || projectRequired || sourceRequired}
                >
                  {addModalTab === 'local' ? t('create') : t('install')}
                </button>
              </div>
            </footer>
          </div>
        </div>
      </div>
    </div>
  )
}

export default memo(AddSkillModal)
