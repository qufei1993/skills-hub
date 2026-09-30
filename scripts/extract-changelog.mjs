import fs from 'node:fs'
import { prependDownloads } from './release-downloads.mjs'

function normalizeVersion(input) {
  const v = String(input || '').trim()
  if (!v) return null
  return v.startsWith('v') ? v.slice(1) : v
}

function extractSection(changelogText, version) {
  const lines = changelogText.split(/\r?\n/)
  const headerRe = new RegExp(`^##\\s+\\[${escapeRegExp(version)}\\](\\s*-\\s*.*)?$`)
  const altHeaderRe = new RegExp(`^##\\s+${escapeRegExp(version)}(\\s*-\\s*.*)?$`)
  const headerVRe = new RegExp(`^##\\s+\\[v${escapeRegExp(version)}\\](\\s*-\\s*.*)?$`)
  const altHeaderVRe = new RegExp(`^##\\s+v${escapeRegExp(version)}(\\s*-\\s*.*)?$`)

  let start = -1
  for (let i = 0; i < lines.length; i += 1) {
    const line = lines[i].trimEnd()
    if (headerRe.test(line) || altHeaderRe.test(line) || headerVRe.test(line) || altHeaderVRe.test(line)) {
      start = i + 1
      break
    }
  }
  if (start === -1) return null

  let end = lines.length
  for (let i = start; i < lines.length; i += 1) {
    if (/^##\s+/.test(lines[i])) {
      end = i
      break
    }
  }

  const body = lines.slice(start, end).join('\n').trim()
  return body || null
}

function listVersions(changelogText) {
  const lines = changelogText.split(/\r?\n/)
  const versions = []
  for (const rawLine of lines) {
    const line = rawLine.trimEnd()
    const m =
      line.match(/^##\s+\[(?<v>v?\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?)\](?:\s*-.*)?$/) ||
      line.match(/^##\s+(?<v>v?\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?)(?:\s*-.*)?$/)
    if (m?.groups?.v) versions.push(m.groups.v)
  }
  return [...new Set(versions)]
}

function escapeRegExp(str) {
  return str.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
}

const [tagOrVersion, changelogPathArg] = process.argv.slice(2)
const version = normalizeVersion(tagOrVersion)
if (!version) {
  console.error('Usage: node scripts/extract-changelog.mjs <tag-or-version> [CHANGELOG.md]')
  process.exit(2)
}

const changelogPath = changelogPathArg || 'CHANGELOG.md'
if (!fs.existsSync(changelogPath)) {
  console.error(`CHANGELOG not found: ${changelogPath}`)
  process.exit(2)
}

const changelogText = fs.readFileSync(changelogPath, 'utf8')
const section = extractSection(changelogText, version)
if (!section) {
  console.error(`Version section not found in ${changelogPath}: ${version}`)
  const available = listVersions(changelogText)
  if (available.length) {
    console.error(`Available versions found: ${available.join(', ')}`)
  } else {
    console.error('No version headers detected. Expected headers like: "## [0.1.1] - 2026-01-24"')
  }
  process.exit(3)
}

const assetsIndex = process.argv.indexOf('--assets')
const languageIndex = process.argv.indexOf('--language')
const output = assetsIndex < 0 ? section : prependDownloads(section, {
  tag: `v${version}`,
  assets: fs.readdirSync(process.argv[assetsIndex + 1]),
  language: languageIndex < 0 ? 'en' : process.argv[languageIndex + 1],
})
process.stdout.write(output + '\n')
