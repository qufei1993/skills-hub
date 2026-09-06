import { describe, expect, it } from 'vitest'
import { resources } from './resources'

const flattenKeys = (value: unknown, prefix = ''): string[] => {
  if (!value || typeof value !== 'object') return [prefix]

  return Object.entries(value).flatMap(([key, child]) =>
    flattenKeys(child, prefix ? `${prefix}.${key}` : key),
  )
}

const flattenMessages = (
  value: unknown,
  prefix = '',
): Record<string, string> => {
  if (typeof value === 'string') return { [prefix]: value }
  if (!value || typeof value !== 'object') return {}

  return Object.fromEntries(
    Object.entries(value).flatMap(([key, child]) =>
      Object.entries(
        flattenMessages(child, prefix ? `${prefix}.${key}` : key),
      ),
    ),
  )
}

const interpolationTokens = (message: string) =>
  [...(message.match(/\{\{[^}]+\}\}|<[^>]+>/g) ?? [])].sort()

const logicalKeys = (value: unknown) =>
  [
    ...new Set(
      flattenKeys(value).map((key) => key.replace(/_(one|other)$/, '')),
    ),
  ].sort()

describe('translation resources', () => {
  it('provides Korean translations for every interface message', () => {
    const englishKeys = flattenKeys(resources.en.translation).sort()
    const koreanKeys = flattenKeys(resources.ko.translation).sort()

    expect(koreanKeys).toEqual(englishKeys)
    expect(resources.ko.translation.languageOptions.ko).toBe('한국어')
  })

  it('keeps the same logical messages in every supported language', () => {
    const englishKeys = logicalKeys(resources.en.translation)

    expect(logicalKeys(resources.zh.translation)).toEqual(englishKeys)
    expect(logicalKeys(resources.ko.translation)).toEqual(englishKeys)
  })

  it('preserves interpolation variables and markup in Korean messages', () => {
    const englishMessages = flattenMessages(resources.en.translation)
    const koreanMessages = flattenMessages(resources.ko.translation)

    for (const [key, englishMessage] of Object.entries(englishMessages)) {
      expect(interpolationTokens(koreanMessages[key]), key).toEqual(
        interpolationTokens(englishMessage),
      )
    }
  })
})
