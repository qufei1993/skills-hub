import assert from 'node:assert/strict'
import { mkdtempSync, writeFileSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { spawnSync } from 'node:child_process'
const { it } = process.env.VITEST ? await import('vitest') : await import('node:test')
const api = await import('./release-downloads.mjs').catch(() => ({}))

it('lists available desktop installers using readable labels and versioned direct links', () => {
 const table = api.renderDownloads({tag:'v0.11.0',language:'zh',assets:[
  'Skills-Hub-v0.11.0-macOS-aarch64.dmg',
  'Skills-Hub-v0.11.0-macOS-x86_64.dmg',
  'Skills-Hub-v0.11.0-Windows-x64.exe',
  'skillshub-cli-0.11.0-linux-x64', 'updater.json',
 ]})
 assert.match(table, /Apple 芯片/)
 assert.match(table, /Intel 芯片/)
 assert.match(table, /Intel \/ AMD/)
 assert.equal((table.match(/https:\/\//g) ?? []).length, 3)
 assert.match(table, /releases\/download\/v0\.11\.0\/Skills-Hub-v0\.11\.0-Windows-x64\.exe/)
 assert.doesNotMatch(table, /Linux|updater\.json|skillshub-cli-/)
 assert.match(table, /启用 AI 管理时自动下载/)
})

it('omits unavailable installers and rejects an empty desktop download list', () => {
 const table = api.renderDownloads({tag:'v0.12.0',language:'en',assets:['Skills-Hub-v0.12.0-Windows-x64.exe']})
 assert.match(table, /### Downloads/)
 assert.match(table, /v0\.12\.0/)
 assert.doesNotMatch(table, /macOS/)
 assert.throws(() => api.renderDownloads({tag:'v0.11.0',language:'zh',assets:['skillshub-cli-0.11.0-linux-x64']}))
})

it('groups both Linux formats by architecture and links only the available version assets', () => {
 const assets = [
  'Skills-Hub-v0.11.0-Linux-x64.deb', 'Skills-Hub-v0.11.0-Linux-x64.AppImage',
  'Skills-Hub-v0.11.0-Linux-arm64.deb', 'Skills-Hub-v0.11.0-Linux-arm64.AppImage',
  'Skills-Hub-v0.11.0-Linux-x64.AppImage.sig', 'Skills-Hub-v0.10.1-Linux-x64.deb',
 ]
 for (const language of ['en', 'zh']) {
  const table = api.renderDownloads({tag:'v0.11.0',language,assets})
  const rows = table.split('\n').filter(line => line.startsWith('| Linux |'))
  assert.equal(rows.length,2)
  assert.match(rows[0],/Intel \/ AMD/)
  assert.match(rows[1],/ARM64/)
  for (const name of assets.slice(0,4)) assert.ok(table.includes(`https://github.com/qufei1993/skills-hub/releases/download/v0.11.0/${name}`))
  assert.doesNotMatch(table,/\.sig|v0\.10\.1|macOS|Windows/)
 }
 const partial = api.renderDownloads({tag:'v0.11.0',language:'en',assets:[assets[1]]})
 assert.match(partial,/Download .AppImage/)
 assert.doesNotMatch(partial,/\.deb|ARM64/)
})

it('release extraction replaces an existing table with links from the actual assets', () => {
 const dir = mkdtempSync(path.join(tmpdir(),'release-downloads-'))
 try {
  const changelog = path.join(dir,'CHANGELOG.md')
  writeFileSync(changelog,'## [0.12.0]\n\n### Downloads\n\nold-link\n\n### Added\n\n- New feature\n\n## [0.11.0]\n\n- Previous version\n')
  writeFileSync(path.join(dir,'Skills-Hub-v0.12.0-Windows-x64.exe'),'installer')
  const result = spawnSync(process.execPath,['scripts/extract-changelog.mjs','v0.12.0',changelog,'--assets',dir,'--language','en'],{encoding:'utf8'})
  assert.equal(result.status,0,result.stderr)
  assert.equal((result.stdout.match(/### Downloads/g) ?? []).length,1)
  assert.match(result.stdout,/releases\/download\/v0\.12\.0\//)
  assert.match(result.stdout,/- New feature/)
  assert.doesNotMatch(result.stdout,/old-link|Previous version/)
 } finally { rmSync(dir,{recursive:true,force:true}) }
})
