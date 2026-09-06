import { useCallback, useEffect, useRef, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { openUrl } from '@tauri-apps/plugin-opener'
import {
  AlertTriangle,
  CheckCircle2,
  ChevronDown,
  ChevronRight,
  ChevronUp,
  Cloud,
  Clock3,
  CircleHelp,
  ExternalLink,
  FolderGit2,
  GitMerge,
  Laptop,
  LoaderCircle,
  LockKeyhole,
  LogIn,
  Monitor,
  Package,
  Pencil,
  RefreshCw,
  Save,
  Search,
  Settings,
  ShieldCheck,
  X,
} from 'lucide-react'
import { toast } from 'sonner'
import DeviceCodeCopy from './DeviceCodeCopy'
import ToolSyncNotice from './ToolSyncNotice'
import { SiGitee, SiGithub, SiGitlab } from 'react-icons/si'
import type { TFunction } from 'i18next'
import type {
  DeviceSyncChangeSummary,
  DeviceSyncConfigDto,
  DeviceSyncConflict,
  DeviceSyncDevice,
  DeviceSyncHistoryEntry,
  DeviceSyncOAuthAvailability,
  DeviceSyncOAuthPoll,
  DeviceSyncOAuthStart,
  DeviceSyncPendingOAuth,
  DeviceSyncProvider,
  DeviceSyncRemoteRepository,
  DeviceSyncRunResult,
  DeviceSyncStatus,
  DeviceSyncTrashEntry,
} from './types'
import {
  REPOSITORY_LOAD_TIMEOUT_MS,
  getSyncFailureKind,
  buildDeviceSyncForm,
  selectSyncRepository,
  changeSyncRepositoryUrl,
  isDeviceSyncScheduleValid,
  classifyRepositoryLoadFailure,
  getDeviceSyncControls,
  getDeviceConnectionState,
  getDeviceSyncExperience,
  getDeviceSyncSetupProgress,
  getDeviceSyncRunOutcome,
  filterRepositories,
  getOtherDeviceSummary,
  getRepositoryDisplayName,
  reduceRepositoryPicker,
  shouldLoadRepositoryPicker,
  withTimeout,
} from './deviceSyncState'
import type { DeviceSyncActivity, RepositoryLoadState } from './deviceSyncState'

type LoadOptions = {
  refreshRepositories?: boolean
}

type ConflictResolution = 'keep_local' | 'use_remote' | 'keep_both'

type DeviceSyncPageProps = {
  active: boolean
  isTauri: boolean
  onSkillsChanged: () => Promise<void>
  onOpenToolIssues: () => void
  toolLabels?: Record<string, string>
  onConflictCountChange: (count: number) => void
  t: TFunction
}

const DeviceSyncPage = ({
  active,
  isTauri,
  onSkillsChanged,
  onOpenToolIssues,
  toolLabels,
  onConflictCountChange,
  t,
}: DeviceSyncPageProps) => {
  const [config, setConfig] = useState<DeviceSyncConfigDto | null>(null)
  const [status, setStatus] = useState<DeviceSyncStatus | null>(null)
  const [history, setHistory] = useState<DeviceSyncHistoryEntry[]>([])
  const [devices, setDevices] = useState<DeviceSyncDevice[]>([])
  const [syncHelpOpen, setSyncHelpOpen] = useState(false)
  const [conflicts, setConflicts] = useState<DeviceSyncConflict[]>([])
  const [trash, setTrash] = useState<DeviceSyncTrashEntry[]>([])
  const [preview, setPreview] = useState<DeviceSyncChangeSummary | null>(null)
  const [form, setForm] = useState(buildDeviceSyncForm)
  const [busy, setBusy] = useState<string | null>(null)
  const [oauthAvailability, setOauthAvailability] = useState<DeviceSyncOAuthAvailability[]>([])
  const [oauthFlow, setOauthFlow] = useState<DeviceSyncOAuthStart | null>(null)
  const [repositories, setRepositories] = useState<DeviceSyncRemoteRepository[]>([])
  const [repositoryLoadState, setRepositoryLoadState] = useState<RepositoryLoadState>('idle')
  const [repositoryPickerOpen, setRepositoryPickerOpen] = useState(false)
  const [repositorySearch, setRepositorySearch] = useState('')
  const [settingsDrawerOpen, setSettingsDrawerOpen] = useState(false)
  const automationSectionRef = useRef<HTMLElement>(null)
  const focusAutomationRef = useRef(false)
  const [initialLoadFailed, setInitialLoadFailed] = useState(false)
  const [activityTab, setActivityTab] = useState<DeviceSyncActivity>('devices')
  const [expandedConflictId, setExpandedConflictId] = useState<string | null>(null)
  const [editingDeviceId, setEditingDeviceId] = useState<string | null>(null)
  const [deviceAliasDraft, setDeviceAliasDraft] = useState('')
  const [conflictSelections, setConflictSelections] = useState<Record<string, ConflictResolution>>({})
  const repositoryRequestRef = useRef(0)
  const activeRepositoryRequestRef = useRef<{
    key: string
    promise: Promise<DeviceSyncRemoteRepository[]>
  } | null>(null)
  const initializedRef = useRef(false)
  const settingsCloseButtonRef = useRef<HTMLButtonElement>(null)

  const loadRepositories = useCallback(
    async (
      provider: DeviceSyncProvider,
      credentialKey: string | null,
      forceRefresh = false,
    ) => {
      const requestId = ++repositoryRequestRef.current
      const requestKey = `${provider}:${credentialKey ?? ''}`
      const existingRequest = activeRepositoryRequestRef.current
      const request =
        !forceRefresh && existingRequest?.key === requestKey
          ? existingRequest.promise
          : invoke<DeviceSyncRemoteRepository[]>('list_device_sync_repositories', {
              providerId: provider,
              credentialKey,
            })
      if (request !== existingRequest?.promise) {
        activeRepositoryRequestRef.current = { key: requestKey, promise: request }
        void request.then(
          () => {
            if (activeRepositoryRequestRef.current?.promise === request) {
              activeRepositoryRequestRef.current = null
            }
          },
          () => {
            if (activeRepositoryRequestRef.current?.promise === request) {
              activeRepositoryRequestRef.current = null
            }
          },
        )
      }
      setRepositoryLoadState('loading')
      try {
        const result = await withTimeout(
          request,
          REPOSITORY_LOAD_TIMEOUT_MS,
        )
        if (requestId !== repositoryRequestRef.current) return
        setRepositories(result.filter((repository) => repository.private))
        setRepositoryLoadState('loaded')
      } catch (error) {
        if (requestId === repositoryRequestRef.current) {
          if (activeRepositoryRequestRef.current?.promise === request) {
            activeRepositoryRequestRef.current = null
          }
          setRepositories([])
          const failure = classifyRepositoryLoadFailure(error)
          setRepositoryLoadState(
            failure === 'timeout'
              ? 'timeout'
              : failure === 'credential'
                ? 'credential-error'
                : 'error',
          )
        }
        throw error
      }
    },
    [],
  )

  const load = useCallback(async ({ refreshRepositories = false }: LoadOptions = {}) => {
    if (!isTauri) return
    const [nextConfig, nextStatus, nextHistory, nextConflicts, nextTrash, availability, pendingOAuth] =
      await Promise.all([
        invoke<DeviceSyncConfigDto | null>('get_device_sync_config'),
        invoke<DeviceSyncStatus>('get_device_sync_status'),
        invoke<DeviceSyncHistoryEntry[]>('get_device_sync_history'),
        invoke<DeviceSyncConflict[]>('get_device_sync_conflicts'),
        invoke<DeviceSyncTrashEntry[]>('get_device_sync_trash'),
        invoke<DeviceSyncOAuthAvailability[]>('get_device_sync_oauth_availability'),
        invoke<DeviceSyncPendingOAuth | null>('get_device_sync_pending_oauth'),
      ])
    setConfig(nextConfig)
    const nextDevices = nextConfig
      ? await invoke<DeviceSyncDevice[]>('get_device_sync_devices')
      : []
    setDevices(nextDevices)
    setStatus(nextStatus)
    setHistory(nextHistory)
    setConflicts(nextConflicts)
    setExpandedConflictId((current) => current ?? nextConflicts[0]?.id ?? null)
    setTrash(nextTrash)
    setOauthAvailability(availability)
    onConflictCountChange(nextConflicts.length)
    const nextForm = buildDeviceSyncForm(nextConfig)
    if (pendingOAuth) {
      nextForm.provider = pendingOAuth.provider
      nextForm.oauthCredentialKey = pendingOAuth.credential_key
      nextForm.accountLogin = pendingOAuth.account.login
      nextForm.username = pendingOAuth.account.login
      if (refreshRepositories) {
        void loadRepositories(pendingOAuth.provider, pendingOAuth.credential_key).catch(
          () => undefined,
        )
      }
    }
    setForm(nextForm)
  }, [isTauri, loadRepositories, onConflictCountChange])

  const refreshSyncActivity = useCallback(async () => {
    if (!isTauri) return
    const [nextStatus, nextDevices, nextHistory, nextConflicts, nextTrash] = await Promise.all([
      invoke<DeviceSyncStatus>('get_device_sync_status'),
      invoke<DeviceSyncDevice[]>('get_device_sync_devices'),
      invoke<DeviceSyncHistoryEntry[]>('get_device_sync_history'),
      invoke<DeviceSyncConflict[]>('get_device_sync_conflicts'),
      invoke<DeviceSyncTrashEntry[]>('get_device_sync_trash'),
    ])
    setStatus(nextStatus)
    setDevices(nextDevices)
    setHistory(nextHistory)
    setConflicts(nextConflicts)
    setExpandedConflictId((current) => current ?? nextConflicts[0]?.id ?? null)
    setTrash(nextTrash)
    onConflictCountChange(nextConflicts.length)
  }, [isTauri, onConflictCountChange])

  useEffect(() => {
    if (!isTauri) return
    let disposed = false
    let unlisten: (() => void) | undefined
    void listen<boolean>('device-sync-completed', ({ payload }) => {
      if (disposed) return
      void refreshSyncActivity().catch(() => undefined)
      if (payload) void onSkillsChanged().catch(() => undefined)
    }).then((stop) => { if (disposed) stop(); else unlisten = stop }).catch(() => undefined)
    return () => { disposed = true; unlisten?.() }
  }, [isTauri, onSkillsChanged, refreshSyncActivity])

  useEffect(() => {
    if (!active || initializedRef.current) return
    initializedRef.current = true
    setInitialLoadFailed(false)
    void load().catch((error) => {
      initializedRef.current = false
      setInitialLoadFailed(true)
      toast.error(String(error))
    })
  }, [active, load])

  useEffect(() => {
    if (!active || !config || busy) return
    const timer = setInterval(() => {
      void refreshSyncActivity().catch(() => undefined)
    }, 3_000)
    return () => clearInterval(timer)
  }, [active, busy, config, refreshSyncActivity])

  useEffect(() => {
    if (!settingsDrawerOpen) return
    if (focusAutomationRef.current) {
      automationSectionRef.current?.focus({ preventScroll: true })
      automationSectionRef.current?.scrollIntoView?.({ block: 'start' })
    } else {
      settingsCloseButtonRef.current?.focus()
    }
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') setSettingsDrawerOpen(false)
    }
    window.addEventListener('keydown', closeOnEscape)
    return () => window.removeEventListener('keydown', closeOnEscape)
  }, [settingsDrawerOpen])

  useEffect(() => {
    if (!syncHelpOpen) return
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') setSyncHelpOpen(false)
    }
    window.addEventListener('keydown', closeOnEscape)
    return () => window.removeEventListener('keydown', closeOnEscape)
  }, [syncHelpOpen])

  useEffect(() => {
    if (!oauthFlow) return
    let cancelled = false
    let timer: ReturnType<typeof setTimeout> | undefined
    const poll = async () => {
      try {
        const result = await invoke<DeviceSyncOAuthPoll>('poll_device_sync_oauth', {
          flowId: oauthFlow.flow_id,
        })
        if (cancelled) return
        if (result.status === 'pending') {
          timer = setTimeout(poll, result.interval_seconds * 1000)
          return
        }
        const credentialKey = result.credential_key ?? ''
        setForm((current) => ({
          ...current,
          oauthCredentialKey: credentialKey,
          accountLogin: result.account?.login ?? '',
          username: result.account?.login ?? current.username,
        }))
        setOauthFlow(null)
        toast.success(t('deviceSync.authorizationComplete'))
        setRepositoryPickerOpen(true)
        void loadRepositories(form.provider, credentialKey).catch(() => undefined)
      } catch (error) {
        if (!cancelled) {
          setOauthFlow(null)
          toast.error(String(error))
        }
      }
    }
    timer = setTimeout(poll, oauthFlow.interval_seconds * 1000)
    return () => {
      cancelled = true
      if (timer) clearTimeout(timer)
    }
  }, [form.provider, loadRepositories, oauthFlow, t])

  const runAction = async (name: string, action: () => Promise<void>) => {
    setBusy(name)
    try {
      await action()
    } catch (error) {
      const message = String(error)
      const key = message === 'unsafe shared tool target' ? 'sharedTargetHelp'
        : message.includes('DEVICE_SYNC_VISIBILITY_UNKNOWN') ? 'visibilityUnknownHelp'
        : message.includes('DEVICE_SYNC_PUBLIC_UPLOAD_CONFIRMATION') ? 'publicUploadWarning'
          : message.includes('DEVICE_SYNC_READ_CREDENTIAL_REQUIRED') ? 'readCredentialRequired'
          : message.includes('DEVICE_SYNC_PUBLIC_READ_FAILED') ? 'publicReadFailed' : null
      toast.error(key ? t(`deviceSync.${key}`) : ['sync', 'check'].includes(name) ? t(`deviceSync.failureReasons.${getSyncFailureKind(message)}`) : message)
      if (name === 'sync') {
        setPreview(null)
        await refreshSyncActivity().catch(() => undefined)
      }
    } finally {
      setBusy(null)
    }
  }

  const save = () => {
    if (form.remoteUrl.startsWith('https://') && form.visibility === 'unknown') {
      toast.error(t('deviceSync.visibilityUnknownHelp'))
      return
    }
    if (form.visibility === 'public' && !form.publicUploadConfirmed) {
      toast.error(t('deviceSync.publicUploadWarning'))
      return
    }
    if (form.autoSync && !isDeviceSyncScheduleValid(form.schedule)) {
      toast.error(t('deviceSync.invalidSchedule'))
      return
    }
    const isFirstConnection = config === null
    return runAction(isFirstConnection ? 'initial-sync' : 'save', async () => {
      const saved = await invoke<DeviceSyncConfigDto>('save_device_sync_config', {
        config: {
          provider: form.provider,
          remote_url: form.remoteUrl,
          branch: form.branch,
          username: form.username || null,
          token: form.token || null,
          credential_key: form.oauthCredentialKey || null,
          auto_check: form.autoCheck,
          auto_sync: form.autoSync,
          visibility: form.visibility,
          public_upload_confirmed: form.publicUploadConfirmed,
          auto_sync_schedule: isDeviceSyncScheduleValid(form.schedule) ? form.schedule : null,
        },
      })
      setConfig(saved)
      setForm((current) => ({ ...current, token: '' }))
      if (isFirstConnection) {
        const result = await invoke<DeviceSyncRunResult>('run_device_sync')
        await onSkillsChanged()
        if (getDeviceSyncRunOutcome(result.status) === 'conflicts') {
          await load({ refreshRepositories: false })
          toast.warning(t('deviceSync.syncNeedsResolution'))
          return
        }
      }
      await load({ refreshRepositories: false })
      setRepositoryPickerOpen(false)
      setSettingsDrawerOpen(false)
      toast.success(t(isFirstConnection ? 'deviceSync.setupComplete' : 'deviceSync.saved'))
    })
  }

  const createRepository = () =>
    runAction('create', async () => {
      const repository = await invoke<DeviceSyncRemoteRepository>(
        'create_device_sync_repository',
        {
          providerId: form.provider,
          token: form.token || null,
          credentialKey: form.oauthCredentialKey || null,
          name: 'skills-hub-sync',
        },
      )
      setForm((current) => selectSyncRepository(current, repository))
      setRepositories((current) => [
        repository,
        ...current.filter((item) => item.clone_url !== repository.clone_url),
      ])
      setRepositoryLoadState('loaded')
      setRepositoryPickerOpen(false)
      setRepositorySearch('')
      toast.success(t('deviceSync.repositoryCreated'))
    })

  const startAuthorization = () =>
    runAction('authorize', async () => {
      const flow = await invoke<DeviceSyncOAuthStart>('start_device_sync_oauth', {
        providerId: form.provider,
      })
      setOauthFlow(flow)
      await openUrl(flow.verification_uri_complete ?? flow.verification_uri)
    })

  const cancelAuthorization = async () => {
    if (!oauthFlow) return
    await invoke('cancel_device_sync_oauth', { flowId: oauthFlow.flow_id })
    setOauthFlow(null)
  }

  const changeProvider = (provider: DeviceSyncProvider) => {
    if (oauthFlow) void invoke('cancel_device_sync_oauth', { flowId: oauthFlow.flow_id })
    if (form.oauthCredentialKey) void invoke('clear_device_sync_pending_oauth')
    setOauthFlow(null)
    repositoryRequestRef.current += 1
    activeRepositoryRequestRef.current = null
    setRepositories([])
    setRepositoryLoadState('idle')
    setRepositoryPickerOpen(false)
    setRepositorySearch('')
    setForm((current) => ({
      ...current,
      provider,
      remoteUrl: '',
      visibility: 'unknown',
      publicUploadConfirmed: false,
      oauthCredentialKey: '',
      accountLogin: '',
    }))
  }

  const check = () =>
    runAction('check', async () => {
      const changes = await invoke<DeviceSyncChangeSummary>('check_device_sync')
      setPreview(changes)
      toast.success(t('deviceSync.checkComplete'))
    })

  const sync = () =>
    runAction('sync', async () => {
      const result = await invoke<DeviceSyncRunResult>('run_device_sync')
      setPreview(null)
      await Promise.all([load({ refreshRepositories: false }), onSkillsChanged()])
      if (getDeviceSyncRunOutcome(result.status) === 'conflicts') {
        toast.warning(t('deviceSync.syncNeedsResolution'))
      } else if (result.status === 'success' && Object.values(result.changes).every((count) => count === 0)) {
        toast.info(t('deviceSync.alreadyInSync'))
      } else {
        toast.success(t('deviceSync.syncComplete'))
      }
    })

  const resolve = (id: string, resolution: ConflictResolution) =>
    runAction(id, async () => {
      await invoke('resolve_device_sync_conflict', { conflictId: id, resolution })
      setConflictSelections((current) => {
        const next = { ...current }
        delete next[id]
        return next
      })
      await Promise.all([load({ refreshRepositories: false }), onSkillsChanged()])
      toast.success(t('deviceSync.conflictResolved'))
    })

  const restore = (id: string) =>
    runAction(id, async () => {
      await invoke('restore_device_sync_trash', { trashId: id })
      await Promise.all([load({ refreshRepositories: false }), onSkillsChanged()])
      toast.success(t('deviceSync.restored'))
    })

  const editDeviceAlias = (device: DeviceSyncDevice) => {
    setEditingDeviceId(device.id)
    setDeviceAliasDraft(device.alias ?? '')
  }

  const saveDeviceAlias = (deviceId: string) =>
    runAction(`device-alias:${deviceId}`, async () => {
      const alias = deviceAliasDraft.trim()
      await invoke('set_device_sync_device_alias', {
        deviceId,
        alias: alias || null,
      })
      setDevices((current) =>
        current.map((device) =>
          device.id === deviceId ? { ...device, alias: alias || null } : device,
        ),
      )
      setEditingDeviceId(null)
      setDeviceAliasDraft('')
      toast.success(t(alias ? 'deviceSync.deviceAliasSaved' : 'deviceSync.deviceAliasRemoved'))
    })

  const disconnect = () =>
    runAction('disconnect', async () => {
      await invoke('disconnect_device_sync')
      setConfig(null)
      setStatus(null)
      setDevices([])
      setPreview(null)
      repositoryRequestRef.current += 1
      activeRepositoryRequestRef.current = null
      setRepositories([])
      setRepositoryLoadState('idle')
      setRepositoryPickerOpen(false)
      setRepositorySearch('')
      setSettingsDrawerOpen(false)
      setForm(buildDeviceSyncForm())
      onConflictCountChange(0)
      toast.success(t('deviceSync.disconnected'))
    })

  const changes = preview
  const working = busy !== null
  const synchronizationInProgress =
    busy === 'sync' || busy === 'initial-sync' || status?.is_running === true
  const controls = getDeviceSyncControls({
    configured: config !== null,
    busy: working || synchronizationInProgress,
    remoteUrl: form.remoteUrl,
    token: form.token,
    hasCredential: Boolean(form.oauthCredentialKey || config?.has_credential),
  })
  const providerName = form.provider === 'github' ? 'GitHub' : form.provider === 'gitlab' ? 'GitLab' : 'Gitee'
  const oauthAvailable = oauthAvailability.find((item) => item.provider === form.provider)?.available ?? false
  const authorized = Boolean(form.oauthCredentialKey || (config?.has_credential && config.provider === form.provider))
  const experience = getDeviceSyncExperience({
    configured: config !== null,
    authorized,
    hasRepository: form.remoteUrl.trim().length > 0,
    conflictCount: conflicts.length,
    busyAction: busy,
    isRunning: status?.is_running,
    lastRunStatus: status?.last_run_status,
    pendingChanges: preview
      ? preview.added + preview.updated + preview.deleted + preview.conflicted
      : status?.pending_local_changes,
  })
  const retryRepositories = () => {
    setRepositoryPickerOpen(true)
    void loadRepositories(form.provider, form.oauthCredentialKey || null, true).catch(() => undefined)
  }
  const toggleRepositoryPicker = () => {
    const nextOpen = reduceRepositoryPicker(repositoryPickerOpen, 'toggle')
    setRepositoryPickerOpen(nextOpen)
    setRepositorySearch('')
    if (shouldLoadRepositoryPicker(nextOpen, repositoryLoadState)) {
      retryRepositories()
    }
  }
  const selectRepository = (remoteUrl: string) => {
    const repository = repositories.find((item) => item.clone_url === remoteUrl)
    setForm((current) => repository ? selectSyncRepository(current, repository) : changeSyncRepositoryUrl(current, remoteUrl))
    setRepositoryPickerOpen(reduceRepositoryPicker(true, 'select'))
    setRepositorySearch('')
  }
  const retryInitialLoad = () => {
    initializedRef.current = true
    setInitialLoadFailed(false)
    void load().catch((error) => {
      initializedRef.current = false
      setInitialLoadFailed(true)
      toast.error(String(error))
    })
  }
  const repositoryDisplayName = getRepositoryDisplayName(form.remoteUrl)
  const filteredRepositories = filterRepositories(repositories, repositorySearch)
  const latestCommit = status?.repository_head_commit ?? status?.last_synced_commit
  const deviceStates = devices.map((device) => ({
    device,
    state: getDeviceConnectionState(device, latestCommit),
  }))
  const otherDeviceSummary = getOtherDeviceSummary(
    deviceStates.map(({ device, state }) => ({ is_current: device.is_current, state })),
  )
  const currentDevice = devices.find((device) => device.is_current)
  const setupProgress = getDeviceSyncSetupProgress(authorized, Boolean(form.remoteUrl.trim()))
  const repositoryErrorMessage =
    repositoryLoadState === 'timeout'
      ? t('deviceSync.repositoryLoadTimeout')
      : repositoryLoadState === 'credential-error'
        ? t('deviceSync.repositoryCredentialFailed')
        : t('deviceSync.repositoryLoadFailed')

  useEffect(() => {
    if (experience.defaultActivity === 'conflicts') {
      setActivityTab('conflicts')
    }
  }, [experience.defaultActivity])

  const openSettingsDrawer = (focusAutomation = false) => {
    focusAutomationRef.current = focusAutomation
    if (config) {
      const savedForm = buildDeviceSyncForm(config)
      setForm((current) => ({
        ...savedForm,
        accountLogin: current.accountLogin || config.username || '',
        oauthCredentialKey: current.oauthCredentialKey,
      }))
    }
    setRepositoryPickerOpen(false)
    setRepositorySearch('')
    setSettingsDrawerOpen(true)
  }

  const savedSchedule = config?.auto_sync ? config.auto_sync_schedule : null
  const visibilityNeedsConfirmation = Boolean(config?.remote_url.startsWith('https://') && (!config.visibility || config.visibility === 'unknown'))
  const scheduleState = !savedSchedule ? 'disabled'
    : visibilityNeedsConfirmation || (config?.visibility === 'public' && !config.public_upload_confirmed) ? 'needs_confirmation'
    : synchronizationInProgress ? 'running'
      : conflicts.length ? 'paused'
        : status?.schedule_status?.state === 'disabled' ? 'initializing'
          : status?.schedule_status?.state ?? 'initializing'
  const nextScheduleAt = ['scheduled', 'backoff'].includes(scheduleState)
    ? status?.schedule_status?.next_at : null

  const visibilitySettings = <div className="device-sync-visibility-settings">
<div className="device-sync-visibility-result">{form.visibility === 'unknown' ? <CircleHelp size={14} aria-hidden="true" /> : <ShieldCheck size={14} aria-hidden="true" />}<output aria-label={t('deviceSync.repositoryVisibility')}>{t(`deviceSync.visibility.${form.visibility}`)}</output>{form.visibility !== 'unknown' ? <span>{t('deviceSync.visibilityIdentifiedBy', { platform: providerName })}</span> : null}</div>
    {form.visibility === 'unknown' ? <small>{t('deviceSync.visibilityUnknownHelp')}</small> : null}
    {form.visibility === 'public' ? <label className="device-sync-public-confirmation"><input type="checkbox" checked={form.publicUploadConfirmed} onChange={(event) => setForm({ ...form, publicUploadConfirmed: event.target.checked })} /><span>{t('deviceSync.publicUploadWarning')}</span></label> : null}
  </div>
  const readAccessHelp = !form.remoteUrl.startsWith('https://') ? 'readAccessSsh'
    : form.visibility === 'public' ? 'readAccessPublic'
      : form.visibility === 'unknown' ? 'visibilityUnknownHelp' : 'readAccessPrivate'

  return (
    <div className={`device-sync-page${settingsDrawerOpen ? ' drawer-open' : ''}`} hidden={!active}>
      <div className="device-sync-body">
        {experience.mode === 'dashboard' ? (
          <section className={`device-sync-status-strip ${experience.syncState}`}>
            <div className="device-sync-status-copy">
              {experience.syncState === 'syncing' ? <LoaderCircle className="spin" size={24} /> : ['conflicts', 'changes', 'failed'].includes(experience.syncState) ? <AlertTriangle size={24} /> : <CheckCircle2 size={24} />}
              <span>
                <span className="device-sync-status-title">
                  <strong>{t(experience.syncState === 'healthy' && status?.last_run_status === 'unchanged' ? 'deviceSync.confirmedInSync' : `deviceSync.visualState.${experience.syncState}`, { count: conflicts.length })}</strong>
                  <span className="device-sync-help-anchor">
                    <button
                      className="device-sync-help-trigger"
                      type="button"
                      aria-label={t('deviceSync.syncHelpTrigger')}
                      aria-haspopup="dialog"
                      aria-expanded={syncHelpOpen}
                      aria-controls="device-sync-help"
                      title={t('deviceSync.syncHelpTrigger')}
                      onClick={() => setSyncHelpOpen((open) => !open)}
                    >
                      <CircleHelp size={15} />
                    </button>
                    {syncHelpOpen ? (
                      <div
                        id="device-sync-help"
                        className="device-sync-help-popover"
                        role="dialog"
                        aria-labelledby="device-sync-help-title"
                      >
                        <header>
                          <strong id="device-sync-help-title">{t('deviceSync.syncHelpTitle')}</strong>
                          <button
                            type="button"
                            aria-label={t('deviceSync.syncHelpClose')}
                            onClick={() => setSyncHelpOpen(false)}
                          >
                            <X size={15} />
                          </button>
                        </header>
                        <ol>
                          <li><b>1</b><span>{t('deviceSync.syncHelpFetch')}</span></li>
                          <li><b>2</b><span>{t('deviceSync.syncHelpMerge')}</span></li>
                          <li><b>3</b><span>{t('deviceSync.syncHelpUpload')}</span></li>
                          <li><b>4</b><span>{t('deviceSync.syncHelpOtherDevices')}</span></li>
                        </ol>
                        <p><LockKeyhole size={13} />{t('deviceSync.syncHelpNote')}</p>
                      </div>
                    ) : null}
                  </span>
                </span>
                <small>{t(status?.last_run_status === 'unchanged' ? 'deviceSync.lastConfirmedValue' : 'deviceSync.lastSyncValue', { value: status?.last_run_at ? new Date(status.last_run_at).toLocaleString() : t('deviceSync.never') })}</small>
                {experience.syncState === 'failed' ? <p className="device-sync-failure-reason" role="status">{t(`deviceSync.failureReasons.${getSyncFailureKind(history[0]?.status === 'failed' && history[0]?.finished_at === status?.last_run_at ? history[0].error : null)}`)}</p> : null}
              </span>
            </div>
            <div className="device-sync-map">
              <div className={`device-sync-route${synchronizationInProgress ? ' is-syncing' : ''}`} role="group" aria-label={t('deviceSync.localRepositoryExchange')} aria-busy={synchronizationInProgress}>
                <div className="device-sync-route-node"><span><Laptop size={23} /></span><strong>{currentDevice?.alias || currentDevice?.name || t('deviceSync.thisDevice')}</strong><small>{t('deviceSync.currentDevice')}</small></div>
                <div className="device-sync-exchange">
                  <small>{t(synchronizationInProgress ? 'deviceSync.exchangingContent' : busy === 'check' ? 'deviceSync.checkingRepository' : 'deviceSync.localRepositoryExchange')}</small>
                  <svg className="device-sync-wires" aria-hidden="true">
                    <line className="device-sync-wire" x1="0" y1="9" x2="100%" y2="9" />
                    <line className="device-sync-wire" x1="0" y1="29" x2="100%" y2="29" />
                    <svg x="100%" y="0" width="1" height="38" overflow="visible"><path className="device-sync-wire" d="m-5 4 5 5-5 5" /></svg>
                    <path className="device-sync-wire" d="m5 24-5 5 5 5" />
                    <line className="device-sync-packet" pathLength="100" x1="0" y1="9" x2="100%" y2="9" />
                    <line className="device-sync-packet download" pathLength="100" x1="100%" y1="29" x2="0" y2="29" />
                  </svg>
                </div>
                <div className="device-sync-route-node"><span>{config?.provider === 'github' ? <SiGithub size={23} /> : config?.provider === 'gitlab' ? <SiGitlab size={23} /> : <SiGitee size={23} />}</span><strong>{getRepositoryDisplayName(config?.remote_url ?? '')}</strong><small><LockKeyhole size={11} />{t('deviceSync.platformRepository', { visibility: t(`deviceSync.visibility.${config?.visibility ?? 'unknown'}`), provider: config?.provider === 'github' ? 'GitHub' : config?.provider === 'gitlab' ? 'GitLab' : 'Gitee' })}</small></div>
              </div>
              <aside className="device-sync-other-summary">
                <strong><Monitor size={18} />{otherDeviceSummary.count ? t('deviceSync.otherDeviceCount', { count: otherDeviceSummary.count }) : t('deviceSync.noOtherDevices')}</strong>
                <small>{otherDeviceSummary.count ? (otherDeviceSummary.pendingCount ? t('deviceSync.pendingDevices', { count: otherDeviceSummary.pendingCount }) : t('deviceSync.otherDevicesUpToDate')) : t('deviceSync.noOtherDevicesHelp')}</small>
                <button type="button" onClick={() => otherDeviceSummary.count ? setActivityTab('devices') : setSyncHelpOpen(true)}>{t(otherDeviceSummary.count ? 'deviceSync.viewDeviceRecords' : 'deviceSync.connectAnotherDevice')}</button>
              </aside>
            </div>
            <div className="device-sync-status-actions"><button className="btn btn-secondary" type="button" disabled={!controls.canCheck} onClick={check}>{busy === 'check' ? <LoaderCircle className="spin" size={15} /> : <RefreshCw size={15} />}{t('deviceSync.check')}</button><button className="btn btn-primary" type="button" disabled={!controls.canSync || conflicts.length > 0} onClick={sync}>{synchronizationInProgress ? <LoaderCircle className="spin" size={15} /> : <Cloud size={15} />}{conflicts.length ? t('deviceSync.waitingForConflicts') : t(synchronizationInProgress ? 'deviceSync.exchangingContent' : 'deviceSync.syncLocalRepository')}</button></div>
            <ToolSyncNotice issues={status?.tool_issues ?? []} toolLabels={toolLabels} onOpen={onOpenToolIssues} t={t} />
            <section className={`device-sync-schedule-summary ${scheduleState}`} aria-label={t('deviceSync.scheduleSummary')}>
              <span className="device-sync-schedule-label"><Clock3 size={18} /><strong>{t('deviceSync.autoSync')}</strong><span className="device-sync-schedule-badge">{t(`deviceSync.scheduleState.${scheduleState}`)}</span></span>
              {savedSchedule ? <strong className="device-sync-schedule-frequency">{savedSchedule.mode === 'interval' ? t('deviceSync.scheduleInterval', { count: savedSchedule.minutes }) : t('deviceSync.scheduleDaily', { value: savedSchedule.time })}</strong> : null}
              {nextScheduleAt ? <time dateTime={new Date(nextScheduleAt).toISOString()}>{t(scheduleState === 'backoff' ? 'deviceSync.scheduleRetryAt' : 'deviceSync.scheduleNextAt', { value: new Date(nextScheduleAt).toLocaleString(undefined, { month: 'numeric', day: 'numeric', hour: '2-digit', minute: '2-digit' }) })}</time> : <span>{t(`deviceSync.scheduleHint.${scheduleState}`)}</span>}
              {savedSchedule && ['scheduled', 'backoff'].includes(scheduleState) ? <span>{t('deviceSync.scheduleAppRunning')}</span> : null}
              <button type="button" onClick={() => openSettingsDrawer(true)}>{t('deviceSync.editSchedule')}<ChevronRight size={15} /></button>
            </section>
            <p className="device-sync-independent-note"><CircleHelp size={13} />{t('deviceSync.independentSyncNote')}</p>
            {visibilityNeedsConfirmation ? <p className="device-sync-visibility-notice" role="status"><AlertTriangle size={16} />{t('deviceSync.visibilityUnknownHelp')}<button className="btn btn-secondary" type="button" onClick={() => openSettingsDrawer()}>{t('deviceSync.syncSettings')}</button></p> : null}
          </section>
        ) : (
          <section className="device-sync-setup-status">
            <div className="device-sync-setup-intro"><Cloud size={24} /><span><strong>{t('deviceSync.setupTitle')}</strong><small>{t(initialLoadFailed ? 'deviceSync.initialLoadFailed' : 'deviceSync.setupStatusHelp')}</small></span></div>
            <div className="device-sync-setup-progress" aria-label={t('deviceSync.setupSteps')}>
              {[t('deviceSync.stepAuthorization'), t('deviceSync.stepRepository'), t('deviceSync.firstSync')].map((label, index) => { const step = index + 1; const complete = setupProgress.completedSteps.includes(step); const activeStep = setupProgress.currentStep === step; return <span key={label} className={complete ? 'complete' : activeStep ? 'active' : ''}><i>{complete ? <CheckCircle2 size={15} /> : step}</i><b>{label}</b></span> })}
            </div>
            {initialLoadFailed ? <button className="btn btn-secondary" type="button" onClick={retryInitialLoad}><RefreshCw size={15} />{t('deviceSync.retry')}</button> : null}
          </section>
        )}

        {experience.mode === 'setup' ? (
          <section className="device-sync-panel device-sync-setup-panel">
            <div className="device-sync-panel-head"><div className="device-sync-panel-title"><FolderGit2 size={18} /><div><strong>{t('deviceSync.connectGitPlatform')}</strong><span>{t('deviceSync.connectGitPlatformHelp')}</span></div></div></div>
            <div className="device-sync-setup-content">
              <div className="device-sync-provider-picker" role="group" aria-label={t('deviceSync.provider')}>
                {(['github', 'gitlab', 'gitee'] as DeviceSyncProvider[]).map((provider) => {
                  const ProviderIcon = provider === 'github' ? SiGithub : provider === 'gitlab' ? SiGitlab : SiGitee
                  const label = provider === 'github' ? 'GitHub' : provider === 'gitlab' ? 'GitLab' : 'Gitee'
                  return <button key={provider} className={form.provider === provider ? 'active' : ''} type="button" disabled={working} onClick={() => changeProvider(provider)}><ProviderIcon className={`device-sync-provider-icon ${provider}`} aria-hidden="true" /><span>{label}</span></button>
                })}
              </div>

              <div className="device-sync-oauth">
                {oauthFlow ? <div className="device-sync-oauth-pending"><div><LoaderCircle className="spin" size={18} /><span><strong>{t('deviceSync.waitingAuthorization')}</strong><small>{t('deviceSync.waitingAuthorizationHelp', { provider: providerName })}</small></span></div>{oauthFlow.user_code ? <DeviceCodeCopy key={`${oauthFlow.flow_id}:${oauthFlow.user_code}`} code={oauthFlow.user_code} t={t} /> : null}<div className="device-sync-oauth-actions"><button className="btn btn-secondary" type="button" onClick={() => void openUrl(oauthFlow.verification_uri_complete ?? oauthFlow.verification_uri)}><ExternalLink size={15} />{t('deviceSync.openAuthorizationPage')}</button><button className="btn btn-ghost" type="button" onClick={() => void cancelAuthorization()}>{t('deviceSync.cancelAuthorization')}</button></div></div> : authorized ? <div className="device-sync-oauth-connected"><CheckCircle2 size={18} /><span><strong>{form.accountLogin ? t('deviceSync.authorizedAs', { account: form.accountLogin }) : t('deviceSync.authorized')}</strong><small>{t('deviceSync.authorizationStored')}</small></span><button className="btn btn-secondary" type="button" disabled={working || !oauthAvailable} onClick={startAuthorization}>{t('deviceSync.reauthorize')}</button></div> : <div className="device-sync-oauth-start"><div><LogIn size={19} /><span><strong>{t('deviceSync.oauthTitle', { provider: providerName })}</strong><small>{t('deviceSync.oauthHelp')}</small></span></div><button className="btn btn-primary" type="button" disabled={working || !oauthAvailable} onClick={startAuthorization}>{busy === 'authorize' ? <LoaderCircle className="spin" size={15} /> : <LogIn size={15} />}{t('deviceSync.signInWith', { provider: providerName })}</button>{!oauthAvailable ? <p>{t('deviceSync.oauthUnavailable')}</p> : null}</div>}
              </div>
              <p className="device-sync-security-note"><ShieldCheck size={14} />{t(authorized ? 'deviceSync.authorizationStored' : 'deviceSync.authorizationStorageNote')}</p>

              {authorized ? <div className="device-sync-repository-step"><div className="device-sync-repository-step-head"><span><strong>{t('deviceSync.chooseRepository')}</strong><small>{t('deviceSync.repositoryStorageHelp')}</small></span><div className="device-sync-repository-actions"><button className="btn btn-secondary" type="button" disabled={working || repositoryLoadState === 'loading'} onClick={retryRepositories}>{t(form.remoteUrl ? 'deviceSync.refreshRepositories' : 'deviceSync.loadRepositories')}</button><button className="btn btn-secondary" type="button" disabled={!controls.canCreateRepository} onClick={createRepository}>{busy === 'create' ? <LoaderCircle className="spin" size={15} /> : null}{t('deviceSync.createPrivateRepository')}</button></div></div>
                <button className={`device-sync-repository-current${form.remoteUrl ? ' selected' : ''}`} type="button" aria-expanded={repositoryPickerOpen} aria-controls="setup-device-sync-repositories" onClick={toggleRepositoryPicker}><span><LockKeyhole size={15} /><strong>{form.remoteUrl ? repositoryDisplayName : t('deviceSync.chooseRepositoryPlaceholder')}</strong>{form.remoteUrl ? <small>{t('deviceSync.currentRepository')}</small> : null}</span>{repositoryPickerOpen ? <ChevronUp size={16} /> : <ChevronDown size={16} />}</button>
                {repositoryPickerOpen ? <div id="setup-device-sync-repositories" className="device-sync-repository-picker">{repositoryLoadState === 'loading' ? <div className="device-sync-repository-feedback loading" role="status" aria-live="polite"><LoaderCircle className="spin" size={16} /><span>{t('deviceSync.loadingRepositories')}</span></div> : null}{['error', 'timeout', 'credential-error'].includes(repositoryLoadState) ? <div className="device-sync-repository-feedback error" role="alert"><AlertTriangle size={16} /><span>{repositoryErrorMessage}</span><button type="button" onClick={retryRepositories}>{t('deviceSync.retry')}</button></div> : null}{repositoryLoadState === 'loaded' && repositories.length ? <>{repositories.length > 6 ? <label className="device-sync-repository-search"><Search size={15} /><input type="search" value={repositorySearch} aria-label={t('deviceSync.searchRepositories')} placeholder={t('deviceSync.searchRepositories')} onChange={(event) => setRepositorySearch(event.target.value)} /></label> : null}<div className="device-sync-repository-choices">{filteredRepositories.map((repository) => <label key={repository.clone_url} className={`device-sync-repository-choice${form.remoteUrl === repository.clone_url ? ' selected' : ''}`}><input type="radio" name="device-sync-repository" value={repository.clone_url} checked={form.remoteUrl === repository.clone_url} onChange={(event) => selectRepository(event.target.value)} /><span><strong>{repository.name}</strong>{repository.name === 'skills-hub-sync' ? <small>{t('deviceSync.recommended')}</small> : null}</span><LockKeyhole size={15} /></label>)}</div>{!filteredRepositories.length ? <p className="device-sync-repository-empty">{t('deviceSync.noMatchingRepositories')}</p> : null}</> : null}{repositoryLoadState === 'loaded' && !repositories.length ? <p className="device-sync-repository-empty">{t('deviceSync.noPrivateRepositories')}</p> : null}</div> : null}
              </div> : null}

              <details className="device-sync-advanced"><summary>{t('deviceSync.otherConnectionMethods')}</summary><p>{t('deviceSync.advancedSettingsHelp')}</p><div className="device-sync-advanced-grid"><label className="device-sync-wide"><span>{t('deviceSync.remoteUrl')}</span><input value={form.remoteUrl} placeholder="https://github.com/you/skills-hub-sync.git" onChange={(event) => setForm(changeSyncRepositoryUrl(form, event.target.value))} /></label><label><span>{t('deviceSync.branch')}</span><input value={form.branch} onChange={(event) => setForm({ ...form, branch: event.target.value })} /></label><label><span>{t('deviceSync.username')}</span><input value={form.username} onChange={(event) => setForm({ ...form, username: event.target.value })} /></label><label className="device-sync-wide"><span>{t('deviceSync.token')}</span><input type="password" value={form.token} placeholder={t('deviceSync.tokenPlaceholder')} onChange={(event) => setForm({ ...form, token: event.target.value })} /><small><ShieldCheck size={13} />{t('deviceSync.tokenHelp')}</small></label></div>{visibilitySettings}</details>

              <div className="device-sync-scope"><strong>{t('deviceSync.whatSyncs')}</strong><div><span><Package size={16} /><b>{t('deviceSync.skillContent')}</b></span><span><RefreshCw size={16} /><b>{t('deviceSync.versionHistory')}</b></span><span><GitMerge size={16} /><b>{t('deviceSync.conflictHandling')}</b></span></div><p>{t('deviceSync.localOnlyNote')}</p></div>
            </div>
            {form.remoteUrl ? <div className="device-sync-panel-actions"><span><ShieldCheck size={14} />{t('deviceSync.localOnlyNote')}</span><button className="btn btn-primary" type="button" disabled={!controls.canSave} onClick={save}>{busy === 'initial-sync' ? <LoaderCircle className="spin" size={15} /> : <Cloud size={15} />}{t('deviceSync.startSync')}</button></div> : null}
          </section>
        ) : (
          <>
            {changes ? <section className="device-sync-preview"><strong>{t('deviceSync.previewTitle')}</strong><span>{t('deviceSync.previewSummary', changes)}</span></section> : null}
            <section className="device-sync-panel device-sync-activity-panel"><div className="device-sync-tabs" role="tablist" aria-label={t('deviceSync.activity')}><button className={activityTab === 'devices' ? 'active' : ''} type="button" role="tab" aria-selected={activityTab === 'devices'} onClick={() => setActivityTab('devices')}>{t('deviceSync.devices')}<b className="neutral">{devices.length}</b></button><button className={activityTab === 'history' ? 'active' : ''} type="button" role="tab" aria-selected={activityTab === 'history'} onClick={() => setActivityTab('history')}>{t('deviceSync.history')}</button><button className={activityTab === 'conflicts' ? 'active attention' : ''} type="button" role="tab" aria-selected={activityTab === 'conflicts'} onClick={() => setActivityTab('conflicts')}>{t('deviceSync.conflicts')}{conflicts.length ? <b>{conflicts.length}</b> : null}</button><button className={activityTab === 'trash' ? 'active' : ''} type="button" role="tab" aria-selected={activityTab === 'trash'} onClick={() => setActivityTab('trash')}>{t('deviceSync.trash')}</button><button className="device-sync-settings-trigger" type="button" onClick={() => openSettingsDrawer()}><Settings size={15} />{t('deviceSync.syncSettings')}</button></div>
              {activityTab === 'devices' ? <div className="device-sync-devices"><div className="device-sync-device-row header"><span>{t('deviceSync.device')}</span><span>{t('deviceSync.deviceStatus')}</span><span>{t('deviceSync.lastActive')}</span><span>{t('deviceSync.versionStatus')}</span></div>{deviceStates.length ? deviceStates.map(({ device, state }) => <article className="device-sync-device-row" key={device.id}>{editingDeviceId === device.id ? <form className="device-sync-device-alias-form" onSubmit={(event) => { event.preventDefault(); void saveDeviceAlias(device.id) }}><Monitor size={16} /><input autoFocus maxLength={80} value={deviceAliasDraft} placeholder={device.name} aria-label={t('deviceSync.deviceAlias')} onChange={(event) => setDeviceAliasDraft(event.target.value)} /><button type="submit" disabled={working} aria-label={t('deviceSync.saveDeviceAlias')} title={t('deviceSync.saveDeviceAlias')}>{busy === `device-alias:${device.id}` ? <LoaderCircle className="spin" size={14} /> : <Save size={14} />}</button><button type="button" disabled={working} aria-label={t('deviceSync.cancelDeviceAlias')} title={t('deviceSync.cancelDeviceAlias')} onClick={() => { setEditingDeviceId(null); setDeviceAliasDraft('') }}><X size={14} /></button></form> : <span className="device-sync-device-name"><Monitor size={16} /><span><strong>{device.alias || device.name}</strong>{device.alias ? <small>{device.name}</small> : null}</span>{device.is_current ? <b>{t('deviceSync.currentDevice')}</b> : null}<button type="button" disabled={working} aria-label={t('deviceSync.editDeviceAlias', { name: device.alias || device.name })} title={t('deviceSync.deviceAlias')} onClick={() => editDeviceAlias(device)}><Pencil size={13} /></button></span>}<em className={state}>{t(`deviceSync.deviceState.${state}`)}</em><time>{new Date(device.last_seen_at).toLocaleString()}</time><code className={state === 'pending' ? 'pending' : ''}>{state === 'pending' ? t('deviceSync.versionBehind') : device.last_commit?.slice(0, 8) ?? '—'}</code></article>) : <p className="device-sync-empty">{t('deviceSync.noDevices')}</p>}<p className="device-sync-devices-note"><LockKeyhole size={13} />{t('deviceSync.devicesRouteNote')}</p></div> : null}
              {activityTab === 'history' ? <div className="device-sync-history-table">{status?.last_run_status === 'unchanged' ? <p className="device-sync-no-change-note" role="status">{t('deviceSync.noChangeHistoryNote')}</p> : null}{history.length ? history.slice(0, 8).map((item) => <article key={item.id}><time>{new Date(item.started_at).toLocaleString()}</time><strong>{t(`deviceSync.status.${item.status}`, { defaultValue: item.status })}</strong>{item.status === 'failed' ? <details className="device-sync-failure-details"><summary>{t('deviceSync.failureDetails')}</summary><p>{t(`deviceSync.failureReasons.${getSyncFailureKind(item.error)}`)}</p></details> : <span>{t('deviceSync.previewSummary', item)}</span>}<em className={item.status}>{t(`deviceSync.status.${item.status}`, { defaultValue: item.status })}</em></article>) : <p className="device-sync-empty">{t('deviceSync.noHistory')}</p>}</div> : null}
              {activityTab === 'conflicts' ? <div className="device-sync-conflicts">{conflicts.length ? conflicts.map((conflict) => { const expanded = expandedConflictId === conflict.id; const selection = conflictSelections[conflict.id]; return <article key={conflict.id} className={expanded ? 'expanded' : ''}><button className="device-sync-conflict-summary" type="button" onClick={() => setExpandedConflictId(expanded ? null : conflict.id)}>{expanded ? <ChevronDown size={16} /> : <ChevronRight size={16} />}<Package size={17} /><strong>{conflict.skill_name}</strong><span>{t(conflict.base_commit ? 'deviceSync.sameFilesChanged' : 'deviceSync.missingCommonBaseline')}</span><em>{t('deviceSync.conflictFiles', { count: conflict.files.length })}</em></button>{expanded ? <div className="device-sync-conflict-detail"><div className="device-sync-conflict-files">{conflict.files.map((file) => <code key={file}>{file}</code>)}</div><div className="device-sync-resolution-options">{(['keep_local', 'use_remote'] as ConflictResolution[]).map((resolution) => <button key={resolution} className={selection === resolution ? 'selected' : ''} type="button" onClick={() => setConflictSelections((current) => ({ ...current, [conflict.id]: resolution }))}><span><strong>{t(`deviceSync.resolution.${resolution}.title`)}</strong></span><small>{t(`deviceSync.resolution.${resolution}.help`)}</small></button>)}</div><div className="device-sync-conflict-footer"><span><ShieldCheck size={14} />{t('deviceSync.conflictSafetyNote')}</span><button className="btn btn-primary" type="button" disabled={!selection || working} onClick={() => selection && resolve(conflict.id, selection)}>{busy === conflict.id ? <LoaderCircle className="spin" size={15} /> : null}{t('deviceSync.applyResolution')}</button></div></div> : null}</article> }) : <p className="device-sync-empty">{t('deviceSync.noConflicts')}</p>}</div> : null}
              {activityTab === 'trash' ? <div className="device-sync-trash-list">{trash.length ? trash.slice(0, 8).map((item) => <article key={item.id}><span><strong>{item.skill_name}</strong><small>{new Date(item.deleted_at).toLocaleString()}</small></span><button className="btn btn-secondary" type="button" disabled={working} onClick={() => restore(item.id)}>{busy === item.id ? <LoaderCircle className="spin" size={15} /> : null}{t('deviceSync.restore')}</button></article>) : <p className="device-sync-empty">{t('deviceSync.trashEmpty')}</p>}</div> : null}
            </section>
          </>
        )}
      </div>

      {settingsDrawerOpen && config ? (
        <div className="device-sync-settings-layer" role="presentation" onMouseDown={() => setSettingsDrawerOpen(false)}>
          <aside className="device-sync-settings-drawer" role="dialog" aria-modal="true" aria-labelledby="device-sync-settings-title" onMouseDown={(event) => event.stopPropagation()}>
            <header><div><strong id="device-sync-settings-title">{t('deviceSync.syncSettings')}</strong><span>{t('deviceSync.syncSettingsHelp')}</span></div><button ref={settingsCloseButtonRef} type="button" aria-label={t('cancel')} onClick={() => setSettingsDrawerOpen(false)}><X size={18} /></button></header>
            <div className="device-sync-drawer-content">
              <section><h3>{t('deviceSync.connection')}</h3><div className="device-sync-settings-rows"><div><span><FolderGit2 size={16} />{t('deviceSync.provider')}</span><strong>{providerName}</strong></div><div><span><LogIn size={16} />{t('deviceSync.authorizedAccount')}</span><strong>{form.accountLogin || config.username || '—'}</strong><button type="button" disabled={working || !oauthAvailable} onClick={startAuthorization}>{t('deviceSync.reauthorize')}</button></div><div><span><FolderGit2 size={16} />{t('deviceSync.repository')}</span><strong>{repositoryDisplayName}</strong><button type="button" aria-expanded={repositoryPickerOpen} aria-controls="connected-device-sync-repositories" disabled={working} onClick={toggleRepositoryPicker}>{t(repositoryPickerOpen ? 'deviceSync.collapseRepositories' : 'deviceSync.changeRepository')}{repositoryPickerOpen ? <ChevronUp size={14} /> : <ChevronDown size={14} />}</button></div></div>
                {repositoryPickerOpen ? <div id="connected-device-sync-repositories" className="device-sync-repository-picker">{repositoryLoadState === 'loading' ? <div className="device-sync-repository-feedback loading"><LoaderCircle className="spin" size={16} />{t('deviceSync.loadingRepositories')}</div> : null}{repositoryLoadState === 'loaded' && repositories.length ? <>{repositories.length > 6 ? <label className="device-sync-repository-search"><Search size={15} /><input type="search" value={repositorySearch} aria-label={t('deviceSync.searchRepositories')} placeholder={t('deviceSync.searchRepositories')} onChange={(event) => setRepositorySearch(event.target.value)} /></label> : null}<div className="device-sync-inline-repositories">{filteredRepositories.map((repository) => <label key={repository.clone_url}><input type="radio" name="connected-device-sync-repository" value={repository.clone_url} checked={form.remoteUrl === repository.clone_url} onChange={(event) => selectRepository(event.target.value)} />{repository.name}</label>)}</div>{!filteredRepositories.length ? <p className="device-sync-repository-empty">{t('deviceSync.noMatchingRepositories')}</p> : null}</> : null}{repositoryLoadState === 'loaded' && !repositories.length ? <p className="device-sync-repository-empty">{t('deviceSync.noPrivateRepositories')}</p> : null}{['error', 'timeout', 'credential-error'].includes(repositoryLoadState) ? <div className="device-sync-repository-feedback error"><AlertTriangle size={16} />{repositoryErrorMessage}<button type="button" onClick={retryRepositories}>{t('deviceSync.retry')}</button></div> : null}</div> : null}
              </section>
              <section ref={automationSectionRef} className="device-sync-automation-section" tabIndex={-1} aria-label={t('deviceSync.automation')}><h3>{t('deviceSync.automation')}</h3><div className="device-sync-drawer-options"><label><span><strong>{t('deviceSync.autoCheck')}</strong><small>{t(`deviceSync.${readAccessHelp}`)}</small></span><input type="checkbox" checked={form.autoCheck} onChange={(event) => setForm({ ...form, autoCheck: event.target.checked })} /></label><label><span><strong>{t('deviceSync.autoSync')}</strong><small>{t('deviceSync.autoSyncHelp')}</small></span><input type="checkbox" checked={form.autoSync} onChange={(event) => setForm({ ...form, autoSync: event.target.checked })} /></label></div>
                {form.autoSync ? <div className="device-sync-schedule">
                  <div className="settings-segmented" role="group" aria-label={t('autoUpdateScheduleMode')}>
                    <button type="button" className={form.schedule.mode === 'interval' ? 'active' : ''} aria-pressed={form.schedule.mode === 'interval'} onClick={() => setForm({ ...form, schedule: { mode: 'interval', minutes: 15 } })}>{t('autoUpdateScheduleInterval')}</button>
                    <button type="button" className={form.schedule.mode === 'daily' ? 'active' : ''} aria-pressed={form.schedule.mode === 'daily'} onClick={() => setForm({ ...form, schedule: { mode: 'daily', time: '09:00' } })}>{t('autoUpdateScheduleDaily')}</button>
                  </div>
                  {form.schedule.mode === 'interval' ? <>
                    <div className="device-sync-schedule-presets">{[5, 15, 30, 60].map((minutes) => <button type="button" key={minutes} className={form.schedule.mode === 'interval' && form.schedule.minutes === minutes ? 'active' : ''} onClick={() => setForm({ ...form, schedule: { mode: 'interval', minutes } })}>{t('deviceSync.everyMinutes', { count: minutes })}</button>)}</div>
                    <label className="device-sync-schedule-field"><span>{t('deviceSync.intervalMinutes')}</span><input type="number" min={5} max={43200} step={1} value={Number.isFinite(form.schedule.minutes) ? form.schedule.minutes : ''} aria-invalid={!isDeviceSyncScheduleValid(form.schedule)} onChange={(e) => setForm({ ...form, schedule: { mode: 'interval', minutes: e.target.valueAsNumber } })} /></label>
                  </> : <label className="device-sync-schedule-field"><span>{t('deviceSync.dailyTime')}</span><input type="time" value={form.schedule.time} aria-invalid={!isDeviceSyncScheduleValid(form.schedule)} onChange={(e) => setForm({ ...form, schedule: { mode: 'daily', time: e.target.value } })} /></label>}
                  <p>{t(form.schedule.mode === 'interval' ? 'deviceSync.intervalHelp' : 'deviceSync.dailyHelp')}</p>
                  {!isDeviceSyncScheduleValid(form.schedule) ? <p className="device-sync-schedule-error" role="alert">{t('deviceSync.invalidSchedule')}</p> : null}
                </div> : null}
              </section>
              <section><details className="device-sync-advanced"><summary>{t('deviceSync.advancedSettings')}</summary><p>{t('deviceSync.advancedSettingsHelp')}</p><div className="device-sync-advanced-grid"><label className="device-sync-wide"><span>{t('deviceSync.remoteUrl')}</span><input value={form.remoteUrl} onChange={(event) => setForm(changeSyncRepositoryUrl(form, event.target.value))} /></label><label><span>{t('deviceSync.branch')}</span><input value={form.branch} onChange={(event) => setForm({ ...form, branch: event.target.value })} /></label><label><span>{t('deviceSync.token')}</span><input type="password" value={form.token} placeholder={config.has_credential ? t('deviceSync.tokenStored') : t('deviceSync.tokenPlaceholder')} onChange={(event) => setForm({ ...form, token: event.target.value })} /></label></div>{visibilitySettings}</details></section>
            </div>
            <footer><button className="btn btn-ghost device-sync-disconnect" type="button" disabled={working || synchronizationInProgress} onClick={disconnect}>{t('deviceSync.disconnect')}</button><span /><button className="btn btn-secondary" type="button" onClick={() => setSettingsDrawerOpen(false)}>{t('cancel')}</button><button className="btn btn-primary" type="button" disabled={!controls.canSave} onClick={save}>{busy === 'save' ? <LoaderCircle className="spin" size={15} /> : null}{t('deviceSync.saveChanges')}</button></footer>
          </aside>
        </div>
      ) : null}
    </div>
  )
}

export default DeviceSyncPage
