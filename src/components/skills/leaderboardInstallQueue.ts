export type QueueStage =
  | 'queued'
  | 'downloading'
  | 'syncing'
  | 'completed'
  | 'failed'

export type QueueEntryInput = {
  rank: number
  name: string
  repo: string
  owner: string
  skill_slug: string
}

export type QueueItem = {
  id: string
  key: string
  name: string
  skillSlug: string
  repoUrl: string
  stage: QueueStage
  createdAt: number
  updatedAt: number
  error?: string
}

const ACTIVE_STAGES: QueueStage[] = ['queued', 'downloading', 'syncing']

const buildQueueKey = (entry: QueueEntryInput) => {
  const skillPath = (entry.skill_slug || entry.name).trim()
  return `${entry.owner}/${entry.repo}/${skillPath}`
}

export const enqueueLeaderboardInstall = (
  queue: QueueItem[],
  entry: QueueEntryInput,
  now = Date.now(),
): { items: QueueItem[]; itemId: string; enqueued: boolean } => {
  const key = buildQueueKey(entry)
  const duplicated = queue.find(
    (item) => item.key === key && ACTIVE_STAGES.includes(item.stage),
  )
  if (duplicated) {
    return { items: queue, itemId: duplicated.id, enqueued: false }
  }

  const id = `${key}#${now}-${queue.length + 1}`
  const item: QueueItem = {
    id,
    key,
    name: entry.name,
    skillSlug: (entry.skill_slug || entry.name).trim(),
    repoUrl: `https://github.com/${entry.owner}/${entry.repo}`,
    stage: 'queued',
    createdAt: now,
    updatedAt: now,
  }
  return {
    items: [...queue, item],
    itemId: id,
    enqueued: true,
  }
}

export const pickQueuedInstallIds = (
  queue: QueueItem[],
  runningIds: Set<string>,
  maxParallel: number,
): string[] => {
  if (maxParallel <= 0) {
    return []
  }
  const picked: string[] = []
  for (const item of queue) {
    if (item.stage !== 'queued') {
      continue
    }
    if (runningIds.has(item.id)) {
      continue
    }
    picked.push(item.id)
    if (picked.length >= maxParallel) {
      break
    }
  }
  return picked
}

export const updateQueueItemStage = (
  queue: QueueItem[],
  itemId: string,
  stage: QueueStage,
  now = Date.now(),
  error?: string,
): QueueItem[] =>
  queue.map((item) =>
    item.id !== itemId
      ? item
      : {
          ...item,
          stage,
          updatedAt: now,
          error: stage === 'failed' ? error ?? item.error : undefined,
        },
  )
