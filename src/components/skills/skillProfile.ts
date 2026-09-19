import type { SkillProfile } from './types'

export type PresentationElements = {
  color: boolean
  category: boolean
  englishName: boolean
  zhName: boolean
  summary: boolean
}

export const DEFAULT_PRESENTATION: PresentationElements = {
  color: true,
  category: true,
  englishName: true,
  zhName: true,
  summary: true,
}

export const SUMMARY_SOFT_MIN_CHARS = 12
export const SUMMARY_TARGET_MAX_CHARS = 30

export function normalizeHexColor(value?: string | null): string | null {
  if (!value) return null
  const raw = value.trim()
  const m = raw.match(/^#?([0-9a-fA-F]{6})$/)
  if (!m) return null
  return `#${m[1].toUpperCase()}`
}

export function countCjkAwareLength(value: string): number {
  return Array.from(value.trim()).length
}

export function isAcceptableSummary(value?: string | null): boolean {
  if (!value) return false
  const n = countCjkAwareLength(value)
  return n > 0 && n <= SUMMARY_TARGET_MAX_CHARS
}

export function buildAutoSummary(input: {
  name: string
  description?: string | null
  body?: string | null
}): string {
  const source = (input.description?.trim() || input.body?.trim() || input.name).replace(
    /[`*_>#\-[\]()]+/g,
    ' ',
  )
  const compact = source.replace(/\s+/g, ' ').trim()
  const chars = Array.from(compact)
  if (chars.length <= SUMMARY_TARGET_MAX_CHARS) return chars.join('')
  return chars.slice(0, SUMMARY_TARGET_MAX_CHARS).join('')
}

export function formatSkillTitleLine(input: {
  name: string
  profile?: SkillProfile | null
}): string {
  const category = input.profile?.category?.trim()
  const zh = input.profile?.zh_name?.trim()
  const english = input.name.trim()
  const bits: string[] = []
  if (category) bits.push(`【${category}】`)
  if (english) bits.push(english)
  if (zh) {
    if (english) bits.push('-')
    bits.push(zh)
  }
  return bits.join(' ').replace(/\s+/g, ' ').trim() || english
}

export function formatSkillSummaryLine(input: {
  description?: string | null
  profile?: SkillProfile | null
  emptyLabel?: string
}): string {
  const summary = input.profile?.summary?.trim()
  if (summary) return summary
  const desc = input.description?.trim()
  if (desc) return desc
  return input.emptyLabel ?? ''
}

/** Hub card/list second line: Chinese summary only. Never impersonate English official description. */
export function formatHubCardSummary(input: {
  profile?: SkillProfile | null
  pendingLabel: string
}): { text: string; pending: boolean } {
  const summary = input.profile?.summary?.trim()
  if (summary) return { text: summary, pending: false }
  return { text: input.pendingLabel, pending: true }
}

export function buildExternalDescription(input: {
  name: string
  profile?: SkillProfile | null
  fallbackDescription?: string | null
  elements: PresentationElements
  /** Yan card title already uses zh name — omit zh from desc to avoid duplicate. */
  omitZhName?: boolean
  /** Goose list title already uses english name — omit english from desc to avoid duplicate. */
  omitEnglishName?: boolean
}): string | undefined {
  const { elements } = input
  const category = input.profile?.category?.trim()
  const zh = input.profile?.zh_name?.trim()
  const english = input.name.trim()
  const summary = input.profile?.summary?.trim()

  const titleBits: string[] = []
  if (elements.category && category) titleBits.push(`【${category}】`)
  const includeEnglish = Boolean(elements.englishName && english && !input.omitEnglishName)
  if (includeEnglish && english) titleBits.push(english)
  const includeZh = Boolean(elements.zhName && zh && !input.omitZhName)
  if (includeZh && zh) {
    if (includeEnglish && english) {
      titleBits.push('-')
      titleBits.push(zh)
    } else {
      titleBits.push(zh)
    }
  }

  const parts: string[] = []
  const title = titleBits.join(' ').replace(/\s+/g, ' ').trim()
  if (title) parts.push(title)
  if (elements.summary && summary) parts.push(summary)

  const joined = parts.join(' — ').replace(/\s+/g, ' ').trim()
  if (joined) return joined
  const fallback = input.fallbackDescription?.trim()
  return fallback || undefined
}

/** Yan: title shows zh `name` → omit zh in desc. Keep english id in desc. */
export function buildYanExternalDescription(input: {
  name: string
  profile?: SkillProfile | null
  fallbackDescription?: string | null
  elements: PresentationElements
}): string | undefined {
  return buildExternalDescription({ ...input, omitZhName: true, omitEnglishName: false })
}

/** Goose: list title shows english `name` → omit english in desc. Keep zh in desc. */
export function buildGooseExternalDescription(input: {
  name: string
  profile?: SkillProfile | null
  fallbackDescription?: string | null
  elements: PresentationElements
}): string | undefined {
  return buildExternalDescription({ ...input, omitZhName: false, omitEnglishName: true })
}

export function buildExternalMetadata(input: {
  profile?: SkillProfile | null
  elements: PresentationElements
}): Record<string, string> {
  const out: Record<string, string> = {}
  const category = input.profile?.category?.trim()
  const color = normalizeHexColor(input.profile?.color)
  if (input.elements.category && category) out.category = category
  if (input.elements.color && color) out.color = color
  return out
}

/** Pick description builder by tool key. Never mix Yan/Goose omit rules. */
export function buildToolExternalDescription(
  toolKey: string,
  input: {
    name: string
    profile?: SkillProfile | null
    fallbackDescription?: string | null
    elements: PresentationElements
  },
): string | undefined {
  if (toolKey === 'yan_agent') return buildYanExternalDescription(input)
  if (toolKey === 'goose' || toolKey === 'goose_backup') return buildGooseExternalDescription(input)
  return buildExternalDescription(input)
}
