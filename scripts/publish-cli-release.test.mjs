import assert from 'node:assert/strict'
import { mkdtempSync, writeFileSync, rmSync, readdirSync, readFileSync } from 'node:fs'
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
  const calls=[]; let inspected=false
  await api.publishCliRelease({directory,tag:'v0.11.0',sourceCommit:'a'.repeat(40),gh:async args=>{
   calls.push(args)
   if(args[0]==='api'){if(!inspected){inspected=true;return null}return {draft:true,assets}}
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

it('accepts matching CLI resources alongside desktop assets and publishes only explicitly', async () => {
 await fixture(async ({directory,assets}) => {
  const calls=[]
  await api.publishCliRelease({directory,tag:'v0.11.0',sourceCommit:'a'.repeat(40),publish:true,gh:async args=>{
   calls.push(args)
   if(args[0]==='api') return {draft:true,assets:[...assets,{name:'Skills-Hub.dmg',size:100,digest:'desktop'}]}
  }})
  assert.ok(calls.find(args=>args[1]==='edit').includes('--draft=false'))
 })
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
