import { describe, expect, it } from 'vitest'
import { selectLocalizedReleaseNotes } from './releaseNotes'

const bilingualNotes = [
  '## 中文',
  '',
  '### 变更',
  '- 中文内容',
  '',
  '## English',
  '',
  '### Changed',
  '- English content',
].join('\n')

describe('selectLocalizedReleaseNotes', () => {
  it('selects Chinese notes for Chinese locales', () => {
    expect(selectLocalizedReleaseNotes(bilingualNotes, 'zh-CN')).toBe(
      ['### 变更', '- 中文内容'].join('\n'),
    )
  })

  it('selects English notes for English and unsupported locales', () => {
    const expected = ['### Changed', '- English content'].join('\n')
    expect(selectLocalizedReleaseNotes(bilingualNotes, 'en')).toBe(expected)
    expect(selectLocalizedReleaseNotes(bilingualNotes, 'fr')).toBe(expected)
  })

  it('supports either language section order', () => {
    const reversed = [
      '## English',
      '',
      'English first',
      '',
      '## 中文',
      '',
      '中文第二',
    ].join('\n')

    expect(selectLocalizedReleaseNotes(reversed, 'zh')).toBe('中文第二')
    expect(selectLocalizedReleaseNotes(reversed, 'en-US')).toBe('English first')
  })

  it('keeps legacy or incomplete release notes unchanged', () => {
    const legacy = '### Fixed\n- Legacy release notes'
    const incomplete = '## English\n\nEnglish only'

    expect(selectLocalizedReleaseNotes(legacy, 'zh')).toBe(legacy)
    expect(selectLocalizedReleaseNotes(incomplete, 'en')).toBe(incomplete)
  })

  it('falls back to the full body when the selected section is empty', () => {
    const emptyChinese = '## 中文\n\n## English\n\nEnglish content'

    expect(selectLocalizedReleaseNotes(emptyChinese, 'zh')).toBe(emptyChinese)
  })
})
