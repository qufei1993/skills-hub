import { useCallback, useEffect, useMemo, useState } from 'react'
import './App.css'
import { useTranslation } from 'react-i18next'
import { Toaster, toast } from 'sonner'
import FilterBar from './components/skills/FilterBar'
import Header from './components/skills/Header'
import LoadingOverlay from './components/skills/LoadingOverlay'
import SkillsList from './components/skills/SkillsList'
import LeaderboardTab from './components/skills/LeaderboardTab'
import ToolSkillsTab from './components/skills/ToolSkillsTab'
import AddSkillModal from './components/skills/modals/AddSkillModal'
import DeleteModal from './components/skills/modals/DeleteModal'
import GitPickModal from './components/skills/modals/GitPickModal'
import LocalPickModal from './components/skills/modals/LocalPickModal'
import ImportModal from './components/skills/modals/ImportModal'
import NewToolsModal from './components/skills/modals/NewToolsModal'
import SharedDirModal from './components/skills/modals/SharedDirModal'
import SettingsModal from './components/skills/modals/SettingsModal'
import { buildInstalledSkillNameSet } from './components/skills/leaderboardInstalled'
import { pickLeaderboardCandidateSubpath } from './components/skills/leaderboardInstallResolver'
import type {
  GitSkillCandidate,
  InstallResultDto,
  LocalSkillCandidate,
  ManagedSkill,
  OnboardingPlan,
  ToolOption,
  ToolStatusDto,
  UpdateResultDto,
} from './components/skills/types'

function App() {
  const { t, i18n } = useTranslation()
  const language = i18n.resolvedLanguage ?? i18n.language ?? 'en'
  const languageStorageKey = 'skills-language'
  const themeStorageKey = 'skills-theme'
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
  const [showSettingsModal, setShowSettingsModal] = useState(false)
  const [pendingSharedToggle, setPendingSharedToggle] = useState<{
    skill: ManagedSkill
    toolId: string
  } | null>(null)
  const [searchQuery, setSearchQuery] = useState('')
  const [sortBy, setSortBy] = useState<'updated' | 'name'>('updated')
  const [addModalTab, setAddModalTab] = useState<'local' | 'git'>('git')
  const [activeTab, setActiveTab] = useState<'skills' | 'leaderboard' | 'tools'>('skills')

  const isTauri =
    typeof window !== 'undefined' &&
    Boolean(
      (window as { __TAURI__?: unknown }).__TAURI__ ||
        (window as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__,
    )

  const invokeTauri = useCallback(
    async <T,>(command: string, args?: Record<string, unknown>) => {
      if (!isTauri) {
        throw new Error(t('errors.notTauri'))
      }
      const { invoke } = await import('@tauri-apps/api/core')
      return invoke<T>(command, args)
    },
    [isTauri, t],
  )
  const formatErrorMessage = useCallback(
    (raw: string) => {
      if (raw.includes('skill already exists in central repo')) {
        return t('errors.skillExistsInHub')
      }
      if (raw.startsWith('TARGET_EXISTS|')) {
        return t('errors.targetExists')
      }
      if (raw.startsWith('TOOL_NOT_INSTALLED|')) {
        return t('errors.toolNotInstalled')
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

  useEffect(() => {
    if (isTauri) {
      loadManagedSkills()
    }
  }, [isTauri, loadManagedSkills])

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
    if (isTauri) {
      void loadPlan()
    }
  }, [isTauri, loadPlan])

  useEffect(() => {
    if (!successToastMessage) return
    toast.success(successToastMessage, { duration: 1800 })
    setSuccessToastMessage(null)
  }, [successToastMessage])

  useEffect(() => {
    if (!error) return
    toast.error(formatErrorMessage(error), { duration: 2600 })
    setError(null)
    setActionMessage(null)
  }, [error, formatErrorMessage])

  const toolInfos = useMemo(() => toolStatus?.tools ?? [], [toolStatus])

  const tools: ToolOption[] = useMemo(() => {
    return toolInfos.map((info) => ({
      id: info.key,
      // Prefer i18n label if present; fallback to backend label.
      label: t(`tools.${info.key}`, { defaultValue: info.label }),
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

  const visibleSkills = useMemo(() => {
    const query = searchQuery.trim().toLowerCase()
    const filtered = managedSkills.filter((skill) => {
      if (!query) return true
      return (
        skill.name.toLowerCase().includes(query) ||
        skill.central_path.toLowerCase().includes(query) ||
        skill.source_type.toLowerCase().includes(query)
      )
    })
    const sorted = [...filtered].sort((a, b) => {
      if (sortBy === 'name') {
        return a.name.localeCompare(b.name)
      }
      return (b.updated_at ?? 0) - (a.updated_at ?? 0)
    })
    return sorted
  }, [managedSkills, searchQuery, sortBy])
  const installedSkillNames = useMemo(
    () => Array.from(buildInstalledSkillNameSet(managedSkills)),
    [managedSkills],
  )

  const [storagePath, setStoragePath] = useState<string>(t('notAvailable'))
  const [gitCacheCleanupDays, setGitCacheCleanupDays] = useState<number>(30)
  const [gitCacheTtlSecs, setGitCacheTtlSecs] = useState<number>(60)
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
    setShowSettingsModal(true)
  }, [])

  const handleOpenAdd = useCallback(() => {
    setShowAddModal(true)
  }, [])

  const handleCloseAdd = useCallback(() => {
    if (!loading) setShowAddModal(false)
  }, [loading])

  const handleCloseImport = useCallback(() => {
    if (!loading) setShowImportModal(false)
  }, [loading])

  const handleCloseSettings = useCallback(() => {
    if (!loading) setShowSettingsModal(false)
  }, [loading])

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

  const handleToggleAllGitCandidates = useCallback((checked: boolean) => {
    setGitCandidateSelected(
      Object.fromEntries(gitCandidates.map((c) => [c.subpath, checked])),
    )
  }, [gitCandidates])

  const handleToggleAllLocalCandidates = useCallback(
    (checked: boolean) => {
      setLocalCandidateSelected(
        Object.fromEntries(
          localCandidates.map((c) => [c.subpath, c.valid && checked]),
        ),
      )
    },
    [localCandidates],
  )

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

  const handleRefresh = useCallback(() => {
    void loadManagedSkills()
  }, [loadManagedSkills])

  const handleInstallFromLeaderboard = useCallback(
    async (
      repoUrl: string,
      name?: string,
      skillSlug?: string,
      onProgress?: (phase: 'downloading' | 'syncing') => void,
    ) => {
      setError(null)
      onProgress?.('downloading')
      let created: InstallResultDto
      const normalizedSlug = skillSlug?.trim()
      if (normalizedSlug) {
        const candidates = await invokeTauri<GitSkillCandidate[]>(
          'list_git_skills_cmd',
          { repoUrl },
        )
        const subpath = pickLeaderboardCandidateSubpath(
          candidates,
          normalizedSlug,
          name,
        )
        if (subpath) {
          created = await invokeTauri<InstallResultDto>('install_git_selection', {
            repoUrl,
            subpath,
            name: name || undefined,
          })
        } else {
          created = await invokeTauri<InstallResultDto>('install_git', {
            repoUrl,
            name: name || undefined,
          })
        }
      } else {
        created = await invokeTauri<InstallResultDto>('install_git', {
          repoUrl,
          name: name || undefined,
        })
      }
      const selectedInstalledIds = tools
        .filter((tool) => syncTargets[tool.id] && isInstalled(tool.id))
        .map((t) => t.id)
      const targets = uniqueToolIdsBySkillsDir(selectedInstalledIds)
        .map((id) => tools.find((t) => t.id === id))
        .filter(Boolean) as ToolOption[]
      if (targets.length > 0) {
        onProgress?.('syncing')
        const collectedErrors: { title: string; message: string }[] = []
        for (let i = 0; i < targets.length; i++) {
          const tool = targets[i]
          try {
            await invokeTauri('sync_skill_to_tool', {
              sourcePath: created.central_path,
              skillId: created.skill_id,
              tool: tool.id,
              name: created.name,
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
      setSuccessToastMessage(t('status.gitSkillCreated'))
      await loadManagedSkills()
    },
    [
      invokeTauri,
      isInstalled,
      loadManagedSkills,
      showActionErrors,
      syncTargets,
      t,
      tools,
      uniqueToolIdsBySkillsDir,
    ],
  )


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

  const toggleAll = useCallback(
    (checked: boolean) => {
    if (!plan) return
    const next: Record<string, boolean> = {}
    plan.groups.forEach((group) => {
      next[group.name] = checked
    })
    setSelected(next)
    },
    [plan],
  )

  const handleImport = async () => {
    if (!plan) return
    setLoading(true)
    setLoadingStartAt(Date.now())
    setActionMessage(null)
    setError(null)
    try {
      const collectedErrors: { title: string; message: string }[] = []
      for (const group of plan.groups) {
        if (!selected[group.name]) continue
        const chosenPath = variantChoice[group.name] ?? group.variants[0]?.path
        if (!chosenPath) continue
        const chosenVariantTool =
          group.variants.find((v) => v.path === chosenPath)?.tool ?? null

        setActionMessage(t('actions.importExisting', { name: group.name }))
        const installResult = await invokeTauri<{
          skill_id: string
          central_path: string
        }>('import_existing_skill', {
          sourcePath: chosenPath,
          name: group.name,
        })

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
            const overwrite = Boolean(
              chosenVariantTool &&
                (chosenVariantTool === tool.id ||
                  (sharedToolIdsByToolId[chosenVariantTool] ?? []).includes(
                    tool.id,
                  )),
            )
            await invokeTauri('sync_skill_to_tool', {
              sourcePath: installResult.central_path,
              skillId: installResult.skill_id,
              tool: tool.id,
              name: group.name,
              // 自动接管：如果来源就是该工具目录，同步回该工具时需要替换成指向中心仓库的软链
              overwrite,
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
      setSuccessToastMessage(t('status.importCompleted'))
      setActionMessage(null)
      await loadManagedSkills()
      await loadPlan()
      if (collectedErrors.length > 0) {
        showActionErrors(collectedErrors)
      } else {
        setShowImportModal(false)
      }
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
        const created = await invokeTauri<InstallResultDto>('install_git', {
          repoUrl: url,
          name: gitName.trim() || undefined,
        })
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
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setLoading(false)
      setLoadingStartAt(null)
    }
  }

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
      await loadManagedSkills()
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
          for (let ti = 0; ti < installedIds.length; ti++) {
            const toolId = installedIds[ti]
          const toolLabel = tools.find((t) => t.id === toolId)?.label ?? toolId
          setActionMessage(
            t('actions.syncStep', {
              index: si + 1,
              total: managedSkills.length,
              name: skill.name,
              tool: toolLabel,
            }),
          )
          try {
            await invokeTauri('sync_skill_to_tool', {
              sourcePath: skill.central_path,
              skillId: skill.id,
              tool: toolId,
              name: skill.name,
            })
          } catch (err) {
            const raw = err instanceof Error ? err.message : String(err)
            if (raw.startsWith('TOOL_NOT_INSTALLED|')) continue
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
      isInstalled,
      loadManagedSkills,
      managedSkills,
      showActionErrors,
      t,
      tools,
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

  const runToggleToolForSkill = useCallback(
    async (skill: ManagedSkill, toolId: string) => {
      if (loading) return
      const toolLabel = tools.find((t) => t.id === toolId)?.label ?? toolId
      const target = skill.targets.find((t) => t.tool === toolId)
      const synced = Boolean(target)

      setLoading(true)
      setLoadingStartAt(Date.now())
      setError(null)
      try {
        if (synced) {
          setActionMessage(
            t('actions.unsyncing', { name: skill.name, tool: toolLabel }),
          )
          await invokeTauri('unsync_skill_from_tool', {
            skillId: skill.id,
            tool: toolId,
          })
        } else {
          setActionMessage(
            t('actions.syncing', { name: skill.name, tool: toolLabel }),
          )
          await invokeTauri('sync_skill_to_tool', {
            sourcePath: skill.central_path,
            skillId: skill.id,
            tool: toolId,
            name: skill.name,
          })
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
          // Tool disappeared between detection and click; silently refresh.
          setError(t('errors.toolNotInstalled'))
        } else {
          setError(raw)
        }
      } finally {
        setLoading(false)
        setLoadingStartAt(null)
      }
    },
    [invokeTauri, loadManagedSkills, loading, t, tools],
  )

  const handleToggleToolForSkill = useCallback(
    (skill: ManagedSkill, toolId: string) => {
      if (loading) return
      const shared = sharedToolIdsByToolId[toolId] ?? null
      if (shared && shared.length > 1) {
        setPendingSharedToggle({ skill, toolId })
        return
      }
      void runToggleToolForSkill(skill, toolId)
    },
    [loading, runToggleToolForSkill, sharedToolIdsByToolId],
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
    const shared = sharedToolIdsByToolId[toolId] ?? []
    const others = shared.filter((id) => id !== toolId)
    return {
      toolLabel: toolLabelById[toolId] ?? toolId,
      otherLabels: others.map((id) => toolLabelById[id] ?? id).join(', '),
    }
  }, [pendingSharedToggle, sharedToolIdsByToolId, toolLabelById])

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
        t={t}
      />

      <Header
        language={language}
        loading={loading}
        onToggleLanguage={toggleLanguage}
        onOpenSettings={handleOpenSettings}
        onOpenAdd={handleOpenAdd}
        t={t}
      />

      <main className="skills-main">
        <div className="dashboard-stack">
          <div className="app-tabs">
            <button
              className={`app-tab ${activeTab === 'skills' ? 'active' : ''}`}
              onClick={() => setActiveTab('skills')}
            >
              {t('navSkills')}
            </button>
            <button
              className={`app-tab ${activeTab === 'leaderboard' ? 'active' : ''}`}
              onClick={() => setActiveTab('leaderboard')}
            >
              {t('leaderboard.title')}
            </button>
            <button
              className={`app-tab ${activeTab === 'tools' ? 'active' : ''}`}
              onClick={() => setActiveTab('tools')}
            >
              {t('toolSkills.title')}
            </button>
          </div>

          <div style={{ display: activeTab === 'skills' ? 'block' : 'none' }}>
            <>
              <FilterBar
                sortBy={sortBy}
                searchQuery={searchQuery}
                loading={loading}
                onSortChange={handleSortChange}
                onSearchChange={handleSearchChange}
                onRefresh={handleRefresh}
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
                t={t}
              />
            </>
          </div>

          <div style={{ display: activeTab === 'leaderboard' ? 'block' : 'none' }}>
            <LeaderboardTab
              onInstallSkill={handleInstallFromLeaderboard}
              installedSkillNames={installedSkillNames}
              t={t}
            />
          </div>

          <div style={{ display: activeTab === 'tools' ? 'block' : 'none' }}>
            <ToolSkillsTab
              managedSkills={managedSkills}
              tools={tools}
              installedToolIds={installedToolIds}
              loading={loading}
              formatRelative={formatRelative}
              getSkillSourceLabel={getSkillSourceLabel}
              onToggleTool={handleToggleToolForSkill}
              t={t}
            />
          </div>
        </div>
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
        onSyncTargetChange={handleSyncTargetChange}
        onSubmit={addModalTab === 'local' ? handleCreateLocal : handleCreateGit}
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
          onToggleAll={toggleAll}
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

      <SettingsModal
        open={showSettingsModal}
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
        onRequestClose={handleCloseSettings}
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

      <LocalPickModal
        open={showLocalPickModal}
        loading={loading}
        localCandidates={localCandidates}
        localCandidateSelected={localCandidateSelected}
        onRequestClose={handleCloseLocalPick}
        onCancel={handleCancelLocalPick}
        onToggleAll={handleToggleAllLocalCandidates}
        onToggleCandidate={handleToggleLocalCandidate}
        onInstall={handleInstallSelectedLocalCandidates}
        t={t}
      />

      <GitPickModal
        open={showGitPickModal}
        loading={loading}
        gitCandidates={gitCandidates}
        gitCandidateSelected={gitCandidateSelected}
        onRequestClose={handleCloseGitPick}
        onCancel={handleCancelGitPick}
        onToggleAll={handleToggleAllGitCandidates}
        onToggleCandidate={handleToggleGitCandidate}
        onInstall={handleInstallSelectedCandidates}
        t={t}
      />
      </div>
  )
}

export default App
