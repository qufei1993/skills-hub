// @vitest-environment jsdom

import { beforeEach, describe, expect, it, vi } from 'vitest'

describe('i18n initialization', () => {
  beforeEach(() => {
    window.localStorage.clear()
    vi.resetModules()
  })

  it('restores Korean from the saved interface language', async () => {
    window.localStorage.setItem('skills-language', 'ko')

    const { default: i18n } = await import('./index')

    expect(i18n.resolvedLanguage).toBe('ko')
  })

  it('ignores unsupported saved languages', async () => {
    window.localStorage.setItem('skills-language', 'fr')

    const { default: i18n } = await import('./index')

    expect(i18n.resolvedLanguage).toBe('en')
  })
})
