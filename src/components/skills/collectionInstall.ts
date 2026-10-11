import { buildInstallSyncJobs, normalizeProjectPaths, type InstallScope } from './installScope'
import type { CollectionManifest, CollectionResult, GitSkillCandidate, InstallResultDto, ManagedSkill } from './types'

export type CollectionInvoke = <T>(command: string, args?: Record<string, unknown>) => Promise<T>
export type CollectionInstallOptions = {
  scope?: InstallScope
  projects?: string[]
  tagIds?: number[]
  candidates?: Map<string, GitSkillCandidate[]>
}

export async function installCollectionEntry(
  invoke: CollectionInvoke, manifest: CollectionManifest, index: number, tools: string[],
  previous?: InstallResultDto, onInstalled?: (result: InstallResultDto) => void,
  options: CollectionInstallOptions = {},
): Promise<CollectionResult> {
  const scope = options.scope ?? 'global'
  const projects = normalizeProjectPaths(options.projects ?? [])
  if (tools.length > 0 && scope === 'project' && projects.length === 0) {
    throw new Error('projectSync.projectRequired')
  }
  const skill = manifest.skills[index]
  const source = manifest.sources[skill.source]
  const repoUrl = `https://github.com/${source.repo}/tree/${source.ref}`
  let created = previous
  if (!created) {
    let candidates = options.candidates?.get(repoUrl)
    if (!candidates) {
      candidates = await invoke<GitSkillCandidate[]>('list_git_skills_cmd', { repoUrl })
      options.candidates?.set(repoUrl, candidates)
    }
    const candidate = candidates.find(item => item.subpath.replace(/\\/g, '/') === skill.path && item.name === skill.name)
    if (!candidate) throw new Error('collectionInstall.missing')
    if (candidate.status === 'conflict') throw new Error('collectionInstall.conflict')
    if (candidate.status === 'update') {
      if (tools.length === 0) return { state: 'existing' }
      const managed = await invoke<ManagedSkill[]>('get_managed_skills')
      const existing = managed.find(item => item.id === candidate.existing_skill_id)
      if (!existing) throw new Error('collectionInstall.missing')
      created = { skill_id: existing.id, central_path: existing.central_path, name: existing.name, action: 'unchanged' }
    } else {
      created = await invoke<InstallResultDto>('install_git_selection', { repoUrl, subpath: skill.path, name: skill.name })
    }
    onInstalled?.(created)
  }
  if (created.action === 'installed' && options.tagIds?.length) {
    const managed = await invoke<ManagedSkill[]>('get_managed_skills')
    const current = managed.find(item => item.id === created.skill_id)
    if (!current) throw new Error('collectionInstall.missing')
    await invoke('set_skill_tags', { skillId: created.skill_id, tagIds: [...new Set([...current.tags.map(tag => tag.id), ...options.tagIds])] })
  }
  for (const job of buildInstallSyncJobs(tools, scope, projects)) {
    await invoke('sync_skill_to_tool', {
      sourcePath: created.central_path, skillId: created.skill_id, tool: job.toolId, name: created.name,
      scope: job.scope, ...(job.scope === 'project' ? { projectPath: job.projectPath } : {}), overwriteIfSameContent: true,
    })
  }
  return { state: created.action === 'unchanged' ? 'existing' : 'installed' }
}
