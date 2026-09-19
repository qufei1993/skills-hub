import { readFileSync } from 'node:fs'
import { spawnSync } from 'node:child_process'
import { createRequire } from 'node:module'
import { fileURLToPath, pathToFileURL } from 'node:url'
import path from 'node:path'

const oauthClientIdKeys = [
  'SKILLS_HUB_GITHUB_CLIENT_ID',
  'SKILLS_HUB_GITLAB_CLIENT_ID',
]
const invalid = key => new Error(`Missing or invalid ${key}. Set the public OAuth Client ID in the build environment or pass --oauth-env-file <path>. Never use a user token or client secret.`)

function resolveOAuthClientId(key, env, contents) {
  let value = env[key]
  if (value === undefined) {
    const escapedKey = key.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
    const pattern = new RegExp(`^\\s*(?:export\\s+)?${escapedKey}\\s*=\\s*(.*?)\\s*$`)
    const matches = contents.split(/\r?\n/).map(line => line.match(pattern)).filter(Boolean)
    if (matches.length !== 1) throw invalid(key)
    const literal = matches[0][1].match(/^(?:"([A-Za-z0-9]+)"|'([A-Za-z0-9]+)'|([A-Za-z0-9]+))\s*(?:#.*)?$/)
    if (!literal) throw invalid(key)
    value = literal[1] ?? literal[2] ?? literal[3]
  }
  if (typeof value !== 'string' || !/^[A-Za-z0-9]{8,80}$/.test(value)) throw invalid(key)
  return value
}

export function resolveOAuthClientIds(env, contents = '') {
  return Object.fromEntries(oauthClientIdKeys.map(key => [key, resolveOAuthClientId(key, env, contents)]))
}

async function main(args) {
  let contents = ''
  const index = args.indexOf('--oauth-env-file')
  let oauthFilename
  if (index !== -1) {
    oauthFilename = args[index + 1]
    if (!oauthFilename || oauthFilename.startsWith('--') || args.lastIndexOf('--oauth-env-file') !== index) {
      throw new Error('Missing or invalid --oauth-env-file path.')
    }
    args.splice(index, 2)
  }
  if (oauthClientIdKeys.some(key => process.env[key] === undefined)) {
    if (oauthFilename) {
      // This is data, never a shell script or dotenv environment import.
      try { contents = readFileSync(oauthFilename, 'utf8') } catch { throw new Error('Cannot read --oauth-env-file.') }
    } else {
      const filename = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '.env')
      try { contents = readFileSync(filename, 'utf8') } catch { /* Report the existing missing configuration error below. */ }
    }
  }
  const clientIds = resolveOAuthClientIds(process.env, contents)
  const checkOnlyIndex = args.indexOf('--check-oauth-only')
  const checkOnly = checkOnlyIndex !== -1
  if (checkOnly) {
    console.log('GitHub and GitLab OAuth public Client IDs: configured (values not printed).')
    return
  }
  const devIndex = args.indexOf('--dev')
  const command = devIndex === -1 ? 'build' : 'dev'
  const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
  const env = { ...process.env, ...clientIds }
  delete env.SKILLS_HUB_PREPARE_CLI_SIDECAR
  const { desktopSidecarOptions, prepareCliSidecar } = await import('./prepare-cli-sidecar.mjs')
  prepareCliSidecar({ root, env, ...desktopSidecarOptions(args) })
  if (devIndex !== -1) args.splice(devIndex, 1)
  if (command === 'build') {
    const runnerIndex = args.indexOf('--')
    if (runnerIndex === -1) args.push('--', '--bin', 'app')
    else args.splice(runnerIndex + 1, 0, '--bin', 'app')
  }
  const require = createRequire(import.meta.url)
  const cli = path.join(path.dirname(require.resolve('@tauri-apps/cli/package.json')), 'tauri.js')
  const result = spawnSync(process.execPath, [cli, command, ...args], {
    cwd: root,
    env,
    stdio: 'inherit',
  })
  if (result.error) throw new Error(`Unable to start Tauri ${command}.`)
  process.exitCode = result.status ?? 1
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  try { await main(process.argv.slice(2)) } catch (error) {
    console.error(error.message)
    process.exitCode = 1
  }
}
