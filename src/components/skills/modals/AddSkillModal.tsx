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
import { SUMMARY_TARGET_MAX_CHARS, SUMMARY_SOFT_MIN_CHARS, countCjkAwareLength, isAcceptableSummary, normalizeHexColor } from '../skillProfile'

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
  profileZhName: string
  profileCategory: string
  profileColor: string
  profileSummary: string
  profileNote: string
  onProfileZhNameChange: (value: string) => void
  onProfileCategoryChange: (value: string) => void
  onProfileColorChange: (value: string) => void
  onProfileSummaryChange: (value: string) => void
  onProfileNoteChange: (value: string) => void
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
  profileZhName,
  profileCategory,
  profileColor,
  profileSummary,
  profileNote,
  onProfileZhNameChange,
  onProfileCategoryChange,
  onProfileColorChange,
  onProfileSummaryChange,
  onProfileNoteChange,
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

              <section className="add-profile-card">
                <div className="add-card-header">
                  <div className="add-form-heading">
                    <strong>管理资料</strong>
                    <span>安装时填写颜色、分类、中文名与 12–30 字简介；英文调用名保持不变。</span>
                  </div>
                </div>
                <div className="add-profile-grid">
                  <label className="form-field">
                    <span className="label">中文名称</span>
                    <input className="input" value={profileZhName} onChange={(e) => onProfileZhNameChange(e.target.value)} placeholder="例如：Nature论文写作" />
                  </label>
                  <label className="form-field">
                    <span className="label">分类</span>
                    <input className="input" value={profileCategory} onChange={(e) => onProfileCategoryChange(e.target.value)} placeholder="例如：学术写作" list="skills-hub-profile-categories" />
                    <datalist id="skills-hub-profile-categories">
                      {Array.from(new Set(tags.map((tag) => tag.name))).map((name) => (
                        <option key={name} value={name} />
                      ))}
                    </datalist>
                  </label>
                  <label className="form-field">
                    <span className="label">颜色</span>
                    <div className="input-row">
                      <input type="color" value={normalizeHexColor(profileColor)?.slice(0,7) || '#3B82F6'} onChange={(e) => onProfileColorChange(e.target.value.toUpperCase())} aria-label="颜色选择器" />
                      <input className="input" value={profileColor} onChange={(e) => onProfileColorChange(e.target.value)} placeholder="#3B82F6" />
                    </div>
                  </label>
                  <label className="form-field">
                    <span className="label">功能简介（{countCjkAwareLength(profileSummary)}/{SUMMARY_TARGET_MAX_CHARS}）</span>
                    <input className="input" value={profileSummary} onChange={(e) => onProfileSummaryChange(e.target.value)} placeholder={`自动或手写 ${SUMMARY_SOFT_MIN_CHARS}-${SUMMARY_TARGET_MAX_CHARS} 字`} />
                  </label>
                  <label className="form-field add-profile-note">
                    <span className="label">备注</span>
                    <textarea className="input" rows={2} value={profileNote} onChange={(e) => onProfileNoteChange(e.target.value)} placeholder="仅工作台管理，不写入调用协议" />
                  </label>
                </div>
                {profileSummary.trim() && !isAcceptableSummary(profileSummary) ? (
                  <div className="helper-text" role="status">简介最多 {SUMMARY_TARGET_MAX_CHARS} 字；短于 {SUMMARY_SOFT_MIN_CHARS} 也可以。</div>
                ) : null}
              </section>

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
                  disabled={loading || projectRequired || sourceRequired || (profileSummary.trim().length > 0 && !isAcceptableSummary(profileSummary)) || (profileColor.trim().length > 0 && !normalizeHexColor(profileColor))}
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
