const fs = require('node:fs');
const assert = require('node:assert/strict');
const { performance } = require('node:perf_hooks');
const { JSDOM, VirtualConsole } = require('jsdom');
const TARGET = process.env.SKILLS_REVIEW_HTML || require('node:path').resolve(__dirname,'../skills-hub-prototype.html');
const failures = [], passes = [];

function fresh() {
  const errors = [];
  const vc = new VirtualConsole();
  vc.on('jsdomError', e => errors.push(e.message));
  const dom = new JSDOM(fs.readFileSync(TARGET, 'utf8'), {
    runScripts: 'dangerously', pretendToBeVisual: true,
    url: 'http://localhost/skills-hub-prototype.html', virtualConsole: vc,
    beforeParse(w) { w.addEventListener('error', e => errors.push(String(e.error || e.message))); }
  });
  const w = dom.window, d = w.document;
  const a = {
    w, d, errors,
    ev: code => w.eval(code),
    state: () => JSON.parse(w.eval('JSON.stringify({S,U,projects})')),
    click(selector) { const el = d.querySelector(selector); assert(el, 'Missing control ' + selector); assert(!el.disabled, 'Disabled control ' + selector); el.click(); },
    action(name, id) { a.click(`[data-action="${name}"]${id == null ? '' : `[data-id="${id}"]`}`); },
    input(id, value) { const el = d.getElementById(id); assert(el, 'Missing input ' + id); el.value = value; el.dispatchEvent(new w.Event('input', {bubbles:true})); },
    change(id, value) { const el = d.getElementById(id); assert(el, 'Missing select ' + id); el.value = value; el.dispatchEvent(new w.Event('change', {bubbles:true})); },
    check(name, value, checked = true) { const el = d.querySelector(`input[name="${name}"][value="${value}"]`); assert(el, 'Missing checkbox ' + name + ':' + value); el.checked = checked; el.dispatchEvent(new w.Event('change', {bubbles:true})); },
    text: () => d.getElementById('main').textContent.replace(/\s+/g,' ').trim(),
    finish() { const found = errors.slice(); w.close(); assert.deepEqual(found, [], 'Unexpected runtime errors'); }
  };
  return a;
}

function scale(a) {
  a.ev(`{
    const baseSkill = {...S.skills[0]};
    for(let n=S.skills.length;n<500;n++) S.skills.push({...baseSkill,id:'scale-skill-'+n,name:'Scale skill '+n,desc:'Independent scale fixture '+n,source:'fixture/'+n});
    for(let n=projects.length;n<20;n++) projects.push({id:'scale-project-'+n,name:'Scale project '+n,path:'~/Projects/scale-'+n,tools:['Claude','Codex','Cursor']});
    for(let n=S.presets.length;n<50;n++) {const ids=S.skills.slice(n,n+10).map(s=>s.id);S.presets.push({id:'scale-preset-'+n,name:'Scale preset '+n,desc:'Independent review fixture '+n,memberIds:ids,memberTools:Object.fromEntries(ids.map(id=>[id,['Claude']])),defaultTools:['Claude'],updatedAt:'2026-10-23'});}
  }`);
}

function run(name, fn) {
  const a = fresh(), started = performance.now();
  try { fn(a); a.finish(); const ms = Math.round(performance.now()-started); passes.push({name,ms}); console.log('PASS '+name+' ('+ms+' ms)'); }
  catch (e) { try { a.w.close(); } catch {} failures.push({name,error:e.stack}); console.log('FAIL '+name+'\n'+e.stack); }
}

function applyFrontendToTwoProjects(a) {
  a.click('#navigation [data-action="nav"][data-to="presets"]');
  a.action('applyPreset','frontend');
  assert.equal(a.state().U.page,'apply');
  a.check('apply-scope','project');
  a.check('apply-project','skills-hub');
  a.check('apply-project','website');
  a.check('apply-tool-mode','uniform');
  a.check('apply-tools','Claude');
  a.action('addNext');
  assert.equal(a.state().U.page,'applyPreview');
  a.action('confirmAdd');
  assert.equal(a.state().U.page,'applyResult');
  return a.state().S.applications.slice(-2);
}

function applyOne(a,id,tool='Claude',projectId='website') {
  a.click('#navigation [data-action="nav"][data-to="presets"]');
  a.click('[data-action="presetTab"][data-tab="presets"]');
  a.action('applyPreset',id);
  a.check('apply-scope','project'); a.check('apply-project',projectId);
  a.check('apply-tool-mode','uniform'); a.check('apply-tools',tool);
  a.action('addNext'); a.action('confirmAdd');
  return a.state().S.applications.at(-1);
}

// Tests are added against the built artifact after the implementer reports it ready.
if (process.argv.includes('--contract')) {
  const a=fresh();
  console.log(JSON.stringify({target:TARGET,ui:a.state().U,nav:[...a.d.querySelectorAll('#navigation [data-action]')].map(e=>({label:e.textContent.trim(),action:e.dataset.action,to:e.dataset.to})),publicApi:Object.keys(a.w.__prototype||{}),errors:a.errors},null,2));
  a.w.close();
}

function report() {
  const out={target:TARGET,at:new Date().toISOString(),passed:passes.length,failed:failures.length,passes,failures};
  // Keep the archived report intact; exporting a fresh run is explicit.
  if (process.env.SKILLS_REVIEW_REPORT) fs.writeFileSync(process.env.SKILLS_REVIEW_REPORT,JSON.stringify(out,null,2));
  console.log(JSON.stringify({passed:out.passed,failed:out.failed}));
  if(failures.length)process.exitCode=1;
}

if (!process.argv.includes('--contract')) {
run('Profile is a permanent page, with details/editor outside modal', a=>{
  a.click('#navigation [data-action="nav"][data-to="presets"]');
  assert.equal(a.state().U.page,'presets');
  assert.equal(a.d.querySelector('#modal-root').textContent,'');
  a.action('presetDetail','frontend');
  assert.equal(a.state().U.page,'presetDetail');
  assert(a.text().includes('前端开发'));
  a.action('editPreset','frontend');
  assert.equal(a.state().U.page,'presetEdit');
  assert.equal(a.d.querySelector('#modal-root').textContent,'');
  assert(a.d.querySelector('#editor-search'));
});

run('Project save preserves each member tool assignment without cross-product expansion', a=>{
  a.ev("U.libraryScope='project'; U.project='skills-hub'; render()");
  const before=a.state().S.targets;
  const expected=Object.fromEntries([...new Set(before.filter(t=>t.projectId==='skills-hub').map(t=>t.skillId))].map(id=>[id,before.filter(t=>t.projectId==='skills-hub'&&t.skillId===id).map(t=>t.toolId).sort()]));
  a.action('saveProjectPreset');
  a.input('preset-name','Independent exact project');
  a.action('savePreset');
  const p=a.state().S.presets.find(p=>p.name==='Independent exact project');
  assert(p);
  for(const id of Object.keys(expected)) assert.deepEqual(p.memberTools[id].slice().sort(),expected[id],id+' mapping changed');
  assert.deepEqual(a.state().S.targets,before,'Saving template changed deployment');
  a.ev("projects.push({id:'independent-empty',name:'Independent empty',path:'~/Projects/independent-empty',tools:['Claude','Codex','Cursor']})");
  const planned=JSON.parse(a.ev(`JSON.stringify(candidatePlan({scope:'project',projectId:'independent-empty',skillIds:${JSON.stringify(p.memberIds)},memberTools:${JSON.stringify(p.memberTools)}}))`));
  assert(!planned.some(r=>r.skillId==='review'&&r.toolId==='Claude'),'Unexpected review -> Claude target');
  assert.equal(planned.length,before.filter(t=>t.projectId==='skills-hub').length);
});

run('50 presets / 500 skills / 20 projects are searchable and paginated', a=>{
  scale(a);
  a.click('#navigation [data-action="nav"][data-to="presets"]');
  assert.equal(a.d.querySelectorAll('[data-preset]').length,20,'Preset page unbounded or incomplete');
  a.click('[data-action="presetPage"][data-page="2"]');
  a.click('[data-action="presetPage"][data-page="3"]');
  assert.equal(a.d.querySelectorAll('[data-preset]').length,10);
  a.input('preset-search','Scale preset 49');
  assert.equal(a.d.querySelectorAll('[data-preset]').length,1);
  a.action('presetDetail','scale-preset-49');
  a.action('editPreset','scale-preset-49');
  assert(a.d.querySelector('#editor-search'));
  assert(a.d.querySelectorAll('input[name="preset-member"]').length<=20,'Editor rendered all skills');
  a.input('editor-search','Scale skill 499');
  assert.equal(a.d.querySelectorAll('input[name="preset-member"]').length,1);
  a.check('preset-member','scale-skill-499');
  a.input('editor-search','');
  assert(a.state().U.presetDraft.memberIds.includes('scale-skill-499'),'Selection lost outside search');
});

run('Preset detail return restores search, page and scroll; member drill-down returns to preset', a=>{
  scale(a);
  a.click('#navigation [data-action="nav"][data-to="presets"]');
  a.input('preset-search','Scale preset');
  a.click('[data-action="presetPage"][data-page="2"]');
  a.d.querySelector('#main').scrollTop=320;
  a.action('presetDetail','scale-preset-25');
  assert.equal(a.state().U.page,'presetDetail');
  a.action('detail','scale-skill-25');
  assert.equal(a.state().U.page,'detail');
  const back=a.d.querySelector('[data-action="detailBack"]');
  assert(back,'Skill detail lacks contextual return'); back.click();
  assert.equal(a.state().U.page,'presetDetail');
  assert.equal(a.state().U.presetId,'scale-preset-25');
  a.action('pageBack');
  assert.equal(a.state().U.page,'presets');
  assert.equal(a.state().U.presetSearch,'Scale preset');
  assert.equal(a.state().U.presetPage,2);
  assert.equal(a.d.querySelector('#main').scrollTop,320,'Preset list scroll lost');
});

run('Concrete project cards and bulk actions do not masquerade as global destructive controls', a=>{
  a.ev("U.libraryScope='project'; U.project='skills-hub'; render()");
  assert.equal(a.d.querySelectorAll('.skill-card [role="switch"]').length,0);
  assert.equal(a.d.querySelectorAll('.skill-card [data-name="移至回收站"]').length,0);
  a.action('toggleSelection');
  const dangerous=[...a.d.querySelectorAll('.bulk-selection-bar [data-action="existing"]')].filter(e=>['Skills 启停','移至回收站'].includes(e.dataset.name));
  for(const el of dangerous) assert(/全部|全局|所有部署/.test(el.textContent),'Ambiguous asset action in project bulk bar: '+el.textContent);
});

run('Two different Skills keep independent restore records and restore preserves later targets', a=>{
  a.click('#navigation [data-action="nav"][data-to="updates"]');
  a.action('epUpdatePreview','react');
  a.action('epUpdateConfirm');
  a.action('epUpdatePreview','ui');
  a.action('epUpdateConfirm');
  let s=a.state().S;
  assert(s.lastUpdates?.react,'Missing React restore record');
  assert(s.lastUpdates?.ui,'Missing UI restore record');
  const uiVersion=s.skills.find(x=>x.id==='ui').revision;
  a.ev("S.targets.push({skillId:'react',projectId:null,toolId:'Claude',status:'ok',generation:'independent-later-target',mutationSeq:0}); render()");
  const targets=a.state().S.targets;
  a.action('epUpdateRestore','react');
  a.action('epUpdateRestoreConfirm','react');
  s=a.state().S;
  assert.equal(s.skills.find(x=>x.id==='react').revision,'1.4');
  assert.equal(s.skills.find(x=>x.id==='ui').revision,uiVersion);
  assert.deepEqual(s.targets,targets,'Restoring content changed targets');
  assert(s.lastUpdates.ui,'Unrelated restore record disappeared');
});

run('Restoration refuses subsequent content edits rather than silently overwriting them', a=>{
  a.click('#navigation [data-action="nav"][data-to="updates"]');
  a.action('epUpdatePreview','react'); a.action('epUpdateConfirm');
  a.ev("skill('react').content+='\\nIndependent personal edit'; skill('react').contentSeq++; render()");
  const content=a.state().S.skills.find(s=>s.id==='react').content;
  a.action('epUpdateRestore','react');
  const confirm=a.d.querySelector('[data-action="epUpdateRestoreConfirm"]');
  assert(!confirm||confirm.disabled,'Restoration still confirms over personal edits');
  assert.equal(a.state().S.skills.find(s=>s.id==='react').content,content);
});

run('Multi-project application isolates result/retry and does not expand saved member tool mappings', a=>{
  const before=a.state().S.targets;
  const [first,second]=applyFrontendToTwoProjects(a);
  assert.equal(first.batchId,second.batchId);
  assert.equal(first.projectId,'skills-hub'); assert.equal(second.projectId,'website');
  assert.equal(first.added.length,0);
  assert.equal(second.rows.filter(r=>r.outcome==='failed').length,1);
  const firstRecord=JSON.stringify(first);
  const after=a.state().S.targets;
  a.action('retryApplication',second.id);
  const state=a.state().S;
  assert.equal(JSON.stringify(state.applications.find(x=>x.id===first.id)),firstRecord,'Retry changed completed peer record');
  assert.equal(state.applications.find(x=>x.id===second.id).status,'complete');
  assert.equal(state.targets.length,after.length+1);
  assert.deepEqual(state.targets.filter(t=>t.projectId===null),before.filter(t=>t.projectId===null),'Global deployment changed');
  assert.equal(new Set(state.targets.map(t=>[t.projectId,t.skillId,t.toolId].join('|'))).size,state.targets.length,'Duplicate targets');
});

run('All application history beyond 20 records and deleted-preset snapshots remain reachable', a=>{
  const records=applyFrontendToTwoProjects(a);
  a.ev(`{const source=S.applications[0];for(let n=3;n<=25;n++){const row=deep(source);row.id='independent-history-'+n;row.seq=n;row.batchId='independent-batch-'+n;S.applications.push(row)}}`);
  a.action('allApplicationRecords');
  assert.equal(a.state().U.presetTab,'records');
  assert.equal(a.d.querySelectorAll('[data-application]').length,20);
  a.click('[data-action="recordPage"][data-page="2"]');
  assert.equal(a.d.querySelectorAll('[data-application]').length,5);
  a.action('applicationResult',records[0].id);
  assert.equal(a.state().U.applicationId,records[0].id);
  assert(a.text().includes('前端开发'));
  a.click('#navigation [data-action="nav"][data-to="presets"]');
  a.click('[data-action="presetTab"][data-tab="presets"]');
  a.action('presetDetail','frontend');
  const deployedBeforeDelete=a.state().S.targets;
  a.action('deletePreset','frontend'); a.action('confirmDeletePreset');
  assert.deepEqual(a.state().S.targets,deployedBeforeDelete,'Deleting a preset changed deployments');
  a.click('#navigation [data-action="nav"][data-to="presets"]');
  a.click('[data-action="presetTab"][data-tab="records"]');
  a.input('records-search',records[0].id);
  assert.equal(a.d.querySelectorAll('[data-application]').length,1);
  assert(a.text().includes('预设已删除'));
  a.action('applicationResult',records[0].id);
  assert(a.text().includes('前端开发'));
  assert.deepEqual(a.state().S.applications.find(r=>r.id===records[0].id).presetSnapshot,records[0].presetSnapshot);
});

run('Project-specific removal changes only chosen shared project targets', a=>{
  a.ev("U.libraryScope='project';U.project='skills-hub';render()");
  const before=a.state().S.targets;
  a.action('manageProjectTargets','react');
  a.check('project-remove-tool','Codex');
  a.action('previewRemoveTargets');
  assert(a.d.querySelector('.dialog').textContent.includes('Cursor'));
  a.action('confirmRemoveTargets');
  const after=a.state().S.targets;
  assert.equal(after.length,before.length-2);
  assert(after.some(t=>t.projectId==='skills-hub'&&t.skillId==='react'&&t.toolId==='Claude'));
  assert.deepEqual(after.filter(t=>t.projectId!=='skills-hub'),before.filter(t=>t.projectId!=='skills-hub'));
});

run('Discarding a dirty project-derived preset returns to the actual originating project view', a=>{
  a.ev("U.libraryScope='project';U.project='skills-hub';U.search='react';render()");
  a.d.querySelector('#main').scrollTop=190;
  a.action('saveProjectPreset');
  a.input('preset-name','Unsaved independent project preset');
  a.action('pageBack');
  assert(a.d.querySelector('.dialog').textContent.includes('未保存'));
  a.action('discardEditor');
  assert.equal(a.state().U.page,'library');
  assert.equal(a.state().U.project,'skills-hub');
  assert.equal(a.state().U.search,'react');
  assert.equal(a.d.querySelector('#main').scrollTop,190);
  assert(!a.state().S.presets.some(p=>p.name==='Unsaved independent project preset'));
});

run('Tool-mode and global/project changes preserve exact saved mapping unless explicitly overridden', a=>{
  a.ev("S.presets.push({id:'independent-map',name:'Independent mapping',desc:'',memberIds:['react','test'],memberTools:{react:['Claude'],test:['Codex']},defaultTools:[]});render()");
  a.click('#navigation [data-action="nav"][data-to="presets"]');
  a.action('applyPreset','independent-map');
  a.check('apply-scope','global');
  let rows=JSON.parse(a.ev('JSON.stringify(addPlan())'));
  assert.deepEqual(rows.map(r=>[r.skillId,r.toolId]),[['react','Claude'],['test','Codex']]);
  a.check('apply-scope','project'); a.check('apply-project','website');
  rows=JSON.parse(a.ev('JSON.stringify(addPlan())'));
  assert.deepEqual(rows.map(r=>[r.skillId,r.toolId]),[['react','Claude'],['test','Codex'],['test','Cursor']]);
  a.check('apply-tool-mode','uniform'); a.check('apply-tools','Claude');
  rows=JSON.parse(a.ev('JSON.stringify(addPlan())'));
  assert.deepEqual(rows.map(r=>[r.skillId,r.toolId]),[['react','Claude'],['test','Claude']]);
  a.check('apply-tool-mode','preset');
  rows=JSON.parse(a.ev('JSON.stringify(addPlan())'));
  assert.deepEqual(rows.map(r=>[r.skillId,r.toolId]),[['react','Claude'],['test','Codex'],['test','Cursor']]);
  assert.deepEqual(a.state().S.presets.find(p=>p.id==='independent-map').memberTools,{react:['Claude'],test:['Codex']});
});

run('Disabled and deleted preset members remain visible and cannot be silently installed', a=>{
  a.ev("skill('react').enabled=false;skill('ui').deleted=true;render()");
  const before=a.state().S.targets;
  a.click('#navigation [data-action="nav"][data-to="presets"]');
  a.action('presetDetail','frontend');
  assert(a.text().includes('已停用')); assert(a.text().includes('已删除'));
  a.action('applyPreset','frontend');
  a.check('apply-scope','global');
  a.check('apply-tool-mode','uniform'); a.check('apply-tools','Claude');
  a.action('addNext');
  assert(a.text().includes('不会自动启用')); assert(a.text().includes('不会重新导入'));
  a.action('confirmAdd');
  const state=a.state().S;
  assert(!state.skills.find(s=>s.id==='react').enabled);
  assert(state.skills.find(s=>s.id==='ui').deleted);
  assert.deepEqual(state.targets.filter(t=>['react','ui'].includes(t.skillId)),before.filter(t=>['react','ui'].includes(t.skillId)));
});

run('Stale application preview requires a fresh confirmation before writing', a=>{
  a.click('#navigation [data-action="nav"][data-to="presets"]');
  a.action('applyPreset','frontend');
  a.check('apply-scope','project'); a.check('apply-project','website');
  a.check('apply-tool-mode','uniform'); a.check('apply-tools','Claude');
  a.action('addNext');
  a.ev("S.targets.push({skillId:'react',projectId:'website',toolId:'Claude',status:'ok',generation:'outside-preview',mutationSeq:0})");
  const before=a.state().S.targets;
  a.action('confirmAdd');
  assert.equal(a.state().U.page,'applyPreview');
  assert.equal(a.state().S.applications.length,0);
  assert.deepEqual(a.state().S.targets,before);
  a.action('confirmAdd');
  const record=a.state().S.applications.at(-1);
  assert.equal(record.rows.find(r=>r.skillId==='react').outcome,'existing');
});

run('Overlapping presets preserve later-used shared targets when the first application is undone', a=>{
  const first=applyOne(a,'frontend','Codex');
  const second=applyOne(a,'reviewing','Codex');
  a.action('allApplicationRecords');
  a.action('applicationResult',first.id);
  a.action('previewUndoApplication',first.id);
  assert(a.text().includes('后续应用仍使用'));
  a.action('confirmUndoApplication',first.id);
  const state=a.state().S;
  assert.equal(state.applications.find(r=>r.id===first.id).undoRemovedKeys.length,2);
  assert.equal(state.targets.filter(t=>t.projectId==='website'&&t.skillId==='test').length,2);
  assert.equal(state.targets.filter(t=>t.projectId==='website'&&t.skillId==='review').length,2);
  assert.equal(state.applications.find(r=>r.id===second.id).status,'complete');
});

run('Old application undo cannot remove a target that was deleted and recreated later', a=>{
  const first=applyOne(a,'frontend','Codex');
  a.ev("S.targets=S.targets.filter(t=>!(t.projectId==='website'&&t.skillId==='test'));S.targets.push(...['Codex','Cursor'].map(toolId=>({skillId:'test',projectId:'website',toolId,status:'ok',generation:'independent-recreated-'+toolId,mutationSeq:0})))");
  a.action('allApplicationRecords');
  a.action('applicationResult',first.id); a.action('previewUndoApplication',first.id);
  assert(a.text().includes('替换')||a.text().includes('改动'));
  a.action('confirmUndoApplication',first.id);
  assert.equal(a.state().S.targets.filter(t=>t.projectId==='website'&&t.skillId==='test'&&t.generation.startsWith('independent-recreated')).length,2);
});

run('Saving an existing preset retains the original list return context', a=>{
  const targetsBeforeEdit=a.state().S.targets;
  a.click('#navigation [data-action="nav"][data-to="presets"]');
  a.input('preset-search','前端');
  a.d.querySelector('#main').scrollTop=180;
  a.action('presetDetail','frontend');
  a.action('editPreset','frontend');
  a.input('preset-description','Independent edited description');
  a.action('savePreset');
  assert.deepEqual(a.state().S.targets,targetsBeforeEdit,'Editing a preset changed deployments');
  assert.equal(a.state().U.page,'presetDetail');
  a.action('pageBack');
  assert.equal(a.state().U.page,'presets');
  assert.equal(a.state().U.presetSearch,'前端');
  assert.equal(a.d.querySelector('#main').scrollTop,180);
});

run('Cancelling record undo preserves return from record detail to its history list', a=>{
  const rec=applyOne(a,'frontend','Codex');
  a.action('allApplicationRecords');
  a.input('records-search',rec.id);
  a.d.querySelector('#main').scrollTop=230;
  a.action('applicationResult',rec.id);
  a.action('previewUndoApplication',rec.id);
  a.action('pageBack');
  assert.equal(a.state().U.page,'applicationDetail');
  a.action('pageBack');
  assert.equal(a.state().U.page,'presets');
  assert.equal(a.state().U.presetTab,'records');
  assert.equal(a.state().U.recordsSearch,rec.id);
  assert.equal(a.d.querySelector('#main').scrollTop,230);
});

run('Confirming record undo preserves return from record detail to its history list', a=>{
  const rec=applyOne(a,'frontend','Codex');
  a.action('allApplicationRecords');
  a.action('applicationResult',rec.id);
  a.action('previewUndoApplication',rec.id);
  a.action('confirmUndoApplication',rec.id);
  assert.equal(a.state().U.page,'applicationDetail');
  a.action('pageBack');
  assert.equal(a.state().U.page,'presets');
  assert.equal(a.state().U.presetTab,'records');
});

run('A copied preset returns through its source detail to the originating filtered list', a=>{
  a.click('#navigation [data-action="nav"][data-to="presets"]');
  a.input('preset-search','前端');
  a.d.querySelector('#main').scrollTop=280;
  a.action('presetDetail','frontend');
  a.action('duplicatePreset','frontend');
  a.action('savePreset');
  assert.equal(a.state().U.page,'presetDetail');
  assert.notEqual(a.state().U.presetId,'frontend');
  a.action('pageBack');
  assert.equal(a.state().U.page,'presetDetail');
  assert.equal(a.state().U.presetId,'frontend');
  a.action('pageBack');
  assert.equal(a.state().U.page,'presets');
  assert.equal(a.state().U.presetSearch,'前端');
  assert.equal(a.d.querySelector('#main').scrollTop,280);
});

report();
}
