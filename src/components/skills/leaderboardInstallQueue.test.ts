import { describe, expect, it } from 'vitest'
import {
  enqueueLeaderboardInstall,
  pickQueuedInstallIds,
  type QueueEntryInput,
  type QueueItem,
  updateQueueItemStage,
} from './leaderboardInstallQueue'

const buildEntry = (name: string, rank: number): QueueEntryInput => ({
  rank,
  name,
  repo: 'skills',
  owner: 'acme',
  skill_slug: name.toLowerCase().replace(/\s+/g, '-'),
})

describe('leaderboardInstallQueue', () => {
  it('enqueues an item with queued stage and repo url', () => {
    const entry = buildEntry('Find Skills', 1)
    const out = enqueueLeaderboardInstall([], entry, 100)

    expect(out.enqueued).toBe(true)
    expect(out.items).toHaveLength(1)
    expect(out.items[0]).toMatchObject({
      name: 'Find Skills',
      skillSlug: 'find-skills',
      repoUrl: 'https://github.com/acme/skills',
      stage: 'queued',
      createdAt: 100,
      updatedAt: 100,
    })
  })

  it('skips duplicate active item for the same leaderboard skill', () => {
    const entry = buildEntry('Find Skills', 1)
    const first = enqueueLeaderboardInstall([], entry, 100)
    const second = enqueueLeaderboardInstall(first.items, entry, 200)

    expect(first.enqueued).toBe(true)
    expect(second.enqueued).toBe(false)
    expect(second.items).toHaveLength(1)
  })

  it('allows requeue after a failed attempt', () => {
    const entry = buildEntry('Find Skills', 1)
    const first = enqueueLeaderboardInstall([], entry, 100)
    const failed = updateQueueItemStage(first.items, first.itemId, 'failed', 120, 'network')
    const second = enqueueLeaderboardInstall(failed, entry, 200)

    expect(second.enqueued).toBe(true)
    expect(second.items).toHaveLength(2)
    expect(second.items[1].stage).toBe('queued')
  })

  it('picks queued items up to max parallel and keeps order', () => {
    const queue: QueueItem[] = [
      {
        id: 'a',
        key: 'acme/skills/a',
        name: 'A',
        skillSlug: 'a',
        repoUrl: 'https://github.com/acme/skills',
        stage: 'queued',
        createdAt: 1,
        updatedAt: 1,
      },
      {
        id: 'b',
        key: 'acme/skills/b',
        name: 'B',
        skillSlug: 'b',
        repoUrl: 'https://github.com/acme/skills',
        stage: 'queued',
        createdAt: 2,
        updatedAt: 2,
      },
      {
        id: 'c',
        key: 'acme/skills/c',
        name: 'C',
        skillSlug: 'c',
        repoUrl: 'https://github.com/acme/skills',
        stage: 'queued',
        createdAt: 3,
        updatedAt: 3,
      },
    ]

    const selected = pickQueuedInstallIds(queue, new Set(['b']), 2)
    expect(selected).toEqual(['a', 'c'])
  })

  it('updates stage and clears error for non-failed stages', () => {
    const queue: QueueItem[] = [
      {
        id: 'a',
        key: 'acme/skills/a',
        name: 'A',
        skillSlug: 'a',
        repoUrl: 'https://github.com/acme/skills',
        stage: 'failed',
        createdAt: 1,
        updatedAt: 1,
        error: 'oops',
      },
    ]

    const updated = updateQueueItemStage(queue, 'a', 'syncing', 3)
    expect(updated[0]).toMatchObject({
      stage: 'syncing',
      updatedAt: 3,
      error: undefined,
    })
  })
})
