import { describe, expect, it } from 'vitest'
import { resolveGithubClientId } from './build-desktop.mjs'
import { spawnSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'

describe('desktop OAuth build configuration', () => {
  it('exits before building when configuration is missing and reports configured without printing its value', () => {
    const env = { ...process.env }
    delete env.SKILLS_HUB_GITHUB_CLIENT_ID
    const run = value => spawnSync(process.execPath, [fileURLToPath(new URL('./build-desktop.mjs', import.meta.url)), '--check-oauth-only'], { env: value, encoding: 'utf8' })
    expect(run(env).status).toBe(1)
    const configured = run({ ...env, SKILLS_HUB_GITHUB_CLIENT_ID: 'Ov23liPublicTest12345' })
    expect(configured.status).toBe(0)
    expect(configured.stdout).toContain('configured')
    expect(configured.stdout + configured.stderr).not.toContain('Ov23liPublicTest12345')
  })
  it('stops a build without a usable public client ID', () => {
    for (const value of [undefined, '', '  ', 'ghp_this_is_a_user_token', '${SECRET}', 'secret value']) {
      expect(() => resolveGithubClientId({ SKILLS_HUB_GITHUB_CLIENT_ID: value })).toThrow('SKILLS_HUB_GITHUB_CLIENT_ID')
    }
  })
  it('reads only the exact public key without expanding other env fields', () => {
    expect(resolveGithubClientId({}, 'USER_TOKEN=private\nSKILLS_HUB_GITHUB_CLIENT_SECRET=private\nexport SKILLS_HUB_GITHUB_CLIENT_ID="Ov23liPublicTest12345" # public\n')).toBe('Ov23liPublicTest12345')
  })
  it('uses explicit build environment instead of local fallback', () => {
    expect(resolveGithubClientId({ SKILLS_HUB_GITHUB_CLIENT_ID: 'Ov23liFromBuild12345' }, 'SKILLS_HUB_GITHUB_CLIENT_ID=Ov23liFromFile12345')).toBe('Ov23liFromBuild12345')
  })
  it('rejects duplicate or malformed keys without echoing input', () => {
    for (const file of ['SKILLS_HUB_GITHUB_CLIENT_ID=first\nSKILLS_HUB_GITHUB_CLIENT_ID=second', 'SKILLS_HUB_GITHUB_CLIENT_ID=$(echo private-secret)', 'SKILLS_HUB_GITHUB_CLIENT_ID="unterminated-private-secret']) {
      try { resolveGithubClientId({}, file); throw new Error('should fail') } catch (error) {
        expect(error.message).toContain('SKILLS_HUB_GITHUB_CLIENT_ID')
        expect(error.message).not.toContain('private-secret')
      }
    }
  })
})
