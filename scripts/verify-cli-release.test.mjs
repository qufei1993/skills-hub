import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
const { it } = process.env.VITEST ? await import('vitest') : await import('node:test')
const api = await import('./verify-cli-release.mjs').catch(() => ({}))
const targets = ['aarch64-apple-darwin','x86_64-apple-darwin','x86_64-pc-windows-msvc','x86_64-unknown-linux-gnu','aarch64-unknown-linux-gnu']
const platforms = ['darwin-arm64','darwin-x64','windows-x64.exe','linux-x64','linux-arm64']
const manifests = targets.map((target,i) => ({version:'0.11.0',sourceCommit:'a'.repeat(40),target,profile:'release',assetName:`skillshub-cli-0.11.0-${platforms[i]}`,size:3,sha256:createHash('sha256').update('abc').digest('hex')}))
it('verifies all five fixed version downloads and published manifests without credentials', async () => {
  assert.equal(typeof api.verifyCliRelease,'function')
  const urls=[]
  await api.verifyCliRelease({manifests, download: async url => { urls.push(url); return Buffer.from(url.endsWith('.json') ? JSON.stringify(manifests.find(m=>url.endsWith(`cli-manifest-${m.target}.json`))) : 'abc') }})
  assert.equal(urls.length,10)
  assert.ok(urls.every(url=>url.startsWith('https://github.com/qufei1993/skills-hub/releases/download/v0.11.0/')))
})
it('refuses incomplete platforms, unavailable bytes and conflicting published identity', async () => {
  assert.equal(typeof api.verifyCliRelease,'function')
  await assert.rejects(api.verifyCliRelease({manifests:manifests.slice(1),download:async()=>Buffer.from('abc')}))
  for (const failure of ['404','hash','commit']) await assert.rejects(api.verifyCliRelease({manifests,download:async url=>{
    if(failure==='404') throw new Error('404')
    if(url.endsWith('.json')) { const m=manifests.find(m=>url.endsWith(`cli-manifest-${m.target}.json`)); return Buffer.from(JSON.stringify({...m,sourceCommit:failure==='commit'?'b'.repeat(40):m.sourceCommit})) }
    return Buffer.from(failure==='hash'?'xyz':'abc')
  }}))
})
