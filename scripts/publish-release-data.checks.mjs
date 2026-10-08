import { test } from 'node:test';
import assert from 'node:assert/strict';
import { buildPublication, publishPublication } from './lib/release-publication.mjs';
const fixture = () => ({ index: { schemaVersion:1, repository:'qufei1993/skills-hub', latest:'v1.2.3', sourceCommit:'a'.repeat(40), syncedAt:'2026-10-07T00:00:00Z', releases:[{version:'v1.2.3', notes:{zh:{path:'/changelog/zh/v1.2.3.md',summary:'说明'},en:null}, installers:[{system:'macos',architecture:'arm64'}]}]},notes:{zh:{'v1.2.3':'中文说明'},en:{}} });
test('snapshot paths are immutable and current pointer is written last',async()=>{
 const data=buildPublication(fixture()); const writes=[];
 await publishPublication(data,{put:async(key)=>writes.push(key)});
 assert.equal(writes.at(-1),'current.json'); assert.equal(writes.filter(k=>k==='current.json').length,1);
 assert.ok(writes.includes(data.pointer.snapshotPath+'index.json'));
 assert.ok(writes.includes(data.pointer.snapshotPath+'changelog/zh/v1.2.3.md'));
});
test('upload failure leaves the published pointer untouched',async()=>{
 const writes=[]; await assert.rejects(publishPublication(buildPublication(fixture()),{put:async(key)=>{writes.push(key);throw Error('offline');}}));
 assert.ok(!writes.includes('current.json'));
});
test('same content generates same snapshot identity; changed notes generate a new one',()=>{
 const a=fixture(); assert.deepEqual(buildPublication(a),buildPublication(fixture()));
 const old=buildPublication(a).pointer.snapshotPath;a.notes.zh['v1.2.3']='新说明';assert.notEqual(buildPublication(a).pointer.snapshotPath,old);
});
test('missing referenced notes or empty installers cannot be published',()=>{
 const a=fixture(); delete a.notes.zh['v1.2.3']; assert.throws(()=>buildPublication(a));
 const b=fixture(); b.index.releases[0].installers=[]; assert.throws(()=>buildPublication(b));
});
test('rollback only activates a completed matching snapshot', async () => {
 const { restorePublication } = await import('./lib/release-publication.mjs');
 const writes=[];const data=fixture();
 const storage={get:async key=>JSON.stringify(key.endsWith('index.json')?data.index:{latest:data.index.latest}),put:async(...args)=>writes.push(args)};
 assert.equal(await restorePublication('b'.repeat(64),storage),'v1.2.3');assert.equal(writes[0][0],'current.json');
 writes.length=0;
 await assert.rejects(restorePublication('b'.repeat(64),{...storage,get:async key=>key.endsWith('index.json')?JSON.stringify(data.index):JSON.stringify({latest:'v9.0.0'})}));
 assert.equal(writes.length,0);await assert.rejects(restorePublication('../collections',storage));
});
test('missing configuration fails without exposing supplied credentials', async () => {
 const { spawnSync } = await import('node:child_process');
 const secret='regression-test-secret-not-for-logs';
 const result=spawnSync(process.execPath,['scripts/publish-release-data.mjs'],{cwd:new URL('../',import.meta.url),encoding:'utf8',env:{...process.env,CLOUDFLARE_ACCOUNT_ID:'invalid',RELEASES_R2_ACCESS_KEY_ID:secret,RELEASES_R2_SECRET_ACCESS_KEY:secret}});
 assert.equal(result.status,1);assert.ok(!`${result.stdout}${result.stderr}`.includes(secret));assert.match(result.stderr,/publication failed/);
});
