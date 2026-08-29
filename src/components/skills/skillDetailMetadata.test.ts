import { describe, expect, it } from 'vitest'
import { getVisibleFrontmatterEntries } from './skillDetailMetadata'

describe('getVisibleFrontmatterEntries', () => {
  it('hides metadata already shown in the detail header', () => {
    expect(
      getVisibleFrontmatterEntries({
        name: 'wechat-article',
        DESCRIPTION: 'Write an article',
        license: 'MIT',
        compatibility: 'Requires network access',
      }),
    ).toEqual([
      ['license', 'MIT'],
      ['compatibility', 'Requires network access'],
    ])
  })

  it('returns no entries when frontmatter only contains name and description', () => {
    expect(
      getVisibleFrontmatterEntries({
        name: 'wechat-article',
        description: 'Write an article',
      }),
    ).toEqual([])
  })
})
