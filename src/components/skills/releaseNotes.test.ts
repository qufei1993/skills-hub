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

  it('selects English notes for English, Korean, and unsupported locales', () => {
    const expected = ['### Changed', '- English content'].join('\n')
    expect(selectLocalizedReleaseNotes(bilingualNotes, 'en')).toBe(expected)
    expect(selectLocalizedReleaseNotes(bilingualNotes, 'ko-KR')).toBe(expected)
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

  it('removes release-page downloads while preserving localized changes and other tables', () => {
    const notes = [
      '## 中文', '', '### 下载安装', '', '| Linux | 下载链接 |', '',
      '桌面用户只需下载安装包，CLI 会在启用 AI 管理时自动下载。', '',
      '### 修复', '- 中文修复', '',
      '## English', '', '### Downloads', '', '| Linux | download-link |', '',
      'Desktop users only need the installer.', '',
      '### Fixed', '- English fix', '', '### Formats', '| Name | Value |',
    ].join('\n')
    expect(selectLocalizedReleaseNotes(notes, 'zh-CN')).toBe('### 修复\n- 中文修复')
    expect(selectLocalizedReleaseNotes(notes, 'ko')).toBe('### Fixed\n- English fix\n\n### Formats\n| Name | Value |')
  })

  it('removes downloads from legacy and download-only bodies without hiding following headings', () => {
    const notes = 'Introduction\n\n### Downloads\n\ninstaller-link\n\n## Changes\n\n- Keep this fix'
    expect(selectLocalizedReleaseNotes(notes, 'en')).toBe('Introduction\n\n## Changes\n\n- Keep this fix')
    expect(selectLocalizedReleaseNotes('### 下载安装\n\ninstaller-link', 'zh')).toBe('')
  })

  it('preserves download headings inside fenced code and unrelated download documentation', () => {
    const notes = '### Fixed\n\n```md\n### Downloads\nexample\n```\n\n### Download API\nAPI changes'
    expect(selectLocalizedReleaseNotes(notes, 'en')).toBe(notes)
  })
})
