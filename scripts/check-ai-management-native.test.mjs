import assert from 'node:assert/strict'
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { spawnSync } from 'node:child_process'
const { it } = process.env.VITEST ? await import('vitest') : await import('node:test')

for (const failAt of ['', 'cli_bridge', 'cli_terminal', 'services::tests::agent_access']) {
  it(`runs all native recovery suites and stops on failure at ${failAt || 'none'}`, () => {
    const directory = mkdtempSync(path.join(tmpdir(), 'native-recovery-'))
    try {
      const bin = path.join(directory, 'bin')
      mkdirSync(bin)
      writeFileSync(path.join(bin, 'cargo'), '#!/usr/bin/env bash\nprintf "%s\\n" "$*" >> "$NATIVE_TEST_LOG"\nif [[ "$*" == *"$NATIVE_FAIL_AT"* && -n "$NATIVE_FAIL_AT" ]]; then exit 17; fi\n', { mode: 0o755 })
      const log = path.join(directory, 'calls')
      const result = spawnSync('bash', ['-c', 'bin=$(cd "$1" && pwd); export PATH="$bin:$PATH"; bash scripts/check-ai-management-native.sh', 'test', bin], {
        encoding: 'utf8', env: { ...process.env, NATIVE_TEST_LOG: log, NATIVE_FAIL_AT: failAt },
      })
      assert.equal(result.status, failAt ? 17 : 0, result.stderr)
      const calls = readFileSync(log, 'utf8').trim().split('\n')
      const suites = ['cli_bridge', 'cli_terminal', 'services::tests::agent_access']
      const count = failAt ? suites.indexOf(failAt) + 1 : 3
      assert.deepEqual(calls, suites.slice(0, count).map(suite => `test --locked --manifest-path src-tauri/Cargo.toml --lib --features cli ${suite}`))
    } finally { rmSync(directory, { recursive: true, force: true }) }
  })
}
