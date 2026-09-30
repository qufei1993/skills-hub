import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
const { it } = process.env.VITEST ? await import('vitest') : await import('node:test')

it('desktop bootstrap updates only an existing AI management installation', () => {
  const bootstrap = readFileSync(new URL('../src-tauri/src/lib.rs', import.meta.url), 'utf8')
  assert.match(bootstrap, /if !status\.auto_update_eligible\(\) \{\s*return Ok\(\(\)\);\s*\}/)
  assert.match(bootstrap, /refresh_installed_ai_management/)
  assert.doesNotMatch(bootstrap, /cli_terminal::configure|CliBridgeStartupState/)
})
