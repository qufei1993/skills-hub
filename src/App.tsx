import { useCallback, useEffect, useMemo, useRef, useState, type MutableRefObject } from 'react'
import type { Update } from '@tauri-apps/plugin-updater'
import './App.css'
import { useTranslation } from 'react-i18next'
import { Toaster, toast } from 'sonner'
import Markdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import ExplorePage from './components/skills/ExplorePage'
import MoreSkillsPage from './components/skills/MoreSkillsPage'
import { EXPLORE_ENABLED } from './features'
import { openExternalUrl } from './utils/openExternalUrl'
import FilterBar from './components/skills/FilterBar'
import SkillDetailView from './components/skills/SkillDetailView'
import Header from './components/skills/Header'
import LoadingOverlay from './components/skills/LoadingOverlay'
import SkillsList from './components/skills/SkillsList'
import TagsPage from './components/skills/TagsPage'
import AddSkillModal from './components/skills/modals/AddSkillModal'
import DeleteModal from './components/skills/modals/DeleteModal'
import EditSkillTagsModal from './components/skills/modals/EditSkillTagsModal'
import GitPickModal from './components/skills/modals/GitPickModal'
import LocalPickModal from './components/skills/modals/LocalPickModal'
import ImportModal from './components/skills/modals/ImportModal'
import NewToolsModal from './components/skills/modals/NewToolsModal'
import ScopeSyncModal from './components/skills/modals/ScopeSyncModal'
import SharedDirModal from './components/skills/modals/SharedDirModal'
import SettingsPage from './components/skills/SettingsPage'
import type {
  FeaturedSkillDto,
  GitSkillCandidate,
  InstallResultDto,
  LocalSkillCandidate,
  ManagedSkill,
  OnboardingPlan,
  OnlineSkillDto,
  TagWithCountDto,
  ToolOption,
  ToolStatusDto,
  UpdateResultDto,
} from './components/skills/types'

type SkillScopeState = Record<
  string,
  {
    scope: 'global' | 'project'
    projects: string[]
  }
>

function App() {
  const { t, i18n } = useTranslation()
  const language = i18n.resolvedLanguage ?? i18n.language ?? 'en'
  const languageStorageKey = 'skills-language'
  const themeStorageKey = 'skills-theme'
  const skillScopeStorageKey = 'skills-project-scope-state-v1'
  const toggleLanguage = useCallback(() => {
    void i18n.changeLanguage(language === 'en' ? 'zh' : 'en')
  }, [i18n, language])
  const [themePreference, setThemePreference] = useState<'system' | 'light' | 'dark'>(
    'system',
  )
  const [systemTheme, setSystemTheme] = useState<'light' | 'dark'>('light')
  const [plan, setPlan] = useState<OnboardingPlan | null>(null)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [selected, setSelected] = useState<Record<string, boolean>>({})
  const [variantChoice, setVariantChoice] = useState<Record<string, string>>({})
  const [syncTargets, setSyncTargets] = useState<Record<string, boolean>>({})
  const [actionMessage, setActionMessage] = useState<string | null>(null)
  const [successToastMessage, setSuccessToastMessage] = useState<string | null>(
    null,
  )
  const [managedSkills, setManagedSkills] = useState<ManagedSkill[]>([])
  const [localPath, setLocalPath] = useState('')
  const [localName, setLocalName] = useState('')
  const [gitUrl, setGitUrl] = useState('')
  const [gitName, setGitName] = useState('')
  const [pendingDeleteId, setPendingDeleteId] = useState<string | null>(null)
  const [gitCandidates, setGitCandidates] = useState<GitSkillCandidate[]>([])
  const [gitCandidatesRepoUrl, setGitCandidatesRepoUrl] = useState<string>('')
  const [showGitPickModal, setShowGitPickModal] = useState(false)
  const [gitCandidateSelected, setGitCandidateSelected] = useState<
    Record<string, boolean>
  >({})
  const [localCandidates, setLocalCandidates] = useState<LocalSkillCandidate[]>([])
  const [localCandidatesBasePath, setLocalCandidatesBasePath] = useState('')
  const [showLocalPickModal, setShowLocalPickModal] = useState(false)
  const [localCandidateSelected, setLocalCandidateSelected] = useState<
    Record<string, boolean>
  >({})
  const [loadingStartAt, setLoadingStartAt] = useState<number | null>(null)
  const [toolStatus, setToolStatus] = useState<ToolStatusDto | null>(null)
  const [showNewToolsModal, setShowNewToolsModal] = useState(false)
  const [showAddModal, setShowAddModal] = useState(false)
  const [showImportModal, setShowImportModal] = useState(false)
  const [pendingSharedToggle, setPendingSharedToggle] = useState<{
    skill: ManagedSkill
    toolId: string
    affectedToolIds?: string[]
  } | null>(null)
  const [updateAvailableVersion, setUpdateAvailableVersion] = useState<string | null>(null)
  const [updateBody, setUpdateBody] = useState<string | null>(null)
  const [updateInstalling, setUpdateInstalling] = useState(false)
  const [updateDone, setUpdateDone] = useState(false)
  const updateObjRef = useRef<Update | null>(null) as MutableRefObject<Update | null>
  const [searchQuery, setSearchQuery] = useState('')
  const [sortBy, setSortBy] = useState<'updated' | 'name'>('updated')
  const [scopeFilter, setScopeFilter] = useState<'all' | 'global' | 'project'>('all')
  const [activeView, setActiveView] = useState<'myskills' | 'explore' | 'moreskills' | 'detail' | 'settings' | 'tags'>('myskills')
  const [detailSkill, setDetailSkill] = useState<ManagedSkill | null>(null)
  const [tags, setTags] = useState<TagWithCountDto[]>([])
  const [selectedTagIds, setSelectedTagIds] = useState<number[]>([])
  const [includeUntagged, setIncludeUntagged] = useState(false)
  const [tagEditorSkill, setTagEditorSkill] = useState<ManagedSkill | null>(null)
  const [pendingDeleteTag, setPendingDeleteTag] = useState<TagWithCountDto | null>(null)
  const [addModalTab, setAddModalTab] = useState<'local' | 'git'>('git')
  const [addModalTagIds, setAddModalTagIds] = useState<number[]>([])
  const [featuredSkills, setFeaturedSkills] = useState<FeaturedSkillDto[]>([])
  const [featuredLoading, setFeaturedLoading] = useState(false)
  const [exploreFilter, setExploreFilter] = useState('')
  const [searchResults, setSearchResults] = useState<OnlineSkillDto[]>([])
  const [searchLoading, setSearchLoading] = useState(false)
  const searchTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const [autoSelectSkillName, setAutoSelectSkillName] = useState<string | null>(null)
  const [scopeModalSkill, setScopeModalSkill] = useState<ManagedSkill | null>(null)
  const [recentProjects, setRecentProjects] = useState<string[]>([])
  const [skillScopeState, setSkillScopeState] = useState<SkillScopeState>({})

  const isTauri =
    typeof window !== 'undefined' &&
    Boolean(
      (window as { __TAURI__?: unknown }).__TAURI__ ||
        (window as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__,
    )

  const invokeTauri = useCallback(
    async <T,>(command: string, args?: Record<string, unknown>) => {
      if (!isTauri) {
        throw new Error('Tauri API is not available')
      }
      const { invoke } = await import('@tauri-apps/api/core')
      return invoke<T>(command, args)
    },
    [isTauri],
  )
  const formatErrorMessage = useCallback(
    (raw: string) => {
      if (raw.includes('CANCELLED|')) {
        return '' // Silently ignore cancelled operations
      }
      if (raw.includes('skill already exists in central repo')) {
        // Extract skill name from path like: skill already exists in central repo: "/path/to/react-best-practices"
        const pathMatch = raw.match(/central repo:\s*"?([^"]+)"?/)
        if (pathMatch) {
          const skillName = pathMatch[1].split('/').pop() ?? ''
          if (skillName) {
            return t('errors.skillExistsInHubNamed', { name: skillName })
          }
        }
        return t('errors.skillExistsInHub')
      }
      if (raw.startsWith('TARGET_EXISTS|')) {
        return t('errors.targetExists')
      }
      if (raw.startsWith('TOOL_NOT_INSTALLED|')) {
        return t('errors.toolNotInstalled')
      }
      if (raw.startsWith('TOOL_NOT_WRITABLE|')) {
        const parts = raw.split('|')
        return t('errors.toolNotWritable', { tool: parts[1] ?? '', path: parts[2] ?? '' })
      }
      if (raw.startsWith('PROJECT_SCOPE_UNSUPPORTED|')) {
        const tool = raw.split('|')[1] ?? ''
        return t('projectSync.unsupportedTool', { tool })
      }
      if (raw.includes('未在该仓库中发现可导入的 Skills')) {
        return t('errors.noSkillsFoundInRepo')
      }
      return raw
    },
    [t],
  )
  const showActionErrors = useCallback(
    (errors: { title: string; message: string }[]) => {
      if (errors.length === 0) return
      const head = errors[0]
      const more =
        errors.length > 1
          ? t('errors.moreCount', { count: errors.length - 1 })
          : ''
      toast.error(
        `${formatErrorMessage(`${head.title}\n${head.message}`)}${more}`,
        { duration: 3200 },
      )
    },
    [formatErrorMessage, t],
  )
  const isSkillNameTaken = useCallback(
    (name: string) =>
      managedSkills.some((skill) => skill.name.toLowerCase() === name.toLowerCase()),
    [managedSkills],
  )

  const formatRelative = (ms: number | null | undefined) => {
    if (!ms) return t('relative.empty')
    const diff = Date.now() - ms
    if (diff < 0) return t('relative.empty')
    const minutes = Math.floor(diff / 60000)
    if (minutes < 1) return t('relative.justNow')
    if (minutes < 60) {
      return t('relative.minutesAgo', { minutes })
    }
    const hours = Math.floor(minutes / 60)
    if (hours < 24) {
      return t('relative.hoursAgo', { hours })
    }
    const days = Math.floor(hours / 24)
    return t('relative.daysAgo', { days })
  }

  const getSkillSourceLabel = (skill: ManagedSkill) => {
    const key = skill.source_type.toLowerCase()
    if (key.includes('git') && skill.source_ref) {
      return skill.source_ref
    }
    return skill.central_path
  }

  const getGithubInfo = (url: string | null | undefined) => {
    if (!url) return null
    const normalized = url.replace(/^git\+/, '')
    try {
      const parsed = new URL(normalized)
      if (!parsed.hostname.includes('github.com')) return null
      const parts = parsed.pathname.split('/').filter(Boolean)
      const owner = parts[0]
      const repo = parts[1]?.replace(/\.git$/, '')
      if (!owner || !repo) return null
      return {
        label: `${owner}/${repo}`,
        href: `https://github.com/${owner}/${repo}`,
      }
    } catch {
      const match = normalized.match(/github\.com\/([^/]+)\/([^/#?]+)/i)
      if (!match) return null
      const owner = match[1]
      const repo = match[2].replace(/\.git$/, '')
      return {
        label: `${owner}/${repo}`,
        href: `https://github.com/${owner}/${repo}`,
      }
    }
  }

  const loadPlan = useCallback(async () => {
    setLoading(true)
    setLoadingStartAt(Date.now())
    setError(null)
    try {
      const result = await invokeTauri<OnboardingPlan>('get_onboarding_plan')
      setPlan(result)
      const defaultSelected: Record<string, boolean> = {}
      const defaultChoice: Record<string, string> = {}
      result.groups.forEach((group) => {
        defaultSelected[group.name] = true
        const first = group.variants[0]
        if (first) {
          defaultChoice[group.name] = first.path
        }
      })
      setSelected(defaultSelected)
      setVariantChoice(defaultChoice)
      return result
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
      return null
    } finally {
      setLoading(false)
      setLoadingStartAt(null)
    }
  }, [invokeTauri])

  const loadManagedSkills = useCallback(async () => {
    try {
      const result = await invokeTauri<ManagedSkill[]>('get_managed_skills')
      setManagedSkills(result)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    }
  }, [invokeTauri])

  const loadTags = useCallback(async () => {
    try {
      const result = await invokeTauri<TagWithCountDto[]>('get_tags')
      setTags(result)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    }
  }, [invokeTauri])

  useEffect(() => {
    if (isTauri) {
      loadManagedSkills()
      loadTags()
    }
  }, [isTauri, loadManagedSkills, loadTags])

  useEffect(() => {
    if (typeof window === 'undefined') return
    try {
      const raw = window.localStorage.getItem(skillScopeStorageKey)
      if (raw) {
        setSkillScopeState(JSON.parse(raw) as SkillScopeState)
      }
    } catch {
      setSkillScopeState({})
    }
  }, [skillScopeStorageKey])

  useEffect(() => {
    if (typeof window === 'undefined') return
    try {
      window.localStorage.setItem(
        skillScopeStorageKey,
        JSON.stringify(skillScopeState),
      )
    } catch {
      // ignore storage failures
    }
  }, [skillScopeState, skillScopeStorageKey])

  useEffect(() => {
    if (!isTauri) return
    invokeTauri<string[]>('get_recent_projects')
      .then((projects) => setRecentProjects(projects))
      .catch(() => {})
  }, [invokeTauri, isTauri])

  useEffect(() => {
    if (typeof window === 'undefined') return
    const stored = window.localStorage.getItem(themeStorageKey)
    if (stored === 'light' || stored === 'dark' || stored === 'system') {
      setThemePreference(stored)
    }
  }, [themeStorageKey])

  useEffect(() => {
    if (typeof window === 'undefined') return
    if (language !== 'en' && language !== 'zh') return
    try {
      window.localStorage.setItem(languageStorageKey, language)
    } catch {
      // ignore storage failures
    }
  }, [language, languageStorageKey])

  useEffect(() => {
    if (typeof window === 'undefined') return
    const media = window.matchMedia('(prefers-color-scheme: dark)')
    const update = () => {
      setSystemTheme(media.matches ? 'dark' : 'light')
    }
    update()
    if (media.addEventListener) {
      media.addEventListener('change', update)
    } else {
      media.addListener(update)
    }
    return () => {
      if (media.removeEventListener) {
        media.removeEventListener('change', update)
      } else {
        media.removeListener(update)
      }
    }
  }, [])

  useEffect(() => {
    if (typeof document === 'undefined') return
    const resolvedTheme =
      themePreference === 'system' ? systemTheme : themePreference
    document.documentElement.dataset.theme = resolvedTheme
    document.documentElement.style.colorScheme = resolvedTheme
    try {
      window.localStorage.setItem(themeStorageKey, themePreference)
    } catch {
      // ignore storage failures
    }
  }, [systemTheme, themePreference, themeStorageKey])

  useEffect(() => {
    if (!isTauri) return
    invokeTauri<string>('get_central_repo_path')
      .then((path) => setStoragePath(path))
      .catch((err) => {
        setError(err instanceof Error ? err.message : String(err))
      })
  }, [isTauri, invokeTauri])

  useEffect(() => {
    if (!isTauri) return
    invokeTauri<number>('get_git_cache_cleanup_days')
      .then((days) => setGitCacheCleanupDays(days))
      .catch((err) => {
        setError(err instanceof Error ? err.message : String(err))
      })
  }, [isTauri, invokeTauri])

  useEffect(() => {
    if (!isTauri) return
    invokeTauri<number>('get_git_cache_ttl_secs')
      .then((secs) => setGitCacheTtlSecs(secs))
      .catch((err) => {
        setError(err instanceof Error ? err.message : String(err))
      })
  }, [isTauri, invokeTauri])

  useEffect(() => {
    if (!isTauri) return
    invokeTauri<string>('get_github_token')
      .then((token) => setGithubToken(token))
      .catch(() => {})
  }, [isTauri, invokeTauri])

  useEffect(() => {
    if (isTauri) {
      void loadPlan()
    }
  }, [isTauri, loadPlan])

  useEffect(() => {
    if (!isTauri) return
    const ignoredVersion = localStorage.getItem('skills-ignored-update-version')
    import('@tauri-apps/plugin-updater')
      .then(({ check }) => check())
      .then(async (update) => {
        if (update && update.version !== ignoredVersion) {
          updateObjRef.current = update
          setUpdateAvailableVersion(update.version)
          // Fetch full release notes from GitHub API
          try {
            const res = await fetch(
              `https://api.github.com/repos/qufei1993/skills-hub/releases/tags/v${update.version}`,
            )
            if (res.ok) {
              const data = await res.json()
              setUpdateBody(data.body ?? update.body ?? null)
            } else {
              setUpdateBody(update.body ?? null)
            }
          } catch {
            setUpdateBody(update.body ?? null)
          }
        }
      })
      .catch(() => {})
  }, [isTauri])

  const handleDismissUpdate = useCallback(() => {
    setUpdateAvailableVersion(null)
    setUpdateBody(null)
  }, [])

  const handleDismissUpdateForever = useCallback(() => {
    if (updateAvailableVersion) {
      localStorage.setItem('skills-ignored-update-version', updateAvailableVersion)
    }
    setUpdateAvailableVersion(null)
    setUpdateBody(null)
  }, [updateAvailableVersion])

  const handleUpdateNow = useCallback(async () => {
    const update = updateObjRef.current
    if (!update) return
    setUpdateInstalling(true)
    try {
      await update.downloadAndInstall()
      setUpdateInstalling(false)
      setUpdateDone(true)
    } catch (err) {
      setUpdateInstalling(false)
      toast.error(err instanceof Error ? err.message : String(err), { duration: 3200 })
    }
  }, [])

  useEffect(() => {
    if (!successToastMessage) return
    toast.success(successToastMessage, { duration: 1800 })
    setSuccessToastMessage(null)
  }, [successToastMessage])

  useEffect(() => {
    if (!error) return
    const msg = formatErrorMessage(error)
    if (msg) toast.error(msg, { duration: 2600 })
    setError(null)
    setActionMessage(null)
  }, [error, formatErrorMessage])

  const toolInfos = useMemo(() => toolStatus?.tools ?? [], [toolStatus])

  const tools: ToolOption[] = useMemo(() => {
    return toolInfos.map((info) => ({
      id: info.key,
      // Prefer i18n label if present; fallback to backend label.
      label: t(`tools.${info.key}`, { defaultValue: info.label }),
      supports_project_scope: info.supports_project_scope,
    }))
  }, [t, toolInfos])

  const toolLabelById = useMemo(() => {
    const out: Record<string, string> = {}
    for (const tool of tools) out[tool.id] = tool.label
    return out
  }, [tools])

  const sharedToolIdsByToolId = useMemo(() => {
    // toolId -> all toolIds that share the same skills_dir.
    const byDir: Record<string, string[]> = {}
    for (const info of toolInfos) {
      const dir = info.skills_dir
      if (!byDir[dir]) byDir[dir] = []
      byDir[dir].push(info.key)
    }
    const out: Record<string, string[]> = {}
    for (const dir of Object.keys(byDir)) {
      const ids = byDir[dir]
      if (ids.length <= 1) continue
      for (const id of ids) out[id] = ids
    }
    return out
  }, [toolInfos])

  const uniqueToolIdsBySkillsDir = useCallback(
    (toolIds: string[]) => {
      // Preserve UI order (tools array order), de-dupe by skills_dir.
      const wanted = new Set(toolIds)
      const seen = new Set<string>()
      const out: string[] = []
      for (const tool of toolInfos) {
        if (!wanted.has(tool.key)) continue
        if (seen.has(tool.skills_dir)) continue
        seen.add(tool.skills_dir)
        out.push(tool.key)
      }
      return out
    },
    [toolInfos],
  )

  const installedToolIds = useMemo(
    () => toolStatus?.installed ?? [],
    [toolStatus],
  )
  const isInstalled = useCallback(
    (id: string) => installedToolIds.includes(id),
    [installedToolIds],
  )
  const installedTools = useMemo(
    () => tools.filter((tool) => installedToolIds.includes(tool.id)),
    [tools, installedToolIds],
  )
  const toolSupportsProjectScope = useCallback(
    (toolId: string) =>
      tools.find((tool) => tool.id === toolId)?.supports_project_scope ?? true,
    [tools],
  )
  const installedProjectToolIds = useMemo(
    () => installedToolIds.filter((toolId) => toolSupportsProjectScope(toolId)),
    [installedToolIds, toolSupportsProjectScope],
  )

  const getSkillProjects = useCallback(
    (skill: ManagedSkill) => {
      const projects = new Set<string>()
      for (const target of skill.targets) {
        if ((target.scope ?? 'global') === 'project' && target.project_path) {
          projects.add(target.project_path)
        }
      }
      return Array.from(projects)
    },
    [],
  )

  const getSkillScope = useCallback(
    (skill: ManagedSkill): 'global' | 'project' => {
      const hasGlobalTarget = skill.targets.some(
        (target) => (target.scope ?? 'global') === 'global',
      )
      const hasProjectTarget = skill.targets.some(
        (target) => (target.scope ?? 'global') === 'project',
      )
      if (hasGlobalTarget && !hasProjectTarget) return 'global'
      if (hasProjectTarget && !hasGlobalTarget) return 'project'
      const stored = skillScopeState[skill.id]?.scope
      if (stored === 'global' || stored === 'project') return stored
      return hasProjectTarget ? 'project' : 'global'
    },
    [skillScopeState],
  )

  const visibleSkills = useMemo(() => {
    const query = searchQuery.trim().toLowerCase()
    const selectedTagSet = new Set(selectedTagIds)
    const hasTagFilter = selectedTagIds.length > 0 || includeUntagged
    const filtered = managedSkills.filter((skill) => {
      if (scopeFilter !== 'all' && getSkillScope(skill) !== scopeFilter) return false
      if (hasTagFilter) {
        const matchesSelectedTag = skill.tags.some((tag) => selectedTagSet.has(tag.id))
        const matchesUntagged = includeUntagged && skill.tags.length === 0
        if (!matchesSelectedTag && !matchesUntagged) return false
      }
      if (!query) return true
      return (
        skill.name.toLowerCase().includes(query) ||
        skill.central_path.toLowerCase().includes(query) ||
        skill.source_type.toLowerCase().includes(query) ||
        skill.tags.some((tag) => tag.name.toLowerCase().includes(query))
      )
    })
    const sorted = [...filtered].sort((a, b) => {
      if (sortBy === 'name') {
        return a.name.localeCompare(b.name)
      }
      return (b.updated_at ?? 0) - (a.updated_at ?? 0)
    })
    return sorted
  }, [
    getSkillScope,
    includeUntagged,
    managedSkills,
    scopeFilter,
    searchQuery,
    selectedTagIds,
    sortBy,
  ])
  const untaggedCount = useMemo(
    () => managedSkills.filter((skill) => skill.tags.length === 0).length,
    [managedSkills],
  )

  const [storagePath, setStoragePath] = useState<string>(t('notAvailable'))
  const [gitCacheCleanupDays, setGitCacheCleanupDays] = useState<number>(30)
  const [gitCacheTtlSecs, setGitCacheTtlSecs] = useState<number>(60)
  const [githubToken, setGithubToken] = useState<string>('')
  const handlePickStoragePath = useCallback(async () => {
    try {
      if (!isTauri) {
        throw new Error(t('errors.notTauri'))
      }
      const { open } = await import('@tauri-apps/plugin-dialog')
      const selected = await open({
        directory: true,
        multiple: false,
        title: t('selectStoragePath'),
      })
      if (!selected || Array.isArray(selected)) return
      const newPath = await invokeTauri<string>('set_central_repo_path', {
        path: selected,
      })
      setStoragePath(newPath)
      await loadManagedSkills()
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    }
  }, [invokeTauri, isTauri, loadManagedSkills, t])
  const handleGitCacheCleanupDaysChange = useCallback(
    async (nextDays: number) => {
      const normalized = Math.max(0, Math.min(nextDays, 3650))
      setGitCacheCleanupDays(normalized)
      if (!isTauri) return
      try {
        const updated = await invokeTauri<number>('set_git_cache_cleanup_days', {
          days: normalized,
        })
        setGitCacheCleanupDays(updated)
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err))
      }
    },
    [invokeTauri, isTauri],
  )
  const handleGitCacheTtlSecsChange = useCallback(
    async (nextSecs: number) => {
      const normalized = Math.max(0, Math.min(nextSecs, 3600))
      setGitCacheTtlSecs(normalized)
      if (!isTauri) return
      try {
        const updated = await invokeTauri<number>('set_git_cache_ttl_secs', {
          secs: normalized,
        })
        setGitCacheTtlSecs(updated)
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err))
      }
    },
    [invokeTauri, isTauri],
  )
  const handleGithubTokenChange = useCallback(
    async (nextToken: string) => {
      setGithubToken(nextToken)
      if (!isTauri) return
      try {
        await invokeTauri('set_github_token', { token: nextToken })
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err))
      }
    },
    [invokeTauri, isTauri],
  )
  const handleClearGitCacheNow = useCallback(async () => {
    if (!isTauri) {
      setError(t('errors.notTauri'))
      return
    }
    try {
      const removed = await invokeTauri<number>('clear_git_cache_now')
      setSuccessToastMessage(t('status.gitCacheCleared', { count: removed }))
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    }
  }, [invokeTauri, isTauri, t])
  const handlePickLocalPath = useCallback(async () => {
    try {
      if (!isTauri) {
        throw new Error(t('errors.notTauri'))
      }
      const { open } = await import('@tauri-apps/plugin-dialog')
      const selected = await open({
        directory: true,
        multiple: false,
        title: t('selectLocalFolder'),
      })
      if (!selected || Array.isArray(selected)) return
      setLocalPath(selected)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    }
  }, [isTauri, t])
  const pendingDeleteSkill = useMemo(
    () => managedSkills.find((skill) => skill.id === pendingDeleteId) ?? null,
    [managedSkills, pendingDeleteId],
  )
  const newlyInstalledToolsText = useMemo(() => {
    if (!toolStatus || toolStatus.newly_installed.length === 0) return ''
    return toolStatus.newly_installed
      .map((id) => tools.find((t) => t.id === id)?.label ?? id)
      .join('、')
  }, [toolStatus, tools])

  const handleOpenSettings = useCallback(() => {
    setActiveView('settings')
  }, [])

  const loadFeaturedSkills = useCallback(async () => {
    if (featuredSkills.length > 0) return
    setFeaturedLoading(true)
    try {
      const result = await invokeTauri<FeaturedSkillDto[]>('get_featured_skills')
      setFeaturedSkills(result)
    } catch {
      // silent — explore tab will show empty state
    } finally {
      setFeaturedLoading(false)
    }
  }, [featuredSkills.length, invokeTauri])

  const handleViewChange = useCallback(
    (view: 'myskills' | 'explore' | 'moreskills' | 'tags') => {
      if (view === 'explore' && !EXPLORE_ENABLED) return
      setActiveView(view)
      if (view === 'explore') {
        loadFeaturedSkills()
      }
      if (view === 'myskills') {
        setDetailSkill(null)
      }
    },
    [loadFeaturedSkills],
  )

  const handleOpenDetail = useCallback((skill: ManagedSkill) => {
    setDetailSkill(skill)
    setActiveView('detail')
  }, [])

  const handleBackToList = useCallback(() => {
    setDetailSkill(null)
    setActiveView('myskills')
  }, [])

  const handleExploreFilterChange = useCallback(
    (value: string) => {
      setExploreFilter(value)
      if (searchTimerRef.current) {
        clearTimeout(searchTimerRef.current)
        searchTimerRef.current = null
      }
      if (value.trim().length < 2) {
        setSearchResults([])
        setSearchLoading(false)
        return
      }
      setSearchLoading(true)
      searchTimerRef.current = setTimeout(async () => {
        try {
          const results = await invokeTauri<OnlineSkillDto[]>(
            'search_skills_online',
            { query: value.trim(), limit: 20 },
          )
          setSearchResults(results)
        } catch {
          toast.error(t('searchError'))
          setSearchResults([])
        } finally {
          setSearchLoading(false)
        }
      }, 500)
    },
    [invokeTauri, t],
  )


  const handleOpenAdd = useCallback(() => {
    setShowAddModal(true)
    setAddModalTagIds([])
  }, [])

  const handleOpenSkillSourceSite = useCallback(
    async (url: string) => {
      try {
        await openExternalUrl(url, isTauri)
      } catch {
        toast.error(t('moreSkillsOpenError'))
      }
    },
    [isTauri, t],
  )

  const applySelectedAddModalTags = useCallback(
    async (skillId: string, skillName: string) => {
      if (addModalTagIds.length === 0) return
      try {
        await invokeTauri('set_skill_tags', {
          skillId,
          tagIds: addModalTagIds,
        })
      } catch {
        toast.error(t('tagsApplyFailed', { name: skillName }))
      }
    },
    [addModalTagIds, invokeTauri, t],
  )

  const handleCancelLoading = useCallback(() => {
    void invokeTauri('cancel_current_operation').catch(() => {})
    setLoading(false)
    setLoadingStartAt(null)
    setActionMessage(null)
  }, [invokeTauri])

  const handleCloseAdd = useCallback(() => {
    if (!loading) {
      setShowAddModal(false)
      setAddModalTagIds([])
    }
  }, [loading])

  const handleCloseImport = useCallback(() => {
    if (!loading) setShowImportModal(false)
  }, [loading])

  const handleCloseSettings = useCallback(() => {
    setActiveView('myskills')
  }, [])

  const handleThemeChange = useCallback(
    (nextTheme: 'system' | 'light' | 'dark') => {
      setThemePreference(nextTheme)
    },
    [],
  )

  const handleCloseNewTools = useCallback(() => {
    if (!loading) setShowNewToolsModal(false)
  }, [loading])

  const handleCloseDelete = useCallback(() => {
    if (!loading) setPendingDeleteId(null)
  }, [loading])

  const handleCloseGitPick = useCallback(() => {
    if (!loading) setShowGitPickModal(false)
  }, [loading])

  const handleCancelGitPick = useCallback(() => {
    if (loading) return
    setShowGitPickModal(false)
    setGitCandidates([])
    setGitCandidateSelected({})
    setGitCandidatesRepoUrl('')
  }, [loading])

  const handleCloseLocalPick = useCallback(() => {
    if (!loading) setShowLocalPickModal(false)
  }, [loading])

  const handleCancelLocalPick = useCallback(() => {
    if (loading) return
    setShowLocalPickModal(false)
    setLocalCandidates([])
    setLocalCandidateSelected({})
    setLocalCandidatesBasePath('')
  }, [loading])

  const handleSortChange = useCallback((value: 'updated' | 'name') => {
    setSortBy(value)
  }, [])

  const handleSearchChange = useCallback((value: string) => {
    setSearchQuery(value)
  }, [])

  const handleScopeFilterChange = useCallback(
    (value: 'all' | 'global' | 'project') => {
      setScopeFilter(value)
    },
    [],
  )

  const handleToggleAddModalTag = useCallback((tagId: number) => {
    setAddModalTagIds((current) =>
      current.includes(tagId)
        ? current.filter((id) => id !== tagId)
        : [...current, tagId],
    )
  }, [])

  const handleToggleTagFilter = useCallback((tagId: number) => {
    setSelectedTagIds((current) =>
      current.includes(tagId)
        ? current.filter((id) => id !== tagId)
        : [...current, tagId],
    )
  }, [])

  const handleToggleUntaggedFilter = useCallback(() => {
    setIncludeUntagged((current) => !current)
  }, [])

  const handleClearTagFilters = useCallback(() => {
    setSelectedTagIds([])
    setIncludeUntagged(false)
  }, [])

  const handleOpenTagsPage = useCallback(() => {
    setActiveView('tags')
  }, [])

  const handleReviewUntagged = useCallback(() => {
    setSelectedTagIds([])
    setIncludeUntagged(true)
    setActiveView('myskills')
  }, [])

  const handleViewTag = useCallback((tagId: number) => {
    setSelectedTagIds([tagId])
    setIncludeUntagged(false)
    setActiveView('myskills')
  }, [])

  const handleCreateTag = useCallback(
    async (name: string) => {
      try {
        await invokeTauri('create_tag', { name })
        await loadTags()
        setSuccessToastMessage(t('tagCreated'))
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err))
      }
    },
    [invokeTauri, loadTags, t],
  )

  const handleRenameTag = useCallback(
    async (tagId: number, name: string) => {
      try {
        const renamed = await invokeTauri<{ id: number; name: string }>('rename_tag', {
          tagId,
          name,
        })
        setSelectedTagIds((current) =>
          current.includes(tagId) ? current.map((id) => (id === tagId ? renamed.id : id)) : current,
        )
        await loadManagedSkills()
        await loadTags()
        setSuccessToastMessage(t('tagRenamed'))
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err))
      }
    },
    [invokeTauri, loadManagedSkills, loadTags, t],
  )

  const handleDeleteTag = useCallback((tag: TagWithCountDto) => {
    setPendingDeleteTag(tag)
  }, [])

  const handleCloseDeleteTag = useCallback(() => {
    if (!loading) setPendingDeleteTag(null)
  }, [loading])

  const handleConfirmDeleteTag = useCallback(async () => {
    if (!pendingDeleteTag) return
    try {
      setLoading(true)
      setLoadingStartAt(Date.now())
      setActionMessage(t('actions.deletingTag', { name: pendingDeleteTag.name }))
      await invokeTauri('delete_tag', { tagId: pendingDeleteTag.id })
      setSelectedTagIds((current) => current.filter((id) => id !== pendingDeleteTag.id))
      await loadManagedSkills()
      await loadTags()
      setPendingDeleteTag(null)
      setSuccessToastMessage(t('tagDeleted'))
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setLoading(false)
      setLoadingStartAt(null)
      setActionMessage(null)
    }
  }, [invokeTauri, loadManagedSkills, loadTags, pendingDeleteTag, t])

  const handleOpenEditTags = useCallback((skill: ManagedSkill) => {
    setTagEditorSkill(skill)
  }, [])

  const handleCloseEditTags = useCallback(() => {
    if (!loading) setTagEditorSkill(null)
  }, [loading])

  const handleSaveSkillTags = useCallback(
    async (skill: ManagedSkill, tagIds: number[]) => {
      try {
        setLoading(true)
        setLoadingStartAt(Date.now())
        setActionMessage(t('actions.updatingTags', { name: skill.name }))
        await invokeTauri('set_skill_tags', { skillId: skill.id, tagIds })
        await loadManagedSkills()
        await loadTags()
        setTagEditorSkill(null)
        setSuccessToastMessage(t('tagsUpdated'))
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err))
      } finally {
        setLoading(false)
        setLoadingStartAt(null)
        setActionMessage(null)
      }
    },
    [invokeTauri, loadManagedSkills, loadTags, t],
  )

  const handleSyncTargetChange = useCallback(
    (toolId: string, checked: boolean) => {
      const shared = sharedToolIdsByToolId[toolId] ?? [toolId]
      if (shared.length > 1) {
        const others = shared.filter((id) => id !== toolId)
        const otherLabels = others.map((id) => toolLabelById[id] ?? id).join(', ')
        const ok = window.confirm(
          t('sharedDirConfirm', {
            tool: toolLabelById[toolId] ?? toolId,
            others: otherLabels,
          }),
        )
        if (!ok) return
      }
      setSyncTargets((prev) => {
        const next = { ...prev }
        for (const id of shared) next[id] = checked
        return next
      })
    },
    [sharedToolIdsByToolId, t, toolLabelById],
  )

  const handleDeletePrompt = useCallback((skillId: string) => {
    setPendingDeleteId(skillId)
  }, [])

  const handleToggleGitCandidate = useCallback(
    (subpath: string, checked: boolean) => {
      setGitCandidateSelected((prev) => ({
        ...prev,
        [subpath]: checked,
      }))
    },
    [],
  )

  const handleToggleLocalCandidate = useCallback(
    (subpath: string, checked: boolean) => {
      setLocalCandidateSelected((prev) => ({
        ...prev,
        [subpath]: checked,
      }))
    },
    [],
  )

  const handleToggleGroup = useCallback((groupName: string, checked: boolean) => {
    setSelected((prev) => ({
      ...prev,
      [groupName]: checked,
    }))
  }, [])

  const handleSelectVariant = useCallback((groupName: string, path: string) => {
    setVariantChoice((prev) => ({
      ...prev,
      [groupName]: path,
    }))
  }, [])

  const handleReviewImport = useCallback(async () => {
    if (plan) {
      setShowImportModal(true)
      return
    }
    const result = await loadPlan()
    if (result) {
      setShowImportModal(true)
    }
  }, [loadPlan, plan])

  useEffect(() => {
    const load = async () => {
      if (!isTauri) return
      try {
        const status = await invokeTauri<ToolStatusDto>('get_tool_status')
        setToolStatus(status)

        // Default-select installed tools for sync targets if user hasn't toggled yet.
        setSyncTargets((prev) => {
          if (Object.keys(prev).length > 0) return prev
          const next: Record<string, boolean> = {}
          for (const t of status.tools) {
            next[t.key] = status.installed.includes(t.key)
          }
          return next
        })

        if (status.newly_installed.length > 0) {
          setShowNewToolsModal(true)
        }
      } catch (err) {
        // Non-fatal; app can still work without detection.
        console.warn(err)
      }
    }
    void load()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isTauri])

  const handleImport = async () => {
    if (!plan) return
    if (!plan.groups.some((group) => selected[group.name])) {
      setError(t('errors.selectAtLeastOneSkill'))
      return
    }
    setLoading(true)
    setLoadingStartAt(Date.now())
    setActionMessage(null)
    setError(null)
    try {
      const collectedErrors: { title: string; message: string }[] = []
      let successCount = 0
      for (const group of plan.groups) {
        if (!selected[group.name]) continue
        const chosenPath = variantChoice[group.name] ?? group.variants[0]?.path
        if (!chosenPath) continue
        const chosenVariant = group.variants.find((v) => v.path === chosenPath)
        const chosenVariantTool = chosenVariant?.tool ?? null
        const chosenFingerprint = chosenVariant?.fingerprint ?? null

        let installResult: {
          skill_id: string
          central_path: string
        }

        try {
          setActionMessage(t('actions.importExisting', { name: group.name }))
          installResult = await invokeTauri<{
            skill_id: string
            central_path: string
          }>('import_existing_skill', {
            sourcePath: chosenPath,
            name: group.name,
          })
          successCount += 1
        } catch (err) {
          collectedErrors.push({
            title: t('errors.importFailedTitle', { name: group.name }),
            message: err instanceof Error ? err.message : String(err),
          })
          continue
        }

        const selectedInstalledIds = tools
          .filter((tool) => syncTargets[tool.id] && isInstalled(tool.id))
          .map((t) => t.id)
        const targets = uniqueToolIdsBySkillsDir(selectedInstalledIds)
          .map((id) => tools.find((t) => t.id === id))
          .filter(Boolean) as ToolOption[]
        for (const tool of targets) {
          setActionMessage(
            t('actions.syncing', { name: group.name, tool: tool.label }),
          )
          try {
            const sharedToolIds = sharedToolIdsByToolId[tool.id] ?? [tool.id]
            const hasSameContentVariant = Boolean(
              chosenFingerprint &&
                group.variants.some(
                  (variant) =>
                    sharedToolIds.includes(variant.tool) &&
                    variant.fingerprint === chosenFingerprint,
                ),
            )
            const overwrite = Boolean(
              (chosenVariantTool &&
                (chosenVariantTool === tool.id || sharedToolIds.includes(chosenVariantTool))) ||
                hasSameContentVariant,
            )
            await invokeTauri('sync_skill_to_tool', {
              sourcePath: installResult.central_path,
              skillId: installResult.skill_id,
              tool: tool.id,
              name: group.name,
              // 自动接管：来源目录或内容一致的已发现目录可安全替换为 Hub 管理的同步目标。
              overwrite,
              overwriteIfSameContent: true,
            })
          } catch (err) {
            const raw = err instanceof Error ? err.message : String(err)
            if (raw.startsWith('TARGET_EXISTS|')) {
              const targetPath = raw.split('|')[1] ?? ''
              collectedErrors.push({
                title: t('errors.syncFailedTitle', {
                  name: group.name,
                  tool: tool.label,
                }),
                message: t('errors.syncTargetExistsMessage', {
                  path: targetPath,
                }),
              })
            } else {
              collectedErrors.push({
                title: t('errors.syncFailedTitle', {
                  name: group.name,
                  tool: tool.label,
                }),
                message: raw,
              })
            }
          }
        }
      }

      setActionMessage(t('status.importCompleted'))
      setActionMessage(null)
      await loadManagedSkills()
      await loadPlan()
      if (collectedErrors.length > 0) {
        showActionErrors(collectedErrors)
      } else if (successCount > 0) {
        setSuccessToastMessage(t('status.importCompleted'))
      }
      setShowImportModal(false)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setLoading(false)
      setLoadingStartAt(null)
    }
  }

  const handleCreateLocal = async () => {
    if (!localPath.trim()) {
      setError(t('errors.requireLocalPath'))
      return
    }
    setLoading(true)
    setLoadingStartAt(Date.now())
    setError(null)
    setActionMessage(t('actions.creatingLocalSkill'))
    try {
      const basePath = localPath.trim()
      const candidates = await invokeTauri<LocalSkillCandidate[]>(
        'list_local_skills_cmd',
        { basePath },
      )
      if (candidates.length === 0) {
        throw new Error(t('errors.noSkillsFoundLocal'))
      }
      if (candidates.length === 1 && candidates[0].valid) {
        const desiredName = localName.trim() || candidates[0].name
        if (isSkillNameTaken(desiredName)) {
          setError(t('errors.skillAlreadyExists', { name: desiredName }))
          return
        }
        const created = await invokeTauri<InstallResultDto>(
          'install_local_selection',
          {
            basePath,
            subpath: candidates[0].subpath,
            name: localName.trim() || undefined,
          },
        )
        await applySelectedAddModalTags(created.skill_id, created.name)
        {
          const selectedInstalledIds = tools
            .filter((tool) => syncTargets[tool.id] && isInstalled(tool.id))
            .map((t) => t.id)
          const targets = uniqueToolIdsBySkillsDir(selectedInstalledIds)
            .map((id) => tools.find((t) => t.id === id))
            .filter(Boolean) as ToolOption[]
          if (targets.length === 0) {
            setError(t('errors.noSyncTargets'))
          } else {
            const collectedErrors: { title: string; message: string }[] = []
            for (let i = 0; i < targets.length; i++) {
              const tool = targets[i]
              setActionMessage(
                t('actions.syncStep', {
                  index: i + 1,
                  total: targets.length,
                  name: created.name,
                  tool: tool.label,
                }),
              )
              try {
                await invokeTauri('sync_skill_to_tool', {
                  sourcePath: created.central_path,
                  skillId: created.skill_id,
                  tool: tool.id,
                  name: created.name,
                  overwriteIfSameContent: true,
                })
              } catch (err) {
                const raw = err instanceof Error ? err.message : String(err)
                collectedErrors.push({
                  title: t('errors.syncFailedTitle', {
                    name: created.name,
                    tool: tool.label,
                  }),
                  message: raw,
                })
              }
            }
            if (collectedErrors.length > 0) showActionErrors(collectedErrors)
          }
        }
        setLocalPath('')
        setLocalName('')
        setActionMessage(t('status.localSkillCreated'))
        setSuccessToastMessage(t('status.localSkillCreated'))
        setActionMessage(null)
        setShowAddModal(false)
        await loadManagedSkills()
        await loadTags()
      } else {
        setLocalCandidatesBasePath(basePath)
        setLocalCandidates(candidates)
        setLocalCandidateSelected(
          Object.fromEntries(candidates.map((c) => [c.subpath, c.valid])),
        )
        setShowLocalPickModal(true)
        setActionMessage(null)
        setLoading(false)
        setLoadingStartAt(null)
        return
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setLoading(false)
      setLoadingStartAt(null)
    }
  }

  const handleCreateGit = async () => {
    if (!gitUrl.trim()) {
      setError(t('errors.requireGitUrl'))
      return
    }
    setLoading(true)
    setLoadingStartAt(Date.now())
    setError(null)
    setActionMessage(t('actions.creatingGitSkill'))
    try {
      const url = gitUrl.trim()
      const isFolderUrl = url.includes('/tree/') || url.includes('/blob/')

      if (isFolderUrl) {
        const candidates = await invokeTauri<GitSkillCandidate[]>(
          'list_git_skills_cmd',
          { repoUrl: url },
        )
        if (candidates.length === 0) {
          throw new Error(t('errors.noSkillsFoundWithHint'))
        }
        if (candidates.length > 1) {
          setGitCandidatesRepoUrl(url)
          setGitCandidates(candidates)
          setGitCandidateSelected(
            Object.fromEntries(candidates.map((c) => [c.subpath, true])),
          )
          setShowGitPickModal(true)
          setActionMessage(null)
          setLoading(false)
          setLoadingStartAt(null)
          return
        }
        if (isSkillNameTaken(candidates[0].name)) {
          setError(t('errors.skillAlreadyExists', { name: candidates[0].name }))
          return
        }
        const created = await invokeTauri<InstallResultDto>(
          'install_git_selection',
          {
            repoUrl: url,
            subpath: candidates[0].subpath,
            name: gitName.trim() || undefined,
          },
        )
        await applySelectedAddModalTags(created.skill_id, created.name)
        {
          const selectedInstalledIds = tools
            .filter((tool) => syncTargets[tool.id] && isInstalled(tool.id))
            .map((t) => t.id)
          const targets = uniqueToolIdsBySkillsDir(selectedInstalledIds)
            .map((id) => tools.find((t) => t.id === id))
            .filter(Boolean) as ToolOption[]
          if (targets.length === 0) {
            setError(t('errors.noSyncTargets'))
          } else {
            const collectedErrors: { title: string; message: string }[] = []
            for (let i = 0; i < targets.length; i++) {
              const tool = targets[i]
              setActionMessage(
                t('actions.syncStep', {
                  index: i + 1,
                  total: targets.length,
                  name: created.name,
                  tool: tool.label,
                }),
              )
              try {
                await invokeTauri('sync_skill_to_tool', {
                  sourcePath: created.central_path,
                  skillId: created.skill_id,
                  tool: tool.id,
                  name: created.name,
                  overwriteIfSameContent: true,
                })
              } catch (err) {
                const raw = err instanceof Error ? err.message : String(err)
              collectedErrors.push({
                title: t('errors.syncFailedTitle', {
                  name: created.name,
                  tool: tool.label,
                }),
                message: raw,
              })
              }
            }
            if (collectedErrors.length > 0) showActionErrors(collectedErrors)
          }
        }
      } else {
        const candidates = await invokeTauri<GitSkillCandidate[]>(
          'list_git_skills_cmd',
          { repoUrl: url },
        )
        if (candidates.length === 0) {
          throw new Error(t('errors.noSkillsFoundWithHint'))
        }
        if (candidates.length === 1) {
          if (isSkillNameTaken(candidates[0].name)) {
            setError(t('errors.skillAlreadyExists', { name: candidates[0].name }))
            return
          }
          const created = await invokeTauri<InstallResultDto>(
            'install_git_selection',
            {
            repoUrl: url,
            subpath: candidates[0].subpath,
            name: gitName.trim() || undefined,
            },
          )
          await applySelectedAddModalTags(created.skill_id, created.name)
          {
            const selectedInstalledIds = tools
              .filter((tool) => syncTargets[tool.id] && isInstalled(tool.id))
              .map((t) => t.id)
            const targets = uniqueToolIdsBySkillsDir(selectedInstalledIds)
              .map((id) => tools.find((t) => t.id === id))
              .filter(Boolean) as ToolOption[]
            if (targets.length === 0) {
              setError(t('errors.noSyncTargets'))
            } else {
              const collectedErrors: { title: string; message: string }[] = []
              for (let i = 0; i < targets.length; i++) {
                const tool = targets[i]
                setActionMessage(
                  t('actions.syncStep', {
                    index: i + 1,
                    total: targets.length,
                    name: created.name,
                    tool: tool.label,
                  }),
                )
                try {
                  await invokeTauri('sync_skill_to_tool', {
                    sourcePath: created.central_path,
                    skillId: created.skill_id,
                    tool: tool.id,
                    name: created.name,
                    overwriteIfSameContent: true,
                  })
                } catch (err) {
                  const raw = err instanceof Error ? err.message : String(err)
                  collectedErrors.push({
                    title: t('errors.syncFailedTitle', {
                      name: created.name,
                      tool: tool.label,
                    }),
                    message: raw,
                  })
                }
              }
              if (collectedErrors.length > 0) showActionErrors(collectedErrors)
            }
          }
        } else if (autoSelectSkillName) {
          // Auto-select the matching skill from online search results.
          // skills.sh name may differ from SKILL.md name (e.g. "json-render-react" vs "react"),
          // so try exact match first, then containment match.
          const target = autoSelectSkillName.toLowerCase()
          const containMatches = candidates.filter((c) => {
            const n = c.name.toLowerCase()
            return target.includes(n) || n.includes(target)
          })
          const match =
            candidates.find((c) => c.name.toLowerCase() === target) ??
            (containMatches.length === 1 ? containMatches[0] : undefined)
          setAutoSelectSkillName(null)
          if (match) {
            if (isSkillNameTaken(match.name)) {
              setError(t('errors.skillAlreadyExists', { name: match.name }))
              return
            }
            const created = await invokeTauri<InstallResultDto>(
              'install_git_selection',
              {
                repoUrl: url,
                subpath: match.subpath,
                name: gitName.trim() || undefined,
              },
            )
            await applySelectedAddModalTags(created.skill_id, created.name)
            {
              const selectedInstalledIds = tools
                .filter((tool) => syncTargets[tool.id] && isInstalled(tool.id))
                .map((t) => t.id)
              const targets = uniqueToolIdsBySkillsDir(selectedInstalledIds)
                .map((id) => tools.find((t) => t.id === id))
                .filter(Boolean) as ToolOption[]
              if (targets.length === 0) {
                setError(t('errors.noSyncTargets'))
              } else {
                const collectedErrors: { title: string; message: string }[] = []
                for (let i = 0; i < targets.length; i++) {
                  const tool = targets[i]
                  setActionMessage(
                    t('actions.syncStep', {
                      index: i + 1,
                      total: targets.length,
                      name: created.name,
                      tool: tool.label,
                    }),
                  )
                  try {
                    await invokeTauri('sync_skill_to_tool', {
                      sourcePath: created.central_path,
                      skillId: created.skill_id,
                      tool: tool.id,
                      name: created.name,
                      overwriteIfSameContent: true,
                    })
                  } catch (err) {
                    const raw = err instanceof Error ? err.message : String(err)
                    collectedErrors.push({
                      title: t('errors.syncFailedTitle', {
                        name: created.name,
                        tool: tool.label,
                      }),
                      message: raw,
                    })
                  }
                }
                if (collectedErrors.length > 0) showActionErrors(collectedErrors)
              }
            }
          } else {
            // No match found, fall back to picker
            setGitCandidatesRepoUrl(url)
            setGitCandidates(candidates)
            setGitCandidateSelected(
              Object.fromEntries(candidates.map((c) => [c.subpath, true])),
            )
            setShowGitPickModal(true)
            setActionMessage(null)
            setLoading(false)
            setLoadingStartAt(null)
            return
          }
        } else {
          setGitCandidatesRepoUrl(url)
          setGitCandidates(candidates)
          setGitCandidateSelected(
            Object.fromEntries(candidates.map((c) => [c.subpath, true])),
          )
          setShowGitPickModal(true)
          setActionMessage(null)
          setLoading(false)
          setLoadingStartAt(null)
          return
        }
      }
      setGitUrl('')
      setGitName('')
      setActionMessage(t('status.gitSkillCreated'))
      setSuccessToastMessage(t('status.gitSkillCreated'))
      setActionMessage(null)
      setShowAddModal(false)
      await loadManagedSkills()
      await loadTags()
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setLoading(false)
      setLoadingStartAt(null)
    }
  }

  const [exploreInstallTrigger, setExploreInstallTrigger] = useState(0)
  const exploreInstallUrlRef = useRef<string | null>(null)

  const handleExploreInstall = useCallback(
    (sourceUrl: string, skillName?: string) => {
      setGitUrl(sourceUrl)
      if (skillName) setAutoSelectSkillName(skillName)
      if (toolStatus) {
        const targets: Record<string, boolean> = {}
        for (const id of toolStatus.installed) {
          targets[id] = true
        }
        setSyncTargets(targets)
      }
      exploreInstallUrlRef.current = sourceUrl
      setExploreInstallTrigger((n) => n + 1)
    },
    [toolStatus],
  )

  useEffect(() => {
    if (exploreInstallTrigger > 0 && exploreInstallUrlRef.current && !loading) {
      exploreInstallUrlRef.current = null
      void handleCreateGit()
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [exploreInstallTrigger])

  const handleInstallSelectedLocalCandidates = async () => {
    const selected = localCandidates.filter(
      (c) => c.valid && localCandidateSelected[c.subpath],
    )
    if (selected.length === 0) {
      setError(t('errors.selectAtLeastOneSkill'))
      return
    }
    if (selected.length > 1 && localName.trim()) {
      setError(t('errors.multiSelectNoCustomName'))
      return
    }
    if (selected.length > 1) {
      const seen = new Set<string>()
      const dup = selected.find((c) => {
        if (seen.has(c.name)) return true
        seen.add(c.name)
        return false
      })
      if (dup) {
        setError(t('errors.duplicateSelectedSkills', { name: dup.name }))
        return
      }
    }
    const desiredName =
      selected.length === 1 && localName.trim()
        ? localName.trim()
        : selected[0].name
    if (selected.length === 1 && isSkillNameTaken(desiredName)) {
      setError(t('errors.skillAlreadyExists', { name: desiredName }))
      return
    }
    const duplicated = selected.find((c) => isSkillNameTaken(c.name))
    if (selected.length > 1 && duplicated) {
      setError(t('errors.skillAlreadyExists', { name: duplicated.name }))
      return
    }

    setLoading(true)
    setLoadingStartAt(Date.now())
    setError(null)
    try {
      const collectedErrors: { title: string; message: string }[] = []
      for (let i = 0; i < selected.length; i++) {
        const candidate = selected[i]
        setActionMessage(
          t('actions.importStep', {
            index: i + 1,
            total: selected.length,
            name: candidate.name,
          }),
        )
        try {
          const created = await invokeTauri<InstallResultDto>(
            'install_local_selection',
            {
              basePath: localCandidatesBasePath,
              subpath: candidate.subpath,
              name: localName.trim() || undefined,
            },
          )
          await applySelectedAddModalTags(created.skill_id, created.name)
          {
            const selectedInstalledIds = tools
              .filter((tool) => syncTargets[tool.id] && isInstalled(tool.id))
              .map((t) => t.id)
            const targets = uniqueToolIdsBySkillsDir(selectedInstalledIds)
              .map((id) => tools.find((t) => t.id === id))
              .filter(Boolean) as ToolOption[]
            if (targets.length === 0) {
              collectedErrors.push({
                title: t('errors.unsyncedTitle', { name: created.name }),
                message: t('errors.noSyncTargets'),
              })
            } else {
              for (let ti = 0; ti < targets.length; ti++) {
                const tool = targets[ti]
                setActionMessage(
                  t('actions.syncStep', {
                    index: ti + 1,
                    total: targets.length,
                    name: created.name,
                    tool: tool.label,
                  }),
                )
                try {
                  await invokeTauri('sync_skill_to_tool', {
                    sourcePath: created.central_path,
                    skillId: created.skill_id,
                    tool: tool.id,
                    name: created.name,
                    overwriteIfSameContent: true,
                  })
                } catch (err) {
                  const raw = err instanceof Error ? err.message : String(err)
                  collectedErrors.push({
                    title: t('errors.syncFailedTitle', {
                      name: created.name,
                      tool: tool.label,
                    }),
                    message: raw,
                  })
                }
              }
            }
          }
        } catch (err) {
          const raw = err instanceof Error ? err.message : String(err)
          collectedErrors.push({
            title: t('errors.importFailedTitle', { name: candidate.name }),
            message: raw,
          })
        }
      }

      setShowLocalPickModal(false)
      setLocalCandidates([])
      setLocalCandidateSelected({})
      setLocalCandidatesBasePath('')
      setLocalPath('')
      setLocalName('')
      setActionMessage(t('status.selectedSkillsInstalled'))
      setSuccessToastMessage(t('status.selectedSkillsInstalled'))
      setActionMessage(null)
      setShowAddModal(false)
      await loadManagedSkills()
      await loadTags()
      if (collectedErrors.length > 0) showActionErrors(collectedErrors)
    } finally {
      setLoading(false)
      setLoadingStartAt(null)
    }
  }

  const handleInstallSelectedCandidates = async () => {
    const selected = gitCandidates.filter((c) => gitCandidateSelected[c.subpath])
    if (selected.length === 0) {
      setError(t('errors.selectAtLeastOneSkill'))
      return
    }
    const duplicated = selected.find((c) => isSkillNameTaken(c.name))
    if (duplicated) {
      setError(t('errors.skillAlreadyExists', { name: duplicated.name }))
      return
    }
    if (selected.length > 1 && gitName.trim()) {
      setError(t('errors.multiSelectNoCustomName'))
      return
    }

    setLoading(true)
    setLoadingStartAt(Date.now())
    setError(null)
    try {
      const collectedErrors: { title: string; message: string }[] = []
      for (let i = 0; i < selected.length; i++) {
        const candidate = selected[i]
        setActionMessage(
          t('actions.importStep', {
            index: i + 1,
            total: selected.length,
            name: candidate.name,
          }),
        )
        try {
          const created = await invokeTauri<InstallResultDto>(
            'install_git_selection',
            {
            repoUrl: gitCandidatesRepoUrl,
            subpath: candidate.subpath,
            name: gitName.trim() || undefined,
            },
          )
          await applySelectedAddModalTags(created.skill_id, created.name)
          {
            const selectedInstalledIds = tools
              .filter((tool) => syncTargets[tool.id] && isInstalled(tool.id))
              .map((t) => t.id)
            const targets = uniqueToolIdsBySkillsDir(selectedInstalledIds)
              .map((id) => tools.find((t) => t.id === id))
              .filter(Boolean) as ToolOption[]
            if (targets.length === 0) {
              collectedErrors.push({
                title: t('errors.unsyncedTitle', { name: created.name }),
                message: t('errors.noSyncTargets'),
              })
            } else {
              for (let ti = 0; ti < targets.length; ti++) {
                const tool = targets[ti]
                setActionMessage(
                  t('actions.syncStep', {
                    index: ti + 1,
                    total: targets.length,
                    name: created.name,
                    tool: tool.label,
                  }),
                )
                try {
                  await invokeTauri('sync_skill_to_tool', {
                    sourcePath: created.central_path,
                    skillId: created.skill_id,
                    tool: tool.id,
                    name: created.name,
                    overwriteIfSameContent: true,
                  })
                } catch (err) {
                  const raw = err instanceof Error ? err.message : String(err)
                  collectedErrors.push({
                    title: t('errors.syncFailedTitle', {
                      name: created.name,
                      tool: tool.label,
                    }),
                    message: raw,
                  })
                }
              }
            }
          }
        } catch (err) {
          const raw = err instanceof Error ? err.message : String(err)
          collectedErrors.push({
            title: t('errors.importFailedTitle', { name: candidate.name }),
            message: raw,
          })
        }
      }

      setShowGitPickModal(false)
      setGitCandidates([])
      setGitCandidateSelected({})
      setGitCandidatesRepoUrl('')
      setGitUrl('')
      setGitName('')
      setActionMessage(t('status.selectedSkillsInstalled'))
      setSuccessToastMessage(t('status.selectedSkillsInstalled'))
      setActionMessage(null)
      setShowGitPickModal(false)
      setGitCandidates([])
      setGitCandidateSelected({})
      setGitCandidatesRepoUrl('')
      setShowAddModal(false)
      await loadManagedSkills()
      await loadTags()
      if (collectedErrors.length > 0) showActionErrors(collectedErrors)
    } finally {
      setLoading(false)
      setLoadingStartAt(null)
    }
  }

  const handleDeleteManaged = async (skill: ManagedSkill) => {
    setLoading(true)
    setLoadingStartAt(Date.now())
    setActionMessage(t('actions.removing', { name: skill.name }))
    setError(null)
    try {
      await invokeTauri('delete_managed_skill', { skillId: skill.id })
      setActionMessage(t('status.skillRemoved'))
      setSuccessToastMessage(t('status.skillRemoved'))
      setActionMessage(null)
      setSkillScopeState((prev) => {
        const next = { ...prev }
        delete next[skill.id]
        return next
      })
      await loadManagedSkills()
      await loadTags()
      setPendingDeleteId(null)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setLoading(false)
      setLoadingStartAt(null)
    }
  }

  const handleSyncAllManagedToTools = useCallback(
    async (toolIds: string[]) => {
      if (managedSkills.length === 0) return
      const installedIds = uniqueToolIdsBySkillsDir(
        toolIds.filter((id) => isInstalled(id)),
      )
      if (installedIds.length === 0) return

      setLoading(true)
      setLoadingStartAt(Date.now())
      setError(null)
      try {
        const collectedErrors: { title: string; message: string }[] = []
        for (let si = 0; si < managedSkills.length; si++) {
          const skill = managedSkills[si]
          const skillScope = getSkillScope(skill)
          const projects = getSkillProjects(skill)
          for (let ti = 0; ti < installedIds.length; ti++) {
            const toolId = installedIds[ti]
            const toolLabel = tools.find((t) => t.id === toolId)?.label ?? toolId
            if (skillScope === 'project') {
              if (!toolSupportsProjectScope(toolId)) continue
              if (projects.length === 0) continue
            }
            setActionMessage(
              t('actions.syncStep', {
                index: si + 1,
                total: managedSkills.length,
                name: skill.name,
                tool: toolLabel,
              }),
            )
            try {
              if (skillScope === 'project') {
                for (const projectPath of projects) {
                  await invokeTauri('sync_skill_to_tool', {
                    sourcePath: skill.central_path,
                    skillId: skill.id,
                    tool: toolId,
                    name: skill.name,
                    overwriteIfSameContent: true,
                    scope: 'project',
                    projectPath,
                  })
                }
              } else {
                await invokeTauri('sync_skill_to_tool', {
                  sourcePath: skill.central_path,
                  skillId: skill.id,
                  tool: toolId,
                  name: skill.name,
                  overwriteIfSameContent: true,
                  scope: 'global',
                })
              }
            } catch (err) {
              const raw = err instanceof Error ? err.message : String(err)
              if (raw.startsWith('TOOL_NOT_INSTALLED|') || raw.startsWith('TOOL_NOT_WRITABLE|')) {
                continue
              }
              collectedErrors.push({
                title: t('errors.syncFailedTitle', {
                  name: skill.name,
                  tool: toolLabel,
                }),
                message: raw,
              })
            }
          }
        }
        setActionMessage(t('status.syncCompleted'))
        setSuccessToastMessage(t('status.syncCompleted'))
        setActionMessage(null)
        await loadManagedSkills()
        if (collectedErrors.length > 0) showActionErrors(collectedErrors)
      } finally {
        setLoading(false)
        setLoadingStartAt(null)
      }
    },
    [
      invokeTauri,
      getSkillProjects,
      getSkillScope,
      isInstalled,
      loadManagedSkills,
      managedSkills,
      showActionErrors,
      t,
      tools,
      toolSupportsProjectScope,
      uniqueToolIdsBySkillsDir,
    ],
  )

  const handleSyncAllNewTools = useCallback(() => {
    if (!toolStatus) return
    setSyncTargets((prev) => {
      const next = { ...prev }
      for (const id of toolStatus.newly_installed) {
        const shared = sharedToolIdsByToolId[id] ?? [id]
        for (const sid of shared) next[sid] = true
      }
      return next
    })
    setShowNewToolsModal(false)
    void handleSyncAllManagedToTools(toolStatus.newly_installed)
  }, [handleSyncAllManagedToTools, sharedToolIdsByToolId, toolStatus])

  const handleOpenScope = useCallback((skill: ManagedSkill) => {
    setScopeModalSkill(skill)
  }, [])

  const handleCloseScope = useCallback(() => {
    if (!loading) setScopeModalSkill(null)
  }, [loading])

  const setSkillScopeAndProjects = useCallback(
    (skillId: string, scope: 'global' | 'project', projects: string[]) => {
      const uniqueProjects = Array.from(new Set(projects.filter(Boolean)))
      setSkillScopeState((prev) => ({
        ...prev,
        [skillId]: {
          scope,
          projects: uniqueProjects,
        },
      }))
    },
    [],
  )

  const handleScopeChange = useCallback(
    async (nextScope: 'global' | 'project', nextProjects: string[]) => {
      const skill = scopeModalSkill
      if (!skill || loading) return
      const projects = Array.from(new Set(nextProjects.filter(Boolean)))
      const hasStaleTargets = skill.targets.some(
        (target) =>
          (target.scope ?? 'global') !== nextScope ||
          (nextScope === 'project' &&
            (target.scope ?? 'global') === 'project' &&
            (!target.project_path || !projects.includes(target.project_path))),
      )
      const activeTargets = skill.targets.filter(
        (target) =>
          (target.scope ?? 'global') !== nextScope ||
          (nextScope === 'project' &&
            (target.scope ?? 'global') === 'project' &&
            (!target.project_path || !projects.includes(target.project_path))),
      )
      const existingProjects = getSkillProjects(skill)
      const projectsChanged =
        projects.length !== existingProjects.length ||
        projects.some((project) => !existingProjects.includes(project))
      if (getSkillScope(skill) === nextScope && !hasStaleTargets && !projectsChanged) {
        return
      }

      setLoading(true)
      setLoadingStartAt(Date.now())
      setError(null)
      try {
        const seen = new Set<string>()
        for (const target of activeTargets) {
          const targetScope = target.scope ?? 'global'
          const key = `${target.tool}|${targetScope}|${target.project_path ?? ''}`
          if (seen.has(key)) continue
          seen.add(key)
          await invokeTauri('unsync_skill_from_tool', {
            skillId: skill.id,
            tool: target.tool,
            scope: targetScope,
            projectPath: target.project_path ?? undefined,
          })
        }
        if (nextScope === 'project' && projects.length > 0) {
          for (const toolId of installedProjectToolIds) {
            for (const projectPath of projects) {
              await invokeTauri('sync_skill_to_tool', {
                sourcePath: skill.central_path,
                skillId: skill.id,
                tool: toolId,
                name: skill.name,
                overwriteIfSameContent: true,
                scope: 'project',
                projectPath,
              })
            }
          }
        } else if (nextScope === 'global') {
          for (const toolId of installedToolIds) {
            try {
                await invokeTauri('sync_skill_to_tool', {
                  sourcePath: skill.central_path,
                  skillId: skill.id,
                  tool: toolId,
                  name: skill.name,
                  overwriteIfSameContent: true,
                  scope: 'global',
                })
            } catch (err) {
              const raw = err instanceof Error ? err.message : String(err)
              if (raw.startsWith('TOOL_NOT_INSTALLED|')) continue
              throw err
            }
          }
        }
        await loadManagedSkills()
        if (nextScope === 'project') {
          for (const projectPath of projects) {
            const saved = await invokeTauri<string[]>('save_recent_project', {
              projectPath,
            })
            setRecentProjects(saved)
          }
        }
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err))
        return
      } finally {
        setLoading(false)
        setLoadingStartAt(null)
      }

      setSkillScopeAndProjects(
        skill.id,
        nextScope,
        nextScope === 'project' ? projects : [],
      )
      setScopeModalSkill(null)
    },
    [
      getSkillProjects,
      getSkillScope,
      installedToolIds,
      installedProjectToolIds,
      invokeTauri,
      loadManagedSkills,
      loading,
      scopeModalSkill,
      setSkillScopeAndProjects,
    ],
  )

  const handlePickProject = useCallback(async () => {
    if (!scopeModalSkill) return undefined
    try {
      if (!isTauri) throw new Error(t('errors.notTauri'))
      const { open } = await import('@tauri-apps/plugin-dialog')
      const selected = await open({
        directory: true,
        multiple: false,
        title: t('projectSync.selectProjectTitle'),
      })
      if (!selected || Array.isArray(selected)) return undefined
      return selected
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
      return undefined
    }
  }, [isTauri, scopeModalSkill, t])

  const runToggleToolForSkill = useCallback(
    async (skill: ManagedSkill, toolId: string) => {
      if (loading) return
      const toolLabel = tools.find((t) => t.id === toolId)?.label ?? toolId
      const skillScope = getSkillScope(skill)
      const projects = getSkillProjects(skill)
      if (skillScope === 'project') {
        if (!toolSupportsProjectScope(toolId)) {
          setError(t('projectSync.unsupportedTool', { tool: toolLabel }))
          return
        }
        if (projects.length === 0) {
          setError(t('projectSync.noProjectsForSync'))
          setScopeModalSkill(skill)
          return
        }
      }
      const matchingTargets = skill.targets.filter(
        (target) => target.tool === toolId && (target.scope ?? 'global') === skillScope,
      )
      const synced = matchingTargets.length > 0

      setLoading(true)
      setLoadingStartAt(Date.now())
      setError(null)
      try {
        if (synced) {
          setActionMessage(
            t('actions.unsyncing', { name: skill.name, tool: toolLabel }),
          )
          if (skillScope === 'project') {
            const targetProjects = Array.from(
              new Set(
                matchingTargets
                  .map((target) => target.project_path)
                  .filter((path): path is string => Boolean(path)),
              ),
            )
            for (const projectPath of targetProjects) {
              await invokeTauri('unsync_skill_from_tool', {
                skillId: skill.id,
                tool: toolId,
                scope: 'project',
                projectPath,
              })
            }
          } else {
            await invokeTauri('unsync_skill_from_tool', {
              skillId: skill.id,
              tool: toolId,
              scope: 'global',
            })
          }
        } else {
          setActionMessage(
            t('actions.syncing', { name: skill.name, tool: toolLabel }),
          )
          if (skillScope === 'project') {
            for (const projectPath of projects) {
              await invokeTauri('sync_skill_to_tool', {
                sourcePath: skill.central_path,
                skillId: skill.id,
                tool: toolId,
                name: skill.name,
                overwriteIfSameContent: true,
                scope: 'project',
                projectPath,
              })
            }
          } else {
            await invokeTauri('sync_skill_to_tool', {
              sourcePath: skill.central_path,
              skillId: skill.id,
              tool: toolId,
              name: skill.name,
              overwriteIfSameContent: true,
              scope: 'global',
            })
          }
        }
        const statusText = synced
          ? t('status.syncDisabled')
          : t('status.syncEnabled')
        setActionMessage(statusText)
        setSuccessToastMessage(statusText)
        setActionMessage(null)
        await loadManagedSkills()
      } catch (err) {
        const raw = err instanceof Error ? err.message : String(err)
        if (raw.startsWith('TARGET_EXISTS|')) {
          const targetPath = raw.split('|')[1] ?? ''
          setError(t('errors.targetExistsDetail', { path: targetPath }))
        } else if (raw.startsWith('TOOL_NOT_INSTALLED|')) {
          setError(t('errors.toolNotInstalled'))
        } else if (raw.startsWith('TOOL_NOT_WRITABLE|')) {
          const parts = raw.split('|')
          setError(t('errors.toolNotWritable', { tool: parts[1] ?? '', path: parts[2] ?? '' }))
        } else {
          setError(raw)
        }
      } finally {
        setLoading(false)
        setLoadingStartAt(null)
      }
    },
    [
      getSkillProjects,
      getSkillScope,
      invokeTauri,
      loadManagedSkills,
      loading,
      t,
      tools,
      toolSupportsProjectScope,
    ],
  )

  const handleToggleToolForSkill = useCallback(
    (skill: ManagedSkill, toolId: string) => {
      if (loading) return
      const skillScope = getSkillScope(skill)
      const currentTarget = skill.targets.find(
        (target) => target.tool === toolId && (target.scope ?? 'global') === skillScope,
      )
      const shared = currentTarget
        ? skill.targets
            .filter(
              (target) =>
                (target.scope ?? 'global') === skillScope &&
                target.target_path === currentTarget.target_path,
            )
            .map((target) => target.tool)
        : sharedToolIdsByToolId[toolId] ?? null
      if (shared && shared.length > 1) {
        setPendingSharedToggle({ skill, toolId, affectedToolIds: shared })
        return
      }
      void runToggleToolForSkill(skill, toolId)
    },
    [getSkillScope, loading, runToggleToolForSkill, sharedToolIdsByToolId],
  )

  const handleUpdateManaged = useCallback(
    async (skill: ManagedSkill) => {
    setLoading(true)
    setLoadingStartAt(Date.now())
    setError(null)
    try {
      setActionMessage(t('actions.updating', { name: skill.name }))
      await invokeTauri<UpdateResultDto>('update_managed_skill', { skillId: skill.id })
      const updatedText = t('status.updated', { name: skill.name })
      setActionMessage(updatedText)
      setSuccessToastMessage(updatedText)
      setActionMessage(null)
      await loadManagedSkills()
    } catch (err) {
      const raw = err instanceof Error ? err.message : String(err)
      setError(raw)
    } finally {
      setLoading(false)
      setLoadingStartAt(null)
    }
    },
    [invokeTauri, loadManagedSkills, t],
  )

  const handleUpdateSkill = useCallback(
    (skill: ManagedSkill) => {
      void handleUpdateManaged(skill)
    },
    [handleUpdateManaged],
  )

  const handleSharedCancel = useCallback(() => {
    if (loading) return
    setPendingSharedToggle(null)
  }, [loading])

  const handleSharedConfirm = useCallback(() => {
    if (!pendingSharedToggle) return
    const payload = pendingSharedToggle
    setPendingSharedToggle(null)
    void runToggleToolForSkill(payload.skill, payload.toolId)
  }, [pendingSharedToggle, runToggleToolForSkill])

  const pendingSharedLabels = useMemo(() => {
    if (!pendingSharedToggle) return null
    const toolId = pendingSharedToggle.toolId
    const shared = pendingSharedToggle.affectedToolIds ?? sharedToolIdsByToolId[toolId] ?? []
    const others = shared.filter((id) => id !== toolId)
    return {
      toolLabel: toolLabelById[toolId] ?? toolId,
      otherLabels: others.map((id) => toolLabelById[id] ?? id).join(', '),
    }
  }, [pendingSharedToggle, sharedToolIdsByToolId, toolLabelById])

  const currentScopeModalSkill = useMemo(() => {
    if (!scopeModalSkill) return null
    return managedSkills.find((skill) => skill.id === scopeModalSkill.id) ?? scopeModalSkill
  }, [managedSkills, scopeModalSkill])

  return (
    <div className="skills-app">
      <Toaster
        position="top-right"
        richColors
        toastOptions={{ duration: 1800 }}
      />
      <LoadingOverlay
        loading={loading}
        actionMessage={actionMessage}
        loadingStartAt={loadingStartAt}
        onCancel={handleCancelLoading}
        t={t}
      />

      <Header
        language={language}
        loading={loading}
        activeView={activeView}
        onToggleLanguage={toggleLanguage}
        onOpenSettings={handleOpenSettings}
        onViewChange={handleViewChange}
        t={t}
      />

      <main className="skills-main">
        {activeView === 'detail' && detailSkill ? (
          <SkillDetailView
            skill={detailSkill}
            onBack={handleBackToList}
            invokeTauri={invokeTauri}
            formatRelative={formatRelative}
            t={t}
          />
        ) : activeView === 'myskills' ? (
          <div className="dashboard-stack">
            <FilterBar
              sortBy={sortBy}
              searchQuery={searchQuery}
              scopeFilter={scopeFilter}
              tags={tags}
              selectedTagIds={selectedTagIds}
              includeUntagged={includeUntagged}
              untaggedCount={untaggedCount}
              totalCount={visibleSkills.length}
              onSortChange={handleSortChange}
              onSearchChange={handleSearchChange}
              onScopeFilterChange={handleScopeFilterChange}
              onToggleTag={handleToggleTagFilter}
              onToggleUntagged={handleToggleUntaggedFilter}
              onClearTags={handleClearTagFilters}
              onManageTags={handleOpenTagsPage}
              t={t}
            />
            <SkillsList
              plan={plan}
              visibleSkills={visibleSkills}
              installedTools={installedTools}
              loading={loading}
              getGithubInfo={getGithubInfo}
              getSkillSourceLabel={getSkillSourceLabel}
              formatRelative={formatRelative}
              onReviewImport={handleReviewImport}
              onUpdateSkill={handleUpdateSkill}
              onDeleteSkill={handleDeletePrompt}
              onToggleTool={handleToggleToolForSkill}
              onOpenScope={handleOpenScope}
              onOpenDetail={handleOpenDetail}
              onEditTags={handleOpenEditTags}
              getSkillScope={getSkillScope}
              getSkillProjects={getSkillProjects}
              t={t}
            />
          </div>
        ) : activeView === 'tags' ? (
          <TagsPage
            tags={tags}
            untaggedCount={untaggedCount}
            loading={loading}
            formatRelative={formatRelative}
            onBack={handleBackToList}
            onReviewUntagged={handleReviewUntagged}
            onViewTag={handleViewTag}
            onCreateTag={handleCreateTag}
            onRenameTag={handleRenameTag}
            onDeleteTag={handleDeleteTag}
            t={t}
          />
        ) : activeView === 'settings' ? (
          <SettingsPage
            isTauri={isTauri}
            language={language}
            storagePath={storagePath}
            gitCacheCleanupDays={gitCacheCleanupDays}
            gitCacheTtlSecs={gitCacheTtlSecs}
            themePreference={themePreference}
            onPickStoragePath={handlePickStoragePath}
            onToggleLanguage={toggleLanguage}
            onThemeChange={handleThemeChange}
            onGitCacheCleanupDaysChange={handleGitCacheCleanupDaysChange}
            onGitCacheTtlSecsChange={handleGitCacheTtlSecsChange}
            onClearGitCacheNow={handleClearGitCacheNow}
            githubToken={githubToken}
            onGithubTokenChange={handleGithubTokenChange}
            onBack={handleCloseSettings}
            t={t}
          />
        ) : activeView === 'moreskills' ? (
          <MoreSkillsPage
            loading={loading}
            onOpenManualAdd={handleOpenAdd}
            onOpenSite={(url) => void handleOpenSkillSourceSite(url)}
            t={t}
          />
        ) : EXPLORE_ENABLED && activeView === 'explore' ? (
          <ExplorePage
            featuredSkills={featuredSkills}
            featuredLoading={featuredLoading}
            exploreFilter={exploreFilter}
            searchResults={searchResults}
            searchLoading={searchLoading}
            managedSkills={managedSkills}
            loading={loading}
            onExploreFilterChange={handleExploreFilterChange}
            onInstallSkill={handleExploreInstall}
            onOpenManualAdd={handleOpenAdd}
            t={t}
          />
        ) : null}
      </main>

      <AddSkillModal
        open={showAddModal}
        loading={loading}
        canClose={!loading}
        addModalTab={addModalTab}
        localPath={localPath}
        localName={localName}
        gitUrl={gitUrl}
        gitName={gitName}
        tags={tags}
        selectedTagIds={addModalTagIds}
        syncTargets={syncTargets}
        installedTools={installedTools}
        toolStatus={toolStatus}
        onRequestClose={handleCloseAdd}
        onTabChange={setAddModalTab}
        onLocalPathChange={setLocalPath}
        onPickLocalPath={handlePickLocalPath}
        onLocalNameChange={setLocalName}
        onGitUrlChange={setGitUrl}
        onGitNameChange={setGitName}
        onToggleTag={handleToggleAddModalTag}
        onSyncTargetChange={handleSyncTargetChange}
        onSubmit={addModalTab === 'local' ? handleCreateLocal : handleCreateGit}
        t={t}
      />

      <EditSkillTagsModal
        key={
          tagEditorSkill
            ? `${tagEditorSkill.id}-${tagEditorSkill.tags.map((tag) => tag.id).join('-')}`
            : 'edit-tags'
        }
        open={Boolean(tagEditorSkill)}
        loading={loading}
        skill={
          tagEditorSkill
            ? managedSkills.find((skill) => skill.id === tagEditorSkill.id) ?? tagEditorSkill
            : null
        }
        tags={tags}
        onRequestClose={handleCloseEditTags}
        onSave={handleSaveSkillTags}
        t={t}
      />

      {showImportModal && plan ? (
        <ImportModal
          open={showImportModal}
          loading={loading}
          plan={plan}
          selected={selected}
          variantChoice={variantChoice}
          onRequestClose={handleCloseImport}
          onToggleGroup={handleToggleGroup}
          onSelectVariant={handleSelectVariant}
          onImport={handleImport}
          t={t}
        />
      ) : null}

      <SharedDirModal
        open={Boolean(pendingSharedToggle)}
        loading={loading}
        toolLabel={pendingSharedLabels?.toolLabel ?? ''}
        otherLabels={pendingSharedLabels?.otherLabels ?? ''}
        onRequestClose={handleSharedCancel}
        onConfirm={handleSharedConfirm}
        t={t}
      />

      <ScopeSyncModal
        key={
          currentScopeModalSkill
            ? `${currentScopeModalSkill.id}-${getSkillScope(currentScopeModalSkill)}`
            : 'scope-modal'
        }
        open={Boolean(currentScopeModalSkill)}
        loading={loading}
        skill={currentScopeModalSkill}
        scope={
          currentScopeModalSkill ? getSkillScope(currentScopeModalSkill) : 'global'
        }
        projects={
          currentScopeModalSkill ? getSkillProjects(currentScopeModalSkill) : []
        }
        recentProjects={recentProjects}
        onRequestClose={handleCloseScope}
        onScopeChange={handleScopeChange}
        onPickProject={handlePickProject}
        t={t}
      />

      <NewToolsModal
        open={Boolean(showNewToolsModal && newlyInstalledToolsText)}
        loading={loading}
        toolsLabelText={newlyInstalledToolsText}
        onLater={handleCloseNewTools}
        onSyncAll={handleSyncAllNewTools}
        t={t}
      />

      <DeleteModal
        open={Boolean(pendingDeleteId)}
        loading={loading}
        skillName={pendingDeleteSkill?.name ?? null}
        onRequestClose={handleCloseDelete}
        onConfirm={() => {
          if (pendingDeleteSkill) void handleDeleteManaged(pendingDeleteSkill)
        }}
        t={t}
      />

      {pendingDeleteTag ? (
        <div className="modal-backdrop" onClick={loading ? undefined : handleCloseDeleteTag}>
          <div
            className="modal modal-delete tag-delete-modal"
            onClick={(event) => event.stopPropagation()}
          >
            <div className="modal-header">
              <div className="modal-title">{t('deleteTagTitle')}</div>
              <button
                className="modal-close"
                type="button"
                onClick={handleCloseDeleteTag}
                disabled={loading}
              >
                ×
              </button>
            </div>
            <div className="modal-body tag-delete-body">
              {t('deleteTagConfirm', {
                name: pendingDeleteTag.name,
                count: pendingDeleteTag.skill_count,
              })}
            </div>
            <div className="modal-footer">
              <button
                className="btn btn-secondary"
                type="button"
                onClick={handleCloseDeleteTag}
                disabled={loading}
              >
                {t('cancel')}
              </button>
              <button
                className="btn btn-danger"
                type="button"
                onClick={() => void handleConfirmDeleteTag()}
                disabled={loading}
              >
                {t('deleteAction')}
              </button>
            </div>
          </div>
        </div>
      ) : null}

      {showLocalPickModal ? (
        <LocalPickModal
          open={showLocalPickModal}
          loading={loading}
          localCandidates={localCandidates}
          localCandidateSelected={localCandidateSelected}
          onRequestClose={handleCloseLocalPick}
          onCancel={handleCancelLocalPick}
          onToggleCandidate={handleToggleLocalCandidate}
          onInstall={handleInstallSelectedLocalCandidates}
          t={t}
        />
      ) : null}

      {showGitPickModal ? (
        <GitPickModal
          open={showGitPickModal}
          loading={loading}
          gitCandidates={gitCandidates}
          gitCandidateSelected={gitCandidateSelected}
          onRequestClose={handleCloseGitPick}
          onCancel={handleCancelGitPick}
          onToggleCandidate={handleToggleGitCandidate}
          onInstall={handleInstallSelectedCandidates}
          t={t}
        />
      ) : null}

      {updateAvailableVersion && (
        <div className="modal-backdrop" onClick={updateInstalling ? undefined : handleDismissUpdate}>
          <div
            className="modal update-modal"
            role="dialog"
            aria-modal="true"
            onClick={(e) => e.stopPropagation()}
          >
            {!updateInstalling && !updateDone && (
              <button
                className="modal-close update-modal-close"
                type="button"
                onClick={handleDismissUpdate}
                aria-label={t('close')}
              >
                ✕
              </button>
            )}
            <div className="update-modal-body">
              <div className="update-modal-title">
                {updateDone ? t('updateInstalledRestart') : t('updateAvailable')}
              </div>
              {!updateDone && (
                <div className="update-modal-text">
                  {t('updateBannerText', { version: updateAvailableVersion })}
                </div>
              )}
              {!updateDone && updateBody && (
                <div className="update-modal-notes">
                  <Markdown remarkPlugins={[remarkGfm]}>{updateBody}</Markdown>
                </div>
              )}
            </div>
            <div className="update-modal-actions">
              {updateDone ? (
                <button
                  className="btn btn-primary"
                  type="button"
                  onClick={handleDismissUpdate}
                >
                  {t('done')}
                </button>
              ) : (
                <>
                  <button
                    className="btn btn-primary"
                    type="button"
                    disabled={updateInstalling}
                    onClick={handleUpdateNow}
                  >
                    {updateInstalling ? t('installingUpdate') : t('updateNow')}
                  </button>
                  {!updateInstalling && (
                    <button
                      className="btn btn-secondary"
                      type="button"
                      onClick={handleDismissUpdateForever}
                    >
                      {t('updateBannerDismiss')}
                    </button>
                  )}
                </>
              )}
            </div>
          </div>
        </div>
      )}
      </div>
  )
}

export default App
