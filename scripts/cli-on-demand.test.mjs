import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
const { it } = process.env.VITEST ? await import('vitest') : await import('node:test')

it('desktop bootstrap never publishes a CLI bridge or keeps startup publication state', () => {
  const bootstrap = readFileSync(new URL('../src-tauri/src/lib.rs', import.meta.url), 'utf8')
  assert.doesNotMatch(bootstrap, /publish_bundled_cli_bridge|publish_cli_bridge|CliBridgeStartupState/)
})
