import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { readFileSync } from 'node:fs'
import { createRequire } from 'node:module'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const { it } = process.env.VITEST ? await import('vitest') : await import('node:test')
const require = createRequire(import.meta.url)
const { load } = require('js-yaml')
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const workflow = load(readFileSync(path.join(root, '.github/workflows/release.yml'), 'utf8'))
const expectedTargets = {
  'aarch64-apple-darwin': 'macos-14',
  'x86_64-apple-darwin': 'macos-15-intel',
  'x86_64-pc-windows-msvc': 'windows-2022',
  'x86_64-unknown-linux-gnu': 'ubuntu-24.04',
  'aarch64-unknown-linux-gnu': 'ubuntu-24.04-arm',
}
const needs = job => typeof job.needs === 'string' ? [job.needs] : job.needs ?? []

it('tag release independently verifies every native target before build or publication', () => {
  const verify = workflow.jobs.verify
  assert.ok(verify, 'tag release requires its own verify job')
  assert.deepEqual(Object.fromEntries(verify.strategy.matrix.include.map(row => [row.target, row.os])), expectedTargets)
  assert.equal(verify.strategy.matrix.include.length, 5)
  assert.equal(verify['runs-on'], '${{ matrix.os }}')
  assert.equal(verify['continue-on-error'], undefined)
  assert.equal(verify.steps.find(step => step.uses === 'actions/checkout@v4')?.with.ref, '${{ github.sha }}')
  const commands = verify.steps.map(step => step.run ?? '').join('\n')
  assert.match(commands, /rustc -vV/)
  assert.match(commands, /--test cli_desktop_compatibility/)
  assert.match(commands, /version --json/)
  assert.match(commands, /doctor --json/)
  assert.ok(verify.steps.some(step => step.if === "runner.os == 'Windows'" && /cargo test --locked --lib cli_bridge\b/.test(step.run)))
  assert.ok(verify.steps.every(step => !step['continue-on-error']))
  for (const name of ['cli-build', 'desktop-build', 'cli-publish', 'assemble-updater-json']) {
    const job = workflow.jobs[name]
    assert.ok(needs(job).includes('verify'), `${name} must require all native verification jobs`)
    assert.equal(job.if, "startsWith(github.ref, 'refs/tags/v')")
  }
})

it('both notarization submissions gate on parsed Accepted results before stapling and asset preparation', () => {
  const steps = workflow.jobs['desktop-build'].steps
  const index = steps.findIndex(step => step.name === 'Notarize macOS desktop when configured')
  const script = steps[index].run
  const submissions = script.split('\n').filter(line => line.includes('xcrun notarytool submit'))
  assert.equal(submissions.length, 1)
  for (const submission of submissions) {
    assert.match(submission, /--output-format json/)
    assert.match(submission, /2>\/dev\/null \| node scripts\/assert-notarization\.mjs/)
  }
  assert.match(script, /set -euo pipefail/)
  assert.equal((script.match(/node scripts\/assert-notarization\.mjs/g) ?? []).length, 1)
  assert.ok(script.lastIndexOf('node scripts/assert-notarization.mjs') < script.indexOf('xcrun stapler staple'))
  assert.match(script, /xcrun stapler validate/)
  assert.match(script, /codesign --verify/)
  assert.ok(index < steps.findIndex(step => step.name === 'Prepare macOS Assets'))
  assert.match(script, /signer sign/)
})

it('notarization gate accepts only Accepted JSON and never echoes untrusted response details', () => {
  const id = '00000000-1111-2222-3333-444444444444'
  const hidden = 'must-not-log-credential-or-profile'
  for (const [input, status, exit] of [
    [JSON.stringify({ status: 'Accepted', id, profile: hidden, message: hidden }), 'Accepted', 0],
    [JSON.stringify({ status: 'Invalid', id, message: hidden }), 'Invalid', 1],
    [JSON.stringify({ status: 'Rejected', id, message: hidden }), 'Rejected', 1],
    [JSON.stringify({ status: 'In Progress', id }), 'In Progress', 1],
    [JSON.stringify({ status: hidden, id }), 'InvalidResponse', 1],
    [JSON.stringify({ status: 'Accepted', id: hidden }), 'InvalidResponse', 1],
    [JSON.stringify({ id }), 'InvalidResponse', 1],
    [`{broken JSON ${hidden}`, 'InvalidResponse', 1],
    ['null', 'InvalidResponse', 1],
  ]) {
    const result = spawnSync(process.execPath, ['scripts/assert-notarization.mjs'], { cwd: root, input, encoding: 'utf8' })
    assert.equal(result.status, exit)
    assert.equal(result.stderr, '')
    const output = JSON.parse(result.stdout)
    assert.deepEqual(Object.keys(output).sort(), ['id', 'status'])
    assert.equal(output.status, status)
    assert.ok(!result.stdout.includes(hidden))
  }
})

it('desktop release has no npm publication dependency and reuses native verified CLI builds', () => {
  assert.deepEqual(needs(workflow.jobs['assemble-updater-json']), ['verify', 'desktop-build', 'cli-publish'])
  for (const job of Object.values(workflow.jobs)) {
    assert.ok(needs(job).every(name => Object.hasOwn(workflow.jobs, name)))
    assert.equal(job.permissions?.['id-token'], undefined)
    for (const step of job.steps ?? []) {
      assert.doesNotMatch(step.run ?? '', /npm publish|cli:package|package-cli\.test/)
    }
  }
  const commands = workflow.jobs['cli-build'].steps.map(step => step.run ?? '').join('\n')
  assert.doesNotMatch(commands, /npm run cli:prepare/)
  assert.ok(workflow.jobs['cli-build'].steps.some(step => step.name === 'Verify reused CLI build'))
  assert.match(commands, /cli-manifest/)
})

it('release notes generate bilingual download tables from actual desktop assets', () => {
  const step = workflow.jobs['assemble-updater-json'].steps.find(item => item.name === 'Generate release notes from changelog')
  assert.match(step.run, /extract-changelog\.mjs "\$TAG" docs\/CHANGELOG\.zh\.md --assets dl --language zh/)
  assert.match(step.run, /extract-changelog\.mjs "\$TAG" CHANGELOG\.md --assets dl --language en/)
})

it('separates CLI and desktop builds and keeps the complete release as a draft', () => {
  assert.equal(workflow.jobs['cli-build'].strategy.matrix.include.length, 5)
  assert.equal(workflow.jobs['desktop-build'].strategy.matrix.include.length, 5)
  assert.ok(needs(workflow.jobs['desktop-build']).includes('cli-build'))
  assert.ok(needs(workflow.jobs['cli-publish']).includes('cli-build'))
  const cliSteps = workflow.jobs['cli-build'].steps
  const notarization = cliSteps.findIndex(step => step.name === 'Notarize macOS CLI when configured')
  assert.ok(notarization >= 0)
  assert.ok(notarization < cliSteps.findIndex(step => step.name === 'Generate final CLI manifest and assets'))
  assert.match(cliSteps[notarization].run, /node scripts\/assert-notarization\.mjs/)
  const publish = workflow.jobs['cli-publish'].steps.map(step => step.run ?? '').join('\n')
  assert.match(publish, /publish-cli-release\.mjs/)
  assert.doesNotMatch(publish, /--publish|verify-cli-release\.mjs/)
  assert.equal(workflow.jobs['cli-publish'].permissions.contents, 'write')
  const finalSteps = workflow.jobs['assemble-updater-json'].steps
  const upload = finalSteps.findIndex(step => step.uses === 'softprops/action-gh-release@v2')
  assert.equal(finalSteps[upload].with.draft, true)
  const verification = finalSteps.findIndex(step => /publish-cli-release\.mjs/.test(step.run ?? ''))
  assert.ok(upload < verification)
  assert.match(finalSteps[verification].run, /--notes-file release-notes\.md/)
  for (const job of Object.values(workflow.jobs)) {
    for (const step of job.steps ?? []) {
      assert.doesNotMatch(step.run ?? '', /--publish|--draft=false/)
      if (step.uses === 'softprops/action-gh-release@v2') assert.equal(step.with.draft, true)
    }
  }
  assert.ok(!finalSteps.some(step => /verify-cli-release\.mjs/.test(step.run ?? '')))
  const desktop = workflow.jobs['desktop-build'].steps
  assert.ok(desktop.some(step => step.uses === 'actions/download-artifact@v4' && step.with.name === 'cli-manifest-${{ matrix.target }}'))
  assert.ok(desktop.some(step => /verify-desktop-bundle\.mjs/.test(step.run ?? '')))
  assert.ok(!desktop.some(step => /npm run cli:prepare/.test(step.run ?? '')))
})
