import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { createRequire } from 'node:module'
const { it } = process.env.VITEST ? await import('vitest') : await import('node:test')
const { load } = createRequire(import.meta.url)('js-yaml')
const workflow = load(readFileSync(new URL('../.github/workflows/release.yml', import.meta.url),'utf8'))
const api = await import('./prepare-linux-assets.mjs').catch(() => ({}))

it('stages complete Linux installers and the unchanged AppImage signature with release names', () => {
 assert.equal(typeof api.prepareLinuxAssets,'function')
 for (const [target,arch] of [['x86_64-unknown-linux-gnu','x64'],['aarch64-unknown-linux-gnu','arm64']]) {
  const root = mkdtempSync(path.join(tmpdir(),'linux-assets-'))
  try {
   const bundle = path.join(root,'bundle')
   for (const format of ['deb','appimage']) mkdirSync(path.join(bundle,format),{recursive:true})
   writeFileSync(path.join(bundle,'deb/Skills Hub_0.11.0.deb'),'deb-bytes')
   writeFileSync(path.join(bundle,'appimage/Skills Hub_0.11.0.AppImage'),'image-bytes')
   writeFileSync(path.join(bundle,'appimage/Skills Hub_0.11.0.AppImage.sig'),'signature\n')
   const output = path.join(root,'release-assets')
   api.prepareLinuxAssets({bundle,output,tag:'v0.11.0',target})
   assert.deepEqual(readdirSync(output).sort(),[`Skills-Hub-v0.11.0-Linux-${arch}.AppImage`,`Skills-Hub-v0.11.0-Linux-${arch}.AppImage.sig`,`Skills-Hub-v0.11.0-Linux-${arch}.deb`])
   assert.equal(readFileSync(path.join(output,`Skills-Hub-v0.11.0-Linux-${arch}.deb`),'utf8'),'deb-bytes')
   assert.equal(readFileSync(path.join(output,`Skills-Hub-v0.11.0-Linux-${arch}.AppImage`),'utf8'),'image-bytes')
   assert.equal(readFileSync(path.join(output,`Skills-Hub-v0.11.0-Linux-${arch}.AppImage.sig`),'utf8'),'signature\n')
   if (process.platform !== 'win32') assert.equal(statSync(path.join(output,`Skills-Hub-v0.11.0-Linux-${arch}.AppImage`)).mode & 0o777,0o755)
   for (const file of ['deb/Skills Hub_0.11.0.deb','appimage/Skills Hub_0.11.0.AppImage','appimage/Skills Hub_0.11.0.AppImage.sig']) {
    const filename = path.join(bundle,file)
    const bytes = readFileSync(filename)
    rmSync(filename)
    assert.throws(() => api.prepareLinuxAssets({bundle,output,tag:'v0.11.0',target}))
    writeFileSync(filename,bytes)
    writeFileSync(filename,'')
    assert.throws(() => api.prepareLinuxAssets({bundle,output,tag:'v0.11.0',target}))
    writeFileSync(filename,bytes)
   }
   writeFileSync(path.join(bundle,'deb/stale.deb'),'stale')
   assert.throws(() => api.prepareLinuxAssets({bundle,output,tag:'v0.11.0',target}))
   assert.throws(() => api.prepareLinuxAssets({bundle,output,tag:'../bad',target}))
   assert.throws(() => api.prepareLinuxAssets({bundle,output,tag:'v0.11.0',target:'x86_64-apple-darwin'}))
  } finally { rmSync(root,{recursive:true,force:true}) }
 }
})

it('generates valid Linux updater entries from signed AppImages without linking deb packages', () => {
 const step = workflow.jobs['assemble-updater-json'].steps.find(step => step.name === 'Generate updater.json')
 const root = mkdtempSync(path.join(tmpdir(),'linux-updater-'))
 try {
  mkdirSync(path.join(root,'dl'))
  for (const arch of ['x64','arm64']) {
   writeFileSync(path.join(root,`dl/Skills-Hub-v0.11.0-Linux-${arch}.AppImage`),'appimage')
   writeFileSync(path.join(root,`dl/Skills-Hub-v0.11.0-Linux-${arch}.AppImage.sig`),`signature-${arch}\r\n`)
   writeFileSync(path.join(root,`dl/Skills-Hub-v0.11.0-Linux-${arch}.deb`),'deb')
  }
  writeFileSync(path.join(root,'release-notes.md'),'Quote " and slash \\ and 中文\nSecond line')
  const result = spawnSync('bash',['-c',step.run],{cwd:root,encoding:'utf8',env:{...process.env,TAG:'v0.11.0',REPO:'qufei1993/skills-hub'}})
  assert.equal(result.status,0,result.stderr)
  const updater = JSON.parse(readFileSync(path.join(root,'updater.json'),'utf8'))
  assert.equal(updater.version,'0.11.0')
  assert.equal(updater.notes,'Quote " and slash \\ and 中文\nSecond line')
  assert.deepEqual(updater.platforms,{
   'linux-x86_64':{signature:'signature-x64',url:'https://github.com/qufei1993/skills-hub/releases/download/v0.11.0/Skills-Hub-v0.11.0-Linux-x64.AppImage'},
   'linux-aarch64':{signature:'signature-arm64',url:'https://github.com/qufei1993/skills-hub/releases/download/v0.11.0/Skills-Hub-v0.11.0-Linux-arm64.AppImage'},
  })
 } finally { rmSync(root,{recursive:true,force:true}) }
})

it('packages both Linux architectures natively and verifies extracted installers before uploading', () => {
 const job = workflow.jobs['desktop-build']
 const linux = job.strategy.matrix.include.filter(row => row.target.includes('linux'))
 assert.deepEqual(linux.map(row => [row.target,row.os,row.platform]),[
  ['x86_64-unknown-linux-gnu','ubuntu-24.04','linux-x64'],
  ['aarch64-unknown-linux-gnu','ubuntu-24.04-arm','linux-arm64'],
 ])
 const steps = job.steps
 const build = steps.findIndex(step => step.name === 'Build Tauri App (Linux)')
 assert.ok(build >= 0)
 assert.equal(steps[build].if,"runner.os == 'Linux'")
 assert.match(steps[build].run,/--bundles deb,appimage/)
 const prepare = steps.findIndex(step => step.name === 'Prepare Linux Assets')
 const verify = steps.findIndex(step => step.name === 'Verify Linux installers')
 assert.ok(build < prepare && prepare < verify)
 assert.match(steps[prepare].run,/prepare-linux-assets\.mjs/)
 assert.match(steps[verify].run,/verify-linux-packages\.sh/)
 assert.ok(verify < steps.findIndex(step => step.name === 'Upload workflow artifacts'))
})
