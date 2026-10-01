import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { copyFileSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { createRequire } from 'node:module'
const { it } = process.env.VITEST ? await import('vitest') : await import('node:test')
const { load } = createRequire(import.meta.url)('js-yaml')
const workflow = load(readFileSync(new URL('../.github/workflows/release.yml', import.meta.url),'utf8'))
const api = await import('./prepare-linux-assets.mjs').catch(() => ({}))

function executeWorkflowStep(run, root, env) {
 writeFileSync(path.join(root, 'workflow-step.sh'), run)
 return spawnSync('bash', ['workflow-step.sh'], {cwd:root, encoding:'utf8', env:{...process.env, ...env}})
}

it('stages complete Linux installers and the unchanged installer signatures with release names', () => {
 assert.equal(typeof api.prepareLinuxAssets,'function')
 for (const [target,arch] of [['x86_64-unknown-linux-gnu','x64'],['aarch64-unknown-linux-gnu','arm64']]) {
  const root = mkdtempSync(path.join(tmpdir(),'linux-assets-'))
  try {
   const bundle = path.join(root,'bundle')
   for (const format of ['deb','appimage']) mkdirSync(path.join(bundle,format),{recursive:true})
   writeFileSync(path.join(bundle,'deb/Skills Hub_0.11.0.deb'),'deb-bytes')
   writeFileSync(path.join(bundle,'deb/Skills Hub_0.11.0.deb.sig'),'deb-signature\n')
   writeFileSync(path.join(bundle,'appimage/Skills Hub_0.11.0.AppImage'),'image-bytes')
   writeFileSync(path.join(bundle,'appimage/Skills Hub_0.11.0.AppImage.sig'),'signature\n')
   const output = path.join(root,'release-assets')
   api.prepareLinuxAssets({bundle,output,tag:'v0.11.0',target})
   assert.deepEqual(readdirSync(output).sort(),[`Skills-Hub-v0.11.0-Linux-${arch}.AppImage`,`Skills-Hub-v0.11.0-Linux-${arch}.AppImage.sig`,`Skills-Hub-v0.11.0-Linux-${arch}.deb`,`Skills-Hub-v0.11.0-Linux-${arch}.deb.sig`])
   assert.equal(readFileSync(path.join(output,`Skills-Hub-v0.11.0-Linux-${arch}.deb`),'utf8'),'deb-bytes')
   assert.equal(readFileSync(path.join(output,`Skills-Hub-v0.11.0-Linux-${arch}.AppImage`),'utf8'),'image-bytes')
   assert.equal(readFileSync(path.join(output,`Skills-Hub-v0.11.0-Linux-${arch}.AppImage.sig`),'utf8'),'signature\n')
   if (process.platform !== 'win32') assert.equal(statSync(path.join(output,`Skills-Hub-v0.11.0-Linux-${arch}.AppImage`)).mode & 0o777,0o755)
   assert.equal(readFileSync(path.join(output,`Skills-Hub-v0.11.0-Linux-${arch}.deb.sig`),'utf8'),'deb-signature\n')
   for (const file of ['deb/Skills Hub_0.11.0.deb.sig','deb/Skills Hub_0.11.0.deb','appimage/Skills Hub_0.11.0.AppImage','appimage/Skills Hub_0.11.0.AppImage.sig']) {
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

it('generates valid Linux updater entries matching each signed installer format', () => {
 const step = workflow.jobs['assemble-updater-json'].steps.find(step => step.name === 'Generate updater.json')
 const root = mkdtempSync(path.join(tmpdir(),'linux-updater-'))
 try {
  mkdirSync(path.join(root,'dl'))
  for (const arch of ['x64','arm64']) {
   writeFileSync(path.join(root,`dl/Skills-Hub-v0.11.0-Linux-${arch}.AppImage`),'appimage')
   writeFileSync(path.join(root,`dl/Skills-Hub-v0.11.0-Linux-${arch}.AppImage.sig`),`signature-${arch}\r\n`)
   writeFileSync(path.join(root,`dl/Skills-Hub-v0.11.0-Linux-${arch}.deb`),'deb')
   writeFileSync(path.join(root,`dl/Skills-Hub-v0.11.0-Linux-${arch}.deb.sig`),`deb-signature-${arch}\r\n`)
  }
  writeFileSync(path.join(root,'updater-notes.md'),'Quote " and slash \\ and 中文\nSecond line')
  const result = executeWorkflowStep(step.run,root,{TAG:'v0.11.0',REPO:'qufei1993/skills-hub'})
  assert.equal(result.status,0,result.stderr)
  const updater = JSON.parse(readFileSync(path.join(root,'updater.json'),'utf8'))
  assert.equal(updater.version,'0.11.0')
  assert.equal(updater.notes,'Quote " and slash \\ and 中文\nSecond line')
  assert.deepEqual(updater.platforms,{
   'linux-x86_64-deb':{signature:'deb-signature-x64',url:'https://github.com/qufei1993/skills-hub/releases/download/v0.11.0/Skills-Hub-v0.11.0-Linux-x64.deb'},
   'linux-aarch64-deb':{signature:'deb-signature-arm64',url:'https://github.com/qufei1993/skills-hub/releases/download/v0.11.0/Skills-Hub-v0.11.0-Linux-arm64.deb'},
   'linux-x86_64':{signature:'signature-x64',url:'https://github.com/qufei1993/skills-hub/releases/download/v0.11.0/Skills-Hub-v0.11.0-Linux-x64.AppImage'},
   'linux-aarch64':{signature:'signature-arm64',url:'https://github.com/qufei1993/skills-hub/releases/download/v0.11.0/Skills-Hub-v0.11.0-Linux-arm64.AppImage'},
  })
 } finally { rmSync(root,{recursive:true,force:true}) }
})

it('adds download tables only to the GitHub page while updater notes contain changes only', () => {
 const root = mkdtempSync(path.join(tmpdir(),'release-notes-'))
 try {
  mkdirSync(path.join(root,'scripts'))
  mkdirSync(path.join(root,'docs'))
  mkdirSync(path.join(root,'dl'))
  for (const script of ['extract-changelog.mjs','release-downloads.mjs']) copyFileSync(new URL(script,import.meta.url),path.join(root,'scripts',script))
  writeFileSync(path.join(root,'CHANGELOG.md'),'## [0.11.0]\n\n### Fixed\n- English fix\n')
  writeFileSync(path.join(root,'docs/CHANGELOG.zh.md'),'## [0.11.0]\n\n### 修复\n- 中文修复\n')
  writeFileSync(path.join(root,'dl/Skills-Hub-v0.11.0-Linux-x64.deb'),'installer')
  const steps = workflow.jobs['assemble-updater-json'].steps.filter(step => /Generate (release|updater) notes from changelog/.test(step.name ?? ''))
  for (const step of steps) {
   const result = executeWorkflowStep(step.run,root,{TAG:'v0.11.0'})
   assert.equal(result.status,0,result.stderr)
  }
  const page = readFileSync(path.join(root,'release-notes.md'),'utf8')
  assert.match(page,/### 下载安装/)
  assert.match(page,/### Downloads/)
  assert.match(page,/Skills-Hub-v0.11.0-Linux-x64.deb/)
  assert.match(page,/- 中文修复/)
  assert.match(page,/- English fix/)
  const updaterFile = path.join(root,'updater-notes.md')
  assert.ok(readdirSync(root).includes('updater-notes.md'),'updater requires separate notes without release-page downloads')
  const updater = readFileSync(updaterFile,'utf8')
  assert.match(updater,/- 中文修复/)
  assert.match(updater,/- English fix/)
  assert.doesNotMatch(updater,/下载安装|Downloads|Skills-Hub-|SmartScreen|Gatekeeper/)
  const generate = workflow.jobs['assemble-updater-json'].steps.find(step => step.name === 'Generate updater.json')
  writeFileSync(path.join(root,'dl/Skills-Hub-v0.11.0-Linux-x64.AppImage'),'appimage')
  writeFileSync(path.join(root,'dl/Skills-Hub-v0.11.0-Linux-x64.AppImage.sig'),'signature')
  const result = executeWorkflowStep(generate.run,root,{TAG:'v0.11.0',REPO:'qufei1993/skills-hub'})
  assert.equal(result.status,0,result.stderr)
  assert.equal(JSON.parse(readFileSync(path.join(root,'updater.json'),'utf8')).notes,updater.trimEnd())
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


const cliDependencies = await import('./verify-linux-cli.mjs')
function elfFixture(dependencies, machine = 183) {
 const strings = Buffer.from('\0' + dependencies.join('\0') + '\0')
 const dynamicOffset = 64 + 3 * 64
 const dynamicSize = (dependencies.length + 1) * 16
 const stringOffset = dynamicOffset + dynamicSize
 const bytes = Buffer.alloc(stringOffset + strings.length)
 bytes.set([0x7f, 0x45, 0x4c, 0x46, 2, 1, 1])
 bytes.writeUInt16LE(machine,18)
 bytes.writeBigUInt64LE(64n,40)
 bytes.writeUInt16LE(64,58)
 bytes.writeUInt16LE(3,60)
 bytes.writeUInt32LE(6,64+64+4)
 bytes.writeBigUInt64LE(BigInt(dynamicOffset),64+64+24)
 bytes.writeBigUInt64LE(BigInt(dynamicSize),64+64+32)
 bytes.writeUInt32LE(2,64+64+40)
 bytes.writeUInt32LE(3,64+128+4)
 bytes.writeBigUInt64LE(BigInt(stringOffset),64+128+24)
 bytes.writeBigUInt64LE(BigInt(strings.length),64+128+32)
 let index = 1
 for (let i=0;i<dependencies.length;i++) {
  bytes.writeBigUInt64LE(1n,dynamicOffset+i*16)
  bytes.writeBigUInt64LE(BigInt(index),dynamicOffset+i*16+8)
  index+=Buffer.byteLength(dependencies[i])+1
 }
 strings.copy(bytes,stringOffset)
 return bytes
}

it('allows standalone Linux CLI dependencies on both native architectures', () => {
 const dependencies=['libdbus-1.so.3','libz.so.1','libgcc_s.so.1','libm.so.6','libc.so.6']
 for (const machine of [62,183]) assert.deepEqual(cliDependencies.verifyLinuxCli(elfFixture(dependencies,machine)),dependencies)
})
it('rejects GUI libraries retained by the ARM64 CLI linker', () => {
 for (const library of ['libwebkit2gtk-4.1.so.0','libgtk-3.so.0','libsoup-3.0.so.0','libjavascriptcoregtk-4.1.so.0','libgdk-3.so.0']) {
  assert.throws(()=>cliDependencies.verifyLinuxCli(elfFixture(['libc.so.6',library])),/CLI_GUI_RUNTIME_DEPENDENCY/)
 }
})
it('rejects malformed executables instead of reporting a dependency-free CLI', () => {
 assert.throws(()=>cliDependencies.verifyLinuxCli(Buffer.from('not an ELF')))
 const truncated=elfFixture(['libc.so.6']).subarray(0,80)
 assert.throws(()=>cliDependencies.verifyLinuxCli(truncated))
})
