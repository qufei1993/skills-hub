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
  last_updated: number
  last_failed: number
  progress: AutoUpdateProgressSnapshotDto
}

export type AutoUpdateRunResultDto = {
  checked: number
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
