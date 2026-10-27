import { useMemo } from 'react'
import type { TFunction } from 'i18next'
import type { ContentPlan } from './ContentReview'
import { buildContentDiff, buildInlineDiff, type ContentDiffRow } from './contentDiff'

type Change = ContentPlan['changes'][number]
type Props = { change: Change; t: TFunction }
const sides = ['before', 'after'] as const

function isAbsent(change: Change, side: typeof sides[number]) {
  return change[`${side}Metadata`] === null || (side === 'before' ? change.kind === 'added' : change.kind === 'removed')
}

function DiffRow({ row, t }: { row: ContentDiffRow; t: TFunction }) {
  const inline = row.changed && row.before && row.after ? buildInlineDiff(row.before.text, row.after.text) : null
  return <div className="content-diff-row">
    {sides.map(side => {
      const line = row[side]
      const opposite = row[side === 'before' ? 'after' : 'before']
      return <div className={`content-diff-line content-diff-line-${line?.kind ?? 'placeholder'} content-diff-line-${side}`} key={side}>
        {line ? <>
          <span className="content-diff-line-number" aria-label={t(`workspace110.diff.${side}Line`, { count: line.lineNumber })}>{line.lineNumber}</span>
          <span className="content-diff-sign" aria-label={line.kind === 'equal' ? undefined : t(`workspace110.diff.${line.kind}`)}>{line.kind === 'added' ? '+' : line.kind === 'removed' ? '−' : ' '}</span>
          <code>{inline ? inline[side].map((part, index) => part.changed ? <mark key={index}>{part.text}</mark> : part.text) : line.text || '\u00a0'}{row.changed && opposite && line.lineEnding !== opposite.lineEnding ? <span className="content-diff-line-ending">{line.lineEnding === 'none' ? t('workspace110.diff.noNewline') : line.lineEnding.toUpperCase()}</span> : null}</code>
        </> : <span aria-hidden="true">{'\u00a0'}</span>}
      </div>
    })}
  </div>
}

function groupRows(rows: ContentDiffRow[]) {
  const groups: { start: number; rows: ContentDiffRow[]; collapsed: boolean }[] = []
  let index = 0
  while (index < rows.length) {
    if (rows[index].changed) {
      groups.push({ start: index, rows: [rows[index++]], collapsed: false })
      continue
    }
    const start = index
    while (index < rows.length && !rows[index].changed) index++
    const leading = start === 0 ? 0 : 3
    const trailing = index === rows.length ? 0 : 3
    if (index - start > 8 && index - start > leading + trailing + 2) {
      if (leading) groups.push({ start, rows: rows.slice(start, start + leading), collapsed: false })
      groups.push({ start: start + leading, rows: rows.slice(start + leading, index - trailing), collapsed: true })
      if (trailing) groups.push({ start: index - trailing, rows: rows.slice(index - trailing, index), collapsed: false })
    } else groups.push({ start, rows: rows.slice(start, index), collapsed: false })
  }
  return groups
}

export default function ContentFileDiff({ change, t }: Props) {
  const textAvailable = sides.every(side => typeof change[side] === 'string' || isAbsent(change, side))
    && sides.every(side => change[`${side}Metadata`]?.kind !== 'directory')
  const diff = useMemo(() => textAvailable ? buildContentDiff(change.before ?? '', change.after ?? '') : null, [change.before, change.after, textAvailable])
  const metadataChanged = JSON.stringify(change.beforeMetadata) !== JSON.stringify(change.afterMetadata)
  const w = (key: string) => t(`workspace110.${key}`)
  return <div className="content-file-diff">
    {diff ? <div className="content-diff-overview">
      <div className="content-diff-counts">
        <span className="content-diff-removed">{t('workspace110.diff.removedLines', { count: diff.removed })}</span>
        <span className="content-diff-added">{t('workspace110.diff.addedLines', { count: diff.added })}</span>
      </div>
      {change.kind === 'modified' && diff.added === 0 && diff.removed === 0 ? <span>{w(metadataChanged ? 'diff.metadataOnly' : 'diff.textUnchanged')}</span> : null}
      {diff.isApproximate ? <span>{w('diff.largeReplacement')}</span> : null}
    </div> : null}
    <div className="content-diff-headings">
      {sides.map(side => {
        const metadata = change[`${side}Metadata`]
        return <div key={side}>
          <strong>{w(side)}</strong>
          {metadata ? <span className={metadataChanged ? `content-diff-metadata content-diff-metadata-${side}` : 'content-diff-metadata'}>
            {metadata.kind === 'directory' ? w('directoryEntry') : null}
            {metadata.mode != null ? <span>{t('workspace110.permissions', { mode: (metadata.mode & 0o7777).toString(8).padStart(4, '0') })}</span> : metadata.readonly != null ? <span>{w(metadata.readonly ? 'readOnlyEntry' : 'writableEntry')}</span> : null}
          </span> : null}
        </div>
      })}
    </div>
    {diff ? <div className="content-diff-code" role="region" aria-label={`${change.path} · ${w('diff.comparison')}`} tabIndex={0}>
      {groupRows(diff.rows).map(group => group.collapsed ? <details className="content-diff-context" key={group.start}>
        <summary>{t('workspace110.diff.unchangedLines', { count: group.rows.length })}</summary>
        {group.rows.map((row, index) => <DiffRow row={row} key={index} t={t} />)}
      </details> : group.rows.map((row, index) => <DiffRow row={row} key={group.start + index} t={t} />))}
      {diff.rows.length === 0 ? <p className="content-diff-empty">{w('diff.emptyFile')}</p> : null}
    </div> : <div className="content-diff-fallback">
      {sides.map(side => <div key={side}><span className="content-diff-mobile-label">{w(side)}</span>{change[`${side}Metadata`]?.kind === 'directory' ? null : typeof change[side] === 'string' ? <pre>{change[side] || w('diff.emptyFile')}</pre> : <p>{w(isAbsent(change, side) ? 'diff.absentFile' : 'diff.unavailable')}</p>}</div>)}
    </div>}
  </div>
}
