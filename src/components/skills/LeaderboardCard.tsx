import { memo, useCallback } from 'react'
import { ExternalLink, Download, Star, TrendingUp } from 'lucide-react'
import { openUrl } from '@tauri-apps/plugin-opener'
import type { TFunction } from 'i18next'

export type LeaderboardEntry = {
  rank: number
  name: string
  repo: string
  owner: string
  skill_slug: string
  description?: string | null
  installs: number
  installs_formatted: string
}

type LeaderboardCardProps = {
  entry: LeaderboardEntry
  onInstall: (entry: LeaderboardEntry) => void
  installState: 'idle' | 'queued' | 'downloading' | 'syncing' | 'completed'
  isInstalled: boolean
  t: TFunction
}

const getRankStyle = (rank: number) => {
  if (rank === 1) return { class: 'rank-gold', emoji: '🥇', gradient: 'from-amber-400 to-yellow-500' }
  if (rank === 2) return { class: 'rank-silver', emoji: '🥈', gradient: 'from-slate-300 to-slate-400' }
  if (rank === 3) return { class: 'rank-bronze', emoji: '🥉', gradient: 'from-orange-400 to-amber-600' }
  return { class: '', emoji: '', gradient: '' }
}

const LeaderboardCard = ({
  entry,
  onInstall,
  installState,
  isInstalled,
  t,
}: LeaderboardCardProps) => {
  const skillPath = (entry.skill_slug || entry.name).trim()
  const skillsShUrl = `https://skills.sh/${encodeURIComponent(entry.owner)}/${encodeURIComponent(entry.repo)}/${encodeURIComponent(skillPath)}`
  const rankStyle = getRankStyle(entry.rank)
  const isTopThree = entry.rank <= 3

  const installLabel = isInstalled
    ? t('leaderboard.installed')
    : installState === 'queued'
      ? t('leaderboard.queued')
      : installState === 'syncing'
        ? t('leaderboard.syncing')
        : installState === 'completed'
          ? t('leaderboard.completed')
          : installState === 'downloading'
          ? t('leaderboard.installing')
          : t('leaderboard.install')

  const handleOpenDetails = useCallback(() => {
    void openUrl(skillsShUrl).catch((error) => {
      console.error('failed to open leaderboard skill url via tauri opener', error)
      if (typeof window !== 'undefined') {
        window.open(skillsShUrl, '_blank', 'noopener,noreferrer')
      }
    })
  }, [skillsShUrl])

  return (
    <div className={`lb-card ${rankStyle.class}`}>
      {/* Rank */}
      <div className={`lb-rank ${rankStyle.class}`}>
        {isTopThree ? (
          <span className="lb-rank-emoji">{rankStyle.emoji}</span>
        ) : (
          <span className="lb-rank-number">{entry.rank}</span>
        )}
      </div>

      {/* Content */}
      <div className="lb-body">
        <div className="lb-top">
          <div className="lb-info">
            <button type="button" onClick={handleOpenDetails} className="lb-name-link">
              <h3 className="lb-name">{entry.name}</h3>
              <ExternalLink size={12} className="lb-name-icon" />
            </button>
            <span className="lb-repo">{entry.owner}/{entry.repo}</span>
          </div>
          <div className={`lb-stats ${rankStyle.class}`}>
            <TrendingUp size={14} />
            <span>{entry.installs_formatted}</span>
          </div>
        </div>

        {entry.description && (
          <p className="lb-desc">{entry.description}</p>
        )}

        <div className="lb-footer">
          {isTopThree && (
            <div className={`lb-badge ${rankStyle.class}`}>
              <Star size={12} />
              <span>TOP {entry.rank}</span>
            </div>
          )}
          <button
            className={`lb-btn ${isTopThree ? rankStyle.class : ''}`}
            onClick={() => onInstall(entry)}
            disabled={isInstalled || installState !== 'idle'}
          >
            <Download size={14} />
            <span>{installLabel}</span>
          </button>
        </div>
      </div>
    </div>
  )
}

export default memo(LeaderboardCard)
