export type OnboardingVariant = {
  tool: string
  name: string
  path: string
  fingerprint?: string | null
  is_link: boolean
  link_target?: string | null
  plugin_name?: string | null
  plugin_version?: string | null
  plugin_scope?: string | null
}

export type OnboardingGroup = {
  name: string
  variants: OnboardingVariant[]
  has_conflict: boolean
}

export type OnboardingPlan = {
  total_tools_scanned: number
  total_skills_found: number
  groups: OnboardingGroup[]
}

export type ToolOption = {
  id: string
  label: string
  avatar?: string | null
  supports_project_scope?: boolean
}

export type SyncMode = 'auto' | 'symlink' | 'junction' | 'copy'

export type TagDto = {
  id: number
  name: string
}

export type TagWithCountDto = TagDto & {
  skill_count: number
  updated_at: number
}

export type ManagedSkill = {
  source_error?: string | null
  source_checked_at?: number | null
  id: string
  name: string
  description?: string | null
  source_type: string
  source_ref?: string | null
  central_path: string
  created_at: number
  updated_at: number
  last_sync_at?: number | null
  enabled: boolean
  status: string
  tags: TagDto[]
  targets: {
    tool: string
    scope: 'global' | 'project' | string
    project_path?: string | null
    mode: string
    status: string
    last_error?: string | null
    target_path: string
    synced_at?: number | null
  }[]
}

export type GitSkillCandidate = {
  name: string
  description?: string | null
  subpath: string
}

export type LocalSkillCandidate = {
  name: string
  description?: string | null
  subpath: string
  valid: boolean
  reason?: string | null
}

export type InstallResultDto = {
  skill_id: string
  name: string
  central_path: string
  content_hash?: string | null
}

export type ToolInfoDto = {
  key: string
  label: string
  avatar?: string | null
  installed: boolean
  enabled: boolean
  is_custom: boolean
  skills_dir: string
  project_skills_dir: string
  supports_project_scope: boolean
  sync_mode: SyncMode
}

export type ToolStatusDto = {
  tools: ToolInfoDto[]
  installed: string[]
  newly_installed: string[]
}

export type StoragePathChangePreview = {
  current_path: string
  new_path: string
  skill_count: number
}

export type CustomToolConfigDto = {
  key: string
  label: string
  avatar?: string | null
  skills_dir: string
  project_skills_dir?: string | null
  sync_mode: SyncMode
  enabled: boolean
}

export type ToolConfigDto = {
  disabled_builtin_tools: string[]
  custom_tools: CustomToolConfigDto[]
}

export type DiscoveryScanSourceDto = {
  key: string
  label: string
  path: string
  enabled: boolean
}

export type DiscoveryScanSettingsDto = {
  sources: DiscoveryScanSourceDto[]
  disabled_source_keys: string[]
}

export type UpdateResultDto = {
  skill_id: string
  name: string
  content_hash?: string | null
  source_revision?: string | null
  updated_targets: string[]
  pending_targets?: string[]
  changed: boolean
}

export type AutoUpdateConfigDto = {
  enabled: boolean
  interval_hours: number
  schedule_type: 'interval' | 'daily'
  interval_value: number
  interval_unit: 'minutes' | 'hours'
  daily_time: string
  local_skill_count: number
  protected_local_skill_count: number
  task_registered: boolean
  task_status_detail: string
  last_run_at?: number | null
  last_started_at?: number | null
  last_finished_at?: number | null
  last_status?: string | null
  last_error?: string | null
  last_checked: number
  last_unchanged: number
  last_updated: number
  last_failed: number
  progress: AutoUpdateProgressSnapshotDto
}

export type AutoUpdateRunResultDto = {
  checked: number
  unchanged: number
  updated: number
  failed: number
  errors: string[]
  progress: AutoUpdateProgressSnapshotDto
}

export type GithubProxyConfigDto = {
  enabled: boolean
  port: number
  url: string
  auto_detected: boolean
}

export type GithubTokenStatusDto = {
  has_token: boolean
}

export type AutoUpdateSkillProgressDto = {
  skill_id: string
  name: string
  reason?: string | null
}

export type AutoUpdateProgressSnapshotDto = {
  total: number
  succeeded: AutoUpdateSkillProgressDto[]
  failed: AutoUpdateSkillProgressDto[]
  running?: AutoUpdateSkillProgressDto | null
  pending: AutoUpdateSkillProgressDto[]
}

export type FeaturedSkillDto = {
  slug: string
  name: string
  summary: string
  downloads: number
  stars: number
  source_url: string
}

export type OnlineSkillDto = {
  name: string
  installs: number
  source: string
  source_url: string
}

export type SkillFileEntry = {
  path: string
  size: number
}

export type DeviceSyncProvider = 'github' | 'gitlab' | 'gitee'

export type DeviceSyncSchedule =
  | { mode: 'interval'; minutes: number }
  | { mode: 'daily'; time: string }

export type RepositoryVisibility = 'public' | 'private' | 'internal' | 'unknown'

export type DeviceSyncConfigDto = {
  visibility?: RepositoryVisibility
  public_upload_confirmed?: boolean
  provider: DeviceSyncProvider
  remote_url: string
  branch: string
  username?: string | null
  auto_check: boolean
  auto_sync: boolean
  auto_sync_schedule?: DeviceSyncSchedule | null
  has_credential: boolean
}

export type DeviceSyncOAuthAvailability = {
  provider: DeviceSyncProvider
  available: boolean
  reason?: string | null
}

export type DeviceSyncOAuthStart = {
  flow_id: string
  verification_uri: string
  verification_uri_complete?: string | null
  user_code?: string | null
  expires_at: number
  interval_seconds: number
}

export type DeviceSyncProviderAccount = {
  login: string
  display_name?: string | null
}

export type DeviceSyncOAuthPoll = {
  provider: DeviceSyncProvider
  status: 'pending' | 'authorized'
  interval_seconds: number
  credential_key?: string | null
  account?: DeviceSyncProviderAccount | null
}

export type DeviceSyncPendingOAuth = {
  provider: DeviceSyncProvider
  credential_key: string
  account: DeviceSyncProviderAccount
}

export type DeviceSyncRemoteRepository = {
  visibility?: RepositoryVisibility
  name: string
  web_url: string
  clone_url: string
  ssh_url?: string | null
  private: boolean
}

export type DeviceSyncChangeSummary = {
  added: number
  updated: number
  deleted: number
  conflicted: number
}

export type DeviceSyncRunResult = {
  status: string
  commit?: string | null
  changes: DeviceSyncChangeSummary
  message: string
}

export type DeviceSyncStatus = {
  tool_issues?: { skill_name: string; tool: string }[]
  schedule_status?: {
    state: 'disabled' | 'initializing' | 'scheduled' | 'backoff' | 'paused' | 'running' | 'waiting' | 'needs_confirmation'
    next_at: number | null
  } | null
  configured: boolean
  is_running: boolean
  provider: DeviceSyncProvider
  remote_url: string
  auto_check: boolean
  auto_sync: boolean
  last_synced_commit?: string | null
  repository_head_commit?: string | null
  pending_local_changes: number
  conflict_count: number
  last_run_status?: string | null
  last_run_at?: number | null
}

export type DeviceSyncDevice = {
  id: string
  name: string
  alias?: string | null
  last_commit?: string | null
  last_seen_at: number
  is_current: boolean
}

export type DeviceSyncHistoryEntry = DeviceSyncChangeSummary & {
  id: string
  started_at: number
  finished_at?: number | null
  status: string
  commit?: string | null
  error?: string | null
}

export type DeviceSyncConflict = {
  id: string
  skill_id: string
  skill_name: string
  files: string[]
  base_commit?: string | null
  created_at: number
  status: string
}

export type DeviceSyncTrashEntry = {
  id: string
  skill_id: string
  skill_name: string
  deleted_at: number
  expires_at: number
}
