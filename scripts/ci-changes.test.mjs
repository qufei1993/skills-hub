import assert from 'node:assert/strict'
import { mkdtempSync, readFileSync, writeFileSync, mkdirSync, rmSync } from 'node:fs'
import { execFileSync } from 'node:child_process'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { createRequire } from 'node:module'
import { needsRustChecks } from './ci-changes.mjs'

const { it } = process.env.VITEST ? await import('vitest') : await import('node:test')
const require = createRequire(import.meta.url)
const { load } = require('js-yaml')
const ci = load(readFileSync(new URL('../.github/workflows/ci.yml', import.meta.url), 'utf8'))

it('skips Rust for frontend and documentation changes only', () => {
  assert.equal(needsRustChecks(['src/App.tsx', 'src/App.css', 'docs/README.zh.md', 'CHANGELOG.md']), false)
  assert.equal(needsRustChecks(['public/icon.svg', 'README.md', 'vite.config.ts']), false)
})

it('requires Rust for native code, bundled skills, dependencies and unknown inputs', () => {
  for (const file of ['src-tauri/src/lib.rs', 'skills/manage-skills-hub/SKILL.md', 'scripts/prepare-cli-sidecar.mjs', 'package-lock.json', '.github/workflows/ci.yml', '.cargo/config.toml', 'unknown-input']) {
    assert.equal(needsRustChecks(['docs/README.md', file]), true, file)
  }
  assert.equal(needsRustChecks([]), true)
})

it('keeps the full native matrix manual and restores Rust cache before compiling', () => {
  assert.ok(Object.hasOwn(ci.on, 'workflow_dispatch'))
  assert.equal(ci.jobs['cli-native'].if, "github.event_name == 'workflow_dispatch'")
  assert.equal(ci.jobs['cli-native'].strategy.matrix.include.length, 5)
  assert.equal(ci.jobs.rust.needs, 'changes')
  assert.equal(ci.jobs.rust.if, "always() && (needs.changes.result != 'success' || needs.changes.outputs.rust == 'true')")
  for (const job of [ci.jobs.rust, ci.jobs['windows-console']]) {
    assert.equal(job.steps[0].if, "needs.changes.result != 'success'")
    assert.equal(job.steps[0].run, 'exit 1')
  }
  const steps = ci.jobs.rust.steps
  assert.ok(steps.findIndex(step => step.uses === 'Swatinem/rust-cache@v2') < steps.findIndex(step => step.run?.includes('prepare-cli-sidecar')))
  const release = load(readFileSync(new URL('../.github/workflows/release.yml', import.meta.url), 'utf8'))
  assert.equal(release.jobs.verify.strategy.matrix.include.length, 5)
  assert.notEqual(release.jobs.verify.if, "github.event_name == 'workflow_dispatch'")
})

it('checks the whole PR while main pushes compare only the pushed range', () => {
  const root = mkdtempSync(path.join(tmpdir(), 'skills-hub-ci-'))
  const git = (...args) => execFileSync('git', args, { cwd: root, encoding: 'utf8' }).trim()
  try {
    git('init')
    git('config', 'user.name', 'CI test')
    git('config', 'user.email', 'ci-test@example.invalid')
    writeFileSync(path.join(root, 'README.md'), 'base')
    git('add', '.')
    git('commit', '-m', 'base')
    const base = git('rev-parse', 'HEAD')
    mkdirSync(path.join(root, 'src-tauri'))
    writeFileSync(path.join(root, 'src-tauri/lib.rs'), 'native change')
    git('add', '.')
    git('commit', '-m', 'native')
    const previous = git('rev-parse', 'HEAD')
    writeFileSync(path.join(root, 'README.md'), 'documentation update')
    git('add', '.')
    git('commit', '-m', 'docs')
    const head = git('rev-parse', 'HEAD')
    const output = path.join(root, 'output')
    const run = (event, comparisonBase) => {
      writeFileSync(output, '')
      execFileSync(process.execPath, [fileURLToPath(new URL('./ci-changes.mjs', import.meta.url))], {
        cwd: root,
        env: { ...process.env, CI_EVENT: event, CI_BASE: comparisonBase, CI_HEAD: head, GITHUB_OUTPUT: output },
      })
      return readFileSync(output, 'utf8')
    }
    assert.equal(run('pull_request', base), 'rust=true\n')
    assert.equal(run('push', previous), 'rust=false\n')
    assert.equal(run('workflow_dispatch', previous), 'rust=true\n')
    assert.equal(run('push', '0'.repeat(40)), 'rust=true\n')
  } finally {
    rmSync(root, { recursive: true, force: true })
  }
})
