import { describe, expect, it } from 'vitest'
import {
  DEFAULT_PRESENTATION,
  buildExternalDescription,
  buildExternalMetadata,
  buildGooseExternalDescription,
  buildToolExternalDescription,
  buildYanExternalDescription,
  formatHubCardSummary,
  formatSkillTitleLine,
} from './skillProfile'

describe('skillProfile presentation helpers', () => {
  const profile = {
    skill_id: 'nature-polishing',
    zh_name: '自然润色',
    category: '学术写作',
    color: '#22C55E',
    summary: '润色学术英文段落与结构',
    note: '备注不算五元',
    summary_source: 'manual' as const,
  }

  it('formats hub title with category, english name and zh name', () => {
    expect(
      formatSkillTitleLine({
        name: 'nature-polishing',
        profile,
      }),
    ).toBe('【学术写作】 nature-polishing - 自然润色')
  })

  it('builds full external description with english name when enabled', () => {
    expect(
      buildExternalDescription({
        name: 'nature-polishing',
        profile,
        fallbackDescription: 'fallback',
        elements: {
          ...DEFAULT_PRESENTATION,
          englishName: true,
          color: true,
        },
      }),
    ).toBe('【学术写作】 nature-polishing - 自然润色 — 润色学术英文段落与结构')
  })

  it('Yan omits zh name to avoid title/desc duplication', () => {
    expect(
      buildYanExternalDescription({
        name: 'nature-polishing',
        profile,
        elements: DEFAULT_PRESENTATION,
      }),
    ).toBe('【学术写作】 nature-polishing — 润色学术英文段落与结构')
  })

  it('Goose omits english name to avoid title/desc duplication', () => {
    expect(
      buildGooseExternalDescription({
        name: 'nature-polishing',
        profile,
        elements: DEFAULT_PRESENTATION,
      }),
    ).toBe('【学术写作】 自然润色 — 润色学术英文段落与结构')
  })

  it('routes builders by tool key without mixing Yan/Goose rules', () => {
    expect(
      buildToolExternalDescription('yan_agent', {
        name: 'nature-polishing',
        profile,
        elements: DEFAULT_PRESENTATION,
      }),
    ).toBe('【学术写作】 nature-polishing — 润色学术英文段落与结构')
    expect(
      buildToolExternalDescription('goose', {
        name: 'nature-polishing',
        profile,
        elements: DEFAULT_PRESENTATION,
      }),
    ).toBe('【学术写作】 自然润色 — 润色学术英文段落与结构')
    expect(
      buildToolExternalDescription('goose_backup', {
        name: 'nature-polishing',
        profile,
        elements: DEFAULT_PRESENTATION,
      }),
    ).toBe('【学术写作】 自然润色 — 润色学术英文段落与结构')
  })

  it('Hub card summary never impersonates English official description', () => {
    expect(
      formatHubCardSummary({
        profile,
        pendingLabel: '待写中文简介',
      }),
    ).toEqual({ text: '润色学术英文段落与结构', pending: false })
    expect(
      formatHubCardSummary({
        profile: { ...profile, summary: '' },
        pendingLabel: '待写中文简介',
      }),
    ).toEqual({ text: '待写中文简介', pending: true })
  })

  it('builds metadata only for selected elements', () => {
    expect(
      buildExternalMetadata({
        profile,
        elements: {
          color: true,
          category: true,
          englishName: true,
          zhName: false,
          summary: false,
        },
      }),
    ).toEqual({
      category: '学术写作',
      color: '#22C55E',
    })
  })
})
