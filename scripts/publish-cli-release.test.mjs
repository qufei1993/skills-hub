import assert from 'node:assert/strict'
import { mkdtempSync, writeFileSync, rmSync, readdirSync, readFileSync } from 'node:fs'
import { spawnSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { cliPlatforms } from './cli-manifest.mjs'
const {it}=process.env.VITEST?await import('vitest'):await import('node:test')
const api=await import('./publish-cli-release.mjs').catch(()=>({}))
function fixture(run) {
 const directory=mkdtempSync(path.join(tmpdir(),'cli-publish-'))
 const digest=bytes=>`sha256:${createHash('sha256').update(bytes).digest('hex')}`
 for(const [target,platform] of Object.entries(cliPlatforms)) {
  const assetName=`skillshub-cli-0.11.0-${platform}${target.includes('windows')?'.exe':''}`
  const manifest={version:'0.11.0',sourceCommit:'a'.repeat(40),target,profile:'release',assetName,size:3,sha256:digest('abc').slice(7)}
  writeFileSync(path.join(directory,assetName),'abc')
  writeFileSync(path.join(directory,`${assetName}.sha256`),`${manifest.sha256}  ${assetName}\n`)
  writeFileSync(path.join(directory,`cli-manifest-${target}.json`),JSON.stringify(manifest)+'\n')
 }
 const assets=readdirSync(directory).map(name=>({name,size:readFileSync(path.join(directory,name)).length,digest:digest(readFileSync(path.join(directory,name)))}))
 return Promise.resolve(run({directory,assets})).finally(()=>rmSync(directory,{recursive:true,force:true}))
}
it('stages CLI resources in the desktop release without publishing it',async()=>{
 assert.equal(typeof api.publishCliRelease,'function')
 await fixture(async({directory,assets})=>{
  const calls=[]; let created=false
  await api.publishCliRelease({directory,tag:'v0.11.0',sourceCommit:'a'.repeat(40),gh:async args=>{
   calls.push(args)
   if(args[0]==='api' && args[1].endsWith('/releases')) return []
   if(args[0]==='api') return created ? {draft:true,assets} : null
   if(args[1]==='create') created=true
  }})
  assert.equal(calls.filter(args=>args[0]==='release'&&args[1]==='create').length,1)
  assert.ok(calls.find(args=>args[1]==='create').includes('--draft'))
  assert.ok(calls.every(args=>args[1]!=='edit'))
  assert.ok(calls.find(args=>args[1]==='create').includes('qufei1993/skills-hub'))
 })
})
it('refuses a conflicting existing release without upload, overwrite or publication',async()=>{
 assert.equal(typeof api.publishCliRelease,'function')
 await fixture(async({directory,assets})=>{
  const calls=[]
  await assert.rejects(api.publishCliRelease({directory,tag:'v0.11.0',sourceCommit:'a'.repeat(40),gh:async args=>{calls.push(args);return {draft:false,assets:assets.map((a,i)=>i? a:{...a,digest:'sha256:'+'b'.repeat(64)})}}}))
  assert.ok(calls.every(args=>args[0]==='api'))
 })
})
it('requires a source commit before any remote inspection or mutation',async()=>{
 await fixture(async({directory,assets})=>{
  const calls=[]
  await assert.rejects(api.publishCliRelease({directory,tag:'v0.11.0',gh:async args=>{calls.push(args);return {draft:false,assets}}}))
  assert.equal(calls.length,0)
 })
})

it('rejects explicit publication before any remote call even with matching assets', async () => {
 await fixture(async ({directory,assets}) => {
  const calls=[]
  await assert.rejects(api.publishCliRelease({directory,tag:'v0.11.0',sourceCommit:'a'.repeat(40),publish:true,gh:async args=>{
   calls.push(args)
   return {draft:true,assets}
  }}), /CLI_RELEASE_DRAFT_ONLY/)
  assert.equal(calls.length,0)
 })
})
it('rejects the command line publication switch before invoking GitHub', () => {
 const result=spawnSync(process.execPath,['scripts/publish-cli-release.mjs','/unused','--publish'],{encoding:'utf8'})
 assert.equal(result.status,1)
 assert.match(result.stderr,/CLI_RELEASE_DRAFT_ONLY/)
})
it('does not add new CLI assets to an already public desktop release', async () => {
 await fixture(async ({directory}) => {
  const calls=[]
  await assert.rejects(api.publishCliRelease({directory,tag:'v0.11.0',sourceCommit:'a'.repeat(40),gh:async args=>{calls.push(args);return {draft:false,assets:[{name:'app.dmg'}]}}}))
  assert.ok(calls.every(args=>args[0]==='api'))
 })
})

it('adds only missing CLI resources to an existing draft and verifies them before publication', async () => {
 await fixture(async ({directory,assets}) => {
  let uploaded=false; const calls=[]
  await api.publishCliRelease({directory,tag:'v0.11.0',sourceCommit:'a'.repeat(40),gh:async args=>{
   calls.push(args)
   if(args[0]==='api') return {draft:true,assets:uploaded?assets:assets.slice(1)}
   if(args[1]==='upload') uploaded=true
  }})
  assert.equal(calls.filter(args=>args[1]==='upload').length,1)
  assert.equal(calls.find(args=>args[1]==='upload').filter(arg=>arg.startsWith(directory)).length,1)
  assert.ok(calls.every(args=>args[1]!=='edit'))
 })
})

it('finds an existing draft through the release list when the tag endpoint returns 404', async () => {
 await fixture(async ({directory,assets}) => {
  const calls=[]
  await api.publishCliRelease({directory,tag:'v0.11.0',sourceCommit:'a'.repeat(40),gh:async args=>{
   calls.push(args)
   if(args[0]==='api' && args[1].includes('/releases/tags/')) return null
   if(args[0]==='api' && args[1].endsWith('/releases')) return [[{tag_name:'v0.10.1',draft:false,assets:[]}],[{tag_name:'v0.11.0',draft:true,assets}]]
   throw new Error('unexpected release mutation')
  }})
  assert.ok(calls.some(args=>args.includes('--paginate') && args.includes('--slurp')))
  assert.ok(calls.every(args=>args[0]==='api'))
 })
})

it('rejects duplicate drafts for one tag before mutating either release', async () => {
 await fixture(async ({directory,assets}) => {
  const calls=[]
  await assert.rejects(api.publishCliRelease({directory,tag:'v0.11.0',sourceCommit:'a'.repeat(40),gh:async args=>{
   calls.push(args)
   if(args[1].includes('/releases/tags/')) return null
   return [[{tag_name:'v0.11.0',draft:true,assets},{tag_name:'v0.11.0',draft:true,assets}]]
  }}), /CLI_RELEASE_IMMUTABLE_CONFLICT/)
  assert.ok(calls.every(args=>args[0]==='api'))
 })
})

it('replaces placeholder notes on the canonical draft and verifies the result', async () => {
 await fixture(async ({directory,assets}) => {
  const notesFile=path.join(tmpdir(),`release-notes-${Date.now()}.md`)
  const body='### Downloads\n| Linux | [Download](https://example.com/app.deb) |\n'
  writeFileSync(notesFile,body)
  const calls=[]; let release={id:123,tag_name:'v0.11.0',draft:true,body:'Native CLI resources.',assets}
  const staleListRelease={...release}
  try {
   await api.publishCliRelease({directory,tag:'v0.11.0',sourceCommit:'a'.repeat(40),notesFile,gh:async args=>{
    calls.push(args)
    if(args.includes('PATCH')) { release={...release,body:args.find(arg=>arg.startsWith('body=')).slice(5)};return release }
    if(args[1].includes('/releases/tags/')) return null
    if(args[1].endsWith('/releases/123')) return release
    return [[staleListRelease]]
   }})
   const patch=calls.find(args=>args.includes('PATCH'))
   assert.ok(patch)
   assert.ok(patch.includes('repos/qufei1993/skills-hub/releases/123'))
   assert.ok(patch.includes('draft=true'))
   assert.ok(patch.includes('tag_name=v0.11.0'))
   assert.ok(patch.includes(`target_commitish=${'a'.repeat(40)}`))
   assert.equal(release.body,body)
   assert.ok(calls.every(args=>!args.includes('--draft=false')))
  } finally {rmSync(notesFile,{force:true})}
 })
})
it('refuses to replace notes on an already public release', async () => {
 await fixture(async ({directory,assets}) => {
  const calls=[]
  await assert.rejects(api.publishCliRelease({directory,tag:'v0.11.0',sourceCommit:'a'.repeat(40),notesFile:'/unused',gh:async args=>{calls.push(args);return {id:123,draft:false,assets}}}),/CLI_RELEASE_NOT_DRAFT/)
  assert.ok(calls.every(args=>!args.includes('PATCH')))
 })
})
