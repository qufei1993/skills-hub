import { memo, type ReactNode, type SetStateAction } from 'react'
import { Check } from 'lucide-react'
import type { TFunction } from 'i18next'
import ScopeSelector from '../ScopeSelector'
import ToolIcon from '../ToolIcon'
import InstallSettingsSections from './InstallSettingsSections'
import { getUnsupportedToolsForScope, isToolUnsupportedForScope, type InstallScope } from '../installScope'
import type { TagWithCountDto, ToolOption, ToolStatusDto } from '../types'

type AddInstallSettingsProps = {
  sharedConfirmation?: ReactNode
  loading: boolean
  tags: TagWithCountDto[]
  selectedTagIds: number[]
  syncTargets: Record<string, boolean>
  installedTools: ToolOption[]
  toolStatus: ToolStatusDto | null
  installScope: InstallScope
  installProjects: string[]
  recentProjects: string[]
  onToggleTag: (tagId: number) => void
  onSyncTargetChange: (toolId: string, checked: boolean) => void
  onInstallScopeChange: (scope: InstallScope) => void
  onInstallProjectsChange: (projects: SetStateAction<string[]>) => void
  onPickProject: () => Promise<string | undefined>
  t: TFunction
}

const AddInstallSettings = ({ sharedConfirmation, loading, tags, selectedTagIds, syncTargets, installedTools, toolStatus,
  installScope, installProjects, recentProjects, onToggleTag, onSyncTargetChange,
  onInstallScopeChange, onInstallProjectsChange, onPickProject, t }: AddInstallSettingsProps) => {
  const unsupportedTools = getUnsupportedToolsForScope(installedTools, installScope)
  const selectedToolCount = installedTools.filter(tool => syncTargets[tool.id] && !isToolUnsupportedForScope(tool, installScope)).length
  return <InstallSettingsSections disabled={loading} t={t}
    toolSummary={selectedToolCount ? t('collectionInstall.toolCount', { count: selectedToolCount }) : t('collectionInstall.libraryOnly')}

    tags={<>
                      {tags.length > 0 ? (
                        <div className="add-tags-list">
                          {tags.map((tag) => {
                            const selected = selectedTagIds.includes(tag.id)
                            return (
                              <button
                                key={tag.id}
                                className={`add-tag-pill${selected ? ' selected' : ''}`}
                                type="button"
                                aria-pressed={selected}
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
                    </>}
    tools={<>
                      {sharedConfirmation}
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
                                  disabled={loading || unsupported}
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
                    </>}
    scope={<>
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
                    </>}
  />
}
export default memo(AddInstallSettings)
