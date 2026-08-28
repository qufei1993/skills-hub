import { describe, expect, it } from 'vitest'
import { supportsRegExpLookbehind } from './markdownCompatibility'

describe('supportsRegExpLookbehind', () => {
  it('returns true when lookbehind assertions are supported', () => {
    expect(supportsRegExpLookbehind((pattern) => new RegExp(pattern))).toBe(true)
  })

  it('returns false when the WebView rejects lookbehind assertions', () => {
    expect(
      supportsRegExpLookbehind(() => {
        throw new SyntaxError('invalid group specifier name')
      }),
    ).toBe(false)
  })
})
