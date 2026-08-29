import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type MutableRefObject,
  type SetStateAction,
} from 'react'
import type { DownloadOptions, Update } from '@tauri-apps/plugin-updater'
import './App.css'
import './figma.css'
import { useTranslation } from 'react-i18next'
import { Toaster, toast } from 'sonner'
import Markdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import ExplorePage from './components/skills/ExplorePage'
import FilterBar from './components/skills/FilterBar'
import SkillDetailView from './components/skills/SkillDetailView'
import Header from './components/skills/Header'
import LoadingOverlay from './components/skills/LoadingOverlay'
import SkillsList from './components/skills/SkillsList'
import TagsPage from './components/skills/TagsPage'
import AddSkillModal from './components/skills/modals/AddSkillModal'
import BulkDeleteModal from './components/skills/modals/BulkDeleteModal'
import BulkSyncModal from './components/skills/modals/BulkSyncModal'
import BulkTagsModal from './components/skills/modals/BulkTagsModal'
import DeleteModal from './components/skills/modals/DeleteModal'
import DiscoveryScanModal from './components/skills/modals/DiscoveryScanModal'
import EditSkillTagsModal from './components/skills/modals/EditSkillTagsModal'
import GitPickModal from './components/skills/modals/GitPickModal'
import LocalPickModal from './components/skills/modals/LocalPickModal'
import ImportModal from './components/skills/modals/ImportModal'
import NewToolsModal from './components/skills/modals/NewToolsModal'
import RenameTagModal from './components/skills/modals/RenameTagModal'
import ScopeSyncModal from './components/skills/modals/ScopeSyncModal'
import SharedDirModal from './components/skills/modals/SharedDirModal'
import SettingsPage from './components/skills/SettingsPage'
import ToolsPage from './components/skills/ToolsPage'
import UpdatesPage from './components/skills/UpdatesPage'
import WindowResizeHandles from './components/WindowResizeHandles'
import {
  getAutoUpdateToastKey,
  shouldKeepWaitingForTriggeredAutoUpdate,
} from './components/skills/autoUpdateSettings'
import {
  getSkillSyncState,
  getToolSyncState,
  isActiveSkillTarget,
} from './components/skills/skillSyncStatus'
import {
  buildInstallSyncJobs,
  filterTargetsForScope,
  getAddedProjectPaths,
  isLatestSaveBatch,
  normalizeProjectSharedTargets,
  normalizeProjectPaths,
  resolveProjectPathsUpdate,
  selectInstallToolIds,
  type InstallScope,
} from './components/skills/installScope'
import type {
  AutoUpdateConfigDto,
  DiscoveryScanSettingsDto,
  FeaturedSkillDto,
  GitSkillCandidate,
  GithubProxyConfigDto,
  InstallResultDto,
  LocalSkillCandidate,
  ManagedSkill,
  OnboardingPlan,
  OnlineSkillDto,
  TagWithCountDto,
  ToolConfigDto,
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

type ActiveView = 'myskills' | 'explore' | 'detail' | 'settings' | 'manage'
type ManagementTab = 'tags' | 'tools' | 'updates'
type UpdaterProxyOptions = { proxy?: string }
type UpdaterDownloadOptions = DownloadOptions & UpdaterProxyOptions

const buildUpdaterProxyOptions = (
  enabled: boolean,
  url: string,
): UpdaterProxyOptions | undefined => {
  const proxy = enabled ? url.trim() : ''
  return proxy ? { proxy } : undefined
}

function App() {
  const { t, i18n } = useTranslation()
  const language = i18n.resolvedLanguage ?? i18n.language ?? 'en'
  const languageStorageKey = 'skills-language'
  const themeStorageKey = 'skills-theme'
  const skillScopeStorageKey = 'skills-project-scope-state-v1'
  const skillViewModeStorageKey = 'skills-view-mode'
  const sidebarCollapsedStorageKey = 'skills-sidebar-collapsed'
  const toggleLanguage = useCallback(() => {
    void i18n.changeLanguage(language === 'en' ? 'zh' : 'en')
  }, [i18n, language])

  useEffect(() => {
    document.documentElement.lang = language.startsWith('zh') ? 'zh-CN' : 'en'
  }, [language])
  const [themePreference, setThemePreference] = useState<'system' | 'light' | 'dark'>(
    'system',
  )
  const [appVersion, setAppVersion] = useState('')
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
  const [toolConfig, setToolConfig] = useState<ToolConfigDto | null>(null)
  const [discoveryScanSettings, setDiscoveryScanSettings] =
    useState<DiscoveryScanSettingsDto | null>(null)
  const [showDiscoveryScanModal, setShowDiscoveryScanModal] = useState(false)
  const [discoveryScanSaving, setDiscoveryScanSaving] = useState(false)
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
  const [updateChecking, setUpdateChecking] = useState(false)
  const [updateInstalling, setUpdateInstalling] = useState(false)
  const [updateDone, setUpdateDone] = useState(false)
  const [showAppUpdateModal, setShowAppUpdateModal] = useState(false)
  const updateObjRef = useRef<Update | null>(null) as MutableRefObject<Update | null>
  const [searchQuery, setSearchQuery] = useState('')
  const [sortBy, setSortBy] = useState<'updated' | 'name'>('updated')
  const [scopeFilter, setScopeFilter] = useState<'all' | 'global' | 'project'>('all')
  const [skillViewMode, setSkillViewMode] = useState<'list' | 'cards'>(() =>
    typeof window !== 'undefined' && window.localStorage.getItem(skillViewModeStorageKey) === 'cards'
      ? 'cards'
      : 'list',
  )
  const [sidebarCollapsed, setSidebarCollapsed] = useState(() =>
    typeof window !== 'undefined' && window.localStorage.getItem(sidebarCollapsedStorageKey) === 'true',
  )
  const [activeView, setActiveView] = useState<ActiveView>('myskills')
  const [managementTab, setManagementTab] = useState<ManagementTab>('tags')
  const [detailSkill, setDetailSkill] = useState<ManagedSkill | null>(null)
  const [tags, setTags] = useState<TagWithCountDto[]>([])
  const [selectedTagIds, setSelectedTagIds] = useState<number[]>([])
  const [includeUntagged, setIncludeUntagged] = useState(false)
  const [tagEditorSkill, setTagEditorSkill] = useState<ManagedSkill | null>(null)
  const [pendingRenameTag, setPendingRenameTag] = useState<TagWithCountDto | null>(null)
  const [pendingDeleteTag, setPendingDeleteTag] = useState<TagWithCountDto | null>(null)
  const [pendingSyncTargetChange, setPendingSyncTargetChange] = useState<{
    toolId: string
    checked: boolean
    affected: string[]
    shared: string[]
  } | null>(null)
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
  const [installScope, setInstallScope] = useState<InstallScope>('global')
  const [installProjects, setInstallProjects] = useState<string[]>([])
  const installProjectsRef = useRef<string[]>([])
  const installProjectsSaveSequenceRef = useRef(0)
  const [skillScopeState, setSkillScopeState] = useState<SkillScopeState>({})
  const [bulkMode, setBulkMode] = useState(false)
  const [bulkSelectedIds, setBulkSelectedIds] = useState<string[]>([])
  const [showBulkSyncModal, setShowBulkSyncModal] = useState(false)
  const [showBulkDeleteModal, setShowBulkDeleteModal] = useState(false)
  const [showBulkTagsModal, setShowBulkTagsModal] = useState(false)
  const [bulkSyncToolIds, setBulkSyncToolIds] = useState<string[]>([])

  const isTauri =
    typeof window !== 'undefined' &&
    Boolean(
      (window as { __TAURI__?: unknown }).__TAURI__ ||
        (window as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__,
    )

  useEffect(() => {
    try {
      window.localStorage.setItem(skillViewModeStorageKey, skillViewMode)
    } catch {
      // ignore storage failures
    }
  }, [skillViewMode])

  useEffect(() => {
    try {
      window.localStorage.setItem(sidebarCollapsedStorageKey, String(sidebarCollapsed))
    } catch {
      // ignore storage failures
    }
  }, [sidebarCollapsed])

  useEffect(() => {
    if (!isTauri) return
    let active = true
    void import('@tauri-apps/api/app')
      .then(({ getVersion }) => getVersion())
      .then((version) => {
        if (active) setAppVersion(version)
      })
      .catch(() => undefined)
    return () => {
      active = false
    }
  }, [isTauri])

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

  const loadPlan = useCallback(async (showLoading = true) => {
    if (showLoading) {
      setLoading(true)
      setLoadingStartAt(Date.now())
    }
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
      if (showLoading) {
        setLoading(false)
        setLoadingStartAt(null)
      }
    }
  }, [invokeTauri])

  const loadDiscoveryScanSettings = useCallback(async () => {
    if (!isTauri) return null
    try {
      const result = await invokeTauri<DiscoveryScanSettingsDto>(
        'get_discovery_scan_settings',
      )
      setDiscoveryScanSettings(result)
      return result
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
      return null
    }
  }, [invokeTauri, isTauri])

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
    if (!isTauri) return
    invokeTauri<GithubProxyConfigDto>('get_github_proxy_config')
      .then((config) => setGithubProxyConfig(config))
      .catch((err) => {
        setError(err instanceof Error ? err.message : String(err))
      })
      .finally(() => setGithubProxyConfigLoaded(true))
  }, [isTauri, invokeTauri])

  useEffect(() => {
    if (!isTauri) return
    invokeTauri<AutoUpdateConfigDto>('get_auto_update_config')
      .then((config) => setAutoUpdateConfig(config))
      .catch((err) => {
        setError(err instanceof Error ? err.message : String(err))
      })
  }, [isTauri, invokeTauri])

  useEffect(() => {
    if (!isTauri) return
    invokeTauri<ToolConfigDto>('get_tool_config')
      .then((config) => setToolConfig(config))
      .catch((err) => {
        setError(err instanceof Error ? err.message : String(err))
      })
  }, [isTauri, invokeTauri])

  useEffect(() => {
    void loadDiscoveryScanSettings()
  }, [loadDiscoveryScanSettings])

  useEffect(() => {
    if (isTauri) {
      void loadPlan(false)
    }
  }, [isTauri, loadPlan])

  const handleDismissUpdate = useCallback(() => {
    setShowAppUpdateModal(false)
  }, [])

  const handleOpenUpdate = useCallback(() => {
    if (updateAvailableVersion) setShowAppUpdateModal(true)
  }, [updateAvailableVersion])

  const handleDismissUpdateForever = useCallback(() => {
    if (updateAvailableVersion) {
      localStorage.setItem('skills-ignored-update-version', updateAvailableVersion)
    }
    updateObjRef.current = null
    setShowAppUpdateModal(false)
    setUpdateAvailableVersion(null)
    setUpdateBody(null)
  }, [updateAvailableVersion])

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
  const enabledToolInfos = useMemo(
    () => toolInfos.filter((info) => info.enabled),
    [toolInfos],
  )

  const tools: ToolOption[] = useMemo(() => {
    return enabledToolInfos.map((info) => ({
      id: info.key,
      // Prefer i18n label if present; fallback to backend label.
      label: t(`tools.${info.key}`, { defaultValue: info.label }),
      avatar: info.avatar,
      supports_project_scope: info.supports_project_scope,
    }))
  }, [t, enabledToolInfos])

  const toolLabelById = useMemo(() => {
    const out: Record<string, string> = {}
    for (const tool of tools) out[tool.id] = tool.label
    return out
  }, [tools])

  const sharedToolIdsByToolId = useMemo(() => {
    // toolId -> all toolIds that share the same skills_dir.
    const byDir: Record<string, string[]> = {}
    for (const info of enabledToolInfos) {
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
  }, [enabledToolInfos])

  const sharedProjectToolIdsByToolId = useMemo(() => {
    const byDir: Record<string, string[]> = {}
    for (const info of enabledToolInfos) {
      const dir = info.project_skills_dir
      if (!byDir[dir]) byDir[dir] = []
      byDir[dir].push(info.key)
    }
    const out: Record<string, string[]> = {}
    for (const ids of Object.values(byDir)) {
      if (ids.length <= 1) continue
      for (const id of ids) out[id] = ids
    }
    return out
  }, [enabledToolInfos])

  const uniqueToolIdsBySkillsDir = useCallback(
    (toolIds: string[]) => {
      // Preserve UI order (tools array order), de-dupe by skills_dir.
      const wanted = new Set(toolIds)
      const seen = new Set<string>()
      const out: string[] = []
      for (const tool of enabledToolInfos) {
        if (!wanted.has(tool.key)) continue
        if (seen.has(tool.skills_dir)) continue
        seen.add(tool.skills_dir)
        out.push(tool.key)
      }
      return out
    },
    [enabledToolInfos],
  )

  const uniqueToolIdsByProjectSkillsDir = useCallback(
    (toolIds: string[]) => {
      const wanted = new Set(toolIds)
      const seen = new Set<string>()
      const out: string[] = []
      for (const tool of enabledToolInfos) {
        if (!wanted.has(tool.key)) continue
        if (seen.has(tool.project_skills_dir)) continue
        seen.add(tool.project_skills_dir)
        out.push(tool.key)
      }
      return out
    },
    [enabledToolInfos],
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

  const syncInstalledSkill = useCallback(
    async (
      created: InstallResultDto,
    ): Promise<{ title: string; message: string }[]> => {
      const toolIds = selectInstallToolIds(
        tools,
        syncTargets,
        installedToolIds,
        installScope,
        uniqueToolIdsBySkillsDir,
        uniqueToolIdsByProjectSkillsDir,
      )
      const jobs = buildInstallSyncJobs(
        toolIds,
        installScope,
        installProjects,
      )

      if (jobs.length === 0) {
        return [
          {
            title: t('errors.unsyncedTitle', { name: created.name }),
            message:
              installScope === 'project' &&
              normalizeProjectPaths(installProjects).length === 0
                ? t('projectSync.projectRequired')
                : t('errors.noSyncTargets'),
          },
        ]
      }

      const collectedErrors: { title: string; message: string }[] = []
      for (let index = 0; index < jobs.length; index++) {
        const job = jobs[index]
        const toolLabel = toolLabelById[job.toolId] ?? job.toolId
        setActionMessage(
          t('actions.syncStep', {
            index: index + 1,
            total: jobs.length,
            name: created.name,
            tool: toolLabel,
          }),
        )
        try {
          await invokeTauri('sync_skill_to_tool', {
            sourcePath: created.central_path,
            skillId: created.skill_id,
            tool: job.toolId,
            name: created.name,
            scope: job.scope,
            ...(job.scope === 'project'
              ? { projectPath: job.projectPath }
              : {}),
            overwriteIfSameContent: true,
          })
        } catch (err) {
          collectedErrors.push({
            title: t('errors.syncFailedTitle', {
              name: created.name,
              tool: toolLabel,
            }),
            message: err instanceof Error ? err.message : String(err),
          })
        }
      }

      return collectedErrors
    },
    [
      installProjects,
      installScope,
      installedToolIds,
      invokeTauri,
      syncTargets,
      t,
      toolLabelById,
      tools,
      uniqueToolIdsBySkillsDir,
      uniqueToolIdsByProjectSkillsDir,
    ],
  )

  const resetInstallScope = useCallback(() => {
    setInstallScope('global')
    installProjectsRef.current = []
    installProjectsSaveSequenceRef.current += 1
    setInstallProjects([])
  }, [])

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
  const bulkSelectedSkills = useMemo(() => {
    const selectedSet = new Set(bulkSelectedIds)
    return managedSkills.filter((skill) => selectedSet.has(skill.id))
  }, [bulkSelectedIds, managedSkills])

  const bulkSelectedNames = useMemo(
    () => bulkSelectedSkills.map((skill) => skill.name),
    [bulkSelectedSkills],
  )
  const bulkHasDisabledSelected = useMemo(
    () => bulkSelectedSkills.some((skill) => skill.enabled === false),
    [bulkSelectedSkills],
  )
  const bulkShouldEnable = useMemo(
    () =>
      bulkSelectedSkills.length > 0 &&
      bulkSelectedSkills.every((skill) => skill.enabled === false),
    [bulkSelectedSkills],
  )
  const allVisibleBulkSelected = useMemo(() => {
    if (visibleSkills.length === 0) return false
    const selectedSet = new Set(bulkSelectedIds)
    return visibleSkills.every((skill) => selectedSet.has(skill.id))
  }, [bulkSelectedIds, visibleSkills])

  useEffect(() => {
    const existingIds = new Set(managedSkills.map((skill) => skill.id))
    setBulkSelectedIds((current) => current.filter((id) => existingIds.has(id)))
  }, [managedSkills])

  const [storagePath, setStoragePath] = useState<string>(t('notAvailable'))
  const [gitCacheCleanupDays, setGitCacheCleanupDays] = useState<number>(30)
  const [gitCacheTtlSecs, setGitCacheTtlSecs] = useState<number>(60)
  const [githubToken, setGithubToken] = useState<string>('')
  const [githubProxyConfig, setGithubProxyConfig] =
    useState<GithubProxyConfigDto>({
      enabled: false,
      port: 7890,
      url: '',
      auto_detected: false,
    })
  const [githubProxyConfigLoaded, setGithubProxyConfigLoaded] = useState(false)
  const [autoUpdateConfig, setAutoUpdateConfig] =
    useState<AutoUpdateConfigDto | null>(null)
  const [autoUpdateTriggering, setAutoUpdateTriggering] = useState(false)
  const autoUpdateLastRunRef = useRef<number | null>(null)
  const updaterProxyOptions = useMemo(
    () => buildUpdaterProxyOptions(githubProxyConfig.enabled, githubProxyConfig.url),
    [githubProxyConfig.enabled, githubProxyConfig.url],
  )

  useEffect(() => {
    if (!isTauri || !githubProxyConfigLoaded) return
    let cancelled = false
    const ignoredVersion = localStorage.getItem('skills-ignored-update-version')
    setUpdateChecking(true)
    void import('@tauri-apps/plugin-updater')
      .then(({ check }) => check(updaterProxyOptions))
      .then(async (update) => {
        if (cancelled) return
        if (update && update.version !== ignoredVersion) {
          updateObjRef.current = update
          setUpdateAvailableVersion(update.version)
          setUpdateDone(false)
          try {
            const body = await invokeTauri<string | null>('get_github_release_notes', {
              version: update.version,
            })
            if (cancelled) return
            setUpdateBody(body ?? update.body ?? null)
          } catch {
            if (cancelled) return
            setUpdateBody(update.body ?? null)
          }
        }
      })
      .catch(() => {})
      .finally(() => {
        if (!cancelled) setUpdateChecking(false)
      })
    return () => {
      cancelled = true
    }
  }, [githubProxyConfigLoaded, invokeTauri, isTauri, updaterProxyOptions])

  const handleUpdateNow = useCallback(async () => {
    const update = updateObjRef.current
    if (!update) return
    setUpdateInstalling(true)
    try {
      await update.downloadAndInstall(
        undefined,
        updaterProxyOptions as UpdaterDownloadOptions | undefined,
      )
      setUpdateInstalling(false)
      setUpdateDone(true)
    } catch (err) {
      setUpdateInstalling(false)
      toast.error(err instanceof Error ? err.message : String(err), { duration: 3200 })
    }
  }, [updaterProxyOptions])

  useEffect(() => {
    if (!isTauri) return
    if (activeView !== 'manage' || managementTab !== 'updates') return

    let cancelled = false
    const refreshAutoUpdateConfig = async () => {
      const config = await invokeTauri<AutoUpdateConfigDto>('get_auto_update_config')
      if (cancelled) return

      const previousLastRun = autoUpdateLastRunRef.current
      const nextLastRun = config.last_run_at ?? null
      autoUpdateLastRunRef.current = nextLastRun
      setAutoUpdateConfig(config)

      if (
        previousLastRun !== null &&
        nextLastRun !== null &&
        nextLastRun !== previousLastRun &&
        config.last_status !== 'running'
      ) {
        await loadManagedSkills()
      }
    }

    void refreshAutoUpdateConfig().catch(() => {})
    const timer = window.setInterval(() => {
      void refreshAutoUpdateConfig().catch(() => {})
    }, autoUpdateConfig?.last_status === 'running' ? 2000 : 10000)

    return () => {
      cancelled = true
      window.clearInterval(timer)
    }
  }, [
    activeView,
    managementTab,
    autoUpdateConfig?.last_status,
    invokeTauri,
    isTauri,
    loadManagedSkills,
  ])

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
  const handleGithubProxyConfigChange = useCallback(
    async (enabled: boolean, port: number) => {
      const normalizedPort = Math.max(1, Math.min(Math.round(port), 65535))
      setGithubProxyConfig((prev) => ({
        ...prev,
        enabled,
        port: normalizedPort,
        url: enabled ? `http://127.0.0.1:${normalizedPort}` : '',
        auto_detected: false,
      }))
      if (!isTauri) return
      try {
        const saved = await invokeTauri<GithubProxyConfigDto>(
          'set_github_proxy_config',
          {
            enabled,
            port: normalizedPort,
          },
        )
        setGithubProxyConfig(saved)
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err))
        void invokeTauri<GithubProxyConfigDto>('get_github_proxy_config')
          .then((config) => setGithubProxyConfig(config))
          .catch(() => {})
      }
    },
    [invokeTauri, isTauri],
  )
  const handleToolConfigChange = useCallback(
    async (nextConfig: ToolConfigDto) => {
      setToolConfig(nextConfig)
      if (!isTauri) return true
      try {
        const saved = await invokeTauri<ToolConfigDto>('set_tool_config', {
          config: nextConfig,
        })
        setToolConfig(saved)
        const status = await invokeTauri<ToolStatusDto>('get_tool_status')
        setToolStatus(status)
        setSyncTargets((prev) => {
          const installed = new Set(status.installed)
          const next: Record<string, boolean> = {}
          for (const info of status.tools) {
            next[info.key] = Boolean(prev[info.key]) && installed.has(info.key)
          }
          return next
        })
        toast.success(t('toolManagement.saved'), { duration: 1600 })
        return true
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err))
        try {
          const [config, status] = await Promise.all([
            invokeTauri<ToolConfigDto>('get_tool_config'),
            invokeTauri<ToolStatusDto>('get_tool_status'),
          ])
          setToolConfig(config)
          setToolStatus(status)
        } catch {
          // Preserve the original save error.
        }
        return false
      }
    },
    [invokeTauri, isTauri, t],
  )
  const handleAutoUpdateConfigChange = useCallback(
    async (
      enabled: boolean,
      schedule: {
        scheduleType: 'interval' | 'daily'
        intervalValue: number
        intervalUnit: 'minutes' | 'hours'
        dailyTime: string
      },
    ) => {
      const normalizedIntervalValue = schedule.intervalUnit === 'minutes'
        ? Math.max(15, Math.min(schedule.intervalValue, 24 * 30 * 60))
        : Math.max(1, Math.min(schedule.intervalValue, 24 * 30))
      const normalizedDailyTime = /^\d{2}:\d{2}$/.test(schedule.dailyTime)
        ? schedule.dailyTime
        : '03:00'
      const normalizedIntervalHours = schedule.scheduleType === 'daily'
        ? 24
        : schedule.intervalUnit === 'minutes'
          ? Math.max(1, Math.ceil(normalizedIntervalValue / 60))
          : normalizedIntervalValue
      const previousEnabled = autoUpdateConfig?.enabled
      setAutoUpdateConfig((prev) => ({
        enabled,
        interval_hours: normalizedIntervalHours,
        schedule_type: schedule.scheduleType,
        interval_value: normalizedIntervalValue,
        interval_unit: schedule.intervalUnit,
        daily_time: normalizedDailyTime,
        local_skill_count: prev?.local_skill_count ?? 0,
        protected_local_skill_count: prev?.protected_local_skill_count ?? 0,
        task_registered: prev?.task_registered ?? false,
        task_status_detail: prev?.task_status_detail ?? '',
        last_run_at: prev?.last_run_at ?? null,
        last_started_at: prev?.last_started_at ?? null,
        last_finished_at: prev?.last_finished_at ?? null,
        last_status: prev?.last_status ?? null,
        last_error: prev?.last_error ?? null,
        last_checked: prev?.last_checked ?? 0,
        last_updated: prev?.last_updated ?? 0,
        last_failed: prev?.last_failed ?? 0,
        progress: prev?.progress ?? {
          total: 0,
          succeeded: [],
          failed: [],
          running: null,
          pending: [],
        },
      }))
      if (!isTauri) return
      try {
        const updated = await invokeTauri<AutoUpdateConfigDto>(
          'set_auto_update_config',
          {
            enabled,
            intervalHours: normalizedIntervalHours,
            scheduleType: schedule.scheduleType,
            intervalValue: normalizedIntervalValue,
            intervalUnit: schedule.intervalUnit,
            dailyTime: normalizedDailyTime,
          },
        )
        setAutoUpdateConfig(updated)
        setSuccessToastMessage(t(getAutoUpdateToastKey(previousEnabled, enabled)))
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err))
        void invokeTauri<AutoUpdateConfigDto>('get_auto_update_config')
          .then((config) => setAutoUpdateConfig(config))
          .catch(() => {})
      }
    },
    [autoUpdateConfig?.enabled, invokeTauri, isTauri, t],
  )
  const handleTriggerAutoUpdateTaskNow = useCallback(async () => {
    if (!isTauri || autoUpdateTriggering) return
    const triggeredAt = Date.now()
    setAutoUpdateTriggering(true)
    setError(null)
    setAutoUpdateConfig((prev) => prev ? {
      ...prev,
      last_run_at: triggeredAt,
      last_started_at: triggeredAt,
      last_finished_at: null,
      last_status: 'running',
      last_error: null,
      last_checked: 0,
      last_updated: 0,
      last_failed: 0,
      progress: {
        total: 0,
        succeeded: [],
        failed: [],
        running: null,
        pending: [],
      },
    } : prev)
    try {
      await invokeTauri('trigger_auto_update_task_now_cmd')
      setSuccessToastMessage(t('autoUpdateTaskTriggered'))
      let sawRunning = false
      for (let attempt = 0; attempt < 30; attempt += 1) {
        await new Promise((resolve) => window.setTimeout(resolve, 2000))
        const latestConfig = await invokeTauri<AutoUpdateConfigDto>(
          'get_auto_update_config',
        )
        if (latestConfig.last_status === 'running') {
          sawRunning = true
        }
        const keepWaiting = shouldKeepWaitingForTriggeredAutoUpdate(
          latestConfig,
          triggeredAt,
          sawRunning,
        )
        if (keepWaiting && latestConfig.last_status !== 'running') {
          continue
        }
        setAutoUpdateConfig(latestConfig)
        if (!keepWaiting) {
          await loadManagedSkills()
          break
        }
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setAutoUpdateTriggering(false)
    }
  }, [autoUpdateTriggering, invokeTauri, isTauri, loadManagedSkills, t])
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
    setShowAddModal(false)
    setActiveView('settings')
  }, [])

  const handleOpenDiscoveryScanSettings = useCallback(() => {
    setShowDiscoveryScanModal(true)
    void loadDiscoveryScanSettings()
  }, [loadDiscoveryScanSettings])

  const handleCloseDiscoveryScanSettings = useCallback(() => {
    if (!discoveryScanSaving) setShowDiscoveryScanModal(false)
  }, [discoveryScanSaving])

  const handleSaveDiscoveryScanSettings = useCallback(
    async (disabledSourceKeys: string[]) => {
      if (!isTauri) return
      setDiscoveryScanSaving(true)
      try {
        const saved = await invokeTauri<DiscoveryScanSettingsDto>(
          'set_discovery_scan_config',
          {
            config: { disabled_source_keys: disabledSourceKeys },
          },
        )
        setDiscoveryScanSettings(saved)
        await loadPlan(false)
        setShowDiscoveryScanModal(false)
        toast.success(t('discoveryScan.saved'), { duration: 1600 })
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err))
      } finally {
        setDiscoveryScanSaving(false)
      }
    },
    [invokeTauri, isTauri, loadPlan, t],
  )

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
    (view: 'myskills' | 'explore' | 'manage') => {
      setShowAddModal(false)
      setActiveView(view)
      if (view !== 'myskills') {
        setBulkMode(false)
        setBulkSelectedIds([])
        setShowBulkSyncModal(false)
        setShowBulkDeleteModal(false)
        setShowBulkTagsModal(false)
      }
      if (view === 'explore') {
        loadFeaturedSkills()
      }
      if (view === 'manage') {
        setManagementTab('tags')
      }
      if (view === 'myskills') {
        setDetailSkill(null)
      }
    },
    [loadFeaturedSkills],
  )

  const handleOpenDetail = useCallback((skill: ManagedSkill) => {
    setBulkMode(false)
    setBulkSelectedIds([])
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


  const handleOpenAdd = useCallback((tab: 'git' | 'local' = 'git') => {
    resetInstallScope()
    setAddModalTab(tab)
    setShowAddModal(true)
    setAddModalTagIds([])
  }, [resetInstallScope])

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
      resetInstallScope()
    }
  }, [loading, resetInstallScope])

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
    setShowAddModal(true)
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
    setShowAddModal(true)
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
    setManagementTab('tags')
    setActiveView('manage')
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

  const handleToggleBulkMode = useCallback(() => {
    setBulkMode((current) => {
      const next = !current
      if (!next) {
        setBulkSelectedIds([])
        setShowBulkSyncModal(false)
        setShowBulkDeleteModal(false)
        setShowBulkTagsModal(false)
      }
      return next
    })
  }, [])

  const handleToggleBulkSelection = useCallback((skillId: string) => {
    setBulkSelectedIds((current) =>
      current.includes(skillId)
        ? current.filter((id) => id !== skillId)
        : [...current, skillId],
    )
  }, [])

  const handleSelectVisibleSkills = useCallback(() => {
    setBulkMode(true)
    setBulkSelectedIds((current) => {
      const visibleIds = visibleSkills.map((skill) => skill.id)
      const visibleSet = new Set(visibleIds)
      const allVisibleSelected =
        visibleIds.length > 0 && visibleIds.every((id) => current.includes(id))
      if (allVisibleSelected) {
        return current.filter((id) => !visibleSet.has(id))
      }
      return Array.from(new Set([...current, ...visibleIds]))
    })
  }, [visibleSkills])

  const handleOpenBulkSync = useCallback(() => {
    if (bulkSelectedIds.length === 0) return
    if (bulkSelectedSkills.some((skill) => skill.enabled === false)) {
      toast.error(t('bulk.enableBeforeSync'))
      return
    }
    const installedToolSet = new Set(installedToolIds)
    const selectedToolSet = new Set<string>()
    for (const skill of bulkSelectedSkills) {
      const skillScope = getSkillScope(skill)
      for (const target of skill.targets) {
        if ((target.scope ?? 'global') !== skillScope) continue
        if (!installedToolSet.has(target.tool)) continue
        selectedToolSet.add(target.tool)
      }
    }
    setBulkSyncToolIds(
      installedTools
        .map((tool) => tool.id)
        .filter((toolId) => selectedToolSet.has(toolId)),
    )
    setShowBulkSyncModal(true)
  }, [
    bulkSelectedIds.length,
    bulkSelectedSkills,
    getSkillScope,
    installedToolIds,
    installedTools,
    t,
  ])

  const handleToggleBulkSyncTool = useCallback((toolId: string) => {
    setBulkSyncToolIds((current) =>
      current.includes(toolId)
        ? current.filter((id) => id !== toolId)
        : [...current, toolId],
    )
  }, [])

  const handleCloseBulkSync = useCallback(() => {
    if (!loading) setShowBulkSyncModal(false)
  }, [loading])

  const handleOpenBulkDelete = useCallback(() => {
    if (bulkSelectedIds.length === 0) return
    setShowBulkDeleteModal(true)
  }, [bulkSelectedIds.length])

  const handleCloseBulkDelete = useCallback(() => {
    if (!loading) setShowBulkDeleteModal(false)
  }, [loading])

  const handleOpenBulkTags = useCallback(() => {
    if (bulkSelectedIds.length === 0) return
    setShowBulkTagsModal(true)
  }, [bulkSelectedIds.length])

  const handleCloseBulkTags = useCallback(() => {
    if (!loading) setShowBulkTagsModal(false)
  }, [loading])

  const handleConfirmBulkTags = useCallback(
    async (addTagIds: number[], removeTagIds: number[]) => {
      if (bulkSelectedSkills.length === 0) return
      if (addTagIds.length === 0 && removeTagIds.length === 0) return

      const addSet = new Set(addTagIds)
      const removeSet = new Set(removeTagIds)
      const errors: { title: string; message: string }[] = []
      setLoading(true)
      setLoadingStartAt(Date.now())
      setError(null)
      try {
        for (let index = 0; index < bulkSelectedSkills.length; index++) {
          const skill = bulkSelectedSkills[index]
          setActionMessage(
            t('bulk.tagsProgress', {
              current: index + 1,
              total: bulkSelectedSkills.length,
              name: skill.name,
            }),
          )
          const nextTagIds = new Set<number>()
          for (const tag of skill.tags) {
            if (!removeSet.has(tag.id)) nextTagIds.add(tag.id)
          }
          for (const tagId of addSet) nextTagIds.add(tagId)

          try {
            await invokeTauri('set_skill_tags', {
              skillId: skill.id,
              tagIds: Array.from(nextTagIds),
            })
          } catch (err) {
            errors.push({
              title: t('bulk.tagsFailedTitle', { name: skill.name }),
              message: err instanceof Error ? err.message : String(err),
            })
          }
        }

        await loadManagedSkills()
        await loadTags()
        setShowBulkTagsModal(false)
        if (errors.length > 0) {
          showActionErrors(errors)
        } else {
          setBulkSelectedIds([])
          setBulkMode(false)
          setSuccessToastMessage(
            t('bulk.tagsSuccess', { count: bulkSelectedSkills.length }),
          )
        }
      } finally {
        setLoading(false)
        setLoadingStartAt(null)
        setActionMessage(null)
      }
    },
    [
      bulkSelectedSkills,
      invokeTauri,
      loadManagedSkills,
      loadTags,
      showActionErrors,
      t,
    ],
  )

  const restoreSkillSavedTargets = useCallback(
    async (skill: ManagedSkill) => {
      const seen = new Set<string>()
      for (const target of skill.targets) {
        const scope = target.scope === 'project' ? 'project' : 'global'
        const projectPath = target.project_path ?? undefined
        const key = `${target.tool}|${scope}|${projectPath ?? ''}`
        if (seen.has(key)) continue
        seen.add(key)

        if (scope === 'global' && !installedToolIds.includes(target.tool)) continue
        if (scope === 'project') {
          if (!projectPath) continue
          if (!toolSupportsProjectScope(target.tool)) continue
        }

        await invokeTauri('sync_skill_to_tool', {
          sourcePath: skill.central_path,
          skillId: skill.id,
          tool: target.tool,
          name: skill.name,
          overwriteIfSameContent: true,
          scope,
          ...(scope === 'project' ? { projectPath } : {}),
        })
      }
    },
    [installedToolIds, invokeTauri, toolSupportsProjectScope],
  )

  const handleToggleSkillEnabled = useCallback(
    async (skill: ManagedSkill) => {
      const nextEnabled = skill.enabled === false
      setLoading(true)
      setLoadingStartAt(Date.now())
      setError(null)
      setActionMessage(
        nextEnabled
          ? t('bulk.enableProgress', { current: 1, total: 1, name: skill.name })
          : t('bulk.disableProgress', { current: 1, total: 1, name: skill.name }),
      )
      try {
        await invokeTauri('set_skill_enabled', {
          skillId: skill.id,
          enabled: nextEnabled,
        })
        if (nextEnabled) {
          await restoreSkillSavedTargets(skill)
        }
        await loadManagedSkills()
        setSuccessToastMessage(
          nextEnabled
            ? t('skillEnabled', { name: skill.name })
            : t('skillDisabled', { name: skill.name }),
        )
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err))
      } finally {
        setLoading(false)
        setLoadingStartAt(null)
        setActionMessage(null)
      }
    },
    [invokeTauri, loadManagedSkills, restoreSkillSavedTargets, t],
  )

  const handleToggleBulkEnabled = useCallback(async () => {
    if (bulkSelectedSkills.length === 0) return
    const nextEnabled = bulkShouldEnable
    const affectedSkills = bulkSelectedSkills.filter(
      (skill) => (skill.enabled !== false) !== nextEnabled,
    )
    if (affectedSkills.length === 0) return

    const errors: { title: string; message: string }[] = []
    setLoading(true)
    setLoadingStartAt(Date.now())
    setError(null)
    try {
      for (let index = 0; index < affectedSkills.length; index++) {
        const skill = affectedSkills[index]
        setActionMessage(
          nextEnabled
            ? t('bulk.enableProgress', {
                current: index + 1,
                total: affectedSkills.length,
                name: skill.name,
              })
            : t('bulk.disableProgress', {
                current: index + 1,
                total: affectedSkills.length,
                name: skill.name,
              }),
        )
        try {
          await invokeTauri('set_skill_enabled', {
            skillId: skill.id,
            enabled: nextEnabled,
          })
          if (nextEnabled) {
            await restoreSkillSavedTargets(skill)
          }
        } catch (err) {
          errors.push({
            title: nextEnabled
              ? t('bulk.enableFailedTitle', { name: skill.name })
              : t('bulk.disableFailedTitle', { name: skill.name }),
            message: err instanceof Error ? err.message : String(err),
          })
        }
      }
      await loadManagedSkills()
      if (errors.length > 0) {
        showActionErrors(errors)
      } else {
        setBulkSelectedIds([])
        setBulkMode(false)
        setSuccessToastMessage(
          nextEnabled
            ? t('bulk.enableSuccess', { count: affectedSkills.length })
            : t('bulk.disableSuccess', { count: affectedSkills.length }),
        )
      }
    } finally {
      setLoading(false)
      setLoadingStartAt(null)
      setActionMessage(null)
    }
  }, [
    bulkSelectedSkills,
    bulkShouldEnable,
    invokeTauri,
    loadManagedSkills,
    restoreSkillSavedTargets,
    showActionErrors,
    t,
  ])

  const handleConfirmBulkSync = useCallback(async () => {
    if (bulkSelectedSkills.length === 0) return

    const errors: { title: string; message: string }[] = []
    setLoading(true)
    setLoadingStartAt(Date.now())
    setError(null)
    try {
      let completed = 0
      const selectedBaseToolSet = new Set(bulkSyncToolIds)
      const total = Math.max(1, bulkSelectedSkills.length * installedToolIds.length)
      for (const skill of bulkSelectedSkills) {
        const skillScope = getSkillScope(skill)
        const projects = getSkillProjects(skill)
        const sharedByToolId =
          skillScope === 'project'
            ? sharedProjectToolIdsByToolId
            : sharedToolIdsByToolId
        const selectedToolSet = new Set<string>()
        for (const toolId of selectedBaseToolSet) {
          const shared = sharedByToolId[toolId] ?? [toolId]
          for (const id of shared) selectedToolSet.add(id)
        }

        const activeInstalledToolIds =
          skillScope === 'project' ? installedProjectToolIds : installedToolIds

        const targetsToRemove = skill.targets.filter(
          (target) =>
            (target.scope ?? 'global') === skillScope &&
            activeInstalledToolIds.includes(target.tool) &&
            !selectedToolSet.has(target.tool),
        )

        const seenRemoveKeys = new Set<string>()
        for (const target of targetsToRemove) {
          const key = `${target.tool}|${target.scope}|${target.project_path ?? ''}`
          if (seenRemoveKeys.has(key)) continue
          seenRemoveKeys.add(key)
          completed += 1
          const toolLabel = toolLabelById[target.tool] ?? target.tool
          setActionMessage(
            t('bulk.unsyncProgress', {
              current: Math.min(completed, total),
              total,
              name: skill.name,
              tool: toolLabel,
            }),
          )
          try {
            await invokeTauri('unsync_skill_from_tool', {
              skillId: skill.id,
              tool: target.tool,
              scope: skillScope,
              projectPath: target.project_path ?? undefined,
            })
          } catch (err) {
            errors.push({
              title: t('errors.syncFailedTitle', {
                name: skill.name,
                tool: toolLabel,
              }),
              message: err instanceof Error ? err.message : String(err),
            })
          }
        }

        for (const toolId of Array.from(selectedToolSet).filter((id) =>
          activeInstalledToolIds.includes(id),
        )) {
          completed += 1
          const toolLabel = toolLabelById[toolId] ?? toolId
          setActionMessage(
            t('bulk.syncProgress', {
              current: Math.min(completed, total),
              total,
              name: skill.name,
              tool: toolLabel,
            }),
          )
          try {
            if (skillScope === 'project') {
              if (!toolSupportsProjectScope(toolId)) {
                throw new Error(t('projectSync.unsupportedTool', { tool: toolLabel }))
              }
              if (projects.length === 0) {
                throw new Error(t('projectSync.noProjectsForSync'))
              }
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
            errors.push({
              title: t('errors.syncFailedTitle', {
                name: skill.name,
                tool: toolLabel,
              }),
              message: err instanceof Error ? err.message : String(err),
            })
          }
        }
      }
      await loadManagedSkills()
      setShowBulkSyncModal(false)
      setBulkSyncToolIds([])
      if (errors.length > 0) {
        showActionErrors(errors)
      } else {
        setBulkSelectedIds([])
        setBulkMode(false)
        setSuccessToastMessage(
          t('bulk.syncSuccess', {
            count: bulkSelectedSkills.length,
            tools: bulkSyncToolIds.length,
          }),
        )
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setLoading(false)
      setLoadingStartAt(null)
      setActionMessage(null)
    }
  }, [
    bulkSelectedSkills,
    bulkSyncToolIds,
    getSkillProjects,
    getSkillScope,
    installedProjectToolIds,
    installedToolIds,
    invokeTauri,
    loadManagedSkills,
    sharedProjectToolIdsByToolId,
    sharedToolIdsByToolId,
    showActionErrors,
    t,
    toolLabelById,
    toolSupportsProjectScope,
  ])

  const handleConfirmBulkDelete = useCallback(async () => {
    if (bulkSelectedSkills.length === 0) return

    const errors: { title: string; message: string }[] = []
    setLoading(true)
    setLoadingStartAt(Date.now())
    setError(null)
    try {
      for (let index = 0; index < bulkSelectedSkills.length; index++) {
        const skill = bulkSelectedSkills[index]
        setActionMessage(
          t('bulk.deleteProgress', {
            current: index + 1,
            total: bulkSelectedSkills.length,
            name: skill.name,
          }),
        )
        try {
          await invokeTauri('delete_managed_skill', { skillId: skill.id })
        } catch (err) {
          errors.push({
            title: skill.name,
            message: err instanceof Error ? err.message : String(err),
          })
        }
      }
      await loadManagedSkills()
      await loadTags()
      setShowBulkDeleteModal(false)
      if (errors.length > 0) {
        showActionErrors(errors)
      } else {
        setBulkSelectedIds([])
        setBulkMode(false)
        setSuccessToastMessage(
          t('bulk.deleteSuccess', { count: bulkSelectedSkills.length }),
        )
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setLoading(false)
      setLoadingStartAt(null)
      setActionMessage(null)
    }
  }, [
    bulkSelectedSkills,
    invokeTauri,
    loadManagedSkills,
    loadTags,
    showActionErrors,
    t,
  ])

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
        setPendingRenameTag(null)
        setSuccessToastMessage(t('tagRenamed'))
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err))
      }
    },
    [invokeTauri, loadManagedSkills, loadTags, t],
  )

  const handleCloseRenameTag = useCallback(() => {
    if (!loading) setPendingRenameTag(null)
  }, [loading])

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

  const applySyncTargetChange = useCallback(
    (checked: boolean, affected: string[], shared: string[]) => {
      setSyncTargets((prev) => {
        const next = { ...prev }
        for (const id of affected) next[id] = checked
        if (installScope === 'project') {
          for (const id of shared) {
            if (!toolSupportsProjectScope(id)) next[id] = false
          }
        }
        return next
      })
    },
    [installScope, toolSupportsProjectScope],
  )

  const handleSyncTargetChange = useCallback(
    (toolId: string, checked: boolean) => {
      const sharedByToolId =
        installScope === 'project'
          ? sharedProjectToolIdsByToolId
          : sharedToolIdsByToolId
      const shared = sharedByToolId[toolId] ?? [toolId]
      const affected =
        installScope === 'project'
          ? shared.filter(
              (id) => isInstalled(id) && toolSupportsProjectScope(id),
            )
          : shared
      if (affected.length > 1) {
        setPendingSyncTargetChange({ toolId, checked, affected, shared })
        return
      }
      applySyncTargetChange(checked, affected, shared)
    },
    [
      applySyncTargetChange,
      installScope,
      isInstalled,
      sharedProjectToolIdsByToolId,
      sharedToolIdsByToolId,
      toolSupportsProjectScope,
    ],
  )

  const handleSyncTargetChangeCancel = useCallback(() => {
    if (!loading) setPendingSyncTargetChange(null)
  }, [loading])

  const handleSyncTargetChangeConfirm = useCallback(() => {
    if (!pendingSyncTargetChange) return
    const { checked, affected, shared } = pendingSyncTargetChange
    setPendingSyncTargetChange(null)
    applySyncTargetChange(checked, affected, shared)
  }, [applySyncTargetChange, pendingSyncTargetChange])

  const pendingSyncTargetLabels = useMemo(() => {
    if (!pendingSyncTargetChange) return null
    const others = pendingSyncTargetChange.affected.filter(
      (id) => id !== pendingSyncTargetChange.toolId,
    )
    return {
      toolLabel:
        toolLabelById[pendingSyncTargetChange.toolId] ??
        pendingSyncTargetChange.toolId,
      otherLabels: others.map((id) => toolLabelById[id] ?? id).join(', '),
    }
  }, [pendingSyncTargetChange, toolLabelById])

  const handleInstallScopeChange = useCallback(
    (nextScope: InstallScope) => {
      setInstallScope(nextScope)
      if (nextScope === 'project') {
        setSyncTargets((current) => {
          const filtered = filterTargetsForScope(
            current,
            installedTools,
            nextScope,
          )
          return normalizeProjectSharedTargets(
            filtered,
            installedTools,
            sharedProjectToolIdsByToolId,
          )
        })
      }
    },
    [installedTools, sharedProjectToolIdsByToolId],
  )

  const handleInstallProjectsChange = useCallback(
    async (nextProjects: SetStateAction<string[]>) => {
      const normalizedProjects = resolveProjectPathsUpdate(
        installProjectsRef.current,
        nextProjects,
      )
      const addedProjects = getAddedProjectPaths(
        installProjectsRef.current,
        normalizedProjects,
      )
      installProjectsRef.current = normalizedProjects
      setInstallProjects(normalizedProjects)
      if (addedProjects.length === 0) return

      const saveSequence = installProjectsSaveSequenceRef.current + 1
      installProjectsSaveSequenceRef.current = saveSequence
      try {
        let savedProjects: string[] | null = null
        for (const projectPath of addedProjects) {
          savedProjects = await invokeTauri<string[]>('save_recent_project', {
            projectPath,
          })
        }
        if (
          savedProjects &&
          isLatestSaveBatch(
            saveSequence,
            installProjectsSaveSequenceRef.current,
          )
        ) {
          setRecentProjects(savedProjects)
        }
      } catch (err) {
        if (
          isLatestSaveBatch(
            saveSequence,
            installProjectsSaveSequenceRef.current,
          )
        ) {
          setError(err instanceof Error ? err.message : String(err))
        }
      }
    },
    [invokeTauri],
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
        const syncErrors = await syncInstalledSkill(created)
        if (syncErrors.length > 0) showActionErrors(syncErrors)
        setLocalPath('')
        setLocalName('')
        setActionMessage(t('status.localSkillCreated'))
        setSuccessToastMessage(t('status.localSkillCreated'))
        setActionMessage(null)
        resetInstallScope()
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
        const syncErrors = await syncInstalledSkill(created)
        if (syncErrors.length > 0) showActionErrors(syncErrors)
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
          const syncErrors = await syncInstalledSkill(created)
          if (syncErrors.length > 0) showActionErrors(syncErrors)
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
            const syncErrors = await syncInstalledSkill(created)
            if (syncErrors.length > 0) showActionErrors(syncErrors)
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
      resetInstallScope()
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
      resetInstallScope()
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
    [resetInstallScope, toolStatus],
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
          const syncErrors = await syncInstalledSkill(created)
          collectedErrors.push(...syncErrors)
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
      resetInstallScope()
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
          const syncErrors = await syncInstalledSkill(created)
          collectedErrors.push(...syncErrors)
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
      resetInstallScope()
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
              collectedErrors.push({
                title: t('errors.syncFailedTitle', {
                  name: skill.name,
                  tool: toolLabel,
                }),
                message: raw.startsWith('TOOL_NOT_INSTALLED|')
                  ? t('errors.toolNotInstalled')
                  : raw.startsWith('TOOL_NOT_WRITABLE|')
                    ? t('errors.toolNotWritable', {
                        tool: raw.split('|')[1] ?? toolLabel,
                        path: raw.split('|')[2] ?? '',
                      })
                    : raw,
              })
            }
          }
        }
        await loadManagedSkills()
        if (collectedErrors.length > 0) {
          showActionErrors(collectedErrors)
        } else {
          setSuccessToastMessage(t('status.syncCompleted'))
        }
        setActionMessage(null)
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
        await loadManagedSkills()
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
  }, [isTauri, t])

  const runToggleToolForSkill = useCallback(
    async (skill: ManagedSkill, toolId: string) => {
      if (loading) return
      if (skill.enabled === false) {
        toast.error(t('bulk.enableBeforeSync'))
        return
      }
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
        (target) =>
          target.tool === toolId &&
          (target.scope ?? 'global') === skillScope &&
          isActiveSkillTarget(target),
      )
      const synced = getToolSyncState(skill, toolId, skillScope) === 'synced'

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
        await loadManagedSkills()
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
      await loadManagedSkills()
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

  const globalSkillCount = managedSkills.filter(
    (skill) => getSkillScope(skill) === 'global',
  ).length
  const projectSkillCount = managedSkills.length - globalSkillCount
  const enabledSkillCount = managedSkills.filter((skill) => skill.enabled !== false).length
  const syncIssueCount = managedSkills.filter((skill) => {
    const state = getSkillSyncState(skill)
    return state === 'source-error' || state === 'partial' || state === 'failed'
  }).length
  const unsyncedSkillCount = managedSkills.filter(
    (skill) => getSkillSyncState(skill) === 'idle',
  ).length
  const dashboardSyncStatus =
    syncIssueCount > 0
      ? { className: 'error', label: t('stats.needsAttention', { count: syncIssueCount }) }
      : unsyncedSkillCount > 0
        ? { className: 'idle', label: t('stats.notSyncedCount', { count: unsyncedSkillCount }) }
        : enabledSkillCount === managedSkills.length
          ? { className: 'healthy', label: t('stats.allNormal') }
          : { className: 'idle', label: t('stats.enabledCount', { count: enabledSkillCount }) }
  const pendingUpdateCount = autoUpdateConfig?.last_failed ?? 0

  const handleManagementTabChange = (tab: ManagementTab) => {
    setShowAddModal(false)
    setManagementTab(tab)
    setActiveView('manage')
  }

  return (
    <div className={`skills-app${isTauri ? ' is-tauri' : ''}${sidebarCollapsed ? ' sidebar-collapsed' : ''}`}>
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
        activeView={activeView}
        managementTab={managementTab}
        skillCount={managedSkills.length}
        tagCount={tags.length}
        toolCount={toolStatus?.tools.length ?? 0}
        updateCount={pendingUpdateCount}
        appVersion={appVersion}
        updateAvailableVersion={updateAvailableVersion}
        updateChecking={updateChecking}
        updateInstalling={updateInstalling}
        updateDone={updateDone}
        collapsed={sidebarCollapsed}
        onToggleCollapsed={() => setSidebarCollapsed((collapsed) => !collapsed)}
        onOpenSettings={handleOpenSettings}
        onOpenUpdate={handleOpenUpdate}
        onViewChange={handleViewChange}
        onManagementTabChange={handleManagementTabChange}
        t={t}
      />
      <WindowResizeHandles enabled={isTauri} />

      <main className="skills-main">
        {activeView === 'detail' && detailSkill ? (
          <SkillDetailView
            skill={detailSkill}
            onBack={handleBackToList}
            invokeTauri={invokeTauri}
            formatRelative={formatRelative}
            tools={installedTools}
            scope={getSkillScope(detailSkill)}
            projects={getSkillProjects(detailSkill)}
            t={t}
          />
        ) : activeView === 'myskills' ? (
          <div className="dashboard-stack">
            <section className="dashboard-stats" aria-label={t('navMySkills')}>
              <article>
                <span>{t('stats.managed')}</span>
                <strong>{managedSkills.length}</strong>
              </article>
              <article>
                <span>{t('stats.global')}</span>
                <strong>{globalSkillCount}</strong>
              </article>
              <article>
                <span>{t('stats.project')}</span>
                <strong>{projectSkillCount}</strong>
              </article>
              <article className={`dashboard-status-card${syncIssueCount > 0 ? ' actionable' : ''}`}>
                <button
                  className="dashboard-status-button"
                  type="button"
                  disabled={syncIssueCount === 0}
                  onClick={() => handleManagementTabChange('updates')}
                  aria-label={
                    syncIssueCount > 0
                      ? `${dashboardSyncStatus.label}，${t('stats.viewIssues')}`
                      : dashboardSyncStatus.label
                  }
                  title={syncIssueCount > 0 ? t('stats.viewIssues') : undefined}
                >
                  <span>{t('stats.syncStatus')}</span>
                  <strong className={`status-summary ${dashboardSyncStatus.className}`}>
                    <i />{dashboardSyncStatus.label}
                  </strong>
                  {syncIssueCount > 0 ? (
                    <span className="dashboard-status-action">
                      {t('stats.viewIssues')} <span aria-hidden="true">→</span>
                    </span>
                  ) : null}
                </button>
              </article>
            </section>
            <FilterBar
              sortBy={sortBy}
              searchQuery={searchQuery}
              scopeFilter={scopeFilter}
              tags={tags}
              selectedTagIds={selectedTagIds}
              includeUntagged={includeUntagged}
              untaggedCount={untaggedCount}
              totalCount={visibleSkills.length}
              bulkMode={bulkMode}
              bulkSelectedCount={bulkSelectedIds.length}
              viewMode={skillViewMode}
              onSortChange={handleSortChange}
              onSearchChange={handleSearchChange}
              onScopeFilterChange={handleScopeFilterChange}
              onToggleTag={handleToggleTagFilter}
              onToggleUntagged={handleToggleUntaggedFilter}
              onClearTags={handleClearTagFilters}
              onManageTags={handleOpenTagsPage}
              onToggleBulkMode={handleToggleBulkMode}
              onViewModeChange={setSkillViewMode}
              t={t}
            />
            <SkillsList
              plan={plan}
              visibleSkills={visibleSkills}
              installedTools={installedTools}
              loading={loading}
              bulkMode={bulkMode}
              selectedSkillIds={bulkSelectedIds}
              viewMode={skillViewMode}
              getGithubInfo={getGithubInfo}
              getSkillSourceLabel={getSkillSourceLabel}
              formatRelative={formatRelative}
              onReviewImport={handleReviewImport}
              onOpenScanSettings={handleOpenDiscoveryScanSettings}
              onUpdateSkill={handleUpdateSkill}
              onDeleteSkill={handleDeletePrompt}
              onToggleSkillEnabled={handleToggleSkillEnabled}
              onToggleTool={handleToggleToolForSkill}
              onOpenScope={handleOpenScope}
              onOpenDetail={handleOpenDetail}
              onEditTags={handleOpenEditTags}
              onToggleBulkSelection={handleToggleBulkSelection}
              getSkillScope={getSkillScope}
              getSkillProjects={getSkillProjects}
              t={t}
            />
            {bulkMode ? (
              <div className="bulk-action-bar">
                <div className="bulk-action-copy">
                  <strong>{t('bulk.selected', { count: bulkSelectedIds.length })}</strong>
                  <span>{t('bulk.helper')}</span>
                </div>
                <div className="bulk-action-buttons">
                  <button
                    className="btn btn-secondary"
                    type="button"
                    onClick={handleSelectVisibleSkills}
                    disabled={loading || visibleSkills.length === 0}
                  >
                    {allVisibleBulkSelected
                      ? t('bulk.unselectVisible')
                      : t('bulk.selectVisible')}
                  </button>
                  <button
                    className="btn btn-secondary"
                    type="button"
                    onClick={handleOpenBulkTags}
                    disabled={loading || bulkSelectedIds.length === 0}
                  >
                    {t('bulk.tags')}
                  </button>
                  <button
                    className="btn btn-secondary"
                    type="button"
                    onClick={handleOpenBulkSync}
                    disabled={
                      loading ||
                      bulkSelectedIds.length === 0 ||
                      bulkHasDisabledSelected
                    }
                  >
                    {t('bulk.sync')}
                  </button>
                  <button
                    className="btn btn-secondary"
                    type="button"
                    onClick={() => void handleToggleBulkEnabled()}
                    disabled={loading || bulkSelectedIds.length === 0}
                  >
                    {bulkShouldEnable ? t('bulk.enable') : t('bulk.disable')}
                  </button>
                  <button
                    className="btn btn-danger"
                    type="button"
                    onClick={handleOpenBulkDelete}
                    disabled={loading || bulkSelectedIds.length === 0}
                  >
                    {t('bulk.delete')}
                  </button>
                </div>
              </div>
            ) : null}
          </div>
        ) : activeView === 'manage' ? (
          <div className="management-page">
            <div className="management-header">
              <div className="management-heading">
                <h1>{t('manageCenterTitle')}</h1>
                <p>{t('manageCenterHelp')}</p>
              </div>
              <div className="management-tab-list" role="tablist" aria-label={t('manageCenterTitle')}>
                <button
                  className={`management-tab${managementTab === 'tags' ? ' active' : ''}`}
                  type="button"
                  role="tab"
                  aria-selected={managementTab === 'tags'}
                  onClick={() => setManagementTab('tags')}
                >
                  {t('manageTabs.tags')}
                </button>
                <button
                  className={`management-tab${managementTab === 'tools' ? ' active' : ''}`}
                  type="button"
                  role="tab"
                  aria-selected={managementTab === 'tools'}
                  onClick={() => setManagementTab('tools')}
                >
                  {t('manageTabs.tools')}
                </button>
                <button
                  className={`management-tab${managementTab === 'updates' ? ' active' : ''}`}
                  type="button"
                  role="tab"
                  aria-selected={managementTab === 'updates'}
                  onClick={() => setManagementTab('updates')}
                >
                  {t('manageTabs.updates')}
                </button>
              </div>
            </div>
            <div className="management-panel">
              {managementTab === 'tags' ? (
                <TagsPage
                  embedded
                  tags={tags}
                  untaggedCount={untaggedCount}
                  loading={loading}
                  formatRelative={formatRelative}
                  onReviewUntagged={handleReviewUntagged}
                  onViewTag={handleViewTag}
                  onCreateTag={handleCreateTag}
                  onRenameTag={setPendingRenameTag}
                  onDeleteTag={handleDeleteTag}
                  t={t}
                />
              ) : managementTab === 'tools' ? (
                <ToolsPage
                  embedded
                  toolStatus={toolStatus}
                  toolConfig={toolConfig}
                  onToolConfigChange={handleToolConfigChange}
                  t={t}
                />
              ) : (
                <UpdatesPage
                  autoUpdateConfig={autoUpdateConfig}
                  onAutoUpdateConfigChange={handleAutoUpdateConfigChange}
                  onRunAutoUpdateNow={handleTriggerAutoUpdateTaskNow}
                  autoUpdateTriggering={autoUpdateTriggering}
                  t={t}
                />
              )}
            </div>
          </div>
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
            githubProxyConfig={githubProxyConfig}
            onGithubProxyConfigChange={handleGithubProxyConfigChange}
            discoveryScanEnabledCount={
              discoveryScanSettings?.sources.filter((source) => source.enabled).length ?? 0
            }
            discoveryScanSourceCount={discoveryScanSettings?.sources.length ?? 0}
            onOpenDiscoveryScanSettings={handleOpenDiscoveryScanSettings}
            onBack={handleCloseSettings}
            t={t}
          />
        ) : (
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
        )}
      </main>

      <AddSkillModal
        open={showAddModal}
        loading={loading}
        canClose={!loading}
        addModalTab={addModalTab}
        localPath={localPath}
        gitUrl={gitUrl}
        tags={tags}
        selectedTagIds={addModalTagIds}
        syncTargets={syncTargets}
        installedTools={installedTools}
        toolStatus={toolStatus}
        installScope={installScope}
        installProjects={installProjects}
        recentProjects={recentProjects}
        onRequestClose={handleCloseAdd}
        onTabChange={setAddModalTab}
        onLocalPathChange={setLocalPath}
        onPickLocalPath={handlePickLocalPath}
        onGitUrlChange={setGitUrl}
        onToggleTag={handleToggleAddModalTag}
        onSyncTargetChange={handleSyncTargetChange}
        onInstallScopeChange={handleInstallScopeChange}
        onInstallProjectsChange={handleInstallProjectsChange}
        onPickProject={handlePickProject}
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

      <BulkSyncModal
        open={showBulkSyncModal}
        loading={loading}
        selectedCount={bulkSelectedIds.length}
        installedTools={installedTools}
        selectedToolIds={bulkSyncToolIds}
        onToggleTool={handleToggleBulkSyncTool}
        onRequestClose={handleCloseBulkSync}
        onConfirm={handleConfirmBulkSync}
        t={t}
      />

      <BulkTagsModal
        open={showBulkTagsModal}
        loading={loading}
        selectedSkills={bulkSelectedSkills}
        tags={tags}
        onRequestClose={handleCloseBulkTags}
        onConfirm={handleConfirmBulkTags}
        t={t}
      />

      <BulkDeleteModal
        open={showBulkDeleteModal}
        loading={loading}
        skillNames={bulkSelectedNames}
        onRequestClose={handleCloseBulkDelete}
        onConfirm={handleConfirmBulkDelete}
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

      <DiscoveryScanModal
        open={showDiscoveryScanModal}
        saving={discoveryScanSaving}
        settings={discoveryScanSettings}
        onRequestClose={handleCloseDiscoveryScanSettings}
        onSave={handleSaveDiscoveryScanSettings}
        t={t}
      />

      <SharedDirModal
        open={Boolean(pendingSharedToggle)}
        loading={loading}
        toolLabel={pendingSharedLabels?.toolLabel ?? ''}
        otherLabels={pendingSharedLabels?.otherLabels ?? ''}
        onRequestClose={handleSharedCancel}
        onConfirm={handleSharedConfirm}
        t={t}
      />

      <SharedDirModal
        open={Boolean(pendingSyncTargetChange)}
        loading={loading}
        toolLabel={pendingSyncTargetLabels?.toolLabel ?? ''}
        otherLabels={pendingSyncTargetLabels?.otherLabels ?? ''}
        onRequestClose={handleSyncTargetChangeCancel}
        onConfirm={handleSyncTargetChangeConfirm}
        t={t}
      />

      <RenameTagModal
        key={pendingRenameTag?.id ?? 'rename-tag-modal'}
        open={Boolean(pendingRenameTag)}
        loading={loading}
        tag={pendingRenameTag}
        onRequestClose={handleCloseRenameTag}
        onSave={(tagId, name) => void handleRenameTag(tagId, name)}
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

      {showAppUpdateModal && updateAvailableVersion && (
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
