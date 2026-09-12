import { memo, useMemo, useState } from 'react'
import { Layers3, RotateCcw, X } from 'lucide-react'
import type { TFunction } from 'i18next'
import type { TagWithCountDto, ToolOption } from './types'
import type { TemporaryActivationEntry, TemporaryActivationScope } from './temporaryActivation'

type TemporaryActivationModalProps = {
  tools: ToolOption[]
  tags: TagWithCountDto[]
  activeEntries: TemporaryActivationEntry[]
  defaultToolId?: string
  defaultScope?: TemporaryActivationScope
  recentProjects: string[]
  loading: boolean
  onApply: (input: {
    toolIds: string[]
    scope: TemporaryActivationScope
    projectPath?: string
    tagIds: Array<number | null>
  }) => void
  onRestore: (entry: TemporaryActivationEntry) => void
  onRequestClose: () => void
  t: TFunction
}

const TemporaryActivationModal = ({
  tools,
  tags,
  activeEntries,
  defaultToolId,
  defaultScope = 'global',
  recentProjects,
  loading,
  onApply,
  onRestore,
  onRequestClose,
  t,
}: TemporaryActivationModalProps) => {
  const [toolIds, setToolIds] = useState<string[]>(defaultToolId ? [defaultToolId] : tools[0]?.id ? [tools[0].id] : [])
  const [scope, setScope] = useState<TemporaryActivationScope>(defaultScope)
  const [projectPath, setProjectPath] = useState(recentProjects[0] ?? '')
  const [tagIds, setTagIds] = useState<string[]>([])
  const selectedEntry = useMemo(
    () =>
      activeEntries.find(
        (entry) =>
          (entry.toolIds ?? [entry.toolId]).some((id) => toolIds.includes(id)) &&
          entry.scope === scope &&
          (scope === 'global' || entry.projectPath === projectPath),
      ),
    [activeEntries, projectPath, scope, toolIds],
  )

  const toggle = (list: string[], value: string) =>
    list.includes(value) ? list.filter((item) => item !== value) : [...list, value]

  return (
    <div className="modal-backdrop" onClick={loading ? undefined : onRequestClose}>
      <div className="modal temporary-activation-modal" onClick={(event) => event.stopPropagation()} role="dialog" aria-modal="true">
        <div className="modal-header">
          <div>
            <div className="modal-title"><Layers3 size={16} />{t('temporaryActivation.title')}</div>
            <div className="modal-subtitle">{t('temporaryActivation.subtitle')}</div>
          </div>
          <button className="modal-close" type="button" onClick={onRequestClose} aria-label={t('close')}><X size={17} /></button>
        </div>
        <div className="modal-body temporary-activation-body">
          <fieldset>
            <legend>{t('temporaryActivation.tool')}</legend>
            <div className="temporary-activation-checks">
              {tools.map((tool) => (
                <label key={tool.id}>
                  <input
                    type="checkbox"
                    checked={toolIds.includes(tool.id)}
                    onChange={() => setToolIds((current) => toggle(current, tool.id))}
                  />
                  {tool.label}
                </label>
              ))}
            </div>
          </fieldset>
          <label>
            {t('temporaryActivation.scope')}
            <select className="search-input" value={scope} onChange={(event) => setScope(event.target.value as TemporaryActivationScope)}>
              <option value="global">{t('scope.global')}</option>
              <option value="project">{t('scope.project')}</option>
            </select>
          </label>
          {scope === 'project' ? (
            <label>
              {t('temporaryActivation.project')}
              <select className="search-input" value={projectPath} onChange={(event) => setProjectPath(event.target.value)}>
                <option value="">{t('temporaryActivation.projectPlaceholder')}</option>
                {recentProjects.map((project) => <option key={project} value={project}>{project}</option>)}
              </select>
            </label>
          ) : null}
          <fieldset>
            <legend>{t('temporaryActivation.category')}</legend>
            <div className="temporary-activation-checks">
              <label>
                <input
                  type="checkbox"
                  checked={tagIds.includes('')}
                  onChange={() => setTagIds((current) => toggle(current, ''))}
                />
                {t('temporaryActivation.untagged')}
              </label>
              {tags.map((tag) => (
                <label key={tag.id}>
                  <input
                    type="checkbox"
                    checked={tagIds.includes(String(tag.id))}
                    onChange={() => setTagIds((current) => toggle(current, String(tag.id)))}
                  />
                  {tag.name}（{tag.skill_count}）
                </label>
              ))}
            </div>
          </fieldset>
          <div className="temporary-activation-hint">{t('temporaryActivation.hint')}</div>
          {selectedEntry ? (
            <div className="temporary-activation-current">
              <span>{t('temporaryActivation.active')}</span>
              <button className="btn btn-secondary" type="button" disabled={loading} onClick={() => onRestore(selectedEntry)}>
                <RotateCcw size={14} />{t('temporaryActivation.restore')}
              </button>
            </div>
          ) : null}
        </div>
        <div className="modal-footer">
          <button className="btn btn-secondary" type="button" onClick={onRequestClose} disabled={loading}>{t('cancel')}</button>
          <button
            className="btn btn-primary"
            type="button"
            onClick={() =>
              onApply({
                toolIds,
                scope,
                projectPath: scope === 'project' ? projectPath : undefined,
                tagIds: tagIds.map((value) => (value ? Number(value) : null)),
              })
            }
            disabled={loading || toolIds.length === 0 || tagIds.length === 0 || (scope === 'project' && !projectPath)}
          >
            <Layers3 size={14} />{t('temporaryActivation.apply')}
          </button>
        </div>
      </div>
    </div>
  )
}

export default memo(TemporaryActivationModal)
