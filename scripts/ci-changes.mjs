import { execFileSync } from 'node:child_process'
import { appendFileSync } from 'node:fs'
import { pathToFileURL } from 'node:url'

export function needsRustChecks(files) {
  if (files.length === 0) return true
  return files.some(file => !(
    /^(src|public|docs)\//.test(file)
    || /^(README(?:\.[\w-]+)?\.md|CHANGELOG\.md|LICENSE(?:\.md)?|AGENTS\.md)$/.test(file)
    || /^(vite\.config\.ts|vitest\.config\.ts|eslint\.config\.js|tsconfig(?:\.[\w-]+)?\.json|index\.html)$/.test(file)
  ))
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const { CI_EVENT, CI_BASE, CI_HEAD, GITHUB_OUTPUT } = process.env
  let required = true
  if (CI_EVENT !== 'workflow_dispatch' && CI_BASE && !/^0+$/.test(CI_BASE)) {
    if (![CI_BASE, CI_HEAD].every(value => /^[a-f0-9]{40}$/.test(value ?? ''))) {
      throw new Error('Invalid CI comparison commits')
    }
    const range = CI_EVENT === 'pull_request' ? `${CI_BASE}...${CI_HEAD}` : `${CI_BASE}..${CI_HEAD}`
    const files = execFileSync('git', ['diff', '--name-only', '--no-renames', '-z', range], { encoding: 'utf8' }).split('\0').filter(Boolean)
    required = needsRustChecks(files)
  }
  appendFileSync(GITHUB_OUTPUT, `rust=${required}\n`)
}
