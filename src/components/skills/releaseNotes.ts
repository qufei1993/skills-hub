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

const stripDownloadSections = (body: string): string => {
  const kept: string[] = []
  let skipping = false
  let changed = false
  let fence: { marker: string; length: number } | null = null
  for (const line of body.split(/\r?\n/)) {
    const delimiter = line.match(/^ {0,3}(`{3,}|~{3,})(.*)$/)
    if (fence) {
      if (delimiter && delimiter[1][0] === fence.marker && delimiter[1].length >= fence.length && !delimiter[2].trim()) fence = null
      if (!skipping) kept.push(line)
      continue
    }
    if (delimiter) {
      fence = { marker: delimiter[1][0], length: delimiter[1].length }
      if (!skipping) kept.push(line)
      continue
    }
    if (/^###[\t ]+(Downloads|下载安装)[\t ]*$/i.test(line)) {
      skipping = true
      changed = true
      continue
    }
    if (/^#{1,3}[\t ]+/.test(line)) skipping = false
    if (!skipping) kept.push(line)
  }
  return changed ? kept.join('\n').trim() : body
}

export const selectLocalizedReleaseNotes = (
  body: string | null,
  language: string,
): string | null => {
  if (!body) return body

  body = stripDownloadSections(body)

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
