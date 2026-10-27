import { useEffect, useId, useMemo, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import { Check, ChevronDown, Folder, FolderOpen, Search } from 'lucide-react'
import type { TFunction } from 'i18next'
import './project-picker.css'

type ProjectPickerProps = {
  projects: string[]
  value: string
  onChange: (path: string) => void
  onBrowse: () => void
  t: TFunction
}

const projectName = (path: string) => path.replace(/[\\/]+$/, '').split(/[\\/]/).pop() || path

export default function ProjectPicker({ projects, value, onChange, onBrowse, t }: ProjectPickerProps) {
  const [position, setPosition] = useState<{ left: number; top: number; width: number; maxHeight: number } | null>(null)
  const [query, setQuery] = useState('')
  const [activeIndex, setActiveIndex] = useState(0)
  const trigger = useRef<HTMLButtonElement>(null)
  const popup = useRef<HTMLDivElement>(null)
  const search = useRef<HTMLInputElement>(null)
  const browse = useRef<HTMLButtonElement>(null)
  const id = useId()
  const allOptions = useMemo(() => [
    { path: '', name: t('workspace110.allProjects') },
    ...[...new Set(projects)].filter(Boolean).map(path => ({ path, name: projectName(path) })),
  ], [projects, t])
  const options = useMemo(() => allOptions.filter(option => `${option.name} ${option.path}`.toLocaleLowerCase().includes(query.trim().toLocaleLowerCase())), [allOptions, query])
  const activeOption = options[Math.min(activeIndex, options.length - 1)]

  const close = () => { setPosition(null); trigger.current?.focus() }
  const select = (path: string) => { onChange(path); close() }

  useEffect(() => {
    if (!position) return
    search.current?.focus()
    const dismiss = (event: PointerEvent | FocusEvent) => {
      if (event.target instanceof Node && !popup.current?.contains(event.target) && !trigger.current?.contains(event.target)) setPosition(null)
    }
    const reposition = (event: Event) => {
      if (event.target instanceof Node && popup.current?.contains(event.target)) return
      setPosition(null)
    }
    document.addEventListener('pointerdown', dismiss)
    document.addEventListener('focusin', dismiss)
    window.addEventListener('resize', reposition)
    window.addEventListener('scroll', reposition, true)
    return () => {
      document.removeEventListener('pointerdown', dismiss)
      document.removeEventListener('focusin', dismiss)
      window.removeEventListener('resize', reposition)
      window.removeEventListener('scroll', reposition, true)
    }
  }, [position])

  useEffect(() => {
    if (position && activeOption) document.getElementById(`${id}-option-${options.indexOf(activeOption)}`)?.scrollIntoView?.({ block: 'nearest' })
  }, [position, activeOption, id, options])

  return <>
    <button ref={trigger} className="project-picker-trigger" type="button" aria-label={t('workspace110.project')}
      aria-haspopup="dialog" aria-expanded={Boolean(position)} aria-controls={position ? id : undefined} title={value || undefined}
      onClick={() => {
        if (position) { setPosition(null); return }
        const rect = trigger.current!.getBoundingClientRect()
        const width = Math.min(400, window.innerWidth - 24)
        const top = rect.bottom + 8
        setQuery('')
        setActiveIndex(Math.max(0, allOptions.findIndex(option => option.path === value)))
        setPosition({ left: Math.max(12, Math.min(rect.left, window.innerWidth - width - 12)), top, width, maxHeight: Math.min(380, window.innerHeight - top - 12) })
      }}>
      <Folder size={16} aria-hidden="true" />
      <span>{value ? projectName(value) : t('workspace110.allProjects')}</span>
      <ChevronDown size={14} aria-hidden="true" />
    </button>
    {position ? createPortal(<div ref={popup} id={id} className="project-picker-popover" role="dialog" aria-label={t('workspace110.project')} style={position}
      onKeyDown={event => {
        if (event.nativeEvent.isComposing || event.keyCode === 229) return
        if (event.key === 'Escape') { event.preventDefault(); event.stopPropagation(); close() }
        if (event.key === 'Tab' && event.shiftKey && event.target === search.current) { event.preventDefault(); close() }
        if (event.key === 'Tab' && !event.shiftKey && event.target === browse.current) close()
      }}>
      <div className="project-picker-search">
        <Search size={15} aria-hidden="true" />
        <input ref={search} role="combobox" aria-label={t('workspace110.searchProjects')} placeholder={t('workspace110.searchProjects')}
          aria-autocomplete="list" aria-expanded="true" aria-controls={`${id}-list`} aria-activedescendant={activeOption ? `${id}-option-${options.indexOf(activeOption)}` : undefined}
          value={query} onChange={event => { setQuery(event.target.value); setActiveIndex(0) }}
          onKeyDown={event => {
            if (event.nativeEvent.isComposing || event.keyCode === 229) return
            if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
              event.preventDefault()
              setActiveIndex(index => options.length ? (index + (event.key === 'ArrowDown' ? 1 : -1) + options.length) % options.length : 0)
            }
            if (event.key === 'Enter' && activeOption) { event.preventDefault(); select(activeOption.path) }
          }} />
      </div>
      <div className="project-picker-options" role="listbox" id={`${id}-list`} aria-label={t('workspace110.project')}>
        {options.map((option, index) => <button key={option.path} id={`${id}-option-${index}`} type="button" role="option" tabIndex={-1}
          aria-selected={option.path === value} className={`project-picker-option${option === activeOption ? ' is-active' : ''}`}
          onClick={() => select(option.path)}>
          <Folder size={16} aria-hidden="true" />
          <span><strong>{option.name}</strong>{option.path ? <small>{option.path}</small> : null}</span>
          {option.path === value ? <Check size={15} aria-hidden="true" /> : null}
        </button>)}
        {!options.length ? <p className="project-picker-empty" role="status">{t('workspace110.noProjectResults')}</p> : null}
      </div>
      <button ref={browse} type="button" className="project-picker-browse" onClick={() => { close(); onBrowse() }}>
        <FolderOpen size={16} aria-hidden="true" />{t('workspace110.chooseProject')}
      </button>
    </div>, document.body) : null}
  </>
}
