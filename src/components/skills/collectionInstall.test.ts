import { describe, expect, it } from 'vitest'
import { installCollectionEntry } from './collectionInstall'
import type { CollectionManifest } from './types'

const manifest: CollectionManifest = { v: 1, title: 'Design', sources: [{ repo: 'owner/repo', ref: 'a'.repeat(40) }], skills: [{ name: 'design', path: 'skills/design', source: 0 }] }
describe('website collection installation', () => {
  it('preserves an existing same-source skill instead of replacing it', async () => {
    const calls: string[] = []
    const result = await installCollectionEntry(async <T>(command: string) => {
      calls.push(command)
      return [{ name: 'design', subpath: 'skills/design', status: 'update' }] as T
    }, manifest, 0, [])
    expect(result.state).toBe('existing')
    expect(calls).toEqual(['list_git_skills_cmd'])
  })
  it('matches Windows repository paths against the website directory', async () => {
    const calls: string[] = []
    const result = await installCollectionEntry(async <T>(command: string) => {
      calls.push(command)
      return (command === 'list_git_skills_cmd'
        ? [{ name: 'design', subpath: 'skills\\design', status: 'install' }]
        : { skill_id: 'id', central_path: '/tmp/design', name: 'design', action: 'installed' }) as T
    }, manifest, 0, [])
    expect(result.state).toBe('installed')
    expect(calls).toEqual(['list_git_skills_cmd', 'install_git_selection'])
  })
  it('rejects a missing or conflicting entry without installing or distributing it', async () => {
    for (const status of ['conflict', 'missing']) {
      const calls: string[] = []
      await expect(installCollectionEntry(async <T>(command: string) => {
        calls.push(command)
        return (status === 'missing' ? [] : [{ name: 'design', subpath: 'skills/design', status }]) as T
      }, manifest, 0, ['cursor'])).rejects.toThrow()
      expect(calls).toEqual(['list_git_skills_cmd'])
    }
  })
  it('installs the exact pinned path and only distributes to explicitly selected tools', async () => {
    const calls: { command: string; args: unknown }[] = []
    const result = await installCollectionEntry(async <T>(command: string, args?: Record<string, unknown>) => {
      calls.push({ command, args })
      return (command === 'list_git_skills_cmd' ? [{ name: 'design', subpath: 'skills/design', status: 'install' }] : { skill_id: 'new-id', central_path: '/tmp/test/design', name: 'design', action: 'installed' }) as T
    }, manifest, 0, ['cursor'])
    expect(result.state).toBe('installed')
    expect(calls).toEqual([
      { command: 'list_git_skills_cmd', args: { repoUrl: `https://github.com/owner/repo/tree/${'a'.repeat(40)}` } },
      { command: 'install_git_selection', args: { repoUrl: `https://github.com/owner/repo/tree/${'a'.repeat(40)}`, subpath: 'skills/design', name: 'design' } },
      { command: 'sync_skill_to_tool', args: { sourcePath: '/tmp/test/design', skillId: 'new-id', tool: 'cursor', name: 'design', scope: 'global', overwriteIfSameContent: true } },
    ])
  })
  it('retries distribution after a partial failure without reinstalling the Skill', async () => {
    const calls: string[] = []
    await installCollectionEntry(async <T>(command: string) => { calls.push(command); return undefined as T }, manifest, 0, ['cursor'], { skill_id: 'id', central_path: '/tmp/test/design', name: 'design' })
    expect(calls).toEqual(['sync_skill_to_tool'])
  })
})
