/* Interactive MCP design. All filesystem, authorization and connection results are simulated. */
(() => {
  'use strict';
  const clone = value => JSON.parse(JSON.stringify(value));
  const eq = (a, b) => JSON.stringify(a) === JSON.stringify(b);
  const tools = ['Claude', 'Cursor', 'Codex'];
  const routes = ['mcp', 'mcpDetail', 'mcpImport', 'mcpEdit', 'mcpDeploy', 'mcpPreview', 'mcpResult', 'mcpRecord', 'mcpUndo', 'mcpResolve', 'mcpCheck'];
  const isMcp = page => routes.includes(page);
  const B = (label, action, classes = '', attrs = '') => `<button class="button ${classes}" data-mcp="${action}" ${attrs}>${label}</button>`;
  const A = (label, action, attrs = '') => `<button class="link-button" data-mcp="${action}" ${attrs}>${label}</button>`;
  const tag = (text, tone = '') => `<span class="badge ${tone}">${esc(text)}</span>`;
  const pane = (title, body, actions = '', classes = '') => `<section class="mcp-panel ${classes}"><header class="mcp-panel-head"><h2>${title}</h2><div class="actions">${actions}</div></header><div class="mcp-panel-body">${body}</div></section>`;
  const top = (title, subtitle, actions = '') => header(`${title}<span class="new-mark" title="本轮新增 MCP">新增</span>`, subtitle, actions);
  const back = (text = 'MCP', action = 'back', attrs = '') => B(ic('ArrowLeft') + ' ' + text, action, 'ghost tiny back', attrs);
  const timestamp = () => new Date().toLocaleString('zh-CN', { month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit', hour12: false });
  const cfg = (transport, endpoint, args = '', auth = 'none', env = '') => ({ transport, command: transport === 'stdio' ? endpoint : '', args, url: transport === 'http' ? endpoint : '', auth, env });
  const targetId = (serviceId, tool, projectId) => `${serviceId}|${tool}|${projectId || 'global'}`;
  const sourcePath = (tool, projectId) => {
    const root = projectId ? (projects.find(p => p.id === projectId)?.path || '~/Projects/' + projectId) : '~';
    if (tool === 'Claude') return projectId ? `${root}/.mcp.json` : '~/.claude.json';
    if (tool === 'Codex') return `${root}/.codex/config.toml`;
    return `${root}/.cursor/mcp.json`;
  };
  function initialData() {
    const services = [
      { id: 'browser', name: '浏览器', key: 'browser', description: '打开网页、检查界面与验证交互。', icon: 'Monitor', config: cfg('stdio', 'npx', '-y\n@playwright/mcp'), revision: 1, credentialReady: true, createdFrom: '从 Cursor 导入' },
      { id: 'docs', name: '文档检索', key: 'docs', description: '检索项目依赖与工程文档。', icon: 'FileCode2', config: cfg('http', 'https://docs.example/mcp'), revision: 1, credentialReady: true, createdFrom: '手动添加' },
      { id: 'design', name: '设计资源', key: 'design', description: '读取设计稿、组件和样式信息。', icon: 'Layers', config: cfg('http', 'https://design.example/mcp', '', 'oauth'), revision: 1, credentialReady: false, createdFrom: '从项目导入' },
      { id: 'repo', name: '代码仓库', key: 'repository', description: '读取代码、变更请求与问题记录。', icon: 'GitBranch', config: cfg('http', 'https://repository.example/mcp', '', 'token'), revision: 1, credentialReady: false, createdFrom: '手动添加' }
    ];
    const target = (sid, tool, projectId, lastTest, client = 'unknown') => {
      const s = services.find(s => s.id === sid);
      return { id: targetId(sid, tool, projectId), serviceId: sid, tool, projectId, key: s.key, managed: true, enabled: true, config: clone(s.config), baseline: clone(s.config), generation: 'seed-' + targetId(sid, tool, projectId), sequence: 0, client, lastTest: lastTest ? { state: lastTest, at: '示例 · 最近一次检测', toolCount: sid === 'browser' ? 8 : 4 } : null };
    };
    const targets = [target('browser', 'Cursor', null, 'ok'), target('browser', 'Cursor', 'website', null), target('docs', 'Claude', null, 'ok'), target('design', 'Cursor', 'website', 'auth', 'auth'), target('repo', 'Codex', 'skills-hub', 'credential')];
    targets[1].config.args += '\n--browser\nchromium';
    targets[1].sequence++;
    return { services, targets, records: [], next: 1, nextTarget: 1, runtimeReady: true, failures: [], imported: [], sourceRevision: 1, observations: [
      { id: 'observed-docs', name: '文档检索', key: 'docs', description: '与库中配置一致，可关联已有服务。', icon: 'FileCode2', tool: 'Cursor', projectId: null, config: cfg('http', 'https://docs.example/mcp'), mode: 'match', serviceId: 'docs' },
      { id: 'observed-team', name: '文档检索', key: 'docs', description: '名称相同、地址不同；单独保存，不合并。', icon: 'FileCode2', tool: 'Claude', projectId: 'skills-hub', config: cfg('http', 'https://team-docs.example/mcp'), mode: 'different' },
      { id: 'observed-proxy', name: '本地工具代理', key: 'local-tools', description: '保留这一条代理连接，不展开内部服务。', icon: 'Boxes', tool: 'Codex', projectId: null, config: cfg('http', 'http://127.0.0.1:8350/mcp'), mode: 'proxy' }
    ] };
  }
  let M = initialData();
  let F = null, D = null, P = null, I = null, error = '', checkState = null, generation = 0;
  const service = id => M.services.find(s => s.id === id);
  const target = id => M.targets.find(t => t.id === id);
  const record = id => M.records.find(r => r.id === id);
  const scopeName = id => id ? projects.find(p => p.id === id)?.name || id : '全局';
  const scopeDescription = t => `${scopeName(t.projectId)} · ${toolLabel(t.tool)}`;
  const signature = t => t ? JSON.stringify([t.generation, t.sequence, t.config, t.enabled, t.managed]) : null;
  const targetsForService = id => M.targets.filter(t => t.serviceId === id);
  const scopedTargets = id => targetsForService(id).filter(t => U.mcpScope === 'all' || (U.mcpScope === 'global' ? !t.projectId : t.projectId && (U.mcpProject === 'all' || t.projectId === U.mcpProject)));
  const nativeCollision = row => M.targets.find(t => t.tool === row.tool && t.projectId === row.projectId && t.key === row.key && t.id !== row.id);
  const availableObservations = () => M.observations.filter(o => !M.imported.includes(o.id)).map(o => {
    const matched = M.services.find(s => s.key === o.key && eq(s.config, o.config));
    if (matched) return { ...o, name: matched.name, serviceId: matched.id, mode: 'match', description: '与库中当前配置一致，可关联已有服务。' };
    if (o.mode === 'proxy') return o;
    if (M.services.some(s => s.key === o.key)) return { ...o, serviceId: null, mode: 'different', description: '标识相同、当前配置不同；单独保存，不合并。' };
    return { ...o, serviceId: null, mode: 'new', description: '保存已有连接及其使用位置，原文件保持原样。' };
  });
  const importName = o => (I?.names[o.id] || (o.id === 'observed-team' ? '文档检索（团队）' : o.name + '（独立）')).trim();
  const drift = t => t.managed && !eq(t.config, t.baseline);
  const pending = t => !drift(t) && !eq(t.config, service(t.serviceId)?.config);
  const needsAttention = t => drift(t) || pending(t) || ['auth', 'credential', 'runtime', 'failed'].includes(t.lastTest?.state) || t.client === 'reload' || t.client === 'auth';
  const configLabel = t => drift(t) ? '检测到外部修改' : pending(t) ? '有待应用的变更' : !t.managed ? '未纳入管理' : '已配置';
  const testLabel = t => !t.lastTest ? '尚未检测' : ({ ok: '最近检测通过', auth: '需要客户端授权', credential: '缺少本机凭据', runtime: '找不到运行环境', failed: '连接未成功' })[t.lastTest.state];
  const clientLabel = t => ({ unknown: '使用情况未确认', reload: '需重载后验证', auth: '待客户端授权', verified: '已手动确认可用' })[t.client] || '使用情况未确认';
  const compactConfig = c => c.transport === 'stdio' ? [c.command, ...(c.args || '').split('\n').filter(Boolean).map(a => /\s/.test(a) ? JSON.stringify(a) : a)].join(' ').trim() : c.url;
  const jsonConfig = c => JSON.stringify(c.transport === 'stdio' ? { '启动命令': c.command, '命令参数': (c.args || '').split('\n').filter(Boolean), ...(c.env ? { '环境变量': c.env } : {}), ...(c.auth === 'token' ? { '凭据': '<本机凭据引用>' } : {}) } : { '连接方式': 'HTTP', '服务地址': c.url, ...(c.auth === 'token' ? { '凭据': '<本机凭据引用>' } : {}), ...(c.auth === 'oauth' ? { '授权': '在客户端完成' } : {}) }, null, 2);
  const footer = (copy, actions) => `<footer class="mcp-footer"><p>${copy}</p><div class="actions">${actions}</div></footer>`;
  const steps = active => `<div class="mcp-steps" aria-label="分配进度">${['选择目标', '预览变化', '执行结果'].map((n, i) => `${i ? '<i></i>' : ''}<span class="${active === i ? 'active' : ''}"><b>${i + 1}</b>${n}</span>`).join('')}</div>`;
  const problem = () => error ? `<div class="mcp-error" role="alert">${esc(error)}</div>` : '';
  const attr = (key, value) => `data-${key}="${esc(value)}"`;
  function ensureUI() {
    const defaults = { mcpTab: 'services', mcpSearch: '', mcpScope: 'all', mcpProject: 'all', mcpStatus: 'all', mcpId: null, mcpTargetId: null, mcpRecordId: null };
    for (const [k, v] of Object.entries(defaults)) if (U[k] === undefined) U[k] = v;
  }
  function go(page, props = {}, force = false) { error = ''; navigate(page, props, { force }); }
  function list() {
    const scoped = M.services.filter(s => {
      const ts = targetsForService(s.id);
      return U.mcpScope === 'all' || ts.some(t => U.mcpScope === 'global' ? !t.projectId : t.projectId && (U.mcpProject === 'all' || t.projectId === U.mcpProject));
    });
    const rows = scoped.filter(s => (s.name + ' ' + s.key + ' ' + s.description).toLowerCase().includes(U.mcpSearch.toLowerCase())).filter(s => U.mcpStatus !== 'attention' || scopedTargets(s.id).some(needsAttention));
    const unresolved = scoped.flatMap(s => scopedTargets(s.id)).filter(needsAttention).length;
    return top('MCP', '集中配置连接，按工具和项目分配。', B(ic('Download') + ' 导入已有', 'import') + B(ic('Plus') + ' 添加 MCP', 'add', 'primary')) +
      `<div class="mcp-tabs">${B('MCP 服务', 'tab', U.mcpTab === 'services' ? 'active' : '', 'data-tab="services"')}${B('操作记录', 'tab', U.mcpTab === 'records' ? 'active' : '', 'data-tab="records"')}</div>` +
      (U.mcpTab === 'records' ? history() : `<div class="mcp-toolbar"><div class="mcp-scope" role="group" aria-label="MCP 使用范围">${[['all', '全部'], ['global', '全局'], ['project', '项目']].map(([id, label]) => B(label, 'scope', U.mcpScope === id ? 'active' : '', `data-scope="${id}" aria-pressed="${U.mcpScope === id}"`)).join('')}</div><div class="search">${ic('Search')}<input class="field" id="mcp-search" value="${esc(U.mcpSearch)}" placeholder="搜索名称、用途或标识…" aria-label="搜索 MCP"></div>${U.mcpScope === 'project' ? `<select id="mcp-project" aria-label="筛选项目"><option value="all">所有项目</option>${projects.map(p => `<option value="${p.id}" ${U.mcpProject === p.id ? 'selected' : ''}>${esc(p.name)}</option>`).join('')}</select>` : ''}<select id="mcp-status" aria-label="筛选状态"><option value="all">全部状态</option><option value="attention" ${U.mcpStatus === 'attention' ? 'selected' : ''}>需要处理</option></select></div><div class="mcp-list-caption"><span>${rows.length} 个 MCP 服务${U.mcpScope === 'project' && U.mcpProject === 'all' ? ' · 所有项目合并去重' : ''}</span>${unresolved && U.mcpStatus !== 'attention' ? A(`${unresolved} 个使用位置需要处理`, 'attention') : U.mcpSearch || U.mcpStatus !== 'all' ? A('清除筛选', 'clearFilters') : ''}</div><section class="mcp-list" aria-label="MCP 服务列表">${rows.length ? `<div class="mcp-list-head"><span>服务</span><span>使用位置</span><span>需要关注</span><span></span></div>${rows.map(serviceRow).join('')}` : `<div class="mcp-empty">${ic('Search')}<h2>${M.services.length ? '没有匹配的 MCP' : '还没有 MCP 服务'}</h2><p>${M.services.length ? '调整搜索或范围，已有配置不会改变。' : '从已有工具导入，或添加一个新连接。'}</p>${B(M.services.length ? '清除筛选' : '导入已有', M.services.length ? 'clearFilters' : 'import')}</div>`}</section><p class="mcp-quiet-note">${ic('Info')}连接检测反映最近一次检查结果；客户端是否已加载，需要在对应工具中确认。</p>`);
  }
  function serviceRow(s) {
    const ts = scopedTargets(s.id);
    const globalTs = ts.filter(t => !t.projectId), projectTs = ts.filter(t => t.projectId);
    const attention = ts.filter(needsAttention), external = ts.filter(drift), pendingCount = ts.filter(pending);
    let message = !ts.length ? '尚未分配' : external.length ? `${external.length} 处外部修改待核对` : pendingCount.length ? `${pendingCount.length} 处待应用更新` : attention.length ? `${attention.length} 个位置待处理` : '配置已就绪';
    return `<article class="mcp-service-row" data-mcp-service="${s.id}"><div class="mcp-name"><span class="mcp-service-icon">${ic(s.icon)}</span><div class="grow">${A(esc(s.name), 'detail', attr('id', s.id))}<p>${esc(s.description)}</p><small>${s.config.transport === 'stdio' ? '本地命令' : '远程 HTTP'} · ${esc(s.key)}</small></div></div><div class="mcp-use">${globalTs.length ? `<span>全局 ${agents([...new Set(globalTs.map(t => t.tool))])}</span>` : ''}${projectTs.length ? `<span>${[...new Set(projectTs.map(t => t.projectId))].length} 个项目 ${agents([...new Set(projectTs.map(t => t.tool))])}</span>` : ''}${!ts.length ? '<span class="muted">保存后可分配使用位置</span>' : ''}</div><div class="mcp-status-text ${attention.length ? 'warn' : 'muted'}">${ic(attention.length ? 'Info' : 'Check')}${message}</div><div class="mcp-row-actions">${B('管理', 'detail', 'tiny', attr('id', s.id))}</div></article>`;
  }
  function detail() {
    const s = service(U.mcpId);
    if (!s) return missing('该服务已不在库中');
    const ts = targetsForService(s.id), warn = ts.filter(needsAttention);
    const config = `<dl><dt>连接方式</dt><dd>${s.config.transport === 'stdio' ? '本地命令 · stdio' : '远程地址 · HTTP'}</dd><dt>配置标识</dt><dd><code>${esc(s.key)}</code></dd><dt>${s.config.transport === 'stdio' ? '启动命令' : '服务地址'}</dt><dd><code>${esc(compactConfig(s.config))}</code></dd><dt>授权方式</dt><dd>${s.config.auth === 'oauth' ? '在各客户端授权' : s.config.auth === 'token' ? (s.credentialReady ? '本机凭据已保存' : '尚未补充本机凭据') : '不需要额外凭据'}</dd><dt>配置来源</dt><dd>${esc(s.createdFrom)}</dd><dt>定义版本</dt><dd>第 ${s.revision} 次保存</dd></dl><p class="mcp-quiet-note">保存定义不会修改使用位置。编辑后需预览并应用更新。</p>${s.config.auth === 'token' ? `<div class="actions" style="margin-top:14px">${B(s.credentialReady ? '更换凭据' : '补充凭据', 'credential', 'tiny', attr('id', s.id))}</div>` : ''}`;
    return back() + top(esc(s.name), esc(s.description), B('编辑配置', 'edit', '', attr('id', s.id)) + B('分配到…', 'deploy', 'primary', attr('id', s.id))) +
      `<div class="mcp-detail-layout"><div class="mcp-detail-main"><section class="mcp-panel"><header class="mcp-panel-head"><h2>使用位置 <span class="muted">${ts.length}</span></h2><span class="small muted">${warn.length ? warn.length + ' 处需要处理' : '按位置查看配置与连接'}</span></header>${ts.length ? ts.map(targetCard).join('') : `<div class="mcp-empty">${ic('Monitor')}<h2>尚未分配给工具</h2><p>选择工具和范围，预览后再写入配置。</p>${B('选择使用位置', 'deploy', 'primary', attr('id', s.id))}</div>`}</section>${pane('最近操作', serviceHistory(s.id))}</div>${pane('连接定义', config, '', 'mcp-definition')}</div>` + `<div class="mcp-footer"><p>从库中移除前，需要先处理该服务的使用位置。</p>${B('从库中移除', 'deleteService', 'ghost tiny', attr('id', s.id))}</div>`;
  }
  function targetCard(t) {
    return `<article class="mcp-target" data-mcp-target="${esc(t.id)}"><div class="mcp-target-head">${agents([t.tool])}<strong>${esc(scopeName(t.projectId))}</strong><span class="small muted">${toolLabel(t.tool)}</span><div class="actions">${B('移除配置', 'removeTarget', 'ghost tiny', attr('target', t.id))}</div></div><code class="mcp-target-path">${esc(sourcePath(t.tool, t.projectId))} → ${esc(t.key)}</code><div class="mcp-target-states"><div><small>配置</small>${tag(configLabel(t), drift(t) || pending(t) ? 'amber' : '')}${drift(t) ? '<p>当前内容与上次登记的基线不同。</p>' : ''}</div><div><small>连接检测</small>${tag(testLabel(t), t.lastTest?.state === 'ok' ? 'green' : t.lastTest ? 'amber' : '')}<p>${t.lastTest ? esc(t.lastTest.at) + (t.lastTest.state === 'ok' ? ` · 发现 ${t.lastTest.toolCount} 项工具能力` : '') : '检测时才会建立连接。'}</p></div><div><small>客户端</small><span>${clientLabel(t)}</span><p>${t.client === 'verified' ? '用户已在客户端确认；不是持续监测。' : '连接检测不代表客户端已经加载。'}</p></div></div><div class="mcp-target-foot">${drift(t) ? B('核对外部修改', 'resolve', 'tiny primary', attr('target', t.id)) : pending(t) ? B('预览更新', 'updateTarget', 'tiny primary', attr('target', t.id)) : ''}${t.lastTest?.state === 'credential' ? B('补充凭据', 'credential', 'tiny', attr('id', t.serviceId)) : ''}${t.client === 'auth' || t.lastTest?.state === 'auth' ? B('授权指引', 'authGuide', 'tiny', attr('target', t.id)) : ''}${B(t.lastTest ? '重新检测' : '检测连接', 'check', 'tiny', attr('target', t.id))}${t.client !== 'verified' && t.client !== 'auth' ? B('客户端验证', 'clientGuide', 'ghost tiny', attr('target', t.id)) : ''}</div></article>`;
  }
  function serviceHistory(id) {
    const rs = M.records.filter(r => r.rows.some(x => x.serviceId === id)).slice(0, 3);
    return rs.length ? rs.map(r => `<div class="mcp-record"><div class="grow"><strong>${esc(r.title)}</strong><p>${esc(r.at)} · ${recordSummary(r)}</p></div>${B('查看', 'record', 'tiny', attr('id', r.id))}</div>`).join('') : '<p class="muted small">还没有本次演示的操作记录。初始配置已保留。</p>';
  }
  function importPage() {
    if (!I) I = { selected: [], names: {}, scanned: false };
    const observations = availableObservations();
    return back('MCP', 'cancelImport') + top('导入已有 MCP', '先查看配置，再选择交给 Skills Hub 管理的连接。', B(ic('RefreshCw') + (I.scanned ? '重新扫描' : '扫描已有配置'), 'scan')) +
      `<div class="mcp-flow">${note('扫描只读取配置', '范围为已支持的工具和已登记项目。不会启动服务、写回文件或迁移客户端授权。')}${problem()}` +
      (!I.scanned ? `<section class="mcp-panel"><div class="mcp-empty">${ic('Download')}<h2>从已有配置开始</h2><p>本轮原型展示 Claude Code、Cursor、Codex 及 ${projects.length} 个已登记项目。</p>${B('扫描已有配置', 'scan', 'primary')}</div></section>` :
        `<div class="mcp-list-caption"><span>发现 ${observations.length} 条尚未登记的配置 · 已选 ${I.selected.length} 条</span><span class="mcp-readonly">只读扫描</span></div><div class="mcp-choices">${observations.length ? observations.map(o => `<label class="mcp-choice"><input type="checkbox" name="mcp-import-choice" value="${o.id}" ${I.selected.includes(o.id) ? 'checked' : ''}><div class="grow"><div class="row wrap"><strong>${esc(o.name)}</strong>${tag(o.mode === 'match' ? '可关联已有服务' : o.mode === 'different' ? '同名配置不同' : o.mode === 'proxy' ? '代理连接' : '新服务', o.mode === 'different' ? 'amber' : '')}</div><p>${esc(o.description)}</p><code class="mono">${esc(compactConfig(o.config))}</code><div class="mcp-source-info">${agents([o.tool])}<span>${toolLabel(o.tool)} · ${esc(scopeName(o.projectId))}</span></div><code class="mono muted">${esc(sourcePath(o.tool, o.projectId))} → ${esc(o.key)}</code>${o.mode === 'different' && I.selected.includes(o.id) ? `<input class="field" id="mcp-import-name-${o.id}" name="mcp-import-name" data-id="${o.id}" value="${esc(importName(o))}" aria-label="同名配置的独立名称"><p>另存为独立服务；客户端原标识仍为 ${esc(o.key)}。</p>` : ''}</div></label>`).join('') : `<div class="mcp-empty"><h2>扫描范围内的配置均已登记</h2><p>可以在 MCP 服务中查看使用位置。</p>${B('返回 MCP', 'back')}</div>`}</div>${footer('导入仅建立管理记录，原文件内容和已有授权保持原样。', B('取消', 'cancelImport') + B(`纳入管理${I.selected.length ? '（' + I.selected.length + '）' : ''}`, 'confirmImport', 'primary', I.selected.length ? '' : 'disabled'))}`) + '</div>';
  }
  function editor() {
    if (!F) return missing('编辑草稿已结束，请重新选择添加或编辑');
    return back('返回', 'cancelEdit') + top(F.id ? '编辑 MCP 配置' : '添加 MCP', '先保存连接定义，再选择使用位置。') +
      `<div class="mcp-form-grid"><form id="mcp-edit-form" novalidate><div class="mcp-panel"><div class="mcp-panel-body">${problem()}<div class="mcp-split"><div class="mcp-field-group"><label for="mcp-name">显示名称</label><input id="mcp-name" class="field" value="${esc(F.name)}" maxlength="60" placeholder="例如：浏览器" required></div><div class="mcp-field-group"><label for="mcp-key">配置标识</label><input id="mcp-key" class="field mono" value="${esc(F.key)}" placeholder="browser" ${F.id ? 'readonly' : ''}><small>${F.id ? '已有配置使用此标识，本轮不支持重命名。' : '用于客户端配置；字母、数字、点、短横线或下划线。'}</small></div></div><div class="mcp-field-group"><label for="mcp-description">用途说明 <span class="muted">可选</span></label><input id="mcp-description" class="field" value="${esc(F.description)}" maxlength="140" placeholder="这个连接可以帮助你做什么"></div><div class="mcp-field-group"><label>连接方式</label><div class="mcp-inline-options"><label><input type="radio" name="mcp-transport" value="stdio" ${F.config.transport === 'stdio' ? 'checked' : ''}>本地命令</label><label><input type="radio" name="mcp-transport" value="http" ${F.config.transport === 'http' ? 'checked' : ''}>远程 HTTP</label></div></div>${F.config.transport === 'stdio' ? `<div class="mcp-field-group"><label for="mcp-command">启动命令</label><input id="mcp-command" class="field mono" value="${esc(F.config.command)}" placeholder="npx、uvx 或可执行文件路径"><small>保存时不会执行命令，连接检测会单独提示。</small></div><div class="mcp-field-group"><label for="mcp-args">命令参数 <span class="muted">每行一个参数</span></label><textarea id="mcp-args" class="field mono" rows="3" placeholder="-y&#10;@playwright/mcp">${esc(F.config.args)}</textarea><small>带空格的路径放在同一行，不需要额外加引号。</small></div><div class="mcp-field-group"><label for="mcp-env">环境变量 <span class="muted">可选，每行 KEY=VALUE</span></label><textarea id="mcp-env" class="field mono" placeholder="LOG_LEVEL=info">${esc(F.config.env)}</textarea><small>敏感字段在保存时转为本机凭据引用，预览和记录中不显示原值。</small></div>` : `<div class="mcp-field-group"><label for="mcp-url">服务地址</label><input id="mcp-url" class="field mono" value="${esc(F.config.url)}" placeholder="https://service.example/mcp" inputmode="url"><small>支持 HTTP / HTTPS 地址；客户端兼容性在分配时核对。</small></div>`}<div class="mcp-field-group"><label for="mcp-auth">授权方式</label><select id="mcp-auth" class="field"><option value="none" ${F.config.auth === 'none' ? 'selected' : ''}>无需额外凭据</option><option value="token" ${F.config.auth === 'token' ? 'selected' : ''}>本机凭据</option>${F.config.transport === 'http' ? `<option value="oauth" ${F.config.auth === 'oauth' ? 'selected' : ''}>在客户端登录授权</option>` : ''}</select><small>${F.config.auth === 'oauth' ? '各客户端分别完成授权，不能将一个客户端的登录状态复制给另一个。' : F.config.auth === 'token' ? '凭据留在本机，不进入导出、操作记录或项目配置的可分享部分。' : '如服务需要授权，可稍后编辑此定义。'}</small></div>${F.config.auth === 'token' ? `<div class="mcp-field-group"><label for="mcp-secret">凭据 <span class="muted">可稍后补充</span></label><input id="mcp-secret" class="field" type="password" autocomplete="off" placeholder="${F.credentialReady ? '已有本机凭据；留空则保留' : '原型请使用演示值，不要输入真实密钥'}" value=""><small>原型只保存“已补充”状态，不保留输入值。</small></div>` : ''}<details ${F.advancedOpen ? 'open' : ''}><summary>从现有 JSON 填充</summary><div class="mcp-field-group"><label for="mcp-json">单条服务或 mcpServers 配置</label><textarea id="mcp-json" class="field mono" rows="5" placeholder='{"url":"https://service.example/mcp"}'>${esc(F.raw || '')}</textarea></div>${B('解析并填充表单', 'parseJson', 'tiny', 'type="button"')}<p class="mcp-quiet-note">多条配置需要逐条添加；未识别字段会明确提示，不静默丢弃。</p></details></div></div>${footer('仅保存定义。已有使用位置会显示待应用的变更。', B('取消', 'cancelEdit', '', 'type="button"') + B('保存定义', 'saveEdit', 'primary', 'type="submit"'))}</form><aside class="mcp-form-help"><strong>连接定义与使用位置分开</strong><p>这里保存服务地址或启动方式。</p><p>下一步选择工具和项目，预览具体变化后才写入客户端配置。</p><p>已有的个人配置会被保留；检测到外部修改时先核对差异。</p></aside></div>`;
  }
  function deployment() {
    if (!D) return missing('本次分配已结束，请重新选择服务');
    return back('返回服务', 'cancelDeploy') + top('分配 MCP', '选择要使用这些连接的工具和范围。') + `<div class="mcp-flow">${steps(0)}${problem()}${pane('选择服务', `<div class="mcp-choices mcp-deploy-services">${M.services.map(s => `<label class="mcp-choice"><input type="checkbox" name="mcp-deploy-service" value="${s.id}" ${D.serviceIds.includes(s.id) ? 'checked' : ''}><div class="grow"><strong>${esc(s.name)}</strong><p>${esc(s.description)}</p></div><span class="badge">${s.config.transport === 'stdio' ? '本地命令' : '远程 HTTP'}</span></label>`).join('')}</div>`)}<div class="section-title"><h2>使用范围</h2></div><div class="mcp-split"><label class="mcp-choice"><input type="radio" name="mcp-deploy-scope" value="global" ${D.scope === 'global' ? 'checked' : ''}><div><strong>全局</strong><p>写入所选工具的全局配置。</p></div></label><label class="mcp-choice"><input type="radio" name="mcp-deploy-scope" value="project" ${D.scope === 'project' ? 'checked' : ''}><div><strong>指定项目</strong><p>仅对所选项目添加配置。</p></div></label></div>${D.scope === 'project' ? `<div class="mcp-choices" style="margin-top:12px">${projects.map(p => `<label class="mcp-choice"><input type="checkbox" name="mcp-deploy-project" value="${p.id}" ${D.projectIds.includes(p.id) ? 'checked' : ''}><div><strong>${esc(p.name)}</strong><p class="mono">${esc(p.path)}</p></div></label>`).join('')}</div>` : ''}<div class="section-title"><h2>目标工具</h2></div><div class="mcp-inline-options">${tools.map(t => `<label><input type="checkbox" name="mcp-deploy-tool" value="${t}" ${D.tools.includes(t) ? 'checked' : ''}>${agents([t])}${toolLabel(t)}</label>`).join('')}</div><p class="mcp-quiet-note">本轮展示 3 个工具的适配流程。各客户端的授权与加载方式分别处理。</p>${footer('选择和预览不会写入，确认应用后才修改配置。', B('取消', 'cancelDeploy') + B('预览变化', 'preview', 'primary', D.serviceIds.length && D.tools.length && (D.scope === 'global' || D.projectIds.length) ? '' : 'disabled'))}</div>`;
  }
  function buildPlan() {
    const rows = [];
    for (const sid of D.serviceIds) for (const pid of D.scope === 'global' ? [null] : D.projectIds) for (const tool of D.tools) {
      const s = service(sid), id = targetId(sid, tool, pid), t = target(id);
      const unmanaged = M.observations.find(o => !M.imported.includes(o.id) && o.key === s.key && o.tool === tool && o.projectId === pid);
      const collision = nativeCollision({ id, key: s.key, tool, projectId: pid });
      let kind = !t ? 'add' : drift(t) ? 'conflict' : !t.managed ? 'conflict' : pending(t) ? 'update' : 'existing';
      if (unmanaged || collision) kind = 'conflict';
      rows.push({ id, serviceId: sid, name: s.name, key: s.key, tool, projectId: pid, path: sourcePath(tool, pid), kind, before: t ? clone(t) : null, expected: signature(t), observation: unmanaged?.id || null, collisionId: collision?.id || null, definitionRevision: s.revision, config: clone(s.config), reason: collision ? '这个位置的同名标识已属于另一条服务，保留已有配置。' : unmanaged ? '同名配置尚未纳入管理，请先导入或保留。' : drift(t || {}) ? '配置已被外部修改，请先核对差异。' : kind === 'update' ? '只更新本服务条目，其他配置保留。' : kind === 'existing' ? '目标配置与当前定义一致，无需写入。' : '新增本服务条目，其他配置保留。' });
    }
    for (const row of rows) {
      const duplicates = rows.filter(other => other.tool === row.tool && other.projectId === row.projectId && other.key === row.key && other.serviceId !== row.serviceId);
      if (duplicates.length) { row.kind = 'conflict'; row.reason = '本次选择的多条服务使用相同配置标识，请返回只保留其中一条。'; }
    }
    return { rows, sourceRevision: M.sourceRevision, at: timestamp() };
  }
  const planLabels = { add: ['新增', 'blue'], update: ['更新配置', 'blue'], existing: ['已有，保留', ''], conflict: ['冲突，跳过', 'amber'] };
  function preview() {
    if (!P || !D) return missing('预览已失效，请重新选择目标');
    const changes = P.rows.filter(r => ['add', 'update'].includes(r.kind)), blocked = P.rows.filter(r => r.kind === 'conflict'), kept = P.rows.filter(r => r.kind === 'existing');
    return back('调整选择', 'editDeploy') + top('确认配置变化', '按使用位置检查本次写入。') + `<div class="mcp-flow">${steps(1)}${problem()}<div class="mcp-list-caption"><span>${changes.length} 项写入 · ${kept.length} 项已有 · ${blocked.length} 项跳过</span><span>预览于 ${esc(P.at)}</span></div>${blocked.length ? note('冲突项将跳过', '可先完成其余项目，冲突项保留原配置；也可以返回核对后重新预览。', 'warning') : ''}${planGroups(P.rows, false)}${footer('执行前再次检查配置是否变化。写入完成后仍需在客户端加载或授权。', B('返回调整', 'editDeploy') + B(changes.length ? `确认写入 ${changes.length} 项` : '记录检查结果', 'apply', 'primary'))}</div>`;
  }
  function planGroups(rows, results) {
    const groups = [...new Set(rows.map(r => `${r.projectId || 'global'}|${r.tool}`))];
    return groups.map(key => {
      const group = rows.filter(r => `${r.projectId || 'global'}|${r.tool}` === key), first = group[0];
      return `<section class="mcp-panel mcp-plan-group"><header class="mcp-panel-head"><div><h2>${esc(scopeName(first.projectId))} · ${toolLabel(first.tool)}</h2><p class="mono">${esc(first.path || sourcePath(first.tool, first.projectId))}</p></div>${agents([first.tool])}</header>${group.map(r => {
        const status = results ? ({ added: ['已新增', 'green'], updated: ['已更新', 'green'], existing: ['已有，保留', ''], blocked: ['已跳过', 'amber'], failed: ['写入失败', 'red'], imported: ['已纳入管理', 'green'], removed: ['已移除', ''], undone: ['已撤销', ''], protected: ['保留后续修改', 'amber'] })[r.outcome] || ['已处理', ''] : planLabels[r.kind];
        return `<div class="mcp-plan-line"><div><strong>${esc(r.name)}</strong><p>${esc(results ? r.error || r.message || r.reason : r.reason)}</p><code>${esc(r.key || service(r.serviceId)?.key)} · ${esc(compactConfig(r.config || {}))}</code></div><div class="mcp-plan-status">${tag(status[0], status[1])}${!results && r.kind === 'update' ? B('查看差异', 'planDiff', 'ghost tiny', attr('id', r.id)) : ''}${r.kind === 'conflict' && target(r.id) ? B('核对变化', 'resolve', 'ghost tiny', attr('target', r.id)) : ''}</div></div>`;
      }).join('')}</section>`;
    }).join('');
  }
  function resultPage() {
    const r = record(U.mcpRecordId);
    if (!r) return missing('操作记录不存在');
    const failed = r.rows.filter(x => x.outcome === 'failed').length, writable = r.rows.some(x => ['added', 'updated', 'imported', 'removed'].includes(x.outcome));
    return back('MCP', 'back') + `<div class="mcp-result-heading ${failed ? 'has-failure' : ''}">${ic(failed ? 'Info' : 'Check')}<div><h1>${failed ? '部分配置已完成' : r.kind === 'import' ? '已纳入管理' : '本次操作已完成'}</h1><p>${recordSummary(r)} · ${esc(r.at)}</p></div></div><div class="mcp-flow">${r.kind === 'apply' ? steps(2) : ''}${problem()}${r.kind === 'import' ? note('已有文件保持原样', '仅记录来源与当前内容。未启动服务，未写回配置，原有授权保持不变。', 'success') : note('配置写入与客户端生效分开', '请在对应工具中重载并检查；需要登录的连接还需在该客户端完成授权。')}${planGroups(r.rows, true)}${footer(failed ? '已成功的项目保留；重试只处理本次未成功写入的项目。' : '记录保留本次快照，之后编辑服务定义不会改写这份记录。', B('查看操作记录', 'record', '', attr('id', r.id)) + (failed ? B(`重试 ${failed} 项失败`, 'retry', 'primary', attr('id', r.id)) : B('查看 MCP 服务', 'back', 'primary')))}${writable ? `<p class="mcp-quiet-note">需要恢复时，可在操作记录中预览撤销范围。</p>` : ''}</div>`;
  }
  function recordSummary(r) {
    const count = code => r.rows.filter(x => code.includes(x.outcome)).length;
    const parts = [];
    if (count(['added', 'updated', 'imported', 'removed'])) parts.push(count(['added', 'updated', 'imported', 'removed']) + ' 项完成');
    if (count(['existing'])) parts.push(count(['existing']) + ' 项已有');
    if (count(['failed'])) parts.push(count(['failed']) + ' 项失败');
    if (count(['blocked', 'protected'])) parts.push(count(['blocked', 'protected']) + ' 项保留');
    if (r.undo?.length) parts.push(r.undo.filter(x => x.status === 'undone').length + ' 项已撤销');
    return parts.join(' · ') || '未修改配置';
  }
  function history() {
    return `<div class="mcp-list-caption"><span>${M.records.length} 次操作 · 本页演示记录</span><span>按最近执行排序</span></div><section class="mcp-list">${M.records.length ? M.records.map(r => `<article class="mcp-record">${ic(r.kind === 'import' ? 'Download' : 'History')}<div class="grow"><strong>${esc(r.title)}</strong><p>${esc(r.at)} · ${recordSummary(r)}</p></div>${B('查看记录', 'record', 'tiny', attr('id', r.id))}</article>`).join('') : `<div class="mcp-empty">${ic('History')}<h2>还没有操作记录</h2><p>导入、分配和移除后，结果及撤销入口会保留在这里。</p>${B('查看 MCP 服务', 'tab', '', 'data-tab="services"')}</div>`}</section>`;
  }
  function recordPage() {
    const r = record(U.mcpRecordId);
    if (!r) return missing('操作记录不存在');
    const canUndo = r.rows.some(x => ['added', 'updated', 'imported', 'removed'].includes(x.outcome) && !r.undo?.some(u => u.id === x.id && u.status === 'undone'));
    return back('操作记录', 'history') + top(esc(r.title), `${esc(r.at)} · ${recordSummary(r)}`, canUndo ? B('预览撤销', 'undo', '', attr('id', r.id)) : '') + `<div class="mcp-flow">${problem()}${r.undo ? note('撤销结果已记录', `${r.undo.filter(u => u.status === 'undone').length} 项已撤销；${r.undo.filter(u => u.status === 'protected').length} 项因后续修改或使用而保留。`) : ''}${planGroups(r.rows, true)}${footer('此处展示执行时的配置快照。凭据仅显示引用，不记录原值。', r.rows.some(x => x.outcome === 'failed') ? B('重试失败项', 'retry', 'primary', attr('id', r.id)) : B('返回操作记录', 'history'))}</div>`;
  }
  function undoCandidates(r) {
    return r.rows.filter(x => ['added', 'updated', 'imported', 'removed'].includes(x.outcome)).map(row => {
      const t = target(row.id), already = r.undo?.some(u => u.id === row.id && u.status === 'undone');
      const laterUse = M.records.some(other => other.sequence > r.sequence && other.rows.some(x => x.id === row.id && ['added', 'updated', 'existing', 'imported'].includes(x.outcome) && !other.undo?.some(u => u.id === x.id && u.status === 'undone')));
      const definitionMissing = !service(row.serviceId);
      const protectedChange = definitionMissing || (row.outcome === 'removed' ? !!t || !!nativeCollision(row) || M.observations.some(o => !M.imported.includes(o.id) && o.key === row.key && o.tool === row.tool && o.projectId === row.projectId) : signature(t) !== row.afterSignature || laterUse);
      return { id: row.id, name: row.name, target: scopeDescription(row), allowed: !already && !protectedChange, already, reason: already ? '此前已撤销，无需重复处理。' : definitionMissing ? '库中定义已移除，此记录不能恢复使用位置。如需使用，请重新添加并分配。' : laterUse ? '后续操作仍在使用，保留此配置。' : protectedChange ? '此位置有后续修改或已重新创建，保留当前内容。' : row.outcome === 'imported' ? '解除本次管理登记，原配置保留。' : row.outcome === 'removed' ? '恢复本次移除的条目。' : row.outcome === 'updated' ? '恢复此条目更新前的内容。' : '移除本次新增的条目。' };
    });
  }
  function undoPage() {
    const r = record(U.mcpRecordId);
    if (!r) return missing('操作记录不存在');
    const rows = undoCandidates(r), safe = rows.filter(x => x.allowed).length;
    return back('返回记录', 'record', attr('id', r.id)) + top('撤销本次操作', '再次核对当前内容，只恢复仍安全的条目。') + `<div class="mcp-flow">${problem()}${note('保护后续修改和使用', '撤销不会回退整份配置文件；个人改动、其他操作的使用和已重新创建的条目会保留。')}<section class="mcp-panel">${rows.map(row => `<div class="mcp-plan-line"><div><strong>${esc(row.name)}</strong><p>${esc(row.target)}</p><p>${esc(row.reason)}</p></div><div class="mcp-plan-status">${tag(row.already ? '已撤销' : row.allowed ? '可以撤销' : '保留', row.allowed ? '' : 'amber')}</div></div>`).join('')}</section>${footer(`${safe} 项可以撤销；执行时会再次检查。`, B('取消', 'record', '', attr('id', r.id)) + B(`确认撤销 ${safe} 项`, 'confirmUndo', 'primary', `${attr('id', r.id)} ${safe ? '' : 'disabled'}`))}</div>`;
  }
  function resolvePage() {
    const t = target(U.mcpTargetId), s = t && service(t.serviceId);
    if (!t || !s) return missing('这个使用位置已经改变');
    return back('返回服务', 'detail', attr('id', s.id)) + top('核对外部修改', `${esc(s.name)} · ${esc(scopeName(t.projectId))} · ${toolLabel(t.tool)}`) + `<div class="mcp-flow">${problem()}${note('此位置已暂停覆盖', '当前文件与登记基线不同。先决定保留方式，再继续管理；其他使用位置不受影响。', 'warning')}<p class="mcp-quiet-note" style="margin:0 0 15px"><code>${esc(sourcePath(t.tool, t.projectId))} → ${esc(t.key)}</code></p><div class="mcp-diff"><div><h3>上次登记的配置</h3><pre>${esc(jsonConfig(t.baseline))}</pre></div><div><h3>当前文件中的配置</h3><pre>${esc(jsonConfig(t.config))}</pre></div></div><div class="section-title"><h2>如何处理这个位置</h2></div><div class="mcp-choices"><label class="mcp-choice"><input type="radio" name="mcp-resolve-mode" value="separate" checked><div><strong>保留外部修改，另存为独立服务</strong><p>只重新登记当前内容，不写回文件；其他目标继续使用原定义。</p></div></label><label class="mcp-choice"><input type="radio" name="mcp-resolve-mode" value="release"><div><strong>保留外部修改，停止管理此位置</strong><p>原文件和连接保留，以后由原工具维护。</p></div></label></div>${footer('原型不提供强制覆盖外部改动。', B('暂不处理', 'detail', '', attr('id', s.id)) + B('确认保留方式', 'confirmResolve', 'primary', attr('target', t.id)))}</div>`;
  }
  function checkPage() {
    const t = target(U.mcpTargetId), s = t && service(t.serviceId);
    if (!t || !s) return missing('这个使用位置已经改变');
    const result = checkState?.targetId === t.id ? checkState : null;
    return back('返回服务', 'detail', attr('id', s.id)) + top('检测连接', `${esc(s.name)} · ${esc(scopeName(t.projectId))} · ${toolLabel(t.tool)}`) + `<div class="mcp-flow">${problem()}${note(t.config.transport === 'stdio' ? '检测会启动所选本地命令' : '检测会连接服务地址', t.config.transport === 'stdio' ? '正式功能会短暂运行下方命令，完成握手并读取能力列表后关闭。此原型仅模拟结果，不执行命令。' : '正式功能只建立连接并读取能力列表，不调用业务工具。此原型仅模拟结果。')}<section class="mcp-panel"><div class="mcp-panel-body"><dl class="mcp-definition"><dt class="small muted">${t.config.transport === 'stdio' ? '待运行命令' : '连接地址'}</dt><dd style="margin:8px 0 0" class="mono">${esc(compactConfig(t.config))}</dd></dl><div class="mcp-test-list"><div class="mcp-test-step">${ic('FileCode2')}<div class="grow">配置检查<small>${drift(t) ? '外部修改尚未核对，检测已暂停。' : '检查本服务条目与当前目标。'}</small></div>${tag(drift(t) ? '需先核对' : '通过', drift(t) ? 'amber' : '')}</div><div class="mcp-test-step">${ic('Monitor')}<div class="grow">${t.config.transport === 'stdio' ? '运行环境与凭据' : '连接与授权'}<small>${result ? result.message : '点击检测后显示结果。'}</small></div>${tag(result ? result.state === 'ok' ? '通过' : '需处理' : '等待检测', result && result.state !== 'ok' ? 'amber' : '')}</div><div class="mcp-test-step">${ic('Boxes')}<div class="grow">握手与能力读取<small>${result?.state === 'ok' ? '演示：成功读取 ' + result.toolCount + ' 项工具能力；未执行工具。' : '前置条件通过后继续。'}</small></div>${tag(result?.state === 'ok' ? '通过' : '未完成', result?.state === 'ok' ? 'green' : '')}</div></div></div></section>${result && result.state !== 'ok' ? `<div class="actions" style="margin-top:16px">${result.state === 'credential' ? B('补充本机凭据', 'credential', '', attr('id', s.id)) : result.state === 'auth' ? B('查看客户端授权步骤', 'authGuide', '', attr('target', t.id)) : result.state === 'runtime' ? B('查看环境修复步骤', 'runtimeGuide') : B('编辑连接定义', 'edit', '', attr('id', s.id))}</div>` : ''}${footer(result?.state === 'ok' ? '检测通过仅表示这次连接成功，仍需在客户端中确认可用。' : '所有检测结果均为交互演示，不代表本机实际环境。', B('返回服务', 'detail', '', attr('id', s.id)) + B(result ? '重新检测' : '开始检测', 'runCheck', 'primary', `${attr('target', t.id)} ${drift(t) ? 'disabled' : ''}`))}</div>`;
  }
  function missing(text) { return back() + top('MCP', '') + `<div class="mcp-empty"><h2>${text}</h2><p>演示状态在刷新后重置。</p>${B('返回 MCP', 'back', 'primary')}</div>`; }

  function startEditor(id) {
    const s = service(id);
    F = s ? { id: s.id, name: s.name, key: s.key, description: s.description, config: clone(s.config), credentialReady: s.credentialReady, raw: '', advancedOpen: false } : { id: null, name: '', key: '', description: '', config: cfg('stdio', 'npx'), credentialReady: false, raw: '', advancedOpen: false };
    F.original = JSON.stringify(editorValue());
    go('mcpEdit', { mcpId: id || null });
  }
  function editorValue() { return F && { name: F.name, key: F.key, description: F.description, config: F.config, credentialReady: F.credentialReady, raw: F.raw }; }
  const dirty = () => F && JSON.stringify(editorValue()) !== F.original;
  let pendingLeave = null, pendingEditorLocation = null;
  function guardLeave(callback) {
    if (U.page !== 'mcpEdit' || !dirty()) return false;
    pendingLeave = callback;
    pendingEditorLocation = { hash: routeHash(), context: captureContext() };
    modal('放弃未保存的 MCP 配置？', '已有定义与使用位置保持原样。', '<p class="muted small">本次表单修改尚未保存，离开后会丢弃。</p>', '<span></span><div class="actions">' + B('继续编辑', 'close') + B('放弃并离开', 'discardEdit', 'danger') + '</div>');
    return true;
  }
  function cleanConfig(c) {
    const clean = clone(c);
    clean.env = (clean.env || '').split('\n').map(line => {
      const pos = line.indexOf('=');
      return pos > 0 && /(key|token|secret|password|credential|authorization)/i.test(line.slice(0, pos)) ? line.slice(0, pos + 1) + '<本机凭据引用>' : line;
    }).join('\n');
    return clean;
  }
  function saveEditor() {
    if (!F) return;
    const name = F.name.trim(), key = F.key.trim();
    if (!name) error = '请输入显示名称。';
    else if (M.services.some(s => s.id !== F.id && s.name === name)) error = '库中已有同名服务，请使用不同的显示名称。';
    else if (!/^[a-zA-Z0-9][a-zA-Z0-9_.-]*$/.test(key)) error = '配置标识需以字母或数字开头，仅包含字母、数字、点、短横线或下划线。';
    else if (!F.id && M.services.some(s => s.key === key)) error = '此配置标识已被使用，请修改标识或编辑已有服务。';
    else if (F.config.transport === 'stdio' && !F.config.command.trim()) error = '请输入启动命令。';
    else if (F.config.transport === 'http') {
      try {
        const url = new URL(F.config.url);
        if (!['http:', 'https:'].includes(url.protocol) || url.username || url.password) throw new Error();
        if ([...url.searchParams.keys()].some(k => /(token|secret|password|api.?key|authorization)/i.test(k))) { error = '请勿把密钥放进地址参数，改用本机凭据。'; }
      } catch { error = '请输入有效的 HTTP / HTTPS 地址，地址中不能嵌入用户名或密码。'; }
    }
    if (error) { render(true); $('#mcp-name')?.focus(); return; }
    const before = service(F.id), id = F.id || 'custom-' + M.next++;
    const c = cleanConfig(F.config);
    const secret = $('#mcp-secret')?.value;
    const ready = F.config.auth === 'token' ? !!secret || F.credentialReady : F.config.auth !== 'oauth';
    const s = { id, name, key, description: F.description.trim() || '尚未填写用途说明。', config: c, credentialReady: ready, icon: before?.icon || (c.transport === 'stdio' ? 'Monitor' : 'Cloud'), revision: (before?.revision || 0) + 1, createdFrom: before?.createdFrom || '手动添加' };
    if (before) M.services[M.services.indexOf(before)] = s; else M.services.push(s);
    F = null; error = ''; go('mcpDetail', { mcpId: id }, true); toast('连接定义已保存；尚未修改使用位置');
  }
  function parseJson() {
    try {
      let obj = JSON.parse(F.raw || '{}');
      if (obj.mcpServers) {
        if (Object.keys(obj).some(k => k !== 'mcpServers')) throw new Error('请只粘贴 mcpServers 字段或单条服务配置；其他客户端设置不会被导入。');
        const entries = Object.entries(obj.mcpServers);
        if (entries.length !== 1) throw new Error('每次解析一条服务配置，请先保留需要添加的一条。');
        const [key, value] = entries[0]; obj = value;
        if (!F.id) F.key = key;
        if (!F.name) F.name = key;
      }
      if (!obj || Array.isArray(obj) || typeof obj !== 'object') throw new Error('请输入单条服务对象。');
      const allowed = ['command', 'args', 'env', 'url', 'type', 'headers'];
      const unknown = Object.keys(obj).filter(k => !allowed.includes(k));
      if (unknown.length) throw new Error('包含尚未支持的字段：' + unknown.join('、') + '。请保留原配置，不会自动丢弃这些字段。');
      if (!obj.command && !obj.url) throw new Error('配置中需要 command 或 url。');
      if (obj.command && obj.url) throw new Error('请选择本地 command 或远程 url，不能同时使用。');
      if ('command' in obj && typeof obj.command !== 'string') throw new Error('command 必须是字符串。');
      if ('url' in obj && typeof obj.url !== 'string') throw new Error('url 必须是字符串。');
      if ('args' in obj && (!Array.isArray(obj.args) || obj.args.some(v => typeof v !== 'string' || !v || /[\r\n]/.test(v)))) throw new Error('args 必须是非空字符串组成的数组，每项为一个参数，不能包含换行。');
      if (obj.type && !['http', 'stdio', 'streamable-http'].includes(obj.type)) throw new Error('此连接类型尚未在本轮适配，请保留原配置。');
      if ((obj.type === 'stdio' && obj.url) || (['http', 'streamable-http'].includes(obj.type) && obj.command)) throw new Error('type 与 command / url 不一致，请核对连接方式。');
      const headers = obj.headers === undefined ? {} : obj.headers;
      if (!headers || typeof headers !== 'object' || Array.isArray(headers) || Object.values(headers).some(v => typeof v !== 'string')) throw new Error('headers 必须是字符串键值对象。');
      const unsupportedHeaders = Object.keys(headers).filter(k => !/^(authorization|x-api-key)$/i.test(k));
      if (unsupportedHeaders.length) throw new Error('自定义请求头尚未适配：' + unsupportedHeaders.join('、') + '。不会静默丢弃。');
      const env = obj.env === undefined ? {} : obj.env;
      if (!env || typeof env !== 'object' || Array.isArray(env) || Object.values(env).some(v => typeof v !== 'string' || /[\r\n]/.test(v)) || Object.keys(env).some(k => !/^[A-Za-z_][A-Za-z0-9_]*$/.test(k))) throw new Error('env 必须是字符串键值对象，使用有效变量名，变量值不能包含换行。');
      const secretEnv = Object.keys(env).some(k => /(key|token|secret|password|credential|authorization)/i.test(k));
      F.config = cleanConfig(cfg(obj.url ? 'http' : 'stdio', obj.url || obj.command, Array.isArray(obj.args) ? obj.args.join('\n') : '', Object.keys(headers).length || secretEnv ? 'token' : 'none', Object.entries(env).map(([k,v]) => `${k}=${v}`).join('\n')));
      F.credentialReady = Object.keys(headers).length > 0 || secretEnv;
      F.raw = ''; F.advancedOpen = false; error = ''; render(true); toast('已填充表单；敏感值已转为凭据状态，不保留原值');
    } catch (e) { error = e instanceof SyntaxError ? 'JSON 格式不正确，请检查引号、逗号和括号。' : e.message; F.advancedOpen = true; render(true); }
  }
  function startDeploy(id, onlyTarget = null) {
    D = { serviceIds: id ? [id] : [], scope: onlyTarget?.projectId ? 'project' : 'global', projectIds: onlyTarget?.projectId ? [onlyTarget.projectId] : [], tools: onlyTarget ? [onlyTarget.tool] : [], returnId: id || null };
    P = null; go('mcpDeploy', { mcpId: id || null });
  }
  function newRecord(kind, title, rows) {
    const sequence = M.next++;
    const r = { id: 'mcp-operation-' + sequence, sequence, kind, title, at: timestamp(), rows, undo: null };
    M.records.unshift(r); return r;
  }
  function executeRow(row, retry = false) {
    const t = target(row.id), s = service(row.serviceId);
    if (!s || s.revision !== row.definitionRevision) { row.outcome = 'blocked'; row.error = '连接定义已改变，请重新预览。'; return; }
    if (signature(t) !== row.expected) { row.outcome = 'blocked'; row.error = '目标在预览后发生变化，已保留当前内容。'; return; }
    if (nativeCollision(row)) { row.outcome = 'blocked'; row.error = '该位置的配置标识已被其他服务使用，已保留。'; return; }
    if (row.kind === 'conflict') { row.outcome = 'blocked'; row.error = row.reason; return; }
    if (row.kind === 'existing') { row.outcome = 'existing'; row.afterSignature = signature(t); return; }
    if (row.tool === 'Codex' && row.projectId === 'website' && !M.failures.includes(row.id)) {
      M.failures.push(row.id); row.outcome = 'failed'; row.error = '演示：配置文件暂时被占用，本条未写入。可重试。'; return;
    }
    const newTarget = t ? { ...t, config: clone(row.config), baseline: clone(row.config), sequence: t.sequence + 1, lastTest: null, client: row.config.auth === 'oauth' ? 'auth' : 'reload' } : { id: row.id, serviceId: row.serviceId, tool: row.tool, projectId: row.projectId, key: row.key, managed: true, enabled: true, config: clone(row.config), baseline: clone(row.config), generation: 'mcp-created-' + M.nextTarget++, sequence: 0, lastTest: null, client: row.config.auth === 'oauth' ? 'auth' : 'reload' };
    if (t) M.targets[M.targets.indexOf(t)] = newTarget; else M.targets.push(newTarget);
    row.outcome = t ? 'updated' : 'added'; row.afterSignature = signature(newTarget); row.error = null;
    row.message = newTarget.client === 'auth' ? '配置已写入，请在该客户端完成授权。' : '配置已写入，需在该客户端重载并验证。';
  }
  function confirmApply() {
    if (!P) return;
    if (P.sourceRevision !== M.sourceRevision || P.rows.some(row => signature(target(row.id)) !== row.expected || service(row.serviceId)?.revision !== row.definitionRevision)) {
      error = '预览后配置发生了变化，本次没有写入。请返回调整并重新预览。'; render(true); return;
    }
    const rows = clone(P.rows), r = newRecord('apply', `分配 ${new Set(rows.map(x => x.serviceId)).size} 个 MCP`, rows);
    for (const row of rows) executeRow(row);
    P = null; go('mcpResult', { mcpRecordId: r.id }, true);
  }
  function confirmImport() {
    if (!I?.selected.length) return;
    const candidates = availableObservations().filter(o => I.selected.includes(o.id));
    const newNames = candidates.filter(o => o.mode !== 'match').map(o => o.mode === 'different' ? importName(o) : o.name);
    if (newNames.some(n => !n || M.services.some(s => s.name === n)) || new Set(newNames).size !== newNames.length) { error = '独立服务名称不能为空或与库中已有名称重复。'; render(true); return; }
    const rows = [];
    for (const o of candidates) {
      let s = o.mode === 'match' ? service(o.serviceId) : null;
      if (!s) {
        s = { id: 'imported-' + M.next++, name: o.mode === 'different' ? importName(o) : o.name, key: o.key, description: o.mode === 'proxy' ? '保留已有代理连接，由原管理工具维护内部服务。' : o.description, icon: o.icon, config: clone(o.config), revision: 1, credentialReady: o.config.auth === 'none', createdFrom: `从 ${toolLabel(o.tool)} 导入` };
        M.services.push(s);
      }
      const id = targetId(s.id, o.tool, o.projectId);
      const t = { id, serviceId: s.id, tool: o.tool, projectId: o.projectId, key: o.key, managed: true, enabled: true, config: clone(o.config), baseline: clone(o.config), generation: 'mcp-imported-' + M.nextTarget++, sequence: 0, lastTest: null, client: o.config.auth === 'oauth' ? 'auth' : 'unknown', observationId: o.id };
      M.targets.push(t); M.imported.push(o.id);
      rows.push({ id, serviceId: s.id, name: s.name, key: s.key, tool: o.tool, projectId: o.projectId, path: sourcePath(o.tool, o.projectId), outcome: 'imported', config: clone(o.config), before: null, afterSignature: signature(t), observationId: o.id, message: '仅建立管理记录；源配置内容未改变。' });
    }
    M.sourceRevision++; const r = newRecord('import', `导入 ${rows.length} 条已有配置`, rows); I = null; go('mcpResult', { mcpRecordId: r.id }, true);
  }
  function retryRecord(id) {
    const r = record(id); if (!r) return;
    for (const row of r.rows.filter(x => x.outcome === 'failed')) executeRow(row, true);
    go('mcpResult', { mcpRecordId: id }, true); toast('已重新处理失败项，其他结果保持原样');
  }
  function confirmUndo(id) {
    const r = record(id); if (!r) return;
    const candidates = undoCandidates(r), previous = r.undo || [];
    const results = candidates.map(c => {
      if (c.already) return previous.find(x => x.id === c.id);
      if (!c.allowed) return { id: c.id, status: 'protected', reason: c.reason };
      const row = r.rows.find(x => x.id === c.id), t = target(c.id);
      if (row.outcome === 'added') M.targets = M.targets.filter(t => t.id !== c.id);
      else if (row.outcome === 'updated') M.targets[M.targets.indexOf(t)] = { ...clone(row.before), sequence: (t?.sequence || 0) + 1 };
      else if (row.outcome === 'imported') { M.targets = M.targets.filter(t => t.id !== c.id); M.imported = M.imported.filter(x => x !== row.observationId); M.sourceRevision++; }
      else if (row.outcome === 'removed') M.targets.push({ ...clone(row.before), generation: 'mcp-restored-' + M.nextTarget++, sequence: 0, lastTest: null, client: row.before.config.auth === 'oauth' ? 'auth' : 'reload' });
      return { id: c.id, status: 'undone', at: timestamp() };
    });
    r.undo = results; go('mcpRecord', { mcpRecordId: id }, true); toast('撤销结果已保存');
  }
  function removeTarget(id) {
    const t = target(id); if (!t) return;
    if (drift(t)) { go('mcpResolve', { mcpTargetId: id, mcpId: t.serviceId }); return; }
    const s = service(t.serviceId), expected = signature(t);
    pendingRemoval = { id, expected };
    modal('移除此使用位置的 MCP 配置？', `${s.name} · ${scopeDescription(t)}`, `<p class="small muted">仅删除以下条目。库中定义、其他工具和项目保留。</p><p class="mono" style="margin-top:12px">${esc(sourcePath(t.tool, t.projectId))} → ${esc(t.key)}</p>`, '<span></span><div class="actions">' + B('取消', 'close') + B('确认移除此位置', 'confirmRemove', 'danger', attr('target', id)) + '</div>');
  }
  let pendingRemoval = null;
  function confirmRemove(id) {
    const t = target(id), s = t && service(t.serviceId);
    if (!t || signature(t) !== pendingRemoval?.expected) { closeModal(); error = '确认期间配置已变化，请核对后重试。'; render(); return; }
    const row = { id, serviceId: t.serviceId, name: s.name, key: t.key, tool: t.tool, projectId: t.projectId, path: sourcePath(t.tool, t.projectId), outcome: 'removed', config: clone(t.config), before: clone(t), afterSignature: null, message: '仅移除此使用位置；库中定义和其他位置保留。' };
    M.targets = M.targets.filter(x => x.id !== id); const r = newRecord('remove', `移除 ${s.name} 的一个使用位置`, [row]); pendingRemoval = null; closeModal(); go('mcpResult', { mcpRecordId: r.id }, true);
  }
  function showCredential(id) {
    const s = service(id); if (!s) return;
    modal('补充本机凭据', s.name, `<label class="form-label" for="mcp-credential-input">访问凭据</label><input id="mcp-credential-input" class="field" type="password" autocomplete="off" placeholder="请输入演示值，不要使用真实密钥"><p class="mcp-quiet-note">原型仅保存“凭据已补充”状态，不保存输入内容。正式产品使用本机安全存储。</p>`, '<span></span><div class="actions">' + B('取消', 'close') + B('保存凭据', 'saveCredential', 'primary', attr('id', id)) + '</div>');
  }
  function nativeGuide(t, auth) {
    modal(auth ? '在客户端完成授权' : '在客户端确认可用', `${service(t.serviceId).name} · ${scopeDescription(t)}`, `<div class="mcp-native-guide">${auth ? '授权由所选客户端维护，各工具分别登录。' : '配置写入后，确认当前客户端已经加载。'}<ol><li>打开 ${toolLabel(t.tool)}${t.projectId ? ' 中的「' + esc(scopeName(t.projectId)) + '」项目' : ''}。</li><li>${auth ? '进入 MCP 连接设置，选择此服务并完成登录。' : '重载 MCP 配置或重新打开会话。'}</li><li>确认服务及工具能力出现在客户端中。</li></ol></div><p class="mcp-quiet-note">这是交互演示；下方按钮仅模拟你已完成客户端操作，不会实际打开软件或登录。</p>`, '<span></span><div class="actions">' + B('稍后处理', 'close') + B(auth ? '演示：已完成授权' : '演示：已确认可用', auth ? 'confirmAuth' : 'confirmClient', 'primary', attr('target', t.id)) + '</div>');
  }
  function runCheck(id) {
    const t = target(id), s = t && service(t.serviceId); if (!s) return;
    if (drift(t)) { error = '先核对外部修改，再检测连接。'; render(true); return; }
    let state = 'ok', message = '演示连接成功，协议握手与能力读取通过。';
    if (t.config.transport === 'stdio' && !M.runtimeReady) { state = 'runtime'; message = '本机尚未找到启动命令所需的运行环境。'; }
    else if (t.config.auth === 'token' && !s.credentialReady) { state = 'credential'; message = '缺少本机凭据，尚未连接服务。'; }
    else if (t.config.auth === 'oauth' && t.client === 'auth') { state = 'auth'; message = '需要在对应客户端完成授权。'; }
    const toolCount = s.id === 'browser' ? 8 : 4;
    checkState = { targetId: id, state, message, toolCount, at: timestamp() };
    t.lastTest = { state, at: checkState.at, toolCount }; render(true);
  }
  function confirmResolve(id) {
    const t = target(id); if (!t) return;
    const mode = $('input[name="mcp-resolve-mode"]:checked')?.value || 'separate', s = service(t.serviceId);
    const observation = { id: 'observed-released-' + M.next++, name: s.name, key: t.key, description: '已保留外部修改，可按需重新导入。', icon: s.icon, tool: t.tool, projectId: t.projectId, config: clone(t.config), mode: 'different' };
    if (mode === 'release') {
      M.observations.push(observation); M.targets = M.targets.filter(x => x.id !== t.id); M.sourceRevision++;
      go('mcpDetail', { mcpId: s.id }, true); toast('已停止管理此位置，文件内容保留'); return;
    }
    const idNew = 'variant-' + M.next++;
    let name = `${s.name}（${scopeName(t.projectId)}）`, n = 2;
    while (M.services.some(x => x.name === name)) name = `${s.name}（${scopeName(t.projectId)} ${n++}）`;
    const next = { ...clone(s), id: idNew, name, config: clone(t.config), revision: 1, createdFrom: '保留外部修改后独立登记' };
    M.services.push(next); t.serviceId = idNew; t.id = targetId(idNew, t.tool, t.projectId); t.baseline = clone(t.config); t.sequence++; t.lastTest = null; t.client = t.config.auth === 'oauth' ? 'auth' : 'unknown';
    go('mcpDetail', { mcpId: idNew }, true); toast('已独立登记当前配置；没有写回文件');
  }
  function showDesign() {
    modal('本轮新增：MCP 管理', '1.1.0 的 Skills 与技能预设保持原设计。', `<div class="mcp-choices"><div class="mcp-choice"><div><strong>一个常驻 MCP 工作区</strong><p>服务列表、详情、编辑、导入、分配、记录均在主内容区完成。</p></div></div><div class="mcp-choice"><div><strong>先看范围，再写入</strong><p>导入只登记；保存定义不部署；分配预览逐工具和项目列出变化。</p></div></div><div class="mcp-choice"><div><strong>能解释状态，也能恢复</strong><p>配置、最近检测、客户端加载分开；冲突保留外改，失败续作，撤销保护后续操作。</p></div></div></div><p class="mcp-quiet-note">所有数据与操作均为模拟。Skills 与 MCP 的组合预设留待基础流程验收后接入，本轮不改技能预设语义。</p>`, '<span></span><div class="actions">' + B('关闭', 'close') + B('从导入开始体验', 'tour', 'primary') + '</div>', true);
  }
  function review() {
    $('#review-rail').innerHTML = `<div class="rail-title"><span>设计说明 · 产品外侧</span>${btn(ic('X'), 'closeReview', 'ghost icon', 'aria-label="关闭说明"')}</div><h2>本轮增加 MCP</h2><p class="rail-intro">沿用 228px 侧栏、蓝色强调、原有列表和项目范围。新增工作区放在技能预设之后。</p><div class="review-item"><strong>核心任务</strong><p>导入已有 → 分配目标 → 看清结果 → 就地修复或撤销。</p></div><div class="review-item"><strong>共存规则</strong><p>仅管理明确选择的条目。检测到外部修改后暂停覆盖；已有代理作为一条连接保留。</p></div><div class="review-item"><strong>三种状态分开</strong><p>配置写入、最近连接检测、客户端加载与授权，分别展示。</p></div><p class="implementation-note">本页使用模拟数据，不运行本机命令，不读取真实密钥，不修改客户端文件。</p><p class="mcp-review-label">演示状态</p><div class="mcp-review-actions">${B('模拟一次外部修改', 'scenarioDrift', 'tiny')}${B('模拟缺少运行环境', 'scenarioRuntime', 'tiny')}${B('恢复初始演示数据', 'reset', 'tiny')}</div><div class="rail-bottom"><h3>与现有页面的关系</h3><p>Skills、技能预设和更新属于 1.1.0 基线。本轮不新增规则中心、代理、进程托管或在线分享。</p>${B('查看本次改动', 'design', 'ghost tiny')}</div>`;
  }
  function reset() {
    generation++; M = initialData(); F = D = P = I = null; error = ''; checkState = null;
    U.mcpTab = 'services'; U.mcpSearch = ''; U.mcpScope = 'all'; U.mcpProject = 'all'; U.mcpStatus = 'all'; U.mcpId = null; U.mcpTargetId = null; U.mcpRecordId = null;
    closeModal(); go('mcp', {}, true);
  }
  function scenario(name) {
    if (name === 'runtime') { M.runtimeReady = false; const t = M.targets.find(t => t.serviceId === 'browser' && !drift(t)); if (t) { t.lastTest = { state: 'runtime', at: timestamp(), toolCount: 0 }; go('mcpCheck', { mcpTargetId: t.id, mcpId: t.serviceId }, true); checkState = { targetId: t.id, state: 'runtime', message: '演示：未找到 Node.js / npx 运行环境。', toolCount: 0 }; render(); } return; }
    const chosen = P?.rows.find(r => ['update', 'existing'].includes(r.kind)) || (P?.rows.find(r => r.kind === 'add'));
    let t = chosen ? target(chosen.id) : M.targets.find(t => t.serviceId === 'browser' && !t.projectId);
    if (!t && chosen) {
      t = { id: chosen.id, serviceId: chosen.serviceId, tool: chosen.tool, projectId: chosen.projectId, key: chosen.key, managed: false, enabled: true, config: clone(chosen.config), baseline: clone(chosen.config), generation: 'external-' + M.nextTarget++, sequence: 0, client: 'unknown', lastTest: null }; M.targets.push(t);
    }
    if (t) { if (t.config.transport === 'stdio') t.config.args += '\n--timeout\n12000'; else t.config.url = t.config.url.replace(/\/?$/, '/external'); t.sequence++; }
    render(true); toast('已模拟外部修改；原型不会自动覆盖');
  }

  const baseRender = render, baseNav = nav, baseRail = reviewRail, baseAskLeave = askLeave, baseReadHash = readHash, baseRouteHash = routeHash, baseCloseModal = closeModal;
  routes.forEach(page => { if (!routePages.includes(page)) routePages.push(page); });
  ['mcpTab', 'mcpSearch', 'mcpScope', 'mcpProject', 'mcpStatus', 'mcpId', 'mcpTargetId', 'mcpRecordId'].forEach(key => { if (!pageFields.includes(key)) pageFields.push(key); });
  nav = function () {
    baseNav();
    const anchor = $('#navigation [data-to="presets"]');
    anchor?.querySelector('.new-mark')?.remove();
    anchor?.insertAdjacentHTML('afterend', `<button class="nav-button ${isMcp(U.page) ? 'active' : ''}" data-action="nav" data-to="mcp" title="MCP" ${isMcp(U.page) ? 'aria-current="page"' : ''}>${ic('Boxes')}<span>MCP <span class="new-mark">新增</span></span></button>`);
    if (isMcp(U.page)) $('#navigation [data-to="presets"]')?.classList.remove('active');
  };
  reviewRail = function () { if (isMcp(U.page)) review(); else { baseRail(); $('#review-rail').insertAdjacentHTML('afterbegin', '<p class="implementation-note">此页为 1.1.0 已确认基线，本轮不重新设计。新增内容请进入 MCP。</p>'); } };
  askLeave = function (callback) { if (!guardLeave(callback)) baseAskLeave(callback); };
  closeModal = function () {
    if (pendingEditorLocation && U.modal === '放弃未保存的 MCP 配置？') {
      const location = pendingEditorLocation;
      pendingEditorLocation = null; pendingLeave = null;
      try { window.history.replaceState({ context: location.context }, '', location.hash); } catch {}
    }
    baseCloseModal();
  };
  readHash = function () {
    const ctx = baseReadHash(); if (!ctx || !isMcp(ctx.page)) return ctx;
    const params = new URLSearchParams(location.hash.split('?')[1] || '');
    ctx.mcpId = params.get('id'); ctx.mcpTargetId = params.get('target'); ctx.mcpRecordId = params.get('record');
    ctx.mcpTab = params.get('tab') || 'services'; return ctx;
  };
  routeHash = function () {
    if (!isMcp(U.page)) return baseRouteHash();
    const p = new URLSearchParams();
    if (U.mcpId && U.page !== 'mcp') p.set('id', U.mcpId);
    if (U.mcpTargetId && ['mcpResolve', 'mcpCheck'].includes(U.page)) p.set('target', U.mcpTargetId);
    if (U.mcpRecordId && ['mcpResult', 'mcpRecord', 'mcpUndo'].includes(U.page)) p.set('record', U.mcpRecordId);
    if (U.page === 'mcp' && U.mcpTab === 'records') p.set('tab', 'records');
    return '#/' + U.page + (p.toString() ? '?' + p : '');
  };
  render = function (keepScroll = false) {
    ensureUI();
    if (!isMcp(U.page)) { baseRender(keepScroll); return; }
    const saved = keepScroll ? captureFormFocus() : null, y = keepScroll ? $('#main').scrollTop : 0;
    nav();
    const pages = { mcp: list, mcpDetail: detail, mcpImport: importPage, mcpEdit: editor, mcpDeploy: deployment, mcpPreview: preview, mcpResult: resultPage, mcpRecord: recordPage, mcpUndo: undoPage, mcpResolve: resolvePage, mcpCheck: checkPage };
    $('#main').innerHTML = `<div class="mcp-page">${(pages[U.page] || list)()}</div>`;
    reviewRail(); hydrate(); $('#main').scrollTop = y;
    document.body.classList.toggle('hide-notes', !U.notes); document.body.classList.toggle('review-open', U.reviewOpen); document.body.classList.toggle('nav-collapsed', U.collapsed); document.body.classList.toggle('mobile-menu', U.menuOpen);
    $('#notes-toggle').setAttribute('aria-pressed', String(U.notes)); $('#notes-toggle').innerHTML = U.notes ? '隐藏标注' : '显示标注'; restoreFormFocus(saved);
  };
  document.addEventListener('click', event => {
    const external = event.target.closest('[data-action]');
    if (external && ['mcp-design', 'mcp-tour', 'reset'].includes(external.dataset.action)) {
      event.preventDefault(); event.stopImmediatePropagation();
      if (external.dataset.action === 'mcp-design') showDesign();
      else if (external.dataset.action === 'mcp-tour') { I = { selected: [], names: {}, scanned: true }; go('mcpImport'); }
      else reset();
      return;
    }
    const button = event.target.closest('[data-mcp]'); if (!button || button.disabled) return;
    event.preventDefault(); event.stopImmediatePropagation();
    const action = button.dataset.mcp, id = button.dataset.id, tid = button.dataset.target;
    if (action === 'back') go('mcp', { mcpTab: 'services' });
    else if (action === 'detail') go('mcpDetail', { mcpId: id });
    else if (action === 'tab') go('mcp', { mcpTab: button.dataset.tab });
    else if (action === 'scope') { U.mcpScope = button.dataset.scope; render(true); }
    else if (action === 'attention') { U.mcpStatus = 'attention'; render(true); }
    else if (action === 'clearFilters') { U.mcpSearch = ''; U.mcpStatus = 'all'; U.mcpScope = 'all'; U.mcpProject = 'all'; render(true); }
    else if (action === 'import') { I = { selected: [], names: {}, scanned: false }; go('mcpImport'); }
    else if (action === 'scan') { if (!I) I = { selected: [], names: {} }; I.scanned = true; I.selected = I.selected.filter(x => !M.imported.includes(x)); error = ''; render(true); }
    else if (action === 'cancelImport') { I = null; go('mcp'); }
    else if (action === 'confirmImport') confirmImport();
    else if (action === 'add') startEditor(null);
    else if (action === 'edit') startEditor(id);
    else if (action === 'cancelEdit') { const returnId = F?.id; go(returnId ? 'mcpDetail' : 'mcp', { mcpId: returnId }); }
    else if (action === 'saveEdit') { error = ''; saveEditor(); }
    else if (action === 'parseJson') parseJson();
    else if (action === 'discardEdit') { const next = pendingLeave; pendingLeave = null; pendingEditorLocation = null; F = null; closeModal(); next?.(); }
    else if (action === 'deploy') startDeploy(id);
    else if (action === 'updateTarget') { const t = target(tid); if (t) { startDeploy(t.serviceId, t); P = buildPlan(); go('mcpPreview', {}, true); } }
    else if (action === 'cancelDeploy') { const returnId = D?.returnId; D = P = null; go(returnId ? 'mcpDetail' : 'mcp', { mcpId: returnId }); }
    else if (action === 'preview') { P = buildPlan(); go('mcpPreview'); }
    else if (action === 'editDeploy') go('mcpDeploy');
    else if (action === 'apply') confirmApply();
    else if (action === 'planDiff') { const row = P?.rows.find(r => r.id === id); if (row) modal('配置变化', `${row.name} · ${scopeDescription(row)}`, `<div class="mcp-diff"><div><h3>当前配置</h3><pre>${esc(jsonConfig(row.before.config))}</pre></div><div><h3>将写入的配置</h3><pre>${esc(jsonConfig(row.config))}</pre></div></div>`); }
    else if (action === 'record') go('mcpRecord', { mcpRecordId: id });
    else if (action === 'history') go('mcp', { mcpTab: 'records' });
    else if (action === 'retry') retryRecord(id);
    else if (action === 'undo') go('mcpUndo', { mcpRecordId: id });
    else if (action === 'confirmUndo') confirmUndo(id);
    else if (action === 'resolve') { const t = target(tid); if (t) go('mcpResolve', { mcpTargetId: tid, mcpId: t.serviceId }); }
    else if (action === 'confirmResolve') confirmResolve(tid);
    else if (action === 'check') { const t = target(tid); if (t) { checkState = null; go('mcpCheck', { mcpTargetId: tid, mcpId: t.serviceId }); } }
    else if (action === 'runCheck') runCheck(tid);
    else if (action === 'credential') showCredential(id);
    else if (action === 'saveCredential') { const val = $('#mcp-credential-input')?.value.trim(); if (!val) { $('#mcp-credential-input')?.focus(); return; } const s = service(id); if (s) s.credentialReady = true; closeModal(); render(true); toast('已记录凭据补充状态，请重新检测连接'); }
    else if (action === 'authGuide' || action === 'clientGuide') { const t = target(tid); if (t) nativeGuide(t, action === 'authGuide'); }
    else if (action === 'confirmAuth') { const t = target(tid); if (t) { t.client = 'verified'; t.lastTest = null; } closeModal(); checkState = null; render(true); toast('已记录演示授权状态，可重新检测'); }
    else if (action === 'confirmClient') { const t = target(tid); if (t) t.client = 'verified'; closeModal(); render(true); toast('已记录手动确认；不代表持续监测'); }
    else if (action === 'runtimeGuide') modal('处理运行环境缺失', '先在本机准备服务需要的运行环境，再重新检测。', `<div class="mcp-native-guide"><ol><li>确认命令所需的 Node.js 或 Python 环境已安装。</li><li>确认桌面应用能找到可执行文件，必要时在连接定义中填写完整路径。</li><li>返回该使用位置，重新检测连接。</li></ol></div><p class="mcp-quiet-note">原型按钮只模拟环境已经准备好，不会安装软件。</p>`, '<span></span><div class="actions">' + B('关闭', 'close') + B('演示：环境已准备好', 'runtimeReady', 'primary') + '</div>');
    else if (action === 'runtimeReady') { M.runtimeReady = true; checkState = null; closeModal(); render(true); toast('已切换为环境就绪的演示状态，请重新检测'); }
    else if (action === 'removeTarget') removeTarget(tid);
    else if (action === 'confirmRemove') confirmRemove(tid);
    else if (action === 'deleteService') {
      const s = service(id); if (!s) return;
      if (targetsForService(id).length) modal('此服务仍有使用位置', s.name, '<p class="muted small">请先在各使用位置移除配置或停止管理，再从库中移除定义。不会批量删除其他项目配置。</p>');
      else modal('从库中移除此服务？', s.name, '<p class="muted small">当前没有受管使用位置。历史记录中的快照仍会保留。</p>', '<span></span><div class="actions">' + B('取消', 'close') + B('确认移除', 'confirmDeleteService', 'danger', attr('id', id)) + '</div>');
    }
    else if (action === 'confirmDeleteService') { if (!targetsForService(id).length) M.services = M.services.filter(s => s.id !== id); closeModal(); go('mcp', {}, true); }
    else if (action === 'close') closeModal();
    else if (action === 'design') showDesign();
    else if (action === 'tour') { closeModal(); I = { selected: [], names: {}, scanned: true }; go('mcpImport'); }
    else if (action === 'scenarioDrift') scenario('drift');
    else if (action === 'scenarioRuntime') scenario('runtime');
    else if (action === 'reset') reset();
  }, true);
  function toggleArray(array, value, checked) { return checked ? [...new Set([...array, value])] : array.filter(v => v !== value); }
  document.addEventListener('change', event => {
    const e = event.target;
    if (!(e.id.startsWith('mcp-') || e.name?.startsWith('mcp-'))) return;
    event.stopImmediatePropagation();
    if (e.id === 'mcp-project') U.mcpProject = e.value;
    else if (e.id === 'mcp-status') U.mcpStatus = e.value;
    else if (e.name === 'mcp-import-choice' && I) I.selected = toggleArray(I.selected, e.value, e.checked);
    else if (e.name === 'mcp-deploy-service' && D) D.serviceIds = toggleArray(D.serviceIds, e.value, e.checked);
    else if (e.name === 'mcp-deploy-project' && D) D.projectIds = toggleArray(D.projectIds, e.value, e.checked);
    else if (e.name === 'mcp-deploy-tool' && D) D.tools = toggleArray(D.tools, e.value, e.checked);
    else if (e.name === 'mcp-deploy-scope' && D) D.scope = e.value;
    else if (e.name === 'mcp-transport' && F) { F.config.transport = e.value; if (e.value === 'stdio' && F.config.auth === 'oauth') F.config.auth = 'none'; }
    else if (e.id === 'mcp-auth' && F) F.config.auth = e.value;
    else return;
    error = ''; render(true);
  }, true);
  document.addEventListener('input', event => {
    const e = event.target; if (!e.id.startsWith('mcp-')) return;
    event.stopImmediatePropagation();
    if (e.id === 'mcp-search') { U.mcpSearch = e.value; render(true); }
    else if (e.name === 'mcp-import-name' && I) I.names[e.dataset.id] = e.value;
    else if (F) {
      const field = ({ 'mcp-name': 'name', 'mcp-key': 'key', 'mcp-description': 'description', 'mcp-json': 'raw' })[e.id];
      const configField = ({ 'mcp-command': 'command', 'mcp-args': 'args', 'mcp-url': 'url', 'mcp-env': 'env' })[e.id];
      if (field) F[field] = e.value;
      else if (configField) F.config[configField] = e.value;
      else if (e.id === 'mcp-secret') F.credentialReady = !!e.value || F.credentialReady;
    }
  }, true);
  document.addEventListener('submit', event => { if (event.target.id === 'mcp-edit-form') { event.preventDefault(); error = ''; saveEditor(); } }, true);
  window.MCP = {
    snapshot: () => clone({ state: M, ui: { page: U.page, scope: U.mcpScope, project: U.mcpProject, status: U.mcpStatus, search: U.mcpSearch }, draft: D, plan: P, error }),
    reset, scenario, showDesign, open: () => go('mcp'), tour: () => { I = { selected: [], names: {}, scanned: true }; go('mcpImport'); },
    routes, version: 'mcp-design-r1'
  };
  ensureUI();
})();
