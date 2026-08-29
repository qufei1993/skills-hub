import { memo, useCallback, useEffect, useMemo, useState } from 'react'
import {
  ArrowLeft,
  ChevronDown,
  ChevronRight,
  Clock,
  Copy,
  File,
  Folder,
  FolderOpen,
  GitBranch,
  Globe2,
} from 'lucide-react'
import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter'
import {
  oneLight,
  oneDark,
} from 'react-syntax-highlighter/dist/esm/styles/prism'
import Markdown from 'react-markdown'
import remarkFrontmatter from 'remark-frontmatter'
import remarkGfm from 'remark-gfm'
import { toast } from 'sonner'
import type { TFunction } from 'i18next'
import { supportsRegExpLookbehind } from './markdownCompatibility'
import { getVisibleFrontmatterEntries } from './skillDetailMetadata'
import {
  getFullySyncedTools,
  getSkillSyncState,
} from './skillSyncStatus'
import ToolIcon from './ToolIcon'
import type { ManagedSkill, SkillFileEntry, ToolOption } from './types'

// ─── Types ───────────────────────────────────────────
type SkillDetailViewProps = {
  skill: ManagedSkill
  onBack: () => void
  invokeTauri: <T>(command: string, args?: Record<string, unknown>) => Promise<T>
  formatRelative: (ms: number | null | undefined) => string
  tools: ToolOption[]
  scope: 'global' | 'project'
  projects: string[]
  t: TFunction
}

type TreeNode = {
  name: string
  path: string // full relative path for files, folder prefix for dirs
  isDir: boolean
  size: number
  children: TreeNode[]
}

const MARKDOWN_REMARK_PLUGINS = supportsRegExpLookbehind()
  ? [remarkFrontmatter, remarkGfm]
  : [remarkFrontmatter]

// ─── Helpers ─────────────────────────────────────────
function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
}

function compactSourcePath(path: string): string {
  if (path.length <= 64) return path
  const separator = path.includes('\\') ? '\\' : '/'
  const prefix = path.startsWith(separator) ? separator : ''
  const parts = path.split(/[\\/]/).filter(Boolean)
  if (parts.length <= 5) return path
  return `${prefix}${parts.slice(0, 3).join(separator)}${separator}…${separator}${parts.slice(-2).join(separator)}`
}

const EXT_LANG: Record<string, string> = {
  ts: 'typescript',
  tsx: 'tsx',
  js: 'javascript',
  jsx: 'jsx',
  py: 'python',
  rs: 'rust',
  go: 'go',
  rb: 'ruby',
  java: 'java',
  kt: 'kotlin',
  swift: 'swift',
  c: 'c',
  cpp: 'cpp',
  h: 'c',
  hpp: 'cpp',
  cs: 'csharp',
  css: 'css',
  scss: 'scss',
  less: 'less',
  html: 'html',
  xml: 'xml',
  json: 'json',
  yaml: 'yaml',
  yml: 'yaml',
  toml: 'toml',
  sh: 'bash',
  bash: 'bash',
  zsh: 'bash',
  sql: 'sql',
  graphql: 'graphql',
  dockerfile: 'docker',
  lua: 'lua',
  r: 'r',
  dart: 'dart',
  php: 'php',
  pl: 'perl',
  ex: 'elixir',
  exs: 'elixir',
  erl: 'erlang',
  hs: 'haskell',
  vim: 'vim',
  ini: 'ini',
  cfg: 'ini',
  diff: 'diff',
  patch: 'diff',
}

function getLang(filename: string): string {
  const lower = filename.toLowerCase()
  if (lower === 'dockerfile' || lower.startsWith('dockerfile.')) return 'docker'
  if (lower === 'makefile' || lower === 'gnumakefile') return 'makefile'
  const ext = lower.split('.').pop() ?? ''
  return EXT_LANG[ext] ?? ''
}

function isMarkdown(filename: string): boolean {
  return /\.(md|mdx|markdown)$/i.test(filename)
}

/** Build a tree from flat file paths */
function buildTree(files: SkillFileEntry[]): TreeNode[] {
  const root: TreeNode[] = []

  for (const f of files) {
    const parts = f.path.split('/')
    let current = root
    for (let i = 0; i < parts.length; i++) {
      const name = parts[i]
      const isLast = i === parts.length - 1
      if (isLast) {
        current.push({
          name,
          path: f.path,
          isDir: false,
          size: f.size,
          children: [],
        })
      } else {
        let dir = current.find((n) => n.isDir && n.name === name)
        if (!dir) {
          dir = {
            name,
            path: parts.slice(0, i + 1).join('/'),
            isDir: true,
            size: 0,
            children: [],
          }
          current.push(dir)
        }
        current = dir.children
      }
    }
  }

  // Sort: dirs first (alphabetical), then files (SKILL.md first, then alphabetical)
  const sortNodes = (nodes: TreeNode[]) => {
    nodes.sort((a, b) => {
      if (a.isDir !== b.isDir) return a.isDir ? -1 : 1
      if (!a.isDir && !b.isDir) {
        const aSkill = a.name.toLowerCase() === 'skill.md'
        const bSkill = b.name.toLowerCase() === 'skill.md'
        if (aSkill !== bSkill) return aSkill ? -1 : 1
      }
      return a.name.localeCompare(b.name)
    })
    for (const n of nodes) {
      if (n.isDir) sortNodes(n.children)
    }
  }
  sortNodes(root)
  return root
}

// ─── FileTreeNode component ─────────────────────────
type FileTreeNodeProps = {
  node: TreeNode
  depth: number
  activeFile: string | null
  expanded: Set<string>
  onToggleDir: (path: string) => void
  onSelectFile: (path: string) => void
}

const FileTreeNode = memo(
  ({
    node,
    depth,
    activeFile,
    expanded,
    onToggleDir,
    onSelectFile,
  }: FileTreeNodeProps) => {
    if (node.isDir) {
      const isOpen = expanded.has(node.path)
      return (
        <>
          <button
            type="button"
            className="tree-item tree-dir"
            style={{ paddingLeft: 12 + depth * 16 }}
            onClick={() => onToggleDir(node.path)}
          >
            <span className="tree-chevron">
              {isOpen ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
            </span>
            {isOpen ? (
              <FolderOpen size={14} className="tree-icon tree-icon-folder" />
            ) : (
              <Folder size={14} className="tree-icon tree-icon-folder" />
            )}
            <span className="tree-name">{node.name}</span>
          </button>
          {isOpen
            ? node.children.map((child) => (
                <FileTreeNode
                  key={child.path}
                  node={child}
                  depth={depth + 1}
                  activeFile={activeFile}
                  expanded={expanded}
                  onToggleDir={onToggleDir}
                  onSelectFile={onSelectFile}
                />
              ))
            : null}
        </>
      )
    }

    return (
      <button
        type="button"
        className={`tree-item tree-file${activeFile === node.path ? ' active' : ''}`}
        style={{ paddingLeft: 12 + depth * 16 + 18 }}
        onClick={() => onSelectFile(node.path)}
      >
        <File size={14} className="tree-icon tree-icon-file" />
        <span className="tree-name">{node.name}</span>
        <span className="tree-size">{formatSize(node.size)}</span>
      </button>
    )
  },
)
FileTreeNode.displayName = 'FileTreeNode'

// ─── FileContent renderer ────────────────────────────
type FileContentRendererProps = {
  filename: string
  content: string
  isDark: boolean
}

function parseFrontmatter(raw: string): {
  meta: Record<string, string> | null
  body: string
} {
  if (!raw.startsWith('---')) return { meta: null, body: raw }
  const end = raw.indexOf('\n---', 3)
  if (end === -1) return { meta: null, body: raw }
  const block = raw.slice(4, end)
  const entries: Record<string, string> = {}
  const lines = block.split('\n')
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i]
    const idx = line.indexOf(':')
    if (idx === -1) continue
    const key = line.slice(0, idx).trim()
    let val = line.slice(idx + 1).trim()
    const blockStyle = val.match(/^([>|])[-+]?$/)?.[1]
    if (blockStyle) {
      const blockLines: string[] = []
      while (i + 1 < lines.length) {
        const next = lines[i + 1]
        if (next.trim() !== '' && !/^\s/.test(next)) break
        blockLines.push(next.replace(/^\s{2}/, ''))
        i++
      }
      val =
        blockStyle === '|'
          ? blockLines.join('\n').trim()
          : blockLines.map((v) => v.trim()).filter(Boolean).join(' ')
    }
    // strip surrounding quotes
    if (
      val.length >= 2 &&
      ((val[0] === '"' && val[val.length - 1] === '"') ||
        (val[0] === "'" && val[val.length - 1] === "'"))
    ) {
      val = val.slice(1, -1)
    }
    if (key) entries[key] = val
  }
  const keys = Object.keys(entries)
  if (keys.length === 0) return { meta: null, body: raw }
  const body = raw.slice(end + 4).replace(/^\n+/, '')
  return { meta: entries, body }
}

const FileContentRenderer = memo(
  ({ filename, content, isDark }: FileContentRendererProps) => {
    if (isMarkdown(filename)) {
      const { meta, body } = parseFrontmatter(content)
      const visibleMetaEntries = getVisibleFrontmatterEntries(meta)
      return (
        <div className="markdown-body">
          {visibleMetaEntries.length > 0 ? (
            <dl className="frontmatter-meta">
              {visibleMetaEntries.map(([key, value]) => (
                <div
                  className="frontmatter-meta-item"
                  data-key={key}
                  key={key}
                >
                  <dt>{key}</dt>
                  <dd>{value}</dd>
                </div>
              ))}
            </dl>
          ) : null}
          <Markdown
            remarkPlugins={MARKDOWN_REMARK_PLUGINS}
            components={{
              code: ({ className, children, ...rest }) => {
                const match = /language-(\w+)/.exec(className ?? '')
                const inline = !match
                if (inline) {
                  return (
                    <code className="md-inline-code" {...rest}>
                      {children}
                    </code>
                  )
                }
                return (
                  <SyntaxHighlighter
                    style={isDark ? oneDark : oneLight}
                    language={match[1]}
                    PreTag="div"
                    customStyle={{
                      margin: 0,
                      borderRadius: 6,
                      fontSize: 13,
                    }}
                  >
                    {String(children).replace(/\n$/, '')}
                  </SyntaxHighlighter>
                )
              },
            }}
          >
            {body}
          </Markdown>
        </div>
      )
    }

    const lang = getLang(filename)
    if (lang) {
      return (
        <SyntaxHighlighter
          style={isDark ? oneDark : oneLight}
          language={lang}
          showLineNumbers
          lineNumberStyle={{
            minWidth: '3em',
            paddingRight: '1em',
            color: isDark ? '#636d83' : '#9ca3af',
            userSelect: 'none',
          }}
          customStyle={{
            margin: 0,
            padding: '16px 0',
            background: 'transparent',
            fontSize: 13,
            lineHeight: 1.7,
          }}
        >
          {content}
        </SyntaxHighlighter>
      )
    }

    // Plain text with line numbers
    return (
      <SyntaxHighlighter
        style={isDark ? oneDark : oneLight}
        language="text"
        showLineNumbers
        lineNumberStyle={{
          minWidth: '3em',
          paddingRight: '1em',
          color: isDark ? '#636d83' : '#9ca3af',
          userSelect: 'none',
        }}
        customStyle={{
          margin: 0,
          padding: '16px 0',
          background: 'transparent',
          fontSize: 13,
          lineHeight: 1.7,
        }}
      >
        {content}
      </SyntaxHighlighter>
    )
  },
)
FileContentRenderer.displayName = 'FileContentRenderer'

// ─── Main component ──────────────────────────────────
const SkillDetailView = ({
  skill,
  onBack,
  invokeTauri,
  formatRelative,
  tools,
  scope,
  projects,
  t,
}: SkillDetailViewProps) => {
  const [files, setFiles] = useState<SkillFileEntry[]>([])
  const [activeFile, setActiveFile] = useState<string | null>(null)
  const [fileContent, setFileContent] = useState('')
  const [loadingFiles, setLoadingFiles] = useState(true)
  const [loadingContent, setLoadingContent] = useState(false)
  const [expanded, setExpanded] = useState<Set<string>>(new Set())

  const isDark =
    document.documentElement.getAttribute('data-theme') === 'dark'

  const tree = useMemo(() => buildTree(files), [files])

  useEffect(() => {
    let cancelled = false
    const load = async () => {
      setLoadingFiles(true)
      try {
        const result = await invokeTauri<SkillFileEntry[]>('list_skill_files', {
          centralPath: skill.central_path,
        })
        if (cancelled) return
        setFiles(result)
        // Start with all folders collapsed
        setExpanded(new Set())
        if (result.length > 0) {
          setActiveFile(result[0].path)
        }
      } catch {
        if (!cancelled) {
          toast.error(t('detail.readError'))
        }
      } finally {
        if (!cancelled) setLoadingFiles(false)
      }
    }
    void load()
    return () => {
      cancelled = true
    }
  }, [invokeTauri, skill.central_path, t])

  useEffect(() => {
    if (!activeFile) return
    let cancelled = false
    const load = async () => {
      setLoadingContent(true)
      try {
        const content = await invokeTauri<string>('read_skill_file', {
          centralPath: skill.central_path,
          filePath: activeFile,
        })
        if (!cancelled) setFileContent(content)
      } catch (err) {
        if (!cancelled) {
          const msg = err instanceof Error ? err.message : String(err)
          setFileContent(msg)
        }
      } finally {
        if (!cancelled) setLoadingContent(false)
      }
    }
    void load()
    return () => {
      cancelled = true
    }
  }, [activeFile, invokeTauri, skill.central_path])

  const handleSelectFile = useCallback((path: string) => {
    setActiveFile(path)
  }, [])

  const handleToggleDir = useCallback((path: string) => {
    setExpanded((prev) => {
      const next = new Set(prev)
      if (next.has(path)) {
        next.delete(path)
      } else {
        next.add(path)
      }
      return next
    })
  }, [])

  const isGitSource = skill.source_type.toLowerCase().includes('git')
  const sourceValue = skill.source_ref?.trim() || skill.central_path
  const sourceLabel = isGitSource
    ? sourceValue.replace(/^https?:\/\/(www\.)?github\.com\//, '')
    : compactSourcePath(sourceValue)

  const SourceIcon = isGitSource ? GitBranch : Folder

  const handleCopySource = useCallback(async () => {
    try {
      await navigator.clipboard.writeText(sourceValue)
      toast.success(t(isGitSource ? 'detail.sourceCopied' : 'detail.pathCopied'))
    } catch {
      toast.error(t('copyFailed'))
    }
  }, [isGitSource, sourceValue, t])

  const syncedTools = useMemo(() => {
    return getFullySyncedTools(skill, tools, scope)
  }, [scope, skill, tools])

  const visibleTools = syncedTools.slice(0, 3)
  const hiddenToolCount = syncedTools.length - visibleTools.length
  const visibleTags = skill.tags.slice(0, 3)
  const hiddenTagCount = skill.tags.length - visibleTags.length
  const projectNames = projects.map((projectPath) => {
    const parts = projectPath.split(/[\\/]/).filter(Boolean)
    return parts.at(-1) ?? projectPath
  })
  const scopeLabel =
    scope === 'global'
      ? t('scope.globalBadge')
      : projectNames.length === 1
        ? projectNames[0]
        : t('scope.projectCount', { count: projectNames.length })
  const syncState = getSkillSyncState(skill)
  const syncStatus = {
    disabled: t('detail.syncDisabled'),
    'source-error': t('detail.sourceError'),
    healthy: t('detail.syncHealthy'),
    partial: t('detail.syncPartialFailed'),
    failed: t('detail.syncFailed'),
    idle: t('detail.notSynced'),
  }[syncState]

  return (
    <div className="detail-view">
      <div className="detail-header">
        <button className="detail-back-btn" type="button" onClick={onBack}>
          <ArrowLeft size={16} />
          {t('detail.back')}
        </button>
        <div className="detail-summary">
          <div className="detail-title-row">
            <div className="detail-skill-name">{skill.name}</div>
            <span className={`detail-sync-status ${syncState}`}>
              <i aria-hidden="true" />
              {syncStatus}
            </span>
          </div>
          {skill.description ? (
            <p className="detail-desc">{skill.description}</p>
          ) : null}
        </div>
        <div className="detail-metadata-rail">
          <div className="detail-context-row">
            <span
              className={`detail-scope ${scope}`}
              title={scope === 'project' ? projects.join('\n') : undefined}
            >
              {scope === 'global' ? <Globe2 size={13} /> : <Folder size={13} />}
              {scopeLabel}
            </span>

            {visibleTools.length > 0 ? (
              <div className="detail-context-group">
                <span className="detail-context-label">{t('detail.syncedTo')}</span>
                <div className="detail-tool-list" title={syncedTools.map((tool) => tool.label).join(', ')}>
                  {visibleTools.map((tool) => (
                    <span className="detail-tool-item" key={tool.id}>
                      <ToolIcon
                        toolKey={tool.id}
                        label={tool.label}
                        avatar={tool.avatar}
                      />
                      <span>{tool.label}</span>
                    </span>
                  ))}
                  {hiddenToolCount > 0 ? (
                    <span
                      className="detail-overflow-text"
                      title={t('detail.moreTools', { count: hiddenToolCount })}
                    >
                      +{hiddenToolCount}
                    </span>
                  ) : null}
                </div>
              </div>
            ) : null}

            {visibleTags.length > 0 ? (
              <div className="detail-tag-list">
                {visibleTags.map((tag) => (
                  <span className="detail-tag" key={tag.id}>
                    # {tag.name}
                  </span>
                ))}
                {hiddenTagCount > 0 ? (
                  <span
                    className="detail-tag"
                    title={t('detail.moreTags', { count: hiddenTagCount })}
                  >
                    +{hiddenTagCount}
                  </span>
                ) : null}
              </div>
            ) : null}
          </div>
          <div className="detail-provenance-row">
            <button
              className="detail-source-copy"
              type="button"
              title={sourceValue}
              aria-label={`${isGitSource ? t('detail.copySource') : t('detail.copyPath')}：${sourceValue}`}
              onClick={() => void handleCopySource()}
            >
              <SourceIcon size={13} />
              <span className="detail-source-text">{sourceLabel}</span>
              <span className="detail-copy-action" aria-hidden="true">
                <Copy size={13} />
              </span>
            </button>
            <span className="detail-meta-item">
              <Clock size={13} />
              {formatRelative(skill.updated_at)}
            </span>
            <span className="detail-meta-item">
              <File size={13} />
              {t('detail.fileCount', { count: files.length })}
            </span>
          </div>
        </div>
      </div>

      <div className="detail-body">
        <div className="detail-file-list">
          <div className="file-list-title">{t('detail.files')}</div>
          {loadingFiles ? (
            <div className="detail-loading">
              <div className="detail-spinner" />
              {t('detail.loadingFiles')}
            </div>
          ) : files.length === 0 ? (
            <div className="detail-loading">{t('detail.noFiles')}</div>
          ) : (
            <div className="file-tree">
              {tree.map((node) => (
                <FileTreeNode
                  key={node.path}
                  node={node}
                  depth={0}
                  activeFile={activeFile}
                  expanded={expanded}
                  onToggleDir={handleToggleDir}
                  onSelectFile={handleSelectFile}
                />
              ))}
            </div>
          )}
        </div>

        <div className="detail-file-content">
          {activeFile ? (
            <>
              <div className="file-content-header">
                <span className="file-content-path">
                  <File size={14} />
                  {activeFile}
                </span>
                <span className="file-content-size">
                  {formatSize(
                    files.find((f) => f.path === activeFile)?.size ?? 0,
                  )}
                </span>
              </div>
              {loadingContent ? (
                <div className="detail-loading" style={{ height: 200 }}>
                  <div className="detail-spinner" />
                  {t('detail.loadingContent')}
                </div>
              ) : (
                <div className="file-content-body">
                  <FileContentRenderer
                    filename={activeFile}
                    content={fileContent}
                    isDark={isDark}
                  />
                </div>
              )}
            </>
          ) : (
            <div className="detail-loading" style={{ height: 200 }}>
              {loadingFiles ? t('detail.loadingFiles') : t('detail.noFiles')}
            </div>
          )}
        </div>
      </div>
    </div>
  )
}

export default memo(SkillDetailView)
