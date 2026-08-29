type ReleaseNotesLanguage = 'en' | 'zh'

type LanguageSection = {
  language: ReleaseNotesLanguage
  lineIndex: number
}

const parseLanguageHeading = (line: string): ReleaseNotesLanguage | null => {
  const match = line.match(/^##[\t ]+(中文|English)[\t ]*$/i)
  if (!match) return null
  return match[1].toLowerCase() === 'english' ? 'en' : 'zh'
}

export const selectLocalizedReleaseNotes = (
  body: string | null,
  language: string,
): string | null => {
  if (!body) return body

  const lines = body.split(/\r?\n/)
  const sections: LanguageSection[] = []
  for (let lineIndex = 0; lineIndex < lines.length; lineIndex += 1) {
    const sectionLanguage = parseLanguageHeading(lines[lineIndex])
    if (sectionLanguage) sections.push({ language: sectionLanguage, lineIndex })
  }

  const hasEnglish = sections.some((section) => section.language === 'en')
  const hasChinese = sections.some((section) => section.language === 'zh')
  if (!hasEnglish || !hasChinese) return body

  const targetLanguage: ReleaseNotesLanguage = language.toLowerCase().startsWith('zh')
    ? 'zh'
    : 'en'
  const targetIndex = sections.findIndex(
    (section) => section.language === targetLanguage,
  )
  const target = sections[targetIndex]
  const end = sections[targetIndex + 1]?.lineIndex ?? lines.length
  const localizedBody = lines.slice(target.lineIndex + 1, end).join('\n').trim()

  return localizedBody || body
}
