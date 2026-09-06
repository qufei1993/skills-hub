import { readFileSync } from 'node:fs'
import { spawnSync } from 'node:child_process'
import { createRequire } from 'node:module'
import { fileURLToPath, pathToFileURL } from 'node:url'
import path from 'node:path'

const key = 'SKILLS_HUB_GITHUB_CLIENT_ID'
const invalid = () => new Error(`Missing or invalid ${key}. Set the public OAuth Client ID in the build environment or pass --oauth-env-file <path>. Never use a user token or client secret.`)

export function resolveGithubClientId(env, contents = '') {
  let value = env[key]
  if (value === undefined) {
    const matches = contents.split(/\r?\n/).map(line => line.match(/^\s*(?:export\s+)?SKILLS_HUB_GITHUB_CLIENT_ID\s*=\s*(.*?)\s*$/)).filter(Boolean)
    if (matches.length !== 1) throw invalid()
    const literal = matches[0][1].match(/^(?:"([A-Za-z0-9]+)"|'([A-Za-z0-9]+)'|([A-Za-z0-9]+))\s*(?:#.*)?$/)
    if (!literal) throw invalid()
    value = literal[1] ?? literal[2] ?? literal[3]
  }
  if (typeof value !== 'string' || !/^[A-Za-z0-9]{8,80}$/.test(value)) throw invalid()
  return value
}

function main(args) {
  let contents = ''
  const index = args.indexOf('--oauth-env-file')
  if (index !== -1) {
    const filename = args[index + 1]
    if (!filename || filename.startsWith('--') || args.lastIndexOf('--oauth-env-file') !== index) throw invalid()
    // This is data, never a shell script or dotenv environment import.
    try { contents = readFileSync(filename, 'utf8') } catch { throw new Error('Cannot read --oauth-env-file.') }
    args.splice(index, 2)
  }
  const clientId = resolveGithubClientId(process.env, contents)
  const checkOnly = args.includes('--check-oauth-only')
  if (checkOnly) {
    console.log('GitHub OAuth public Client ID: configured (value not printed).')
    return
  }
  const require = createRequire(import.meta.url)
  const cli = path.join(path.dirname(require.resolve('@tauri-apps/cli/package.json')), 'tauri.js')
  const result = spawnSync(process.execPath, [cli, 'build', ...args], {
    cwd: path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..'),
    env: { ...process.env, [key]: clientId },
    stdio: 'inherit',
  })
  if (result.error) throw new Error('Unable to start the desktop build.')
  process.exitCode = result.status ?? 1
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  try { main(process.argv.slice(2)) } catch (error) {
    console.error(error.message)
    process.exitCode = 1
  }
}
