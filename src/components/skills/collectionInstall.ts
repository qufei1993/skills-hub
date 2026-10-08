import type { CollectionManifest, CollectionResult, GitSkillCandidate, InstallResultDto } from './types'
export type CollectionInvoke = <T>(command: string, args?: Record<string, unknown>) => Promise<T>
export async function installCollectionEntry(
  invoke: CollectionInvoke, manifest: CollectionManifest, index: number, tools: string[],
  previous?: InstallResultDto, onInstalled?: (result: InstallResultDto) => void,
): Promise<CollectionResult> {
  const skill = manifest.skills[index]
  const source = manifest.sources[skill.source]
  const repoUrl = `https://github.com/${source.repo}/tree/${source.ref}`
  let created = previous
  if (!created) {
    const candidates = await invoke<GitSkillCandidate[]>('list_git_skills_cmd', { repoUrl })
    const candidate = candidates.find(item => item.subpath.replace(/\\/g, '/') === skill.path && item.name === skill.name)
    if (!candidate) throw new Error('collectionInstall.missing')
    if (candidate.status === 'conflict') throw new Error('collectionInstall.conflict')
    if (candidate.status === 'update') return { state: 'existing' }
    created = await invoke<InstallResultDto>('install_git_selection', { repoUrl, subpath: skill.path, name: skill.name })
    onInstalled?.(created)
  }
  for (const tool of tools) {
    await invoke('sync_skill_to_tool', { sourcePath: created.central_path, skillId: created.skill_id, tool, name: created.name, scope: 'global', overwriteIfSameContent: true })
  }
  return { state: created.action === 'unchanged' ? 'existing' : 'installed' }
}
