import assert from 'node:assert/strict'
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, existsSync, rmSync, symlinkSync, readdirSync } from 'node:fs'
import { spawnSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
const { it } = process.env.VITEST ? await import('vitest') : await import('node:test')
const script = fileURLToPath(new URL('./install-cli.sh', import.meta.url))

function fixture(run) {
  const root = mkdtempSync(path.join(tmpdir(), 'cli-install-'))
  const home = path.join(root, "user's home")
  const mocks = path.join(root, 'mocks')
  mkdirSync(home); mkdirSync(mocks)
  const binary = '#!/bin/sh\necho fixture-cli\n'
  writeFileSync(path.join(root, 'binary'), binary)
  const hash = createHash('sha256').update(binary).digest('hex')
  writeFileSync(path.join(mocks, 'uname'), '#!/bin/sh\ncase "$1" in -s) echo "$TEST_OS";; -m) echo "$TEST_ARCH";; esac\n', { mode: 0o755 })
  writeFileSync(path.join(mocks, 'curl'), `#!/bin/bash
out=''
for ((i=1;i<=$#;i++)); do
  arg="\${!i}"
  if [[ "$arg" == '-o' ]]; then ((i+=1)); out="\${!i}"; fi
  if [[ "$arg" == https://* ]]; then url="$arg"; fi
done
if [[ "$url" == */releases/latest ]]; then printf '%s' "https://github.com/qufei1993/skills-hub-cli/releases/tag/\${TEST_TAG}"; exit; fi
printf '%s\\n' "$url" >> "$TEST_ROOT/requests"
[[ "\${TEST_FAIL:-}" == download ]] && exit 22
if [[ "$url" == *.sha256 ]]; then
  name="\${url##*/}"; name="\${name%.sha256}"
  [[ "\${TEST_FAIL:-}" == filename ]] && name=wrong-file
  printf '%s  %s\\n' '${hash}' "$name" > "$out"
else
  cp "$TEST_ROOT/binary" "$out"
  [[ "\${TEST_FAIL:-}" == checksum ]] && echo corrupt >> "$out"
fi
exit 0
`, { mode: 0o755 })
  const env = { ...process.env, HOME: home, SHELL: '/bin/zsh', ZDOTDIR: home, PATH: `${mocks}:/usr/bin:/bin`, TMPDIR: root, TEST_ROOT: root, TEST_OS: 'Darwin', TEST_ARCH: 'arm64', TEST_TAG: 'v0.11.0' }
  const install = (overrides = {}) => spawnSync('bash', [script], { env: { ...env, ...overrides }, encoding: 'utf8' })
  try { run({ root, home, install, binary, dest: path.join(home, '.local/bin/skillshub-cli') }) }
  finally { rmSync(root, { recursive: true, force: true }) }
}

it('installs and upgrades atomically with a valid checksum and idempotent shell setup', () => fixture(({ home, install, binary, dest }) => {
  for (let i = 0; i < 2; i++) { const result = install(); assert.equal(result.status, 0, result.stderr) }
  assert.equal(readFileSync(dest, 'utf8'), binary)
  const rc = readFileSync(path.join(home, '.zshrc'), 'utf8')
  assert.equal(rc.split('export PATH=').length, 2)
  const loaded = spawnSync('bash', ['-c', 'source "$1"; command -v skillshub-cli', 'test', path.join(home, '.zshrc')], { encoding: 'utf8' })
  assert.equal(loaded.stdout.trim(), dest)
  assert.equal(existsSync(path.join(home, '.skills-hub')), false)
}))

it('maps all four supported Unix targets to release asset names', () => {
  for (const [os, arch, target] of [['Darwin','arm64','darwin-arm64'], ['Darwin','x86_64','darwin-x64'], ['Linux','aarch64','linux-arm64'], ['Linux','x86_64','linux-x64']]) {
    fixture(({ root, install }) => {
      const result = install({ TEST_OS: os, TEST_ARCH: arch })
      assert.equal(result.status, 0, result.stderr)
      assert.match(readFileSync(path.join(root, 'requests'), 'utf8'), /https:\/\/github.com\/qufei1993\/skills-hub-cli\/releases\/download\//)
      assert.match(readFileSync(path.join(root, 'requests'), 'utf8'), new RegExp(`skillshub-cli-0.11.0-${target}`))
    })
  }
})

it('leaves the existing CLI and shell profile untouched on download or checksum errors', () => {
  for (const failure of ['download', 'checksum', 'filename']) fixture(({ root, home, dest, install }) => {
    mkdirSync(path.dirname(dest), { recursive: true }); writeFileSync(dest, 'old-cli')
    assert.notEqual(install({ TEST_FAIL: failure }).status, 0)
    assert.equal(readFileSync(dest, 'utf8'), 'old-cli')
    assert.equal(existsSync(path.join(home, '.zshrc')), false)
    assert.equal(readdirSync(root).some(name => name.startsWith('skillshub-install.')), false)
  })
})

it('rejects unsupported targets and malformed release tags before installation', () => {
  for (const overrides of [{ TEST_OS: 'FreeBSD' }, { TEST_ARCH: 'riscv64' }, { TEST_TAG: 'v0.11.0/evil' }]) fixture(({ install, dest }) => {
    assert.notEqual(install(overrides).status, 0)
    assert.equal(existsSync(dest), false)
  })
})

it('does not overwrite a symlink to a desktop-managed binary', () => fixture(({ root, dest, install }) => {
  const other = path.join(root, 'desktop-cli'); writeFileSync(other, 'desktop')
  mkdirSync(path.dirname(dest), { recursive: true }); symlinkSync(other, dest)
  assert.notEqual(install().status, 0)
  assert.equal(readFileSync(other, 'utf8'), 'desktop')
}))

it('configures both Bash login and interactive shells without duplicating entries', () => fixture(({ home, dest, install }) => {
  writeFileSync(path.join(home, '.bash_profile'), '# existing profile\n')
  const result = install({ SHELL: '/bin/bash' })
  assert.equal(result.status, 0, result.stderr)
  for (const name of ['.bash_profile', '.bashrc']) {
    const loaded = spawnSync('bash', ['-c', 'source "$1"; command -v skillshub-cli', 'test', path.join(home, name)], { encoding: 'utf8' })
    assert.equal(loaded.stdout.trim(), dest)
  }
}))
