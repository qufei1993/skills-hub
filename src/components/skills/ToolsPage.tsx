import { memo, useRef, useState, type ChangeEvent } from 'react'
import {
  ArrowLeft,
  ChevronDown,
  ImagePlus,
  Pencil,
  Plus,
  Save,
  Trash2,
  X,
} from 'lucide-react'
import type { TFunction } from 'i18next'
import type {
  CustomToolConfigDto,
  SyncMode,
  ToolConfigDto,
  ToolStatusDto,
} from './types'
import ToolIcon from './ToolIcon'
import ConfirmActionModal from './modals/ConfirmActionModal'

const AVATAR_INPUT_MAX_BYTES = 2 * 1024 * 1024
const AVATAR_SIZE = 128

const createAvatarDataUrl = (file: File) =>
  new Promise<string>((resolve, reject) => {
    const objectUrl = URL.createObjectURL(file)
    const image = new Image()
    image.onload = () => {
      try {
        const canvas = document.createElement('canvas')
        canvas.width = AVATAR_SIZE
        canvas.height = AVATAR_SIZE
        const context = canvas.getContext('2d')
        if (!context) {
          reject(new Error('Canvas is unavailable'))
          return
        }
        const cropSize = Math.min(image.naturalWidth, image.naturalHeight)
        const sourceX = (image.naturalWidth - cropSize) / 2
        const sourceY = (image.naturalHeight - cropSize) / 2
        context.drawImage(
          image,
          sourceX,
          sourceY,
          cropSize,
          cropSize,
          0,
          0,
          AVATAR_SIZE,
          AVATAR_SIZE,
        )
        resolve(canvas.toDataURL('image/png'))
      } catch (error) {
        reject(error instanceof Error ? error : new Error(String(error)))
      } finally {
        URL.revokeObjectURL(objectUrl)
      }
    }
    image.onerror = () => {
      URL.revokeObjectURL(objectUrl)
      reject(new Error('Image could not be decoded'))
    }
    image.src = objectUrl
  })

type ToolsPageProps = {
  toolStatus: ToolStatusDto | null
  toolConfig: ToolConfigDto | null
  onToolConfigChange: (config: ToolConfigDto) => Promise<boolean>
  embedded?: boolean
  onBack?: () => void
  t: TFunction
}

const ToolsPage = ({
  toolStatus,
  toolConfig,
  onToolConfigChange,
  embedded = false,
  onBack,
  t,
}: ToolsPageProps) => {
  const [customToolName, setCustomToolName] = useState('')
  const [customToolAvatar, setCustomToolAvatar] = useState<string | null>(null)
  const [customToolSkillsDir, setCustomToolSkillsDir] = useState('')
  const [customToolProjectDir, setCustomToolProjectDir] = useState('')
  const [customToolSyncMode, setCustomToolSyncMode] = useState<SyncMode>('auto')
  const [editingCustomToolKey, setEditingCustomToolKey] = useState<string | null>(null)
  const [pendingRemoveCustomTool, setPendingRemoveCustomTool] = useState<CustomToolConfigDto | null>(null)
  const [savingCustomTool, setSavingCustomTool] = useState(false)
  const [avatarError, setAvatarError] = useState('')
  const [showMissingTools, setShowMissingTools] = useState(false)
  const avatarInputRef = useRef<HTMLInputElement>(null)
  const customToolFormRef = useRef<HTMLElement>(null)
  const customToolNameInputRef = useRef<HTMLInputElement>(null)
  const effectiveToolConfig = toolConfig ?? {
    disabled_builtin_tools: [],
    custom_tools: [],
  }
  const disabledBuiltinTools = new Set(effectiveToolConfig.disabled_builtin_tools)
  const customToolsByKey = new Map(
    effectiveToolConfig.custom_tools.map((tool) => [tool.key, tool]),
  )

  const makeCustomToolKey = (label: string) => {
    const slug = label
      .trim()
      .toLowerCase()
      .replace(/[^a-z0-9_-]+/g, '_')
      .replace(/^_+|_+$/g, '')
    const base = slug && /^[a-z]/.test(slug) ? slug : 'custom_tool'
    const prefix = `custom_${base}`
    const existingKeys = new Set(effectiveToolConfig.custom_tools.map((tool) => tool.key))
    if (!existingKeys.has(prefix)) return prefix
    let suffix = 2
    while (existingKeys.has(`${prefix}_${suffix}`)) suffix += 1
    return `${prefix}_${suffix}`
  }

  const updateToolConfig = (next: ToolConfigDto) =>
    onToolConfigChange({
      disabled_builtin_tools: [...next.disabled_builtin_tools],
      custom_tools: next.custom_tools.map((tool) => ({ ...tool })),
    })

  const resetCustomToolForm = () => {
    setEditingCustomToolKey(null)
    setCustomToolName('')
    setCustomToolAvatar(null)
    setCustomToolSkillsDir('')
    setCustomToolProjectDir('')
    setCustomToolSyncMode('auto')
    setAvatarError('')
  }

  const editCustomTool = (key: string) => {
    const tool = effectiveToolConfig.custom_tools.find((item) => item.key === key)
    if (!tool) return
    setEditingCustomToolKey(tool.key)
    setCustomToolName(tool.label)
    setCustomToolAvatar(tool.avatar ?? null)
    setCustomToolSkillsDir(tool.skills_dir)
    setCustomToolProjectDir(tool.project_skills_dir ?? '')
    setCustomToolSyncMode(tool.sync_mode)
    setAvatarError('')
    window.requestAnimationFrame(() => {
      customToolFormRef.current?.scrollIntoView({ block: 'nearest' })
      customToolNameInputRef.current?.focus()
      customToolNameInputRef.current?.select()
    })
  }

  const setBuiltinToolEnabled = (key: string, enabled: boolean) => {
    const disabled = new Set(effectiveToolConfig.disabled_builtin_tools)
    if (enabled) {
      disabled.delete(key)
    } else {
      disabled.add(key)
    }
    void updateToolConfig({
      ...effectiveToolConfig,
      disabled_builtin_tools: [...disabled],
    })
  }

  const setCustomToolEnabled = (key: string, enabled: boolean) => {
    void updateToolConfig({
      ...effectiveToolConfig,
      custom_tools: effectiveToolConfig.custom_tools.map((tool) =>
        tool.key === key ? { ...tool, enabled } : tool,
      ),
    })
  }

  const removeCustomTool = async () => {
    if (!pendingRemoveCustomTool || savingCustomTool) return
    setSavingCustomTool(true)
    try {
      const saved = await updateToolConfig({
        ...effectiveToolConfig,
        custom_tools: effectiveToolConfig.custom_tools.filter(
          (tool) => tool.key !== pendingRemoveCustomTool.key,
        ),
      })
      if (!saved) return
      if (editingCustomToolKey === pendingRemoveCustomTool.key) {
        resetCustomToolForm()
      }
      setPendingRemoveCustomTool(null)
    } finally {
      setSavingCustomTool(false)
    }
  }

  const saveCustomTool = async () => {
    const label = customToolName.trim()
    const skillsDir = customToolSkillsDir.trim()
    if (!label || !skillsDir || savingCustomTool) return
    const nextToolValues = {
      label,
      avatar: customToolAvatar,
      skills_dir: skillsDir,
      project_skills_dir: customToolProjectDir.trim() || null,
      sync_mode: customToolSyncMode,
    }
    const customTools = editingCustomToolKey
      ? effectiveToolConfig.custom_tools.map((tool) =>
          tool.key === editingCustomToolKey
            ? { ...tool, ...nextToolValues }
            : tool,
        )
      : [
          ...effectiveToolConfig.custom_tools,
          {
            key: makeCustomToolKey(label),
            ...nextToolValues,
            enabled: true,
          } satisfies CustomToolConfigDto,
        ]
    setSavingCustomTool(true)
    try {
      const saved = await updateToolConfig({
        ...effectiveToolConfig,
        custom_tools: customTools,
      })
      if (saved) resetCustomToolForm()
    } finally {
      setSavingCustomTool(false)
    }
  }

  const handleAvatarChange = async (event: ChangeEvent<HTMLInputElement>) => {
    const file = event.currentTarget.files?.[0]
    event.currentTarget.value = ''
    if (!file) return
    setAvatarError('')
    if (!file.type.startsWith('image/')) {
      setAvatarError(t('toolManagement.avatarTypeError'))
      return
    }
    if (file.size > AVATAR_INPUT_MAX_BYTES) {
      setAvatarError(t('toolManagement.avatarSizeError'))
      return
    }
    try {
      setCustomToolAvatar(await createAvatarDataUrl(file))
    } catch {
      setAvatarError(t('toolManagement.avatarReadError'))
    }
  }

  const tools = toolStatus?.tools ?? []
  const isToolEnabled = (tool: (typeof tools)[number]) => {
    if (tool.is_custom) {
      return customToolsByKey.get(tool.key)?.enabled ?? tool.enabled
    }
    return !disabledBuiltinTools.has(tool.key)
  }
  const primaryTools = tools.filter((tool) => tool.installed || tool.is_custom)
  const missingTools = tools.filter((tool) => !tool.installed && !tool.is_custom)
  const totalCount = tools.length
  const enabledCount = tools.filter(isToolEnabled).length
  const detectedCount = tools.filter((tool) => tool.installed).length
  const customCount = tools.filter((tool) => tool.is_custom).length

  const renderToolCard = (tool: (typeof tools)[number]) => {
    const enabled = isToolEnabled(tool)
    return (
      <div
        className={`tool-card${!tool.installed ? ' missing' : ''}${editingCustomToolKey === tool.key ? ' editing' : ''}`}
        key={tool.key}
      >
        <div className="tool-card-head">
          <div className="tool-card-title">
            <ToolIcon
              toolKey={tool.key}
              label={tool.label}
              avatar={tool.avatar}
              className="tool-card-avatar"
            />
            <span className="tool-management-name">
              {t(`tools.${tool.key}`, { defaultValue: tool.label })}
            </span>
          </div>
          <div className="tool-management-actions">
            <button
              type="button"
              className={`settings-toggle${enabled ? ' checked' : ''}`}
              aria-pressed={enabled}
              onClick={() => {
                if (tool.is_custom) {
                  setCustomToolEnabled(tool.key, !enabled)
                } else {
                  setBuiltinToolEnabled(tool.key, !enabled)
                }
              }}
            >
              <span className="settings-toggle-knob" />
            </button>
          </div>
        </div>
        <div className="tool-card-badges">
          <span
            className={`tool-management-status ${
              tool.installed ? 'installed' : 'missing'
            }`}
          >
            {tool.installed
              ? t('toolManagement.detected')
              : t('toolManagement.notDetected')}
          </span>
          {tool.is_custom ? (
            <span className="tool-management-custom">
              {t('toolManagement.custom')}
            </span>
          ) : null}
          {tool.is_custom ? (
            <span className="tool-management-mode">
              {t(`toolManagement.syncModes.${tool.sync_mode ?? 'auto'}`)}
            </span>
          ) : null}
        </div>
        <div className="tool-card-path mono" title={tool.skills_dir}>
          {tool.skills_dir}
        </div>
        {tool.project_skills_dir ? (
          <div className="tool-card-path" title={tool.project_skills_dir}>
            {t('toolManagement.projectDir', {
              path: tool.project_skills_dir,
            })}
          </div>
        ) : null}
        {tool.is_custom ? (
          <div className="tool-card-footer">
            <div className="tool-card-footer-actions">
              <button
                type="button"
                className="icon-btn"
                title={t('toolManagement.editCustom')}
                aria-label={t('toolManagement.editCustom')}
                onClick={() => editCustomTool(tool.key)}
                disabled={savingCustomTool}
              >
                <Pencil size={16} />
              </button>
              <button
                type="button"
                className="icon-btn danger"
                title={t('toolManagement.removeCustom')}
                aria-label={t('toolManagement.removeCustom')}
                onClick={() =>
                  setPendingRemoveCustomTool(customToolsByKey.get(tool.key) ?? null)
                }
                disabled={savingCustomTool}
              >
                <Trash2 size={16} />
              </button>
            </div>
          </div>
        ) : null}
      </div>
    )
  }

  return (
    <div className={`tools-page${embedded ? ' embedded' : ''}`}>
      {!embedded ? (
        <div className="detail-header">
          <button className="icon-btn" type="button" onClick={onBack} aria-label={t('back')}>
            <ArrowLeft size={18} />
          </button>
          <div>
            <h2>{t('toolManagement.title')}</h2>
            <p>{t('toolManagement.pageHint')}</p>
          </div>
        </div>
      ) : null}

      <div className="tools-page-body">
        <div className="tools-summary-grid">
          <div className="tools-summary-card">
            <span>{t('toolManagement.totalCount')}</span>
            <strong>{totalCount}</strong>
          </div>
          <div className="tools-summary-card">
            <span>{t('toolManagement.enabledCount')}</span>
            <strong>{enabledCount}</strong>
          </div>
          <div className="tools-summary-card">
            <span>{t('toolManagement.detectedCount')}</span>
            <strong>{detectedCount}</strong>
          </div>
          <div className="tools-summary-card">
            <span>{t('toolManagement.customCount')}</span>
            <strong>{customCount}</strong>
          </div>
        </div>

        <div className="tools-content-grid">
        <section className="tools-panel">
          <div className="tools-panel-head">
            <div>
              <h3>{t('toolManagement.builtinSection')}</h3>
              <p>{t('toolManagement.builtinHint')}</p>
            </div>
          </div>
          <div className="tools-list">
            {tools.length > 0 ? (
              <div className="tools-card-grid">
                {primaryTools.length > 0 ? (
                  primaryTools.map(renderToolCard)
                ) : (
                  <div className="settings-helper">
                    {t('toolManagement.noDetectedTools')}
                  </div>
                )}
              </div>
            ) : (
              <div className="settings-helper">{t('detectingTools')}</div>
            )}
          </div>
          {missingTools.length > 0 ? (
            <div className="tools-missing-section">
              <button
                className="tools-missing-toggle"
                type="button"
                onClick={() => setShowMissingTools((open) => !open)}
                aria-expanded={showMissingTools}
              >
                <span>
                  {t('toolManagement.missingSection', {
                    count: missingTools.length,
                  })}
                </span>
                <ChevronDown
                  size={16}
                  className={showMissingTools ? 'expanded' : ''}
                />
              </button>
              {showMissingTools ? (
                <div className="tools-card-grid missing-tools-grid">
                  {missingTools.map(renderToolCard)}
                </div>
              ) : null}
            </div>
          ) : null}
        </section>

        <section className="tools-panel" ref={customToolFormRef}>
          <div className="tools-panel-head">
            <div>
              <h3>
                {editingCustomToolKey
                  ? t('toolManagement.editCustomTitle')
                  : t('toolManagement.customSection')}
              </h3>
              <p>
                {editingCustomToolKey
                  ? t('toolManagement.editCustomHint')
                  : t('toolManagement.customHint')}
              </p>
            </div>
          </div>
          <div className="tool-management-add tools-add-grid">
            <div className="custom-tool-avatar-field">
              <button
                className="custom-tool-avatar-picker"
                type="button"
                onClick={() => avatarInputRef.current?.click()}
                aria-label={t('toolManagement.chooseAvatar')}
              >
                <ToolIcon
                  toolKey="custom_preview"
                  label={customToolName || t('toolManagement.avatarFallback')}
                  avatar={customToolAvatar}
                  className="custom-tool-avatar-preview"
                />
                <span className="custom-tool-avatar-edit" aria-hidden="true">
                  <ImagePlus size={13} />
                </span>
              </button>
              <div className="custom-tool-avatar-copy">
                <strong>{t('toolManagement.avatarLabel')}</strong>
                <small>{t('toolManagement.avatarHint')}</small>
                <div className="custom-tool-avatar-actions">
                  <button
                    className="btn btn-secondary custom-tool-avatar-action"
                    type="button"
                    onClick={() => avatarInputRef.current?.click()}
                  >
                    <ImagePlus size={14} />
                    {t('toolManagement.chooseAvatar')}
                  </button>
                  {customToolAvatar ? (
                    <button
                      className="btn btn-ghost custom-tool-avatar-action"
                      type="button"
                      onClick={() => {
                        setCustomToolAvatar(null)
                        setAvatarError('')
                      }}
                    >
                      <X size={14} />
                      {t('toolManagement.removeAvatar')}
                    </button>
                  ) : null}
                </div>
                {avatarError ? (
                  <small className="custom-tool-avatar-error" role="alert">
                    {avatarError}
                  </small>
                ) : null}
              </div>
              <input
                ref={avatarInputRef}
                className="custom-tool-avatar-input"
                type="file"
                accept="image/png,image/jpeg,image/webp,image/gif,image/svg+xml"
                tabIndex={-1}
                onChange={handleAvatarChange}
              />
            </div>
            <input
              ref={customToolNameInputRef}
              className="settings-input"
              value={customToolName}
              placeholder={t('toolManagement.namePlaceholder')}
              aria-label={t('toolManagement.namePlaceholder')}
              onChange={(event) => setCustomToolName(event.target.value)}
            />
            <input
              className="settings-input mono"
              value={customToolSkillsDir}
              placeholder={t('toolManagement.skillsDirPlaceholder')}
              aria-label={t('toolManagement.skillsDirPlaceholder')}
              onChange={(event) => setCustomToolSkillsDir(event.target.value)}
            />
            <input
              className="settings-input mono"
              value={customToolProjectDir}
              placeholder={t('toolManagement.projectDirPlaceholder')}
              aria-label={t('toolManagement.projectDirPlaceholder')}
              onChange={(event) => setCustomToolProjectDir(event.target.value)}
            />
            <label className="custom-tool-mode-field">
              <span>{t('toolManagement.syncModeLabel')}</span>
              <div className="settings-select-wrap">
                <select
                  className="settings-select"
                  value={customToolSyncMode}
                  onChange={(event) =>
                    setCustomToolSyncMode(event.target.value as SyncMode)
                  }
                >
                  <option value="auto">{t('toolManagement.syncModes.auto')}</option>
                  <option value="symlink">
                    {t('toolManagement.syncModes.symlink')}
                  </option>
                  <option value="junction">
                    {t('toolManagement.syncModes.junction')}
                  </option>
                  <option value="copy">{t('toolManagement.syncModes.copy')}</option>
                </select>
                <ChevronDown className="settings-select-caret" aria-hidden="true" />
              </div>
              <small>
                {t(`toolManagement.syncModeHints.${customToolSyncMode}`)}
              </small>
            </label>
            {editingCustomToolKey ? (
              <div className="custom-tool-form-actions">
                <button
                  className="btn btn-secondary"
                  type="button"
                  onClick={resetCustomToolForm}
                  disabled={savingCustomTool}
                >
                  {t('cancel')}
                </button>
                <button
                  className="btn btn-primary tool-management-add-btn"
                  type="button"
                  onClick={() => void saveCustomTool()}
                  disabled={
                    savingCustomTool ||
                    !customToolName.trim() ||
                    !customToolSkillsDir.trim()
                  }
                >
                  <Save size={16} />
                  {t('toolManagement.saveCustom')}
                </button>
              </div>
            ) : (
              <button
                className="btn btn-primary tool-management-add-btn"
                type="button"
                onClick={() => void saveCustomTool()}
                disabled={
                  savingCustomTool ||
                  !customToolName.trim() ||
                  !customToolSkillsDir.trim()
                }
              >
                <Plus size={16} />
                {t('toolManagement.addCustom')}
              </button>
            )}
          </div>
        </section>
        </div>
      </div>
      <ConfirmActionModal
        open={Boolean(pendingRemoveCustomTool)}
        loading={savingCustomTool}
        title={t('toolManagement.removeCustomTitle')}
        body={t('toolManagement.removeCustomBody', {
          name: pendingRemoveCustomTool?.label ?? '',
          path: pendingRemoveCustomTool?.skills_dir ?? '',
        })}
        cancelLabel={t('cancel')}
        confirmLabel={t('toolManagement.removeCustomConfirm')}
        onRequestClose={() => setPendingRemoveCustomTool(null)}
        onConfirm={() => void removeCustomTool()}
      />
    </div>
  )
}

export default memo(ToolsPage)
