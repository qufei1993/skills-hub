#!/usr/bin/env node

/**
 * Aggregates AI Agent Skills from a curated list of high-quality GitHub repositories.
 *
 * Fetches metadata and directory trees for each curated repo, detects individual skills
 * within each repo, and outputs featured-skills.json with one entry per skill.
 *
 * API budget: ~14 requests total (7 Repos API + 7 Trees API).
 *
 * Requires: GITHUB_TOKEN environment variable.
 */

import { existsSync, readFileSync, writeFileSync } from 'node:fs'
import { resolve } from 'node:path'

// Load .env file
const envPath = resolve(import.meta.dirname, '..', '.env')
if (existsSync(envPath)) {
  for (const line of readFileSync(envPath, 'utf-8').split('\n')) {
    const trimmed = line.trim()
    if (!trimmed || trimmed.startsWith('#')) continue
    const idx = trimmed.indexOf('=')
    if (idx === -1) continue
    const key = trimmed.slice(0, idx).trim()
    const value = trimmed.slice(idx + 1).trim()
    if (!process.env[key]) process.env[key] = value
  }
}

const OUTPUT_FILE = 'featured-skills.json'

// Curated high-quality skill repositories (ordered by stars desc)
const CURATED_REPOS = [
  'anthropics/skills',
  'sickn33/antigravity-awesome-skills',
  'K-Dense-AI/claude-scientific-skills',
  'travisvn/awesome-claude-skills',
  'VoltAgent/awesome-agent-skills',
  'anthropics/knowledge-work-plugins',
  'alirezarezvani/claude-skills',
]

const MAX_SKILLS = 300
const CONCURRENCY = 10
const MAX_RATE_LIMIT_WAIT_SECS = 60

// Skill scan bases matching installer.rs SKILL_SCAN_BASES
const SKILL_SCAN_BASES = [
  'skills',
  'skills/.curated',
  'skills/.experimental',
  'skills/.system',
  '.claude/skills',
]

// Directories to skip when scanning root-level subdirs
const ROOT_SKIP_DIRS = new Set([
  'skills', '.claude', '.git', '.github', '.vscode', 'node_modules',
  '.idea', '.DS_Store', 'dist', 'build', 'out', 'target',
  'docs', 'test', 'tests', '__tests__', 'examples', 'src', 'lib',
])

// Category classification rules
const CATEGORY_RULES = [
  { keywords: ['browser', 'automation', 'playwright', 'puppeteer'], category: 'browser-automation' },
  { keywords: ['security', 'audit', 'vulnerability', 'pentest'], category: 'security' },
  { keywords: ['devops', 'deploy', 'infra', 'docker', 'kubernetes'], category: 'devops' },
  { keywords: ['marketing', 'seo', 'ads', 'advertising'], category: 'marketing' },
  { keywords: ['database', 'sql', 'postgres', 'mongo'], category: 'database' },
  { keywords: ['git', 'github', 'pr', 'code-review'], category: 'development' },
  { keywords: ['ai', 'llm', 'agent', 'model'], category: 'ai-assistant' },
]

const GITHUB_TOKEN = process.env.GITHUB_TOKEN || ''

// ─── HTTP helpers ───

async function fetchJson(url, retries = 3) {
  const headers = {
    Accept: 'application/vnd.github+json',
    'User-Agent': 'SkillSyncer-aggregator',
  }
  if (GITHUB_TOKEN) {
    headers.Authorization = `Bearer ${GITHUB_TOKEN}`
  }

  for (let attempt = 0; attempt <= retries; attempt++) {
    const res = await fetch(url, { headers })

    if (res.status === 403 || res.status === 429) {
      const resetHeader = res.headers.get('x-ratelimit-reset')
      let waitSecs = resetHeader
        ? Math.max(Number(resetHeader) - Math.floor(Date.now() / 1000), 1)
        : Math.pow(2, attempt + 1)

      if (waitSecs > MAX_RATE_LIMIT_WAIT_SECS) {
        console.warn(`Rate limited, reset in ${waitSecs}s (exceeds max ${MAX_RATE_LIMIT_WAIT_SECS}s) — skipping`)
        return null
      }
      console.warn(`Rate limited (${res.status}), waiting ${waitSecs}s (attempt ${attempt + 1}/${retries + 1})...`)
      await sleep(waitSecs * 1000)
      continue
    }

    if (!res.ok) {
      if (attempt < retries) {
        await sleep(Math.pow(2, attempt) * 1000)
        continue
      }
      return null
    }

    return res.json()
  }
  return null
}

function sleep(ms) {
  return new Promise((r) => setTimeout(r, ms))
}

// Run async tasks with bounded concurrency
async function pMap(items, fn, concurrency) {
  const results = new Array(items.length)
  let idx = 0

  async function worker() {
    while (idx < items.length) {
      const i = idx++
      results[i] = await fn(items[i], i)
    }
  }

  const workers = Array.from({ length: Math.min(concurrency, items.length) }, () => worker())
  await Promise.all(workers)
  return results
}

// ─── Step 1: Fetch curated repo metadata ───

async function fetchRepoMetadata(fullName) {
  const url = `https://api.github.com/repos/${fullName}`
  const data = await fetchJson(url)
  if (!data || !data.full_name) {
    console.warn(`  Skipping ${fullName}: unable to fetch metadata`)
    return null
  }
  return data
}

async function fetchAllRepoMetadata() {
  console.log(`Fetching metadata for ${CURATED_REPOS.length} curated repos...`)
  const results = await pMap(
    CURATED_REPOS,
    async (fullName) => {
      console.log(`  Fetching: ${fullName}`)
      return fetchRepoMetadata(fullName)
    },
    CONCURRENCY,
  )
  const repos = results.filter(Boolean)
  console.log(`Successfully fetched ${repos.length}/${CURATED_REPOS.length} repos`)
  return repos
}

// ─── Step 2: Detect skills in each repo ───

function detectSkillsFromTree(treeItems) {
  const filePaths = new Set()
  const dirPaths = new Set()
  for (const item of treeItems) {
    if (item.type === 'blob') filePaths.add(item.path)
    else if (item.type === 'tree') dirPaths.add(item.path)
  }

  const skills = [] // { dirPath }
  const foundDirs = new Set()

  // Scan SKILL_SCAN_BASES
  for (const base of SKILL_SCAN_BASES) {
    for (const dir of dirPaths) {
      if (!dir.startsWith(base + '/')) continue
      const rest = dir.slice(base.length + 1)
      if (rest.includes('/')) continue // not a direct child

      const hasSkillMd = filePaths.has(dir + '/SKILL.md')
      const isClaudeSkill = base === '.claude/skills'

      if (hasSkillMd || isClaudeSkill) {
        if (!foundDirs.has(dir)) {
          foundDirs.add(dir)
          skills.push({ dirPath: dir })
        }
      }
    }
  }

  // Scan root-level subdirectories (must have SKILL.md or .claude-plugin/plugin.json)
  for (const dir of dirPaths) {
    if (dir.includes('/')) continue
    if (ROOT_SKIP_DIRS.has(dir) || dir.startsWith('.')) continue
    if (foundDirs.has(dir)) continue

    if (filePaths.has(dir + '/SKILL.md') || filePaths.has(dir + '/.claude-plugin/plugin.json')) {
      foundDirs.add(dir)
      skills.push({ dirPath: dir })
    }
  }

  // Single-skill repo: root has SKILL.md and no sub-skills found
  if (skills.length === 0 && filePaths.has('SKILL.md')) {
    skills.push({ dirPath: null })
  }

  return skills
}

async function getRepoTree(owner, repo, branch) {
  const url = `https://api.github.com/repos/${owner}/${repo}/git/trees/${encodeURIComponent(branch)}?recursive=1`
  const data = await fetchJson(url, 2)
  if (!data || !data.tree) return null
  if (data.truncated) {
    console.warn(`  Warning: tree for ${owner}/${repo} was truncated, some skills may be missed`)
  }
  return data.tree
}

// ─── Step 3: Classify ───

function classify(topics, description) {
  const text = [...(topics || []), description || ''].join(' ').toLowerCase()
  for (const rule of CATEGORY_RULES) {
    if (rule.keywords.some((kw) => text.includes(kw))) {
      return rule.category
    }
  }
  return 'general'
}

// ─── SKILL.md helpers ───

function parseSkillMdFrontmatter(content) {
  const lines = content.split('\n')
  if (lines[0].trim() !== '---') return null
  let name = null
  let description = null
  for (let i = 1; i < lines.length; i++) {
    const l = lines[i].trim()
    if (l === '---') break
    if (l.startsWith('name:')) {
      name = l.slice(5).trim().replace(/^["']|["']$/g, '')
    } else if (l.startsWith('description:')) {
      description = l.slice(12).trim().replace(/^["']|["']$/g, '')
    }
  }
  return { name, description }
}

async function fetchSkillMdContent(owner, repo, branch, dirPath) {
  const filePath = dirPath ? `${dirPath}/SKILL.md` : 'SKILL.md'
  const url = `https://api.github.com/repos/${owner}/${repo}/contents/${encodeURIComponent(filePath)}?ref=${encodeURIComponent(branch)}`
  const data = await fetchJson(url, 1)
  if (!data || !data.content) return null
  const content = Buffer.from(data.content, 'base64').toString('utf-8')
  return parseSkillMdFrontmatter(content)
}

// ─── Name helpers ───

function kebabToTitle(name) {
  return name
    .split(/[-_]/)
    .map((w) => w.charAt(0).toUpperCase() + w.slice(1))
    .join(' ')
}

function slugFromDirPath(dirPath, repoName) {
  if (!dirPath) return repoName
  const parts = dirPath.split('/')
  return parts[parts.length - 1]
}

// ─── Main ───

async function main() {
  if (!GITHUB_TOKEN) {
    console.error('Error: GITHUB_TOKEN environment variable is required.')
    console.error('Set it via: export GITHUB_TOKEN=ghp_xxx')
    process.exit(1)
  }

  // Step 1: Fetch curated repo metadata
  const repos = await fetchAllRepoMetadata()

  // Step 2: Detect skills in each repo via Trees API
  console.log('Scanning repo trees for skills...')
  const skillEntries = [] // { repo, dirPath }
  let treeFailures = 0

  await pMap(
    repos,
    async (repo) => {
      const [owner, repoName] = repo.full_name.split('/')
      const tree = await getRepoTree(owner, repoName, repo.default_branch)
      if (!tree) {
        treeFailures++
        return
      }

      const detected = detectSkillsFromTree(tree)
      if (detected.length === 0) {
        // No detectable skill structure — treat whole repo as single skill
        skillEntries.push({ repo, dirPath: null })
        return
      }

      for (const s of detected) {
        skillEntries.push({ repo, dirPath: s.dirPath })
      }
    },
    CONCURRENCY,
  )

  console.log(`Detected ${skillEntries.length} skills across ${repos.length} repos (${treeFailures} tree fetch failures)`)

  // Fallback: if no skills detected, keep existing local file
  if (skillEntries.length === 0) {
    if (existsSync(OUTPUT_FILE)) {
      console.warn('No skills fetched from GitHub — keeping existing local featured-skills.json')
    } else {
      console.error('No skills fetched and no local fallback exists.')
      process.exit(1)
    }
    return
  }

  // Step 3: Sort by stars desc, deduplicate, take top MAX_SKILLS
  skillEntries.sort((a, b) => b.repo.stargazers_count - a.repo.stargazers_count)

  const seenSlugs = new Set()
  const dedupedEntries = skillEntries.filter((entry) => {
    const slug = slugFromDirPath(entry.dirPath, entry.repo.full_name.split('/')[1])
    if (seenSlugs.has(slug)) return false
    seenSlugs.add(slug)
    return true
  })
  console.log(`After dedup: ${dedupedEntries.length} unique skills (removed ${skillEntries.length - dedupedEntries.length} duplicates)`)

  const topEntries = dedupedEntries.slice(0, MAX_SKILLS)

  // Step 4: Fetch SKILL.md for top skills to get real names
  console.log(`Fetching SKILL.md for ${topEntries.length} skills...`)
  const skillMdMap = new Map() // index -> { name, description }
  await pMap(
    topEntries,
    async (entry, i) => {
      const [owner, repoName] = entry.repo.full_name.split('/')
      const md = await fetchSkillMdContent(owner, repoName, entry.repo.default_branch, entry.dirPath)
      if (md) skillMdMap.set(i, md)
    },
    CONCURRENCY,
  )
  console.log(`Fetched SKILL.md for ${skillMdMap.size}/${topEntries.length} skills`)

  // Filter out entries without SKILL.md
  const validEntries = topEntries.filter((_, i) => skillMdMap.has(i))
  console.log(`${validEntries.length} skills have SKILL.md (filtered out ${topEntries.length - validEntries.length})`)

  // Rebuild index mapping after filtering
  const validMdList = validEntries.map((entry, _i) => {
    const origIndex = topEntries.indexOf(entry)
    return { entry, md: skillMdMap.get(origIndex) }
  })

  // Step 5: Build output
  const categorySet = new Set()
  const topSkills = validMdList.map(({ entry, md }) => {
    const { repo, dirPath } = entry
    const repoName = repo.full_name.split('/')[1]
    const slug = slugFromDirPath(dirPath, repoName)
    const name = md.name || slug
    const summary = (md && md.description) || repo.description || ''
    const category = classify(repo.topics, repo.description)
    categorySet.add(category)

    let sourceUrl
    if (dirPath) {
      sourceUrl = `${repo.html_url}/tree/${repo.default_branch}/${dirPath}`
    } else {
      sourceUrl = repo.html_url
    }

    return {
      slug,
      name,
      summary,
      downloads: 0,
      stars: repo.stargazers_count,
      category,
      tags: (repo.topics || []).slice(0, 5),
      source_url: sourceUrl,
      updated_at: repo.updated_at,
    }
  })

  // Re-sort after name update (stars desc, then name asc)
  topSkills.sort((a, b) => b.stars - a.stars || a.name.localeCompare(b.name))

  const categories = Array.from(categorySet).sort()

  const output = {
    updated_at: new Date().toISOString(),
    total: topSkills.length,
    categories,
    skills: topSkills,
  }

  writeFileSync(OUTPUT_FILE, JSON.stringify(output, null, 2) + '\n')
  console.log(`Wrote ${topSkills.length} skills (of ${skillEntries.length} detected) to ${OUTPUT_FILE}`)
}

main().catch((err) => {
  console.error('Fatal error:', err)
  process.exit(1)
})
