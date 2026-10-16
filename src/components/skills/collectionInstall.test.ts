import { describe, expect, it } from 'vitest'
import { installCollectionEntry } from './collectionInstall'
import type { CollectionManifest } from './types'

const manifest: CollectionManifest = { v: 1, title: 'Design', sources: [{ repo: 'owner/repo', ref: 'a'.repeat(40) }], skills: [{ name: 'design', path: 'skills/design', source: 0 }] }
describe('website collection installation', () => {
  it('preserves an existing same-source skill instead of replacing it', async () => {
    const calls: string[] = []
    const result = await installCollectionEntry(async <T>(command: string) => {
      if (command === 'resolve_collection_revision') return 'b'.repeat(40) as T
      calls.push(command)
      return [{ name: 'design', subpath: 'skills/design', status: 'update', existing_skill_id: 'existing' }] as T
    }, manifest, 0, [])
    expect(result.state).toBe('existing')
    expect(calls).toEqual(['list_collection_skills'])
  })
  it('matches Windows repository paths against the website directory', async () => {
    const calls: string[] = []
    const result = await installCollectionEntry(async <T>(command: string) => {
      if (command === 'resolve_collection_revision') return 'b'.repeat(40) as T
      calls.push(command)
      return (command === 'list_collection_skills'
        ? [{ name: 'design', subpath: 'skills\\design', status: 'install' }]
        : { skill_id: 'id', central_path: '/tmp/design', name: 'design', action: 'installed' }) as T
    }, manifest, 0, [])
    expect(result.state).toBe('installed')
    expect(calls).toEqual(['list_collection_skills', 'install_collection_selection'])
  })
  it('rejects a missing or conflicting entry without installing or distributing it', async () => {
    for (const status of ['conflict', 'missing']) {
      const calls: string[] = []
      await expect(installCollectionEntry(async <T>(command: string) => {
      if (command === 'resolve_collection_revision') return 'b'.repeat(40) as T
        calls.push(command)
        return (status === 'missing' ? [] : [{ name: 'design', subpath: 'skills/design', status }]) as T
      }, manifest, 0, ['cursor'])).rejects.toThrow()
      expect(calls).toEqual(['list_collection_skills'])
    }
  })
  it('installs the latest default-branch version at the exact path and only distributes to explicitly selected tools', async () => {
    const calls: { command: string; args: unknown }[] = []
    const result = await installCollectionEntry(async <T>(command: string, args?: Record<string, unknown>) => {
      if (command === 'resolve_collection_revision') return 'b'.repeat(40) as T
      calls.push({ command, args })
      return (command === 'list_collection_skills' ? [{ name: 'design', subpath: 'skills/design', status: 'install' }] : { skill_id: 'new-id', central_path: '/tmp/test/design', name: 'design', action: 'installed' }) as T
    }, manifest, 0, ['cursor'])
    expect(result.state).toBe('installed')
    expect(calls).toEqual([
      { command: 'list_collection_skills', args: { repoUrl: 'https://github.com/owner/repo', revision: 'b'.repeat(40) } },
      { command: 'install_collection_selection', args: { repoUrl: 'https://github.com/owner/repo', revision: 'b'.repeat(40), subpath: 'skills/design', name: 'design' } },
      { command: 'sync_skill_to_tool', args: { sourcePath: '/tmp/test/design', skillId: 'new-id', tool: 'cursor', name: 'design', scope: 'global', overwriteIfSameContent: true } },
    ])
  })
  it('retries distribution after a partial failure without reinstalling the Skill', async () => {
    const calls: string[] = []
    await installCollectionEntry(async <T>(command: string) => {
      if (command === 'resolve_collection_revision') return 'b'.repeat(40) as T; calls.push(command); return undefined as T }, manifest, 0, ['cursor'], { skill_id: 'id', central_path: '/tmp/test/design', name: 'design' })
    expect(calls).toEqual(['sync_skill_to_tool'])
  })
})

it('distributes a same-source existing skill without replacing or tagging it', async () => {
  const calls: { command: string; args: unknown }[] = []
  const result = await installCollectionEntry(async <T>(command: string, args?: Record<string, unknown>) => {
      if (command === 'resolve_collection_revision') return 'b'.repeat(40) as T
    calls.push({ command, args })
    if (command === 'list_collection_skills') return [{ name: 'design', subpath: 'skills/design', status: 'update', existing_skill_id: 'existing' }] as T
    if (command === 'get_managed_skills') return [{ id: 'existing', name: 'design', central_path: '/library/design', tags: [{ id: 9, name: 'keep' }] }] as T
    return undefined as T
  }, manifest, 0, ['cursor'], undefined, undefined, { tagIds: [7] })
  expect(result.state).toBe('existing')
  expect(calls.map(call => call.command)).toEqual(['list_collection_skills', 'get_managed_skills', 'sync_skill_to_tool'])
  expect(calls.at(-1)?.args).toMatchObject({ skillId: 'existing', sourcePath: '/library/design' })
})

it('tags new installs before project distribution and preserves their existing tags', async () => {
  const calls: { command: string; args: unknown }[] = []
  await installCollectionEntry(async <T>(command: string, args?: Record<string, unknown>) => {
      if (command === 'resolve_collection_revision') return 'b'.repeat(40) as T
    calls.push({ command, args })
    if (command === 'get_managed_skills') return [{ id: 'new', tags: [{ id: 9, name: 'keep' }] }] as T
    return undefined as T
  }, manifest, 0, ['cursor'], { skill_id: 'new', name: 'design', central_path: '/library/design', action: 'installed' }, undefined, { tagIds: [7], scope: 'project', projects: ['/work/project'] })
  expect(calls).toEqual([
    { command: 'get_managed_skills', args: undefined },
    { command: 'set_skill_tags', args: { skillId: 'new', tagIds: [9, 7] } },
    { command: 'sync_skill_to_tool', args: { sourcePath: '/library/design', skillId: 'new', tool: 'cursor', name: 'design', scope: 'project', projectPath: '/work/project', overwriteIfSameContent: true } },
  ])
})

it('rejects project distribution without a directory before installing', async () => {
  const calls: string[] = []
  await expect(installCollectionEntry(async <T>(command: string) => {
      if (command === 'resolve_collection_revision') return 'b'.repeat(40) as T; calls.push(command); return undefined as T }, manifest, 0, ['cursor'], undefined, undefined, { scope: 'project', projects: [] })).rejects.toThrow('projectSync.projectRequired')
  expect(calls).toEqual([])
})

it('scans a resolved repository once per attempt for multiple collection entries', async () => {
  let scans = 0
  const cache = new Map()
  const revisions = new Map<string, string>()
  const batch = { ...manifest, skills: [...manifest.skills, { name: 'review', path: 'skills/review', source: 0 }] }
  const invoke = async <T>(command: string) => {
      if (command === 'resolve_collection_revision') return 'b'.repeat(40) as T
    if (command === 'list_collection_skills') {
      scans++
      return [{ name: 'design', subpath: 'skills/design', status: 'install' }, { name: 'review', subpath: 'skills/review', status: 'install' }] as T
    }
    return { skill_id: 'id', central_path: '/library/item', name: 'item', action: 'installed' } as T
  }
  await installCollectionEntry(invoke, batch, 0, [], undefined, undefined, { candidates: cache, revisions })
  await installCollectionEntry(invoke, batch, 1, [], undefined, undefined, { candidates: cache, revisions })
  expect(scans).toBe(1)
})

it('uses the verified same-source ID when the library name differs from the manifest', async () => {
  let distributed: unknown
  await installCollectionEntry(async <T>(command: string, args?: Record<string, unknown>) => {
      if (command === 'resolve_collection_revision') return 'b'.repeat(40) as T
    if (command === 'list_collection_skills') return [{ name: 'design', subpath: 'skills/design', status: 'update', existing_skill_id: 'same-source' }] as T
    if (command === 'get_managed_skills') return [{ id: 'wrong-source', name: 'design', central_path: '/library/wrong' }, { id: 'same-source', name: 'design-custom', central_path: '/library/custom' }] as T
    if (command === 'sync_skill_to_tool') distributed = args
    return undefined as T
  }, manifest, 0, ['cursor'])
  expect(distributed).toMatchObject({ skillId: 'same-source', name: 'design-custom', sourcePath: '/library/custom' })
})

it('resolves latest once per repository across source refs and retries, and never falls back to the published commit', async () => {
  const revisions = new Map<string, string>()
  const batch = { ...manifest, sources: [...manifest.sources, { repo: 'Owner/Repo', ref: 'c'.repeat(40) }], skills: [...manifest.skills, { name: 'review', path: 'skills/review', source: 1 }] }
  let resolutions = 0
  const installs: unknown[] = []
  const invoke = async <T>(command: string, args?: Record<string, unknown>) => {
    if (command === 'resolve_collection_revision') { resolutions++; return 'b'.repeat(40) as T }
    if (command === 'list_collection_skills') return [{ name: 'design', subpath: 'skills/design', status: 'install' }, { name: 'review', subpath: 'skills/review', status: 'install' }] as T
    installs.push(args)
    return { skill_id: 'id', central_path: '/library/item', name: 'item', action: 'installed' } as T
  }
  await installCollectionEntry(invoke, batch, 0, [], undefined, undefined, { revisions })
  await installCollectionEntry(invoke, batch, 1, [], undefined, undefined, { revisions })
  expect(resolutions).toBe(1)
  expect(installs).toEqual([
    { repoUrl: 'https://github.com/owner/repo', revision: 'b'.repeat(40), subpath: 'skills/design', name: 'design' },
    { repoUrl: 'https://github.com/owner/repo', revision: 'b'.repeat(40), subpath: 'skills/review', name: 'review' },
  ])
  const calls: string[] = []
  await expect(installCollectionEntry(async <T>(command: string) => {
    calls.push(command)
    return Promise.reject<T>(new Error('Unable to resolve default branch'))
  }, manifest, 0, [])).rejects.toThrow('Unable to resolve default branch')
  expect(calls).toEqual(['resolve_collection_revision'])
})
