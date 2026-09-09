export type ProfileDraftPreset = {
  id: string
  label: string
  base_url: string
}

export type ProfileDraftConfig = {
  provider: string
  base_url: string
  model: string
  auto_on_install: boolean
}

export type ProfileDraftStatus = {
  configured: boolean
  has_key: boolean
  config: ProfileDraftConfig
  presets: ProfileDraftPreset[]
}

export type ProfileDraftModelItem = {
  id: string
}

export const emptyProfileDraftStatus = (): ProfileDraftStatus => ({
  configured: false,
  has_key: false,
  config: {
    provider: 'custom',
    base_url: '',
    model: '',
    auto_on_install: false,
  },
  presets: [
    { id: 'deepseek', label: 'DeepSeek', base_url: 'https://api.deepseek.com/v1' },
    { id: 'siliconflow', label: '硅基流动', base_url: 'https://api.siliconflow.cn/v1' },
    { id: 'openai', label: 'OpenAI', base_url: 'https://api.openai.com/v1' },
    { id: 'grok', label: 'Grok (xAI)', base_url: 'https://api.x.ai/v1' },
    { id: 'custom', label: '自定义中转', base_url: '' },
  ],
})

export const applyPresetToConfig = (
  config: ProfileDraftConfig,
  presetId: string,
  presets: ProfileDraftPreset[],
): ProfileDraftConfig => {
  const preset = presets.find((item) => item.id === presetId)
  if (!preset) return { ...config, provider: presetId }
  const previous = presets.find((item) => item.id === config.provider)
  const currentUrl = config.base_url.trim()
  const keepCurrent =
    preset.id === 'custom' ||
    (currentUrl.length > 0 && (!previous || currentUrl !== previous.base_url))
  return {
    ...config,
    provider: preset.id,
    base_url: keepCurrent ? currentUrl : preset.base_url,
  }
}
