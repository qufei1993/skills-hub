import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
const { it } = process.env.VITEST ? await import('vitest') : await import('node:test')

it('desktop bootstrap leaves AI management changes to explicit actions', () => {
  const bootstrap = readFileSync(new URL('../src-tauri/src/lib.rs', import.meta.url), 'utf8')
  assert.doesNotMatch(bootstrap, /refresh_installed_ai_management|publish_bundled_cli_bridge|cli_terminal::configure|CliBridgeStartupState/)
})
