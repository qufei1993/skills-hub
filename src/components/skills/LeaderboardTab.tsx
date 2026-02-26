import { memo, useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { Search, Trophy, TrendingUp, Flame } from 'lucide-react'
import type { TFunction } from 'i18next'
import { invoke } from '@tauri-apps/api/core'
import LeaderboardCard, { type LeaderboardEntry } from './LeaderboardCard'
import {
  isLeaderboardEntryInstalled,
} from './leaderboardInstalled'
import {
  enqueueLeaderboardInstall,
  pickQueuedInstallIds,
  updateQueueItemStage,
  type QueueItem,
  type QueueStage,
} from './leaderboardInstallQueue'

type LeaderboardType = 'all' | 'trending' | 'hot'
type InstallProgressPhase = 'downloading' | 'syncing'
type EntryInstallState =
  | 'idle'
  | 'queued'
  | 'downloading'
  | 'syncing'
  | 'completed'
const MAX_PARALLEL_DOWNLOADS = 2

type LeaderboardTabProps = {
  onInstallSkill: (
    repoUrl: string,
    name?: string,
    skillSlug?: string,
    onProgress?: (phase: InstallProgressPhase) => void,
  ) => Promise<void>
  installedSkillNames: string[]
  t: TFunction
}

const LeaderboardTab = ({ onInstallSkill, installedSkillNames, t }: LeaderboardTabProps) => {
  const [leaderboardType, setLeaderboardType] = useState<LeaderboardType>('all')
  const [searchQuery, setSearchQuery] = useState('')
  const [entries, setEntries] = useState<LeaderboardEntry[]>([])
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [installQueue, setInstallQueue] = useState<QueueItem[]>([])
  const runningQueueIdsRef = useRef(new Set<string>())
  const mountedRef = useRef(true)
  const entryQueueKey = useCallback(
    (entry: Pick<LeaderboardEntry, 'owner' | 'repo' | 'skill_slug' | 'name'>) =>
      `${entry.owner}/${entry.repo}/${(entry.skill_slug || entry.name).trim()}`,
    [],
  )

  useEffect(
    () => {
      mountedRef.current = true
      return () => {
        mountedRef.current = false
      }
    },
    [],
  )

  const loadLeaderboard = useCallback(async (rawQuery: string) => {
    setLoading(true)
    setError(null)
    try {
      const query = rawQuery.trim()
      const result = query
        ? await invoke<LeaderboardEntry[]>('search_skills_sh', { query })
        : await invoke<LeaderboardEntry[]>('get_skills_leaderboard', {
            leaderboardType: leaderboardType,
          })
      setEntries(result)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setLoading(false)
    }
  }, [leaderboardType])

  useEffect(() => {
    if (!searchQuery.trim()) {
      void loadLeaderboard('')
      return
    }

    const timer = window.setTimeout(() => {
      void loadLeaderboard(searchQuery)
    }, 250)
    return () => window.clearTimeout(timer)
  }, [loadLeaderboard, searchQuery])

  const filteredEntries = useMemo(() => entries, [entries])
  const installedNameSet = useMemo(
    () => new Set(installedSkillNames),
    [installedSkillNames],
  )

  const setQueueStage = useCallback(
    (itemId: string, stage: QueueStage, error?: string) => {
      if (!mountedRef.current) return
      setInstallQueue((prev) =>
        updateQueueItemStage(prev, itemId, stage, Date.now(), error),
      )
    },
    [],
  )

  const executeQueueItem = useCallback(
    async (item: QueueItem) => {
      setQueueStage(item.id, 'downloading')
      try {
        await onInstallSkill(item.repoUrl, item.name, item.skillSlug, (phase) => {
          if (phase === 'syncing') {
            setQueueStage(item.id, 'syncing')
            return
          }
          setQueueStage(item.id, 'downloading')
        })
        setQueueStage(item.id, 'completed')
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err)
        setQueueStage(item.id, 'failed', message)
      } finally {
        runningQueueIdsRef.current.delete(item.id)
      }
    },
    [onInstallSkill, setQueueStage],
  )

  useEffect(() => {
    if (!mountedRef.current) return
    const available = Math.max(0, MAX_PARALLEL_DOWNLOADS - runningQueueIdsRef.current.size)
    if (available === 0) return

    const ids = pickQueuedInstallIds(installQueue, runningQueueIdsRef.current, available)
    if (ids.length === 0) return

    for (const id of ids) {
      const item = installQueue.find((q) => q.id === id)
      if (!item) continue
      runningQueueIdsRef.current.add(id)
      void executeQueueItem(item)
    }
  }, [executeQueueItem, installQueue])

  const currentStageByEntryKey = useMemo(() => {
    const out = new Map<string, Exclude<EntryInstallState, 'idle'>>()
    for (const item of installQueue) {
      if (
        item.stage === 'queued' ||
        item.stage === 'downloading' ||
        item.stage === 'syncing' ||
        item.stage === 'completed'
      ) {
        out.set(item.key, item.stage)
      } else {
        out.delete(item.key)
      }
    }
    return out
  }, [installQueue])

  const queueStats = useMemo(() => {
    let queued = 0
    let running = 0
    let completed = 0
    let failed = 0
    for (const item of installQueue) {
      if (item.stage === 'queued') {
        queued += 1
      } else if (item.stage === 'downloading' || item.stage === 'syncing') {
        running += 1
      } else if (item.stage === 'completed') {
        completed += 1
      } else {
        failed += 1
      }
    }
    return { queued, running, completed, failed }
  }, [installQueue])

  const handleInstall = useCallback(
    (entry: LeaderboardEntry) => {
      if (isLeaderboardEntryInstalled(entry, installedNameSet)) {
        return
      }
      setInstallQueue((prev) => {
        const out = enqueueLeaderboardInstall(prev, entry, Date.now())
        return out.items
      })
    },
    [installedNameSet],
  )

  const tabs = useMemo(
    () => [
      { id: 'all' as LeaderboardType, label: t('leaderboard.allTime'), icon: Trophy },
      { id: 'trending' as LeaderboardType, label: t('leaderboard.trending'), icon: TrendingUp },
      { id: 'hot' as LeaderboardType, label: t('leaderboard.hot'), icon: Flame },
    ],
    [t]
  )

  return (
    <div className="leaderboard-tab">
      <div className="leaderboard-header-bar">
        <div className="leaderboard-tabs">
          {tabs.map((tab) => {
            const Icon = tab.icon
            return (
              <button
                key={tab.id}
                className={`leaderboard-tab ${leaderboardType === tab.id ? 'active' : ''}`}
                onClick={() => setLeaderboardType(tab.id)}
              >
                <Icon size={16} />
                {tab.label}
              </button>
            )
          })}
        </div>
        <div className="leaderboard-search">
          <Search size={16} className="search-icon" />
          <input
            type="text"
            className="search-input"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder={t('leaderboard.searchPlaceholder')}
          />
        </div>
      </div>

      <div className="leaderboard-queue-panel">
        <div className="leaderboard-queue-head">
          <strong>{t('leaderboard.downloadQueue')}</strong>
          <span>
            {t('leaderboard.queueSummary', {
              queued: queueStats.queued,
              running: queueStats.running,
              completed: queueStats.completed,
              failed: queueStats.failed,
            })}
          </span>
        </div>
        {installQueue.length === 0 ? (
          <div className="leaderboard-queue-empty">{t('leaderboard.queueEmpty')}</div>
        ) : (
          <div className="leaderboard-queue-list">
            {installQueue
              .slice()
              .reverse()
              .map((item) => (
                <div key={item.id} className="leaderboard-queue-item">
                  <span className="leaderboard-queue-name">{item.name}</span>
                  <span className={`leaderboard-queue-stage stage-${item.stage}`}>
                    {item.stage === 'queued'
                      ? t('leaderboard.queued')
                      : item.stage === 'downloading'
                        ? t('leaderboard.installing')
                        : item.stage === 'syncing'
                          ? t('leaderboard.syncing')
                          : item.stage === 'completed'
                            ? t('leaderboard.completed')
                            : t('leaderboard.failed')}
                  </span>
                </div>
              ))}
          </div>
        )}
      </div>

      {loading ? (
        <div className="leaderboard-loading">{t('loading')}</div>
      ) : error ? (
        <div className="leaderboard-error">{error}</div>
      ) : filteredEntries.length === 0 ? (
        <div className="leaderboard-empty">
          {searchQuery ? t('skillsEmpty') : t('leaderboard.empty')}
        </div>
      ) : (
        <div className="leaderboard-list">
          {filteredEntries.map((entry) => (
            <LeaderboardCard
              key={`${entry.owner}/${entry.repo}/${entry.name}`}
              entry={entry}
              onInstall={handleInstall}
              installState={
                currentStageByEntryKey.get(entryQueueKey(entry)) ?? 'idle'
              }
              isInstalled={isLeaderboardEntryInstalled(entry, installedNameSet)}
              t={t}
            />
          ))}
        </div>
      )}
    </div>
  )
}

export default memo(LeaderboardTab)
