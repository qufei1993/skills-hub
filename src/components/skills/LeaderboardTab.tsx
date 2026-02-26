import { memo, useCallback, useEffect, useMemo, useState } from 'react'
import { Search, Trophy, TrendingUp, Flame } from 'lucide-react'
import type { TFunction } from 'i18next'
import { invoke } from '@tauri-apps/api/core'
import LeaderboardCard, { type LeaderboardEntry } from './LeaderboardCard'

type LeaderboardType = 'all' | 'trending' | 'hot'

type LeaderboardTabProps = {
  onInstallSkill: (repoUrl: string, name?: string) => Promise<void>
  t: TFunction
}

const LeaderboardTab = ({ onInstallSkill, t }: LeaderboardTabProps) => {
  const [leaderboardType, setLeaderboardType] = useState<LeaderboardType>('all')
  const [searchQuery, setSearchQuery] = useState('')
  const [entries, setEntries] = useState<LeaderboardEntry[]>([])
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [installingRank, setInstallingRank] = useState<number | null>(null)

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

  const handleInstall = useCallback(
    async (entry: LeaderboardEntry) => {
      const repoUrl = `https://github.com/${entry.owner}/${entry.repo}`
      setInstallingRank(entry.rank)
      try {
        await onInstallSkill(repoUrl, entry.name)
      } catch {
        // Error is handled by parent component
      } finally {
        setInstallingRank(null)
      }
    },
    [onInstallSkill]
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
              installing={installingRank === entry.rank}
              t={t}
            />
          ))}
        </div>
      )}
    </div>
  )
}

export default memo(LeaderboardTab)
