import { test } from 'node:test';
import assert from 'node:assert/strict';
import { buildPublication } from './lib/release-publication.mjs';
import { buildPagesDirectory } from './lib/release-pages.mjs';
const fixture = () => ({ index: { schemaVersion:1, repository:'qufei1993/skills-hub', latest:'v1.2.3', releases:[{version:'v1.2.3', notes:{zh:{path:'/changelog/zh/v1.2.3.md'},en:null}, installers:[{system:'macos',architecture:'arm64'}]}]},notes:{zh:{'v1.2.3':'中文说明'},en:{}} });
const baseUrl = 'https://releases.example.com/';
function serve(files) {
 return async (url, options) => {
  assert.equal(options.headers, undefined);
  const path = new URL(url).pathname.slice(1);
  return new Response(files.get(path) ?? 'Missing', {status:files.has(path)?200:404});
 };
}
test('first deployment requires explicit bootstrap, and emits CORS/cache/404 configuration', async()=>{
 const fetchImpl=serve(new Map());
 await assert.rejects(buildPagesDirectory(fixture(),{baseUrl,fetchImpl}));
 const result=await buildPagesDirectory(fixture(),{baseUrl,fetchImpl,bootstrap:true});
 assert.ok(result.files.has('current.json'));assert.ok(result.files.has('404.html'));
 assert.match(result.files.get('_headers'),/Access-Control-Allow-Origin: \*/);
 assert.match(result.files.get('_headers'),/no-store/);assert.match(result.files.get('_headers'),/immutable/);
});
test('new deployments preserve all previously published snapshots and notes', async()=>{
 const old=await buildPagesDirectory(fixture(),{baseUrl,fetchImpl:serve(new Map()),bootstrap:true});
 const changed=fixture();changed.notes.zh['v1.2.3']='更新说明';
 const next=await buildPagesDirectory(changed,{baseUrl,fetchImpl:serve(old.files)});
 assert.notEqual(next.pointer.snapshotPath,old.pointer.snapshotPath);
 assert.equal(next.files.get(old.pointer.snapshotPath+'changelog/zh/v1.2.3.md'),'中文说明');
 assert.equal(next.files.get(next.pointer.snapshotPath+'changelog/zh/v1.2.3.md'),'更新说明');
 assert.equal(JSON.parse(next.files.get('archive.json')).snapshots.length,2);
 const repeated=await buildPagesDirectory(changed,{baseUrl,fetchImpl:serve(next.files)});
 assert.equal(JSON.parse(repeated.files.get('archive.json')).snapshots.length,2);
});
test('corrupt or incomplete history fails closed even with bootstrap enabled', async()=>{
 const old=await buildPagesDirectory(fixture(),{baseUrl,fetchImpl:serve(new Map()),bootstrap:true});
 old.files.delete(old.pointer.snapshotPath+'changelog/zh/v1.2.3.md');
 await assert.rejects(buildPagesDirectory(fixture(),{baseUrl,fetchImpl:serve(old.files),bootstrap:true}));
 const clean=await buildPagesDirectory(fixture(),{baseUrl,fetchImpl:serve(new Map()),bootstrap:true});
 clean.files.set(clean.pointer.snapshotPath+'changelog/zh/v1.2.3.md','tampered');
 await assert.rejects(buildPagesDirectory(fixture(),{baseUrl,fetchImpl:serve(clean.files)}));
});
test('unsafe archive paths, inconsistent pointers and private URL credentials are rejected',async()=>{
 for(const hash of ['../collections','x'.repeat(64)]) {
  const files=new Map([['archive.json',JSON.stringify({schemaVersion:1,snapshots:[hash]})]]);
  await assert.rejects(buildPagesDirectory(fixture(),{baseUrl,fetchImpl:serve(files)}));
 }
 const files=new Map([['archive.json',JSON.stringify({schemaVersion:1,snapshots:[]})],['current.json',JSON.stringify({schemaVersion:1,snapshotPath:'snapshots/missing/'})]]);
 await assert.rejects(buildPagesDirectory(fixture(),{baseUrl,fetchImpl:serve(files)}));
 await assert.rejects(buildPagesDirectory(fixture(),{baseUrl:'https://secret@example.com/',fetchImpl:serve(files)}));
});
test('network failures never initialize an empty replacement archive',async()=>{
 await assert.rejects(buildPagesDirectory(fixture(),{baseUrl,bootstrap:true,fetchImpl:async()=>new Response('offline',{status:503})}));
});
test('content identities are deterministic and incomplete new snapshots are rejected',()=>{
 assert.deepEqual(buildPublication(fixture()),buildPublication(fixture()));
 const data=fixture();delete data.notes.zh['v1.2.3'];assert.throws(()=>buildPublication(data));
});
test('invalid public source fails without exposing supplied credentials', async()=>{
 const {spawnSync}=await import('node:child_process');const secret='test-secret-not-for-logs';
 const result=spawnSync(process.execPath,['scripts/publish-release-data.mjs'],{cwd:new URL('../',import.meta.url),encoding:'utf8',env:{...process.env,RELEASES_BASE_URL:`https://${secret}@example.com/`,GH_TOKEN:secret}});
 assert.equal(result.status,1);assert.ok(!`${result.stdout}${result.stderr}`.includes(secret));assert.match(result.stderr,/no deployment/);
});

test('bootstrap never discards an existing pointer when archive is missing', async()=>{
 const files=new Map([['current.json',JSON.stringify({schemaVersion:1,snapshotPath:'snapshots/old/'})]]);
 await assert.rejects(buildPagesDirectory(fixture(),{baseUrl,fetchImpl:serve(files),bootstrap:true}));
});
