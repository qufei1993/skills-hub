/* Independent behavioral checks for the next MCP interaction draft. */
const fs = require('node:fs');
const path = require('node:path');
const assert = require('node:assert/strict');
const { JSDOM, VirtualConsole } = require('jsdom');

function makeApp(hash = '') {
  // Exercise the same generated entry that readers open, not a separate assembly.
  const html = fs.readFileSync(path.join(__dirname, 'index.html'), 'utf8');
  const errors = [];
  const virtualConsole = new VirtualConsole();
  virtualConsole.on('jsdomError', error => errors.push(error.message));
  const dom = new JSDOM(html, {
    url: 'http://localhost:4178/next/' + hash,
    runScripts: 'dangerously',
    pretendToBeVisual: true,
    virtualConsole,
    beforeParse(window) {
      window.addEventListener('error', event => errors.push(event.error?.stack || event.message));
    },
  });
  const w = dom.window;
  const d = w.document;
  return {
    w, d, errors,
    eval: code => w.eval(code),
    read: () => JSON.parse(JSON.stringify(w.MCP.snapshot())),
    text: () => d.querySelector('#main').textContent.replace(/\s+/g, ' ').trim(),
    select(selector) {
      const element = d.querySelector(selector);
      assert(element, 'Missing control: ' + selector);
      return element;
    },
    click(selector) {
      const element = this.select(selector);
      assert(!element.disabled, 'Disabled control: ' + selector);
      element.click();
    },
    set(selector, value, eventName = 'input') {
      const element = this.select(selector);
      if (element.type === 'checkbox' || element.type === 'radio') element.checked = value;
      else element.value = value;
      element.dispatchEvent(new w.Event(eventName, { bubbles: true }));
    },
    close() {
      assert.deepEqual(errors, [], 'Uncaught browser errors');
      w.close();
    },
  };
}

const results = [];
function check(name, task) {
  let app;
  try {
    app = makeApp();
    task(app);
    app.close();
    results.push({ name, pass: true });
    process.stdout.write('PASS ' + name + '\n');
  } catch (error) {
    app?.w.close();
    results.push({ name, pass: false, error: error.message });
    process.stdout.write('FAIL ' + name + ': ' + error.message + '\n');
  }
}

async function checkAsync(name, task) {
  let app;
  try {
    app = makeApp();
    await task(app);
    app.close();
    results.push({ name, pass: true });
    process.stdout.write('PASS ' + name + '\n');
  } catch (error) {
    app?.w.close();
    results.push({ name, pass: false, error: error.message });
    process.stdout.write('FAIL ' + name + ': ' + error.message + '\n');
  }
}

// Test groups are intentionally driven through rendered controls.

const action = (name, extra = '') => `[data-mcp="${name}"]${extra}`;
const detail = (a, id) => a.click(action('detail', `[data-id="${id}"]`));
const nativeKey = t => [t.tool, t.projectId || 'global', t.key].join('|');
function physical(state) {
  const values = state.targets.map(t => ({ key: nativeKey(t), config: t.config }));
  for (const observation of state.observations.filter(o => !state.imported.includes(o.id))) {
    values.push({ key: nativeKey(observation), config: observation.config });
  }
  return values.sort((a, b) => a.key.localeCompare(b.key));
}
function allocate(a, id, scope, toolIds, projectIds = []) {
  a.eval("MCP.open()");
  detail(a, id);
  a.click(action('deploy'));
  a.set(`input[name="mcp-deploy-scope"][value="${scope}"]`, true, 'change');
  for (const project of projectIds) a.set(`input[name="mcp-deploy-project"][value="${project}"]`, true, 'change');
  for (const tool of toolIds) a.set(`input[name="mcp-deploy-tool"][value="${tool}"]`, true, 'change');
  a.click(action('preview'));
}
function edit(a, id, selector, value) {
  a.eval('MCP.open()');
  detail(a, id);
  a.click(action('edit'));
  a.set(selector, value);
  a.click(action('saveEdit'));
}

async function run() {
  check('01 native navigation, routes and outer review controls', a => {
    assert.equal(a.read().ui.page, 'mcp');
    assert.equal(a.d.querySelectorAll('#navigation [data-to="mcp"]').length, 1);
    for (const page of ['library', 'presets', 'mcp']) {
      a.click(`[data-action="nav"][data-to="${page}"]`);
      assert.equal(a.eval('U.page'), page);
      assert(a.w.location.hash.startsWith('#/' + page));
      assert(a.select(`#navigation [data-to="${page}"]`).classList.contains('active'));
    }
    a.click('[data-action="mcp-design"]');
    assert(a.d.querySelector('#modal-root').textContent.includes('本轮新增'));
    a.click(action('close'));
    a.click('[data-action="mcp-tour"]');
    assert.equal(a.read().ui.page, 'mcpImport');
    assert.equal(a.d.querySelectorAll('input[name="mcp-import-choice"]').length, 3);
    a.click('[data-action="reset"]');
    assert.equal(a.read().ui.page, 'mcp');
    assert.equal(a.read().state.records.length, 0);
  });

  check('02 scope and search are read-only, with contextual attention counts', a => {
    const before = a.read().state;
    a.click(action('scope', '[data-scope="global"]'));
    a.set('#mcp-status', 'attention', 'change');
    assert.equal(a.d.querySelectorAll('[data-mcp-service]').length, 0, 'Project-only issues must not appear as global issues');
    a.click(action('clearFilters'));
    a.click(action('scope', '[data-scope="project"]'));
    a.set('#mcp-project', 'website', 'change');
    a.set('#mcp-search', 'not-a-real-service');
    assert(a.text().includes('没有匹配的 MCP'));
    assert.deepEqual(a.read().state, before);
    a.click(action('clearFilters'));
    assert.equal(a.d.querySelectorAll('[data-mcp-service]').length, 4);
  });

  check('03 import starts unselected and changes management only', a => {
    const before = physical(a.read().state);
    a.click(action('import'));
    a.click(action('scan'));
    assert.equal(a.d.querySelectorAll('input[name="mcp-import-choice"]:checked').length, 0);
    assert(a.select(action('confirmImport')).disabled);
    for (const id of ['observed-docs', 'observed-team', 'observed-proxy']) {
      a.set(`input[name="mcp-import-choice"][value="${id}"]`, true, 'change');
    }
    assert(a.text().includes('另存为独立服务'));
    a.click(action('confirmImport'));
    const s = a.read().state;
    assert.deepEqual(physical(s), before);
    assert.equal(s.services.length, 6);
    assert.equal(s.imported.length, 3);
    assert.equal(s.records[0].kind, 'import');
    assert(s.records[0].rows.every(r => r.outcome === 'imported'));
    assert(s.services.some(s => s.name === '文档检索（团队）'));
    const proxy = s.services.find(s => s.key === 'local-tools');
    assert.equal(proxy.config.url, 'http://127.0.0.1:8350/mcp');
  });

  check('04 native entry collisions remain blocked after independent import', a => {
    a.click(action('import')); a.click(action('scan'));
    a.set('input[name="mcp-import-choice"][value="observed-team"]', true, 'change');
    a.click(action('confirmImport'));
    const before = physical(a.read().state);
    allocate(a, 'docs', 'project', ['Claude'], ['skills-hub']);
    assert.equal(a.read().plan.rows[0].kind, 'conflict');
    a.click(action('apply'));
    assert.deepEqual(physical(a.read().state), before);
    assert.equal(a.read().state.records[0].rows[0].outcome, 'blocked');
  });

  check('05 add validates fields, saves only the definition, and preserves secrets', a => {
    const before = physical(a.read().state);
    a.click(action('add')); a.click(action('saveEdit'));
    assert.equal(a.read().ui.page, 'mcpEdit');
    assert(a.d.querySelector('[role="alert"]'));
    a.set('#mcp-name', '演示服务');
    a.set('#mcp-key', 'demo-service');
    a.set('#mcp-env', 'API_TOKEN=secret-demo-value\nLOG_LEVEL=debug');
    a.set('#mcp-auth', 'token', 'change');
    a.set('#mcp-secret', 'private-demo-value');
    a.click(action('saveEdit'));
    assert.equal(a.read().ui.page, 'mcpDetail');
    assert.equal(a.read().state.services.length, 5);
    assert.deepEqual(physical(a.read().state), before);
    assert(!JSON.stringify(a.read()).includes('secret-demo-value'));
    assert(!JSON.stringify(a.read()).includes('private-demo-value'));
    assert(!a.d.body.textContent.includes('secret-demo-value'));
  });

  check('06 normal edit previews an update and protects unrelated targets', a => {
    const before = a.read().state;
    edit(a, 'browser', '#mcp-args', '-y\n@playwright/mcp\n--headless');
    assert.deepEqual(a.read().state.targets, before.targets, 'Saving must not deploy');
    assert(a.text().includes('有待应用的变更'));
    a.click(action('updateTarget', '[data-target="browser|Cursor|global"]'));
    assert.equal(a.read().plan.rows[0].kind, 'update');
    a.click(action('apply'));
    const after = a.read().state;
    assert.equal(after.records[0].rows[0].outcome, 'updated');
    assert.equal(after.targets.find(t => t.id === 'browser|Cursor|global').config.args, '-y\n@playwright/mcp\n--headless');
    assert.deepEqual(after.targets.filter(t => t.id !== 'browser|Cursor|global'), before.targets.filter(t => t.id !== 'browser|Cursor|global'));
    assert.equal(after.targets.find(t => t.id === 'browser|Cursor|global').lastTest, null);
  });

  check('07 a saved transport change does not change detection of the deployed target', a => {
    detail(a, 'browser'); a.click(action('edit'));
    a.set('input[name="mcp-transport"][value="http"]', true, 'change');
    a.set('#mcp-url', 'https://new-browser.example/mcp'); a.click(action('saveEdit'));
    a.click(action('check', '[data-target="browser|Cursor|global"]'));
    assert(a.text().includes('检测会启动所选本地命令'));
    assert(a.text().includes('npx -y @playwright/mcp'));
  });

  check('08 partial failure retry is scoped and repeated apply is idempotent', a => {
    allocate(a, 'docs', 'project', ['Claude', 'Codex'], ['website']);
    const beforeApply = physical(a.read().state);
    assert.equal(a.read().plan.rows.length, 2);
    assert.deepEqual(physical(a.read().state), beforeApply);
    a.click(action('apply'));
    const first = a.read().state;
    assert.equal(first.records[0].rows.filter(r => r.outcome === 'failed').length, 1);
    const success = first.targets.find(t => t.id === 'docs|Claude|website');
    a.click(action('retry'));
    const retried = a.read().state;
    assert.deepEqual(retried.targets.find(t => t.id === success.id), success);
    assert(retried.records[0].rows.every(r => r.outcome === 'added'));
    allocate(a, 'docs', 'project', ['Claude', 'Codex'], ['website']);
    assert(a.read().plan.rows.every(r => r.kind === 'existing'));
    const count = a.read().state.targets.length;
    a.click(action('apply'));
    assert.equal(a.read().state.targets.length, count);
  });

  check('09 an external modification invalidates a pending preview', a => {
    allocate(a, 'browser', 'global', ['Cursor']);
    a.eval('MCP.scenario("drift")');
    const externallyChanged = physical(a.read().state);
    a.click(action('apply'));
    assert.equal(a.read().ui.page, 'mcpPreview');
    assert(a.read().error.includes('发生了变化'));
    assert.deepEqual(physical(a.read().state), externallyChanged);
    assert.equal(a.read().state.records.length, 0);
  });

  check('10 conflict resolution preserves physical configuration and isolates the variant', a => {
    const before = physical(a.read().state);
    detail(a, 'browser');
    a.click(action('resolve', '[data-target="browser|Cursor|website"]'));
    assert(a.text().includes('--browser') && a.text().includes('chromium'));
    a.click(action('confirmResolve'));
    const state = a.read().state;
    assert.deepEqual(physical(state), before);
    assert.equal(state.services.length, 5);
    const variant = state.services.find(s => s.id.startsWith('variant-'));
    assert(variant.config.args.includes('--browser\nchromium'));
    assert.equal(state.targets.find(t => t.id === 'browser|Cursor|global').serviceId, 'browser');
    allocate(a, 'browser', 'project', ['Cursor'], ['website']);
    assert.equal(a.read().plan.rows[0].kind, 'conflict');
  });

  check('11 undo keeps external changes and reverts only safe changed targets', a => {
    edit(a, 'browser', '#mcp-args', '-y\n@playwright/mcp\n--headless');
    allocate(a, 'browser', 'global', ['Cursor', 'Claude']); a.click(action('apply'));
    const id = a.read().state.records[0].id;
    a.eval('MCP.scenario("drift")');
    const external = a.read().state.targets.find(t => t.id === 'browser|Cursor|global');
    a.click(action('record', `[data-id="${id}"]`));
    a.click(action('undo'));
    assert(a.text().includes('1 项可以撤销'));
    a.click(action('confirmUndo'));
    const state = a.read().state;
    assert.deepEqual(state.targets.find(t => t.id === external.id), external);
    assert(!state.targets.some(t => t.id === 'browser|Claude|global'));
    assert.equal(state.records[0].undo.filter(r => r.status === 'protected').length, 1);
    assert.equal(state.records[0].undo.filter(r => r.status === 'undone').length, 1);
  });

  check('12 undo respects a later operation that explicitly reused the target', a => {
    allocate(a, 'docs', 'project', ['Claude'], ['website']); a.click(action('apply'));
    const first = a.read().state.records[0].id;
    allocate(a, 'docs', 'project', ['Claude'], ['website']); a.click(action('apply'));
    a.eval(`navigate('mcpRecord', {mcpRecordId:${JSON.stringify(first)}})`);
    a.click(action('undo'));
    assert(a.text().includes('后续操作仍在使用'));
    assert(a.select(action('confirmUndo')).disabled);
  });

  check('13 connection, credential and client authorization states remain separate', a => {
    detail(a, 'design');
    assert(a.text().includes('已配置') && a.text().includes('待客户端授权'));
    a.click(action('check', '[data-target="design|Cursor|website"]'));
    a.click(action('runCheck'));
    assert.equal(a.read().state.targets.find(t => t.id === 'design|Cursor|website').lastTest.state, 'auth');
    a.click(action('authGuide')); a.click(action('confirmAuth')); a.click(action('runCheck'));
    assert.equal(a.read().state.targets.find(t => t.id === 'design|Cursor|website').lastTest.state, 'ok');
    a.eval('MCP.open()'); detail(a, 'repo'); a.click(action('check'));
    a.click(action('runCheck'));
    assert.equal(a.read().state.targets.find(t => t.id === 'repo|Codex|skills-hub').lastTest.state, 'credential');
    a.click(action('credential')); a.set('#mcp-credential-input', 'secret-review-only'); a.click(action('saveCredential'));
    assert(!JSON.stringify(a.read()).includes('secret-review-only'));
    a.click(action('runCheck'));
    const t = a.read().state.targets.find(t => t.id === 'repo|Codex|skills-hub');
    assert.equal(t.lastTest.state, 'ok'); assert.equal(t.client, 'unknown');
  });

  check('14 runtime repair is explicit and does not change target configuration', a => {
    const before = physical(a.read().state);
    a.eval('MCP.scenario("runtime")');
    assert(a.text().includes('运行环境'));
    a.click(action('runtimeGuide')); a.click(action('runtimeReady')); a.click(action('runCheck'));
    assert.equal(a.read().state.targets.find(t => t.id === 'browser|Cursor|global').lastTest.state, 'ok');
    assert.deepEqual(physical(a.read().state), before);
  });

  check('15 leaving an unsaved editor requires a real choice', a => {
    const before = a.read().state;
    a.click(action('add')); a.set('#mcp-name', '尚未保存');
    a.click('[data-action="nav"][data-to="library"]');
    assert.equal(a.read().ui.page, 'mcpEdit');
    assert(a.d.querySelector('#modal-root').textContent.includes('放弃未保存'));
    a.click(action('close'));
    assert.equal(a.select('#mcp-name').value, '尚未保存');
    a.click('[data-action="nav"][data-to="library"]'); a.click(action('discardEdit'));
    assert.equal(a.eval('U.page'), 'library');
    assert.deepEqual(a.read().state, before);
    a.click('[data-action="nav"][data-to="mcp"]'); a.click(action('add'));
    assert.equal(a.select('#mcp-name').value, '');
  });

  check('16 record snapshots survive edits and imported configuration can be released safely', a => {
    a.click(action('import')); a.click(action('scan'));
    a.set('input[name="mcp-import-choice"][value="observed-docs"]', true, 'change'); a.click(action('confirmImport'));
    const r = a.read().state.records[0];
    edit(a, 'docs', '#mcp-name', '重新命名的文档');
    assert.deepEqual(a.read().state.records[0].rows, r.rows);
    const before = physical(a.read().state);
    a.eval(`navigate('mcpRecord', {mcpRecordId:${JSON.stringify(r.id)}})`);
    a.click(action('undo')); a.click(action('confirmUndo'));
    assert.deepEqual(physical(a.read().state), before);
    assert(!a.read().state.imported.includes('observed-docs'));
  });

  check('17 undo after removing a definition never creates an invisible orphan target', a => {
    detail(a, 'docs'); a.click(action('removeTarget')); a.click(action('confirmRemove'));
    const id = a.read().state.records[0].id;
    a.eval('MCP.open()'); detail(a, 'docs'); a.click(action('deleteService')); a.click(action('confirmDeleteService'));
    a.eval(`navigate('mcpRecord', {mcpRecordId:${JSON.stringify(id)}})`);
    a.click(action('undo'));
    if (!a.select(action('confirmUndo')).disabled) a.click(action('confirmUndo'));
    const state = a.read().state;
    assert(state.targets.every(t => state.services.some(s => s.id === t.serviceId)), 'Restored targets must remain visible under a real definition');
  });

  check('18 malformed JSON field types fail visibly without losing the editor', a => {
    a.click(action('add')); a.set('#mcp-name', '类型校验'); a.set('#mcp-key', 'type-check');
    for (const input of [
      { command: 42 },
      { command: 'tool', args: [3] },
      { command: 'tool', env: { CONFIG: { nested: 'value' } } },
      { url: 'https://service.example/mcp', headers: { Authorization: 123 } },
    ]) {
      a.set('#mcp-json', JSON.stringify(input)); a.click(action('parseJson'));
      assert(a.read().error, 'Invalid type must explain the rejected field: ' + JSON.stringify(input));
      assert.equal(a.read().ui.page, 'mcpEdit');
      assert.deepEqual(a.errors, []);
    }
    assert.equal(a.read().state.services.length, 4);
  });

  check('19 JSON import preserves argument boundaries and blocks unsupported fields', a => {
    a.click(action('add')); a.set('#mcp-name', '路径验证'); a.set('#mcp-key', 'path-check');
    a.set('#mcp-json', JSON.stringify({ command: 'tool', args: ['--path', '/tmp/My Project'] })); a.click(action('parseJson'));
    a.click(action('saveEdit'));
    const args = a.read().state.services.find(s => s.key === 'path-check').config.args;
    assert(Array.isArray(args) ? args.includes('/tmp/My Project') : args.includes('"/tmp/My Project"') || args.split('\n').includes('/tmp/My Project'), 'The path must remain one argument');
    a.click(action('edit')); a.set('#mcp-json', JSON.stringify({ command: 'tool', customField: 'keep-me' })); a.click(action('parseJson'));
    assert(a.read().error.includes('customField'));
    assert.equal(a.read().ui.page, 'mcpEdit');
  });

  check('20 restoring a removed entry requires a fresh connection and client check', a => {
    detail(a, 'docs'); a.click(action('removeTarget')); a.click(action('confirmRemove'));
    const id = a.read().state.records[0].id;
    a.click(action('record', `[data-id="${id}"]`)); a.click(action('undo')); a.click(action('confirmUndo'));
    const restored = a.read().state.targets.find(t => t.id === 'docs|Claude|global');
    assert(restored);
    assert.equal(restored.lastTest, null);
    assert.equal(restored.client, 'reload');
  });

  check('21 same native key within one allocation is blocked before any write', a => {
    a.click(action('import')); a.click(action('scan'));
    a.set('input[name="mcp-import-choice"][value="observed-team"]', true, 'change');
    a.click(action('confirmImport'));
    const imported = a.read().state.services.find(s => s.key === 'docs' && s.id !== 'docs');
    const before = physical(a.read().state);
    a.eval('MCP.open()'); detail(a, 'docs'); a.click(action('deploy'));
    a.set(`input[name="mcp-deploy-service"][value="${imported.id}"]`, true, 'change');
    a.set('input[name="mcp-deploy-scope"][value="global"]', true, 'change');
    a.set('input[name="mcp-deploy-tool"][value="Codex"]', true, 'change');
    a.click(action('preview'));
    const duplicateRows = a.read().plan.rows.filter(r => r.key === 'docs' && r.tool === 'Codex' && !r.projectId);
    assert.equal(duplicateRows.length, 2);
    assert(duplicateRows.every(r => r.kind === 'conflict'), 'Both alternatives must be resolved before a native key is written');
    assert(a.text().includes('相同配置标识'));
    a.click(action('apply'));
    assert.deepEqual(physical(a.read().state), before);
    assert(a.read().state.records[0].rows.every(r => r.outcome === 'blocked'));
  });

  check('22 scanning classifies existing connections against the current saved definition', a => {
    edit(a, 'docs', '#mcp-url', 'https://different.example/mcp');
    const before = physical(a.read().state);
    a.eval('MCP.open()'); a.click(action('import')); a.click(action('scan'));
    const choice = a.select('input[name="mcp-import-choice"][value="observed-docs"]').closest('label');
    assert(choice.textContent.includes('同名配置不同'));
    assert(!choice.textContent.includes('与库中当前配置一致'));
    a.set('input[name="mcp-import-choice"][value="observed-docs"]', true, 'change');
    a.click(action('confirmImport'));
    const state = a.read().state;
    assert.deepEqual(physical(state), before, 'Importing an older native configuration must not deploy the saved update');
    assert.equal(state.services.find(s => s.id === 'docs').config.url, 'https://different.example/mcp');
    const imported = state.targets.find(t => t.observationId === 'observed-docs');
    assert(imported && imported.serviceId !== 'docs', 'Different configuration must remain independently identified');
    assert.equal(imported.config.url, 'https://docs.example/mcp');
  });

  await checkAsync('23 browser back preserves editor routing on cancel and discards only by choice', async a => {
    const settleHistory = () => new Promise(resolve => setTimeout(resolve, 35));
    const before = a.read().state;
    detail(a, 'docs'); a.click(action('edit')); a.set('#mcp-name', '尚未保存的名称');
    a.w.history.back(); await settleHistory();
    assert.equal(a.read().ui.page, 'mcpEdit');
    assert(a.select('#modal-root').textContent.includes('放弃未保存'));
    a.click(action('discardEdit'));
    assert.equal(a.read().ui.page, 'mcpDetail');
    assert.equal(a.w.location.hash, '#/mcpDetail?id=docs');
    assert.deepEqual(a.read().state, before);

    a.click(action('edit')); a.set('#mcp-name', '保留的草稿');
    const editorHash = a.w.location.hash;
    a.w.history.back(); await settleHistory(); a.click(action('close'));
    assert.equal(a.read().ui.page, 'mcpEdit');
    assert.equal(a.w.location.hash, editorHash, 'Cancel must restore the URL to the visible editor');
    assert.equal(a.select('#mcp-name').value, '保留的草稿');
    assert.equal(a.w.history.state.context.page, 'mcpEdit');
    a.click(action('cancelEdit')); a.click(action('discardEdit'));
    assert.equal(a.read().ui.page, 'mcpDetail');
    assert.equal(a.w.location.hash, '#/mcpDetail?id=docs');
    assert.deepEqual(a.read().state, before);
  });

  const failed = results.filter(r => !r.pass);
  process.stdout.write(`\n${results.length - failed.length}/${results.length} independent MCP groups passed.\n`);
  process.exitCode = failed.length ? 1 : 0;
}

if (require.main === module) run().catch(error => { process.stderr.write(error.stack + '\n'); process.exitCode = 1; });

if (require.main !== module) module.exports = { makeApp };
