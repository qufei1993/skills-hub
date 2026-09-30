import { spawnSync } from 'node:child_process'
import { readFileSync, readdirSync } from 'node:fs'
import { createHash } from 'node:crypto'
import path from 'node:path'
import { pathToFileURL } from 'node:url'
import { readReleaseManifests } from './verify-cli-release.mjs'
const repository='qufei1993/skills-hub'
function runGh(args) {
  const result=spawnSync('gh',args,{encoding:'utf8',stdio:['ignore','pipe','pipe'],shell:false,maxBuffer:4*1024*1024})
  if(result.error || result.status!==0) {
    if(args[0]==='api' && /\(HTTP 404\)/.test(result.stderr ?? '')) return null
    throw new Error('CLI_RELEASE_PUBLISH_FAILED')
  }
  return args[0]==='api' ? JSON.parse(result.stdout) : undefined
}
export async function publishCliRelease({directory,tag,sourceCommit,publish=false,gh=runGh}) {
  if(!/^[a-f0-9]{40}$/.test(sourceCommit ?? '')) throw new Error('CLI_RELEASE_SOURCE_REQUIRED')
  if(!/^v\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/.test(tag)) throw new Error('CLI_RELEASE_TAG_INVALID')
  const manifests=readReleaseManifests(directory,{version:tag.slice(1),sourceCommit})
  const names=manifests.flatMap(m=>[m.assetName,`${m.assetName}.sha256`,`cli-manifest-${m.target}.json`])
  if(readdirSync(directory).length!==names.length) throw new Error('CLI_RELEASE_UNEXPECTED_ASSETS')
  const files=names.map(name=>({name,bytes:readFileSync(path.join(directory,name))}))
  for(const manifest of manifests) {
    const binary=files.find(f=>f.name===manifest.assetName).bytes
    const checksum=files.find(f=>f.name===`${manifest.assetName}.sha256`).bytes.toString('utf8')
    if(binary.length!==manifest.size || createHash('sha256').update(binary).digest('hex')!==manifest.sha256 || checksum!==`${manifest.sha256}  ${manifest.assetName}\n`) throw new Error('CLI_RELEASE_LOCAL_INTEGRITY_FAILED')
  }
  const inspect=()=>gh(['api',`repos/${repository}/releases/tags/${tag}`])
  let release=await inspect()
  if(!release) {
    await gh(['release','create',tag,'--repo',repository,'--draft','--verify-tag','--title',`Skills Hub ${tag}`,'--notes',`Native CLI resources. Source commit: ${sourceCommit}. Source: https://github.com/qufei1993/skills-hub`,...(tag.includes('-')?['--prerelease']:[]),...names.map(name=>path.join(directory,name))])
    release=await inspect()
  }
  const matches = (asset,file) => asset.size===file.bytes.length && asset.digest===`sha256:${createHash('sha256').update(file.bytes).digest('hex')}`
  if(!release || !Array.isArray(release.assets)) throw new Error('CLI_RELEASE_IMMUTABLE_CONFLICT')
  if(release.assets.some(asset => /^(?:skillshub-cli-|cli-manifest-)/.test(asset.name) && !names.includes(asset.name))) throw new Error('CLI_RELEASE_IMMUTABLE_CONFLICT')
  const missing=[]
  for(const file of files) {
    const asset=release.assets.find(asset=>asset.name===file.name)
    if(asset && !matches(asset,file)) throw new Error('CLI_RELEASE_IMMUTABLE_CONFLICT')
    if(!asset) missing.push(file)
  }
  if(missing.length) {
    if(!release.draft) throw new Error('CLI_RELEASE_IMMUTABLE_CONFLICT')
    await gh(['release','upload',tag,'--repo',repository,...missing.map(file=>path.join(directory,file.name))])
    release=await inspect()
  }
  if(!release || files.some(file=>{
    const asset=release.assets?.find(asset=>asset.name===file.name)
    return !asset || !matches(asset,file)
  })) throw new Error('CLI_RELEASE_IMMUTABLE_CONFLICT')
  if(publish && release.draft) await gh(['release','edit',tag,'--repo',repository,'--draft=false',...(tag.includes('-')?[]:['--latest'])])
}
if(process.argv[1] && import.meta.url===pathToFileURL(path.resolve(process.argv[1])).href) {
 try { await publishCliRelease({directory:process.argv[2],tag:process.env.GITHUB_REF_NAME,sourceCommit:process.env.GITHUB_SHA,publish:process.argv.includes('--publish')}); console.log('CLI resources verified in the shared desktop release.') }
 catch {console.error('CLI_RELEASE_PUBLISH_FAILED: verify release access and immutable version assets.');process.exitCode=1}
}
