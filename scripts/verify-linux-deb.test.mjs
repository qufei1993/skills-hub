import assert from 'node:assert/strict'
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync, rmSync } from 'node:fs'
import path from 'node:path'
import { tmpdir } from 'node:os'
import { spawnSync } from 'node:child_process'
const {it}=process.env.VITEST?await import('vitest'):await import('node:test')
const api=await import('./verify-linux-deb.mjs').catch(()=>({}))
const metadata={package:'skills-hub',version:'0.11.0',architecture:'amd64',depends:'libwebkit2gtk-4.1-0, libc6 (>= 2.39)'}
it('accepts both native architectures with the required glibc and WebKit dependencies',()=>{
 assert.equal(typeof api.verifyDebMetadata,'function')
 for(const architecture of ['amd64','arm64']) api.verifyDebMetadata({...metadata,architecture},{version:'0.11.0',architecture})
})
it('rejects missing or weaker glibc constraints and dependency alternatives',()=>{
 assert.equal(typeof api.verifyDebMetadata,'function')
 for(const depends of ['libwebkit2gtk-4.1-0','libwebkit2gtk-4.1-0, libc6','libwebkit2gtk-4.1-0, libc6 (>= 2.38)','libwebkit2gtk-4.1-0, libc6 (>= 2.39~rc1)','libwebkit2gtk-4.1-0, libc6 (>= 2.39) | alternate-libc','libc6 (>= 2.39)']) {
  assert.throws(()=>api.verifyDebMetadata({...metadata,depends},{version:'0.11.0',architecture:'amd64'}),/LINUX_DEB_DEPENDENCIES_INVALID/)
 }
})
it('rejects wrong package identity, architecture or version',()=>{
 assert.equal(typeof api.verifyDebMetadata,'function')
 for(const change of [{package:''},{version:'0.10.1'},{architecture:'arm64'}]) assert.throws(()=>api.verifyDebMetadata({...metadata,...change},{version:'0.11.0',architecture:'amd64'}),/LINUX_DEB_METADATA_INVALID/)
})
it('configured Debian dependencies satisfy the package baseline while retaining automatic WebKit dependencies',()=>{
 assert.equal(typeof api.verifyDebMetadata,'function')
 const config=JSON.parse(readFileSync('src-tauri/tauri.conf.json','utf8'))
 api.verifyDebMetadata({...metadata,depends:['libwebkit2gtk-4.1-0',...(config.bundle.linux?.deb?.depends ?? [])].join(', ')},{version:'0.11.0',architecture:'amd64'})
})
it('refuses system installation outside a disposable GitHub Actions Linux runner',()=>{
 const result=spawnSync('bash',['scripts/verify-linux-packages.sh','/unused','v0.11.0','x86_64-unknown-linux-gnu','--install'],{encoding:'utf8',env:{...process.env,GITHUB_ACTIONS:'false',RUNNER_OS:'Linux'}})
 assert.equal(result.status,1)
 assert.match(result.stderr,/LINUX_PACKAGE_INSTALL_CI_ONLY/)
})

const nativeDebTest=process.platform==='linux'?it:it.skip
nativeDebTest('reads actual Debian control metadata and rejects packages with insufficient dependencies',()=>{
 const root=mkdtempSync(path.join(tmpdir(),'deb-verification-'))
 try {
  for(const [index,depends] of ['libwebkit2gtk-4.1-0, libc6 (>= 2.39)','libwebkit2gtk-4.1-0','libwebkit2gtk-4.1-0, libc6 (>= 2.38)'].entries()) {
   const staging=path.join(root,`package-${index}`)
   mkdirSync(path.join(staging,'DEBIAN'),{recursive:true})
   const filename=path.join(root,`${index}.deb`)
   writeFileSync(path.join(staging,'DEBIAN/control'),`Package: skills-hub\nVersion: 0.11.0\nArchitecture: amd64\nMaintainer: Package Test <test@example.invalid>\nDescription: isolated metadata fixture\nDepends: ${depends}\n`)
   const built=spawnSync('dpkg-deb',['--build',staging,filename],{encoding:'utf8'})
   assert.equal(built.status,0,built.stderr)
   if(index===0) assert.equal(api.verifyDebPackage(filename,{version:'0.11.0',architecture:'amd64'}).depends,'libwebkit2gtk-4.1-0, libc6 (>= 2.39)')
   else assert.throws(()=>api.verifyDebPackage(filename,{version:'0.11.0',architecture:'amd64'}),/LINUX_DEB_DEPENDENCIES_INVALID/)
  }
 } finally {rmSync(root,{recursive:true,force:true})}
})
