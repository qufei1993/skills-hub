import { describe, expect, it } from 'vitest'
import { applyPresetToConfig, emptyProfileDraftStatus } from './profileDraftSettings'

describe('profileDraftSettings', () => {
  it('applies preset base URL and keeps a custom model id', () => {
    const status = emptyProfileDraftStatus()
    const next = applyPresetToConfig(
      { ...status.config, model: 'deepseek-chat' },
      'deepseek',
      status.presets,
    )
    expect(next.provider).toBe('deepseek')
    expect(next.base_url).toBe('https://api.deepseek.com/v1')
    expect(next.model).toBe('deepseek-chat')
    expect(next.auto_on_install).toBe(false)
  })

  it('keeps a custom relay URL when switching to 自定义中转', () => {
    const status = emptyProfileDraftStatus()
    const customUrl = 'https://api.tyas.cc/v1'
    const next = applyPresetToConfig(
      { ...status.config, provider: 'openai', base_url: customUrl, model: 'gpt-4o' },
      'custom',
      status.presets,
    )
    expect(next.provider).toBe('custom')
    expect(next.base_url).toBe(customUrl)
  })
})
