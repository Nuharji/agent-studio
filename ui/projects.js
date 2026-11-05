/* Agent Studio 1.0 — built around projects: the apps you build with AI. A project is a folder (usually a git
   repository) with its own rights for Claude, its to-do list, its sessions with Claude, its commands (build, test,
   start — run by the app itself, without AI) and its changes (git). The project page has a box like in the Claude
   desktop app with the kinds of work for building apps (feature, fix, tests, clean up, review, release, question);
   "New app" makes the folder and lets Claude set the app up. Rust: project.rs (what a project is), jobs.rs (commands).
   Loaded before app.js: everything here uses app.js's globals (cfg, data, t, esc, startRun …) only when called. */
'use strict';

/* ================= The model ================= */
const COLORS = ['#1ec8dd', '#2680eb', '#8a5cf6', '#e5508a', '#e68619', '#2d9d78', '#d7373f', '#6e6e6e'];
/* The kinds of work: level = the rights they need at least (capped by the project's own level unless said). */
const WORK = [
  {k: 'feature', level: 'edit', icon: '✦'},
  {k: 'fix', level: 'edit', icon: '🐞'},
  {k: 'test', level: 'edit', icon: '✓'},
  {k: 'refactor', level: 'edit', icon: '↻'},
  {k: 'review', level: 'read', icon: '👁'},
  {k: 'release', level: 'publish', icon: '🚀'},
  {k: 'ask', level: 'read', icon: '?'},
];
const workOf = k => WORK.find(w => w.k === k) || WORK[0];
const newProject = (dir, info = {}) => ({id: uid(), name: info.name || baseName(dir), dir, extraDirs: [], color: COLORS[cfg.projects.length % COLORS.length],
  todo: (info.todo || [])[0] || '', tick: false, level: 'commit', commands: defaultAllowed(info), web: true, auto: true, noDelete: true, maxMinutes: 60, model: '',
  rules: '', cmds: [], created: Date.now(), opened: Date.now()});
/* The commands Claude may run in a new project: the ones that build and test it. */
const defaultAllowed = info => (info.commands || []).filter(c => ['build', 'test', 'lint', 'check'].includes(c.kind)).map(c => c.cmd).join(', ');
const projectOf = id => (cfg.projects || []).find(p => p.id === id);
const projectByDir = dir => (cfg.projects || []).find(p => p.dir === dir);
const projDirs = p => [p.dir, ...(p.extraDirs || [])].filter(Boolean);
const info = {};          // project id → project.rs Info (read when shown)
const infoAt = {};
async function loadInfo(p, force) {
  if (!invoke || !p) return null;
  if (!force && info[p.id] && Date.now() - (infoAt[p.id] || 0) < 8000) return info[p.id];
  infoAt[p.id] = Date.now();
  info[p.id] = await invoke('project_info', {dir: p.dir}).catch(() => info[p.id] || null);
  if (info[p.id] && p.todo && !todos[p.todo]) await loadTodo(p.todo);
  return info[p.id];
}
async function loadAllInfo() {
  if (!invoke || !(cfg.projects || []).length) return;
  const list = await invoke('projects_info', {dirs: cfg.projects.map(p => p.dir)}).catch(() => []);
  cfg.projects.forEach((p, i) => { if (list[i]) { info[p.id] = list[i]; infoAt[p.id] = Date.now(); } });
  await Promise.all(cfg.projects.filter(p => p.todo).map(p => loadTodo(p.todo)));
}
const projRuns = id => data.runs.filter(r => r.spec.project === id);
const projSessions = id => data.runs.filter(r => r.spec.project === id && !r.spec.continues);
const projRunning = id => data.runs.some(r => r.spec.project === id && isRunning(r.id));
const projNeeds = id => data.inbox.filter(i => (i.kind === 'question' || i.kind === 'approval') && (runOf(i.runId) || {spec: {}}).spec.project === id).length;
const openTodos = p => p.todo && todos[p.todo] ? (todos[p.todo].items || []).filter(i => !i.done) : [];
const initials = name => String(name || '?').replace(/studio$/i, '').trim().split(/[\s_-]+/).filter(Boolean).slice(0, 2).map(w => w[0]).join('').toUpperCase() || '?';
const tile = (p, big) => `<span class="ptile${big ? ' big' : ''}" style="--pc:${esc(p.color || COLORS[0])}">${esc(initials(p.name))}</span>`;
/* The project's profile for a run: its rights, its folders. */
function projectSpec(p, o = {}) {
  const w = o.work ? workOf(o.work) : null;
  const lv = o.level || (w ? (LEVELS.indexOf(w.level) < LEVELS.indexOf(p.level) || w.k === 'review' || w.k === 'ask' ? w.level : p.level) : p.level);
  return specFrom({...p, id: null, name: '', title: o.title || '', dirs: projDirs(p), level: lv, auto: o.auto ?? p.auto, planOnly: !!o.planOnly,
    model: o.model ?? p.model, rules: [p.rules, o.rules].filter(Boolean).join('\n')},
    {prompt: o.prompt || '', said: o.said || '', project: p.id, work: o.work || '', chat: !!o.chat, ...(o.todo ? {todo: o.todo} : {})});
}

/* ================= Navigation ================= */
/* settings.view: 'home' | 'project' | 'inbox' | 'chats' | 'routines' | 'history'; settings.project, settings.ptab. */
const PTABS = ['overview', 'tasks', 'sessions', 'commands', 'changes', 'setup'];
function openProject(id, tab) {
  const p = projectOf(id); if (!p) return;
  settings.view = 'project'; settings.project = id; if (tab) settings.ptab = tab;
  if (!PTABS.includes(settings.ptab)) settings.ptab = 'overview';
  p.opened = Date.now(); full = null; saveSettings(); saveCfg();
  renderAll();
  loadInfo(p, true).then(() => { if (settings.view === 'project' && settings.project === id) renderView(); renderSide(); });
}
const curProject = () => settings.view === 'project' ? projectOf(settings.project) : null;
function renderSide() {
  const box = $('#side'); if (!box) return;
  const need = needItems().length;
  const nav = [['home', 'v.home', ICONS.home], ['inbox', 'v.inbox', ICONS.inbox], ['chats', 'v.chats', ICONS.chats], ['routines', 'v.routines', ICONS.routines], ['history', 'v.history', ICONS.history]];
  const on = v => !full && settings.view === v;
  const list = [...(cfg.projects || [])].sort((a, b) => (b.pinned ? 1 : 0) - (a.pinned ? 1 : 0) || a.name.localeCompare(b.name));
  box.innerHTML = nav.map(([v, k, ic]) => `<button class="snav${on(v) ? ' on' : ''}" data-act="view${v[0].toUpperCase() + v.slice(1)}">${ic}<span>${esc(t(k))}</span>${v === 'inbox' && need ? `<b class="count">${need}</b>` : ''}</button>`).join('')
    + `<div class="side-h"><span>${esc(t('p.projects'))}</span><button class="side-add" data-act="addProject" title="${esc(tip('addProject'))}">+</button></div>`
    + list.map(p => {
      const i = info[p.id], ch = i && i.git ? i.git.changes.length : 0, run = projRunning(p.id), nd = projNeeds(p.id);
      return `<button class="sproj${settings.view === 'project' && settings.project === p.id && !full ? ' on' : ''}" data-proj="${esc(p.id)}" title="${esc(p.dir)}">${tile(p)}<span class="sname">${esc(p.name)}</span>
        ${nd ? `<b class="count">${nd}</b>` : run ? '<span class="dot running"></span>' : ch ? `<span class="sch" title="${esc(tn('p.changesN', ch))}">${ch}</span>` : ''}</button>`;
    }).join('')
    + `<button class="snew" data-act="newApp">${ICONS.spark}<span>${esc(t('a.newApp'))}</span></button>`;
}

/* ================= Home: all apps ================= */
function viewHome() {
  const ps = cfg.projects || [], running = data.runs.filter(r => isRunning(r.id)), need = needItems();
  let h = `<div class="view-title"><h1>${esc(t('v.home'))}</h1><span class="sub">${esc(t('home.sub'))}</span><span class="flex"></span>
    <button class="btn-sm" data-act="addProject">${esc(t('a.addProject'))}</button><button class="btn-sm primary" data-act="newApp">${esc(t('a.newApp'))}</button></div>`;
  h += signInHint();
  if (!ps.length) return h + `<div class="empty-big"><div class="big-tile">Ag</div><h2>${esc(t('home.emptyTitle'))}</h2><p>${esc(t('home.empty'))}</p>
    <div class="it-actions"><button class="btn-sm primary" data-act="newApp">${esc(t('a.newApp'))}</button><button class="btn-sm" data-act="addProject">${esc(t('a.addProject'))}</button><button class="btn-sm" data-act="newChat">${esc(t('a.newChat'))}</button></div></div>`;
  if (need.length) h += `<div class="pcard approval home-need" data-act="viewInbox"><b>${esc(tn('inbox.count', need.length))}</b> <span class="meta">${esc(need.slice(0, 3).map(i => i.title).join(' · '))}</span><span class="flex"></span><button class="btn-sm primary">${esc(t('inbox.open'))}</button></div>`;
  if (running.length) h += `<div class="group-h">${esc(t('inbox.running'))}</div>` + running.map(r => runRow(r, true)).join('');
  h += `<div class="group-h">${esc(t('p.projects'))}</div><div class="pgrid">${[...ps].sort((a, b) => (b.opened || 0) - (a.opened || 0)).map(projCard).join('')}</div>`;
  const recent = data.runs.filter(r => r.spec.project && !r.spec.continues).slice(0, 8);
  if (recent.length) h += `<div class="group-h">${esc(t('home.recent'))}</div><div class="pcard">${recent.map(r => runRow(lastOf(r), true)).join('')}</div>`;
  return h;
}
function projCard(p) {
  const i = info[p.id] || {}, g = i.git, todo = openTodos(p).length, run = projRunning(p.id), nd = projNeeds(p.id);
  const stack = (i.stack || []).slice(0, 3).map(s => `<span class="chip">${esc(t('stack.' + s))}</span>`).join('');
  return `<div class="pcard proj-card" data-proj="${esc(p.id)}"><div class="pc-head">${tile(p, true)}<div class="pc-name"><b>${esc(p.name)}</b><span class="meta">${esc(i.version ? 'v' + i.version : '')}${g ? ' · ⎇ ' + esc(g.branch) : ''}</span></div>
      ${run ? '<span class="dot running"></span>' : ''}</div>
    <div class="chips">${stack}</div>
    <div class="pc-stats">${nd ? `<span class="badge warn">${esc(tn('p.needN', nd))}</span>` : ''}<span>${esc(tn('p.todoN', todo))}</span>${g ? `<span>${esc(tn('p.changesN', g.changes.length))}</span>${g.ahead ? `<span>↑${g.ahead}</span>` : ''}` : `<span class="meta">${esc(t('p.noGit'))}</span>`}</div>
    ${i.exists === false ? `<div class="meta" style="color:var(--danger)">${esc(t('p.missing'))}</div>` : ''}</div>`;
}
/* One run in a list (home, overview, sessions). */
function runRow(r, withProject, extra = '') {
  const st = stOf(r), f = firstOf(r), p = withProject && projectOf(f.spec.project), w = f.spec.work;
  return `<div class="row${sel && sel.id === r.id ? ' sel' : ''}" data-run="${esc(r.id)}"><span class="dot ${esc(st)}"></span>
    ${p ? `<span class="badge" style="color:${esc(p.color)}">${esc(p.name)}</span>` : ''}${w ? `<span class="wtag">${esc(workOf(w).icon)} ${esc(t('work.' + w))}</span>` : ''}
    <span class="grow" title="${esc(f.spec.said || f.spec.prompt)}">${esc(f.spec.title)}</span>${extra}<span class="meta">${esc(st === 'running' ? (r.last || t('run.starting')).slice(0, 60) : fmtWhen(r.started))}</span>${st === 'done' ? '' : statusBadge(st)}</div>`;
}

/* ================= The project page ================= */
function viewProject() {
  const p = curProject(); if (!p) { settings.view = 'home'; return viewHome(); }
  const i = info[p.id];
  if (!i && invoke) loadInfo(p).then(() => renderView());
  const g = i && i.git, nd = projNeeds(p.id);
  const head = `<div class="proj-head">${tile(p, true)}<div class="ph-main"><h1>${esc(p.name)}</h1>
      <div class="meta ph-sub">${i && i.version ? `<span>v${esc(i.version)}</span>` : ''}${(i && i.stack || []).map(s => `<span class="chip">${esc(t('stack.' + s))}</span>`).join('')}
        ${g ? `<span title="${esc(g.remote)}">⎇ ${esc(g.branch)}</span>${g.tag ? `<span>🏷 ${esc(g.tag)}</span>` : ''}${g.ahead ? `<span class="badge warn" title="${esc(t('p.aheadHint'))}">↑ ${g.ahead}</span>` : ''}${g.behind ? `<span class="badge">↓ ${g.behind}</span>` : ''}` : `<span>${esc(t('p.noGit'))}</span>`}
        <span class="pdir" title="${esc(p.dir)}">${esc(p.dir)}</span></div></div><span class="flex"></span>
      <button class="btn-sm" data-popen="${esc(p.dir)}" title="${esc(t('p.openDirHint'))}">${esc(t('p.openDir'))}</button><button class="btn-sm" data-reveal="${esc(p.dir)}">${esc(t('ui.show'))}</button></div>
    ${i && i.exists === false ? `<div class="pcard problem"><b>${esc(t('p.missing'))}</b> <span class="meta">${esc(p.dir)}</span> <button class="btn-sm" data-ptab="setup">${esc(t('ptab.setup'))}</button></div>` : ''}
    <div class="ptabs">${PTABS.map(k => `<button class="${settings.ptab === k ? 'on' : ''}" data-ptab="${k}">${esc(t('ptab.' + k))}${k === 'tasks' && openTodos(p).length ? ` <b>${openTodos(p).length}</b>` : ''}${k === 'changes' && g && g.changes.length ? ` <b>${g.changes.length}</b>` : ''}${k === 'sessions' && nd ? ' <b class="warn">!</b>' : ''}</button>`).join('')}</div>`;
  const body = {overview: pOverview, tasks: pTasks, sessions: pSessions, commands: pCommands, changes: pChanges, setup: pSetup}[settings.ptab] || pOverview;
  return head + body(p, i || {});
}

/* ---- Overview: the box, open to-dos, sessions, commands, commits ---- */
const pDraft = {};        // project id → {text, work, plan}
function workBox(p) {
  const d = pDraft[p.id] || (pDraft[p.id] = {text: '', work: 'feature', plan: false});
  const w = workOf(d.work), sp = projectSpec(p, {work: d.work});
  return `<div class="composer work-box"><div class="work-chips">${WORK.map(x => `<button class="wchip${d.work === x.k ? ' on' : ''}" data-work="${x.k}" title="${esc(t('work.' + x.k + '.hint'))}">${esc(x.icon)} ${esc(t('work.' + x.k))}</button>`).join('')}</div>
    <textarea id="workIn" rows="2" data-work-in="${esc(p.id)}" data-grow placeholder="${esc(t('work.' + w.k + '.ph', {name: p.name}))}">${esc(d.text)}</textarea>
    <div class="composer-bar"><span class="composer-tag" title="${esc(t('lv.' + sp.level + '.hint'))}">${esc(t('lv.' + sp.level))}${sp.auto ? ' · ' + esc(t('ed.autoShort')) : ''}${sp.web ? ' · 🌐' : ''}</span>
      <span class="composer-chip${d.plan ? ' on' : ''}"><button data-work-plan title="${esc(t('ed.planOnly'))}">📋 ${esc(t('work.planOnly'))}</button></span>
      <span class="flex"></span><button class="composer-btn" data-work-go="${esc(p.id)}" title="${esc(t('det.send'))} (Enter)"${d.text.trim() ? '' : ' disabled'}>&uarr;</button></div></div>
    <div class="composer-foot">${esc(t('work.foot', {dirs: projDirs(p).map(baseName).join(', ')}))}</div>`;
}
function pOverview(p, i) {
  const g = i.git, todo = openTodos(p), sess = projSessions(p.id).slice(0, 6), cmds = allCmds(p, i);
  let h = `<div class="ov-grid"><div class="ov-main">${signInHint()}${workBox(p)}`;
  if ((i.apps || []).length) {
    const fresh = i.apps.filter(a => !projectByDir(a));
    if (fresh.length) h += `<div class="pcard"><h2>${esc(tn('p.appsIn', fresh.length))}<span class="flex"></span><button class="btn-sm" data-add-apps="${esc(p.id)}">${esc(t('p.addApps'))}</button></h2>
      <div class="chips">${fresh.map(a => `<span class="fchip"><span>${esc(baseName(a))}</span><button class="x" data-add-app="${esc(a)}" title="${esc(t('p.addApp'))}">+</button></span>`).join('')}</div><div class="meta" style="margin-top:6px">${esc(t('p.appsHint'))}</div></div>`;
  }
  h += `<div class="pcard"><h2>${esc(t('p.sessions'))}<span class="flex"></span><button class="btn-sm" data-ptab="sessions">${esc(t('p.all'))}</button></h2>
    ${sess.length ? sess.map(r => runRow(lastOf(r))).join('') : `<div class="empty-s">${esc(t('p.noSessions'))}</div>`}</div>`;
  h += `</div><div class="ov-side">`;
  h += `<div class="pcard"><h2>${esc(t('p.todos'))}<span class="flex"></span><button class="btn-sm" data-ptab="tasks">${esc(p.todo ? t('p.all') : t('p.linkTodo'))}</button></h2>
    ${!p.todo ? `<div class="empty-s">${esc(t('p.noTodo'))}</div>` : todo.length ? todo.slice(0, 6).map(it => {
      const r = runForItem(p.todo, it.text), st = r ? stOf(r) : '';
      return `<div class="row"><span class="grow" title="${esc(it.text)}">${esc(it.text)}</span>${st ? statusBadge(st) : ''}${st !== 'running' ? `<button class="btn-sm" data-doitem="${esc(p.todo)}" data-line="${it.line}">${esc(st ? t('todo.again') : t('todo.do'))}</button>` : ''}</div>`;
    }).join('') : `<div class="empty-s">${esc(t('p.todoDone'))}</div>`}</div>`;
  h += `<div class="pcard"><h2>${esc(t('ptab.commands'))}<span class="flex"></span><button class="btn-sm" data-ptab="commands">${esc(t('p.all'))}</button></h2>
    ${cmds.length ? `<div class="cmd-quick">${cmds.slice(0, 5).map(c => cmdButton(p, c)).join('')}</div>` : `<div class="empty-s">${esc(t('p.noCmds'))}</div>`}</div>`;
  if (g) h += `<div class="pcard"><h2>${esc(t('p.git'))}<span class="flex"></span>${g.changes.length ? `<button class="btn-sm" data-ptab="changes">${esc(tn('p.changesN', g.changes.length))}</button>` : ''}</h2>
    ${g.log.slice(0, 6).map(l => { const [hash, msg, when] = l.split('|'); return `<div class="commit"><code>${esc(hash)}</code><span class="grow">${esc(msg)}</span><span class="meta">${esc(when)}</span></div>`; }).join('') || `<div class="empty-s">${esc(t('p.noCommits'))}</div>`}</div>`;
  h += `<div class="pcard"><h2>${esc(t('p.docs'))}</h2>${['readme', 'status', 'changelog'].filter(k => i[k]).map(k => `<button class="btn-sm" data-doc="${esc(i[k])}">${esc(baseName(i[k]))}</button>`).join(' ') || `<div class="empty-s">${esc(t('p.noDocs'))}</div>`}</div>`;
  return h + '</div></div>';
}
async function startWork(id) {
  const p = projectOf(id), d = pDraft[id]; if (!p || !d) return;
  const text = d.text.trim(); if (!text) return $('#workIn') && $('#workIn').focus();
  const w = workOf(d.work);
  const prompt = t('work.' + w.k + '.prompt', {name: p.name, text});
  const title = text.split('\n')[0].replace(/\s+/g, ' ').slice(0, 70);
  const rid = await startRun(projectSpec(p, {work: w.k, title, prompt, said: text, planOnly: d.plan, chat: w.k === 'ask'}), {show: false});
  if (!rid) return;
  pDraft[id] = {text: '', work: d.work, plan: false};
  openFull(rid);
}

/* ---- Tasks: the project's to-do list ---- */
function pTasks(p, i) {
  const cands = (i.todo || []).filter(x => x !== p.todo);
  let h = '';
  if (!p.todo) return `<div class="pcard"><div class="empty-big" style="padding:30px 10px"><h2>${esc(t('p.noTodoTitle'))}</h2><p>${esc(t('p.noTodoHint'))}</p>
    <div class="it-actions">${cands.map(c => `<button class="btn-sm primary" data-ptodo="${esc(c)}">${esc(t('p.useTodo', {name: baseName(c)}))}</button>`).join('')}<button class="btn-sm" data-ptodo-pick>${esc(t('p.pickTodo'))}</button></div></div></div>`;
  h += todoListCard(p.todo, p);
  return h;
}
/* A to-do list as a card: its items by section, "Do it", new items (also used for lists outside a project). */
function todoListCard(path, p) {
  const d = todos[path] || {items: []}, open = d.items.filter(i => !i.done).length;
  let h = `<div class="pcard"><h2><span title="${esc(path)}">${esc(baseName(path))}</span><span class="badge">${esc(tn('todo.openN', open))}</span><span class="flex"></span>
    <label class="switch" style="margin:0;text-transform:none;letter-spacing:0;font-weight:400" title="${esc(t('todo.tickHint'))}"><input type="checkbox" data-ptick="${esc(p.id)}"${p.tick ? ' checked' : ''}><span></span>${esc(t('todo.tick'))}</label>
    <button class="btn-sm" data-doall="${esc(path)}"${open ? '' : ' disabled'}>${esc(t('todo.doAll'))}</button><button class="btn-sm" data-popen="${esc(path)}">${esc(t('p.openFile'))}</button></h2>`;
  if (d.error) h += `<div class="empty-s" style="color:var(--danger)">${esc(d.error)}</div>`;
  else if (!d.items.length) h += `<div class="empty-s">${esc(t('todo.noItems'))}</div>`;
  const shown = itemsShown(d.items, doneAll[path]);
  let sect = null;
  for (const it of shown.items) {
    if (it.section !== sect) { sect = it.section; if (sect) h += `<div class="group-h">${esc(sect)}</div>`; }
    const r = runForItem(path, it.text), st = r ? stOf(r) : '';
    h += `<div class="row d${Math.min(2, it.depth)}${it.done ? ' done' : ''}${r && sel && sel.id === r.id ? ' sel' : ''}"${r ? ` data-run="${esc(r.id)}"` : ''}>
      <input type="checkbox" disabled${it.done ? ' checked' : ''}><span class="grow" title="${esc(it.text)}">${esc(it.text)}</span>${st ? statusBadge(st) : ''}
      ${!it.done && st !== 'running' ? `<button class="btn-sm${st ? '' : ' primary'}" data-doitem="${esc(path)}" data-line="${it.line}">${esc(st ? t('todo.again') : t('todo.do'))}</button>` : ''}</div>`;
  }
  if (shown.hidden || (doneAll[path] && d.items.filter(i => i.done).length > DONE_SHOWN))
    h += `<div class="done-more"><button class="btn-sm" data-done-all="${esc(path)}">${esc(shown.hidden ? tn('todo.doneMore', shown.hidden) : t('todo.doneLess', {n: DONE_SHOWN}))}</button></div>`;
  if (!d.error) {
    const secs = d.sections || [], cur = addSection(path);
    const opts = (d.items.some(i => !i.section) || !secs.length ? [''] : []).concat(secs);
    h += `<div class="todo-add"><input class="field-in" data-add-text="${esc(path)}" value="${esc(addDraft[path] || '')}" placeholder="${esc(t('todo.addPh'))}">
      ${opts.length > 1 ? `<select data-add-sect="${esc(path)}" title="${esc(t('todo.addWhere'))}">${opts.map(s => `<option value="${esc(s)}"${s === cur ? ' selected' : ''}>${esc(s || t('todo.addTop'))}</option>`).join('')}</select>` : ''}
      <button class="btn-sm" data-add="${esc(path)}" title="Enter">${esc(t('todo.add'))}</button><button class="btn-sm" data-add-go="${esc(path)}" title="${esc(t('todo.addGoHint'))}">${esc(t('todo.addGo'))}</button></div>`;
  }
  return h + '</div>';
}

/* ---- Sessions: every conversation with Claude in this project ---- */
function pSessions(p) {
  const list = projSessions(p.id);
  if (!list.length) return `<div class="empty-s">${esc(t('p.noSessions'))}</div>`;
  let day = null, h = '<div class="pcard">';
  for (const f of list) {
    const l = lastOf(f), d = new Date(l.started).toDateString();
    if (d !== day) { day = d; h += `<div class="group-h">${esc(new Date(l.started).toLocaleDateString(locale(), {weekday: 'long', day: 'numeric', month: 'long'}))}</div>`; }
    const files = chainOf(l).reduce((n, x) => n + x.repos.reduce((m, y) => m + y.files.length, 0), 0), commits = chainOf(l).reduce((n, x) => n + x.repos.reduce((m, y) => m + y.commits.length, 0), 0);
    h += runRow(l, false, commits ? `<span class="meta">${esc(tn('run.commitsN', commits))}</span>` : files ? `<span class="meta">${esc(tn('run.filesN', files))}</span>` : '');
  }
  return h + '</div>';
}

/* ---- Commands: build, test, start — run by the app, Claude fixes what fails ---- */
const jobs = {};          // job id → {project, cmd, lines: [{t, err}], code, running, started, ended}
const lastJob = {};       // project id + cmd → job id
let jobSel = {};          // project id → job id shown
const allCmds = (p, i) => {
  const own = (p.cmds || []).filter(c => c.cmd);
  return [...own, ...(i.commands || []).filter(c => !own.some(o => o.cmd === c.cmd) && !(p.hiddenCmds || []).includes(c.cmd))];
};
function cmdButton(p, c) {
  const j = jobs[lastJob[p.id + '\u0000' + c.cmd]], st = !j ? '' : j.running ? 'running' : j.code === 0 ? 'done' : 'failed';
  return `<button class="cmd-btn ${st}" data-job-run="${esc(c.cmd)}" title="${esc(c.cmd)}"><span class="dot ${st}"></span><b>${esc(t('cmd.' + c.kind) !== 'cmd.' + c.kind ? t('cmd.' + c.kind) : c.kind)}</b><code>${esc(c.cmd)}</code></button>`;
}
function pCommands(p, i) {
  const cmds = allCmds(p, i), jid = jobSel[p.id], j = jobs[jid];
  let h = `<div class="pcard"><h2>${esc(t('ptab.commands'))}<span class="flex"></span><span class="meta" style="text-transform:none;letter-spacing:0">${esc(t('cmd.hint'))}</span></h2>
    ${cmds.map(c => {
      const lj = jobs[lastJob[p.id + '\u0000' + c.cmd]], st = !lj ? '' : lj.running ? 'running' : lj.code === 0 ? 'done' : 'failed';
      return `<div class="row"><span class="dot ${st}"></span><span class="wtag">${esc(t('cmd.' + c.kind) !== 'cmd.' + c.kind ? t('cmd.' + c.kind) : c.kind)}</span><code class="grow">${esc(c.cmd)}</code>
        ${lj ? `<span class="meta">${esc(lj.running ? fmtDur(Date.now() - lj.started) : (lj.code === 0 ? '✓ ' : '✗ ') + fmtDur(lj.ended - lj.started))}</span><button class="btn-sm" data-job-show="${esc(lastJob[p.id + '\u0000' + c.cmd])}">${esc(t('cmd.output'))}</button>` : ''}
        ${lj && lj.running ? `<button class="btn-sm" data-job-stop="${esc(lastJob[p.id + '\u0000' + c.cmd])}" style="color:var(--danger)">${esc(t('run.stop'))}</button>` : `<button class="btn-sm primary" data-job-run="${esc(c.cmd)}">${esc(t('cmd.run'))}</button>`}
        <button class="x" data-cmd-del="${esc(c.cmd)}" title="${esc(t('cmd.remove'))}">&#10005;</button></div>`;
    }).join('') || `<div class="empty-s">${esc(t('p.noCmds'))}</div>`}
    <div class="todo-add"><select id="cmdKind">${['build', 'test', 'run', 'lint', 'check', 'other'].map(k => `<option value="${k}">${esc(t('cmd.' + k))}</option>`).join('')}</select>
      <input class="field-in" id="cmdNew" placeholder="${esc(t('cmd.newPh'))}"><button class="btn-sm" data-cmd-add>${esc(t('cmd.add'))}</button><button class="btn-sm" data-cmd-once>${esc(t('cmd.once'))}</button></div></div>`;
  if (j) {
    const failed = !j.running && j.code !== 0;
    h += `<div class="pcard${failed ? ' problem' : j.running ? ' running' : ' done'}"><h2><code style="text-transform:none;letter-spacing:0">${esc(j.cmd)}</code><span class="flex"></span>
      <span class="meta" style="text-transform:none;letter-spacing:0">${esc(j.running ? t('cmd.running') : j.stopped ? t('st.stopped') : t('cmd.exit', {c: j.code ?? '?'}) + ' · ' + fmtDur(j.ended - j.started))}</span>
      ${failed ? `<button class="btn-sm primary" data-job-fix="${esc(jid)}">${esc(t('cmd.fix'))}</button>` : ''}${j.running ? '' : `<button class="btn-sm" data-job-run="${esc(j.cmd)}">${esc(t('cmd.again'))}</button>`}</h2>
      <pre class="console" id="jobOut">${j.lines.map(jobLine).join('')}</pre></div>`;
  }
  return h;
}
const jobLine = l => `<span class="${l.err ? 'e' : ''}">${esc(l.t)}\n</span>`;
async function runJob(p, cmd) {
  if (!invoke) return toast(t('ui.appOnly'));
  const id = uid();
  jobs[id] = {project: p.id, cmd, lines: [], code: null, running: true, started: Date.now(), ended: 0};
  lastJob[p.id + '\u0000' + cmd] = id; jobSel[p.id] = id;
  try { await invoke('job_start', {id, dir: p.dir, cmd}); } catch (e) { jobs[id].running = false; jobs[id].code = -1; jobs[id].ended = Date.now(); jobs[id].lines.push({t: String(e), err: true}); }
  if (settings.view !== 'project' || settings.ptab !== 'commands') openProject(p.id, 'commands'); else renderView();
}
function jobOutput(e) {
  const {id, line, err} = e.payload, j = jobs[id]; if (!j) return;
  j.lines.push({t: line, err}); if (j.lines.length > 4000) j.lines.splice(0, j.lines.length - 4000);
  const box = $('#jobOut');
  if (box && jobSel[j.project] === id && settings.ptab === 'commands') { const end = box.scrollHeight - box.scrollTop - box.clientHeight < 40; box.insertAdjacentHTML('beforeend', jobLine({t: line, err})); if (end) box.scrollTop = box.scrollHeight; }
}
function jobDone(e) {
  const {id, code, stopped} = e.payload, j = jobs[id]; if (!j) return;
  Object.assign(j, {running: false, code: code ?? (stopped ? null : -1), stopped, ended: Date.now()});
  const p = projectOf(j.project);
  if (!stopped) toast(code === 0 ? t('cmd.ok', {c: j.cmd}) : t('cmd.failed', {c: j.cmd, n: code ?? '?'}));
  if (p) loadInfo(p, true).then(() => { if (!typing()) renderView(); renderSide(); });
}
/* Claude fixes what made a command fail: the command and the end of its output go with the request. */
async function fixJob(id) {
  const j = jobs[id], p = j && projectOf(j.project); if (!p) return;
  const tail = j.lines.slice(-120).map(l => l.t).join('\n');
  const rid = await startRun(projectSpec(p, {work: 'fix', title: t('cmd.fixTitle', {c: j.cmd}), said: t('cmd.fixSaid', {c: j.cmd}),
    prompt: t('cmd.fixPrompt', {c: j.cmd, code: j.code ?? '?', out: tail}), rules: ''}), {show: false});
  if (rid) openFull(rid);
}

/* ---- Changes: what is not committed yet ---- */
const diffs = {};         // project id → text
function pChanges(p, i) {
  const g = i.git;
  if (!g) return `<div class="empty-s">${esc(t('p.noGit'))}</div><p><button class="btn-sm" data-pgit-init="${esc(p.id)}">${esc(t('p.gitInit'))}</button></p>`;
  if (diffs[p.id] === undefined && invoke) invoke('project_diff', {dir: p.dir}).then(d => { diffs[p.id] = d; if (settings.ptab === 'changes') renderView(); });
  const canCommit = LEVELS.indexOf(p.level) >= 2;
  let h = `<div class="pcard"><h2>${esc(tn('p.changesN', g.changes.length))}<span class="flex"></span><button class="btn-sm" data-pdiff-reload>${esc(t('p.reload'))}</button>
    ${g.changes.length ? `<button class="btn-sm" data-pwork="review">${esc(t('p.reviewChanges'))}</button><button class="btn-sm primary" data-pwork="commit"${canCommit ? '' : ` disabled title="${esc(t('p.commitNeedsLevel'))}"`}>${esc(t('p.commitChanges'))}</button>` : ''}
    ${g.ahead && LEVELS.indexOf(p.level) >= 3 ? `<button class="btn-sm" data-pwork="push">${esc(t('p.push', {n: g.ahead}))}</button>` : ''}</h2>
    ${g.changes.length ? `<ul class="files">${g.changes.map(c => `<li data-reveal="${esc(p.dir + '/' + c.slice(3).replace(/^"|"$/g, ''))}"><b class="gst">${esc(c.slice(0, 2).trim() || '·')}</b> ${esc(c.slice(3))}</li>`).join('')}</ul>` : `<div class="empty-s">${esc(t('p.clean'))}</div>`}</div>`;
  if (g.changes.length) h += `<div class="pcard"><h2>${esc(t('run.diff'))}</h2>${diffs[p.id] === undefined ? '<div class="empty-s">…</div>' : `<pre class="diff inline">${diffHtml(diffs[p.id])}</pre>`}</div>`;
  return h;
}
const diffHtml = text => (text || t('run.noDiff')).split('\n').map(l => {
  const c = l.startsWith('### ') ? 'f' : l.startsWith('+++') || l.startsWith('---') || l.startsWith('diff ') ? 'h' : l.startsWith('@@') ? 'h' : l.startsWith('+') ? 'a' : l.startsWith('-') ? 'r' : '';
  return c ? `<span class="${c}">${esc(l)}</span>` : esc(l);
}).join('\n');
/* Work on the changes: a review (read only), a commit (Claude writes the message), a push. */
async function changeWork(p, what) {
  const level = what === 'review' ? 'read' : what === 'commit' ? 'commit' : 'publish';
  const rid = await startRun(projectSpec(p, {work: what === 'review' ? 'review' : '', level, auto: false, title: t('chg.' + what + '.title', {name: p.name}), said: t('chg.' + what + '.title', {name: p.name}),
    prompt: t('chg.' + what + '.prompt', {name: p.name})}), {show: false});
  if (rid) { delete diffs[p.id]; openFull(rid); }
}

/* ---- Setup: name, color, folders, to-do list, rights, rules ---- */
function pSetup(p) {
  const key = 'p:' + p.id;
  return `<div class="setup-grid"><div class="pcard"><h2>${esc(t('p.setup'))}</h2>
      <div class="ed-field"><label>${esc(t('ed.name'))}</label><input class="field-in" data-f="${key}|name" value="${esc(p.name)}" style="width:100%"></div>
      <div class="ed-field"><label>${esc(t('p.color'))}</label><div class="colors">${COLORS.map(c => `<button data-pcolor="${c}" class="${p.color === c ? 'on' : ''}" style="background:${c}"></button>`).join('')}</div></div>
      <div class="ed-field"><label>${esc(t('p.folder'))}</label><div class="chips"><span class="fchip" title="${esc(p.dir)}"><span>&lrm;${esc(p.dir)}&lrm;</span></span><button class="btn-sm" data-pdir>${esc(t('p.moveFolder'))}</button></div>
        <div class="hint">${esc(t('p.folderHint'))}</div></div>
      <div class="ed-field"><label>${esc(t('p.extraDirs'))}</label>${dirsField(p.extraDirs || [], key)}<div class="hint">${esc(t('p.extraDirsHint'))}</div></div>
      <div class="ed-field"><label>${esc(t('p.todoFile'))}</label><div class="chips">${p.todo ? `<span class="fchip" title="${esc(p.todo)}"><span>&lrm;${esc(p.todo)}&lrm;</span><button class="x" data-ptodo-off>&#10005;</button></span>` : `<span class="meta">${esc(t('p.noTodo'))}</span>`}<button class="btn-sm" data-ptodo-pick>${esc(t('p.pickTodo'))}</button></div></div>
      <div class="ed-field"><label>${esc(t('ed.rules'))}</label><textarea class="small" data-f="${key}|rules" placeholder="${esc(t('p.rulesPh'))}">${esc(p.rules || '')}</textarea><div class="hint">${esc(t('p.rulesHint'))}</div></div>
      <p><button class="btn-sm" data-ppin>${esc(t(p.pinned ? 'p.unpin' : 'p.pin'))}</button> <button class="btn-sm" data-premove style="color:var(--danger)">${esc(t('p.remove'))}</button></p></div>
    <div class="pcard"><h2>${esc(t('p.rights'))}</h2><p class="meta" style="margin-top:0">${esc(t('p.rightsHint'))}</p>${rightsFields({...p, dirs: projDirs(p)}, key, true)}</div></div>`;
}

/* ================= New app, add a project ================= */
const KINDS = ['studio', 'web', 'tauri', 'rust', 'python', 'free'];
let newAppDraft = null;
function newApp() {
  const o = newAppDraft || (newAppDraft = {name: '', parent: (cfg.projects[0] && dirOf(cfg.projects[0].dir)) || '', kind: 'studio', what: '', git: true, publish: false});
  modal(t('app.title'), '');
  const draw = () => {
    $('#modalBody').innerHTML = `<div class="task-form"><p class="hint" style="margin-top:0">${esc(t('app.hint'))}</p>
      <div class="ed-field"><label>${esc(t('app.name'))}</label><input class="field-in" id="appName" value="${esc(o.name)}" placeholder="${esc(t('app.namePh'))}"></div>
      <div class="ed-field"><label>${esc(t('app.parent'))}</label><div class="chips">${o.parent ? `<span class="fchip" title="${esc(o.parent)}"><span>&lrm;${esc(o.parent)}&lrm;</span></span>` : ''}<button class="btn-sm" id="appParent">${esc(t('app.pickParent'))}</button></div>
        <div class="hint">${esc(o.parent && o.name.trim() ? t('app.willBe', {p: o.parent + '/' + folderName(o.name)}) : t('app.parentHint'))}</div></div>
      <div class="ed-field"><label>${esc(t('app.kind'))}</label><div class="tpl-grid">${KINDS.map(k => `<button data-kind="${k}" class="${o.kind === k ? 'on' : ''}"><b>${esc(t('app.kind.' + k))}</b><span>${esc(t('app.kind.' + k + '.desc'))}</span></button>`).join('')}</div></div>
      <div class="ed-field"><label>${esc(t('app.what'))}</label><textarea id="appWhat" placeholder="${esc(t('app.whatPh'))}">${esc(o.what)}</textarea></div>
      <label class="switch"><input type="checkbox" id="appGit"${o.git ? ' checked' : ''}><span></span>${esc(t('app.git'))}</label></div>`;
  };
  draw();
  $('#modalFoot').hidden = false;
  $('#modalFoot').innerHTML = `<span class="meta">${esc(t('app.foot'))}</span><span class="flex"></span><button class="btn-sm primary" id="appGo">${esc(t('app.go'))}</button>`;
  const b = $('#modalBody');
  b.oninput = e => { if (e.target.id === 'appName') { o.name = e.target.value; const hint = e.target.closest('.task-form').querySelectorAll('.hint')[1]; if (hint) hint.textContent = o.parent && o.name.trim() ? t('app.willBe', {p: o.parent + '/' + folderName(o.name)}) : t('app.parentHint'); } if (e.target.id === 'appWhat') o.what = e.target.value; };
  b.onchange = e => { if (e.target.id === 'appGit') o.git = e.target.checked; };
  b.onclick = async e => {
    const k = e.target.closest('[data-kind]'); if (k) { o.kind = k.dataset.kind; draw(); return; }
    if (e.target.closest('#appParent')) { if (!invoke) return toast(t('ui.appOnly')); const p = await invoke('pick_folder'); if (p) { o.parent = p; draw(); } }
  };
  $('#appGo').onclick = async () => {
    if (!o.name.trim()) { toast(t('app.needName')); return $('#appName').focus(); }
    if (!o.parent) return toast(t('app.needParent'));
    if (!o.what.trim()) { toast(t('app.needWhat')); return $('#appWhat').focus(); }
    if (!invoke) return toast(t('ui.appOnly'));
    let dir;
    try { dir = await invoke('project_make_dir', {path: o.parent + (o.parent.includes('\\') && !o.parent.includes('/') ? '\\' : '/') + folderName(o.name)}); } catch (err) { return toast(String(err)); }
    const p = {...newProject(dir, {name: o.name.trim()}), level: o.git ? 'commit' : 'edit', auto: true, web: true,
      commands: ['git init', ...(o.kind === 'rust' || o.kind === 'tauri' || o.kind === 'studio' ? ['cargo build', 'cargo test', 'cargo init', 'cargo new'] : []), ...(o.kind === 'python' ? ['python3 -m pytest', 'python3 -m venv'] : [])].join(', ')};
    cfg.projects.push(p); saveCfg(); closeModal();
    const prompt = t('app.prompt', {name: o.name.trim(), what: o.what.trim(), kind: t('app.kind.' + o.kind + '.spec'), git: o.git ? t('app.promptGit') : ''});
    const rid = await startRun(projectSpec(p, {work: 'new-app', title: t('app.runTitle', {name: o.name.trim()}), prompt, said: o.what.trim()}), {show: false});
    newAppDraft = null;
    openProject(p.id, 'overview');
    if (rid) openFull(rid);
  };
  setTimeout(() => $('#appName') && $('#appName').focus(), 0);
}
const folderName = n => String(n).trim().replace(/[\\/:*?"<>|]/g, '-').replace(/\s+/g, '-').toLowerCase();
async function addProject(dir) {
  if (!invoke) return toast(t('ui.appOnly'));
  dir = dir || await invoke('pick_folder'); if (!dir) return;
  const had = projectByDir(dir); if (had) return openProject(had.id);
  const i = await invoke('project_info', {dir}).catch(() => ({}));
  const p = newProject(dir, i);
  cfg.projects.push(p); info[p.id] = i; infoAt[p.id] = Date.now(); saveCfg();
  if (p.todo) await loadTodo(p.todo);
  toast(t('p.added', {name: p.name}));
  openProject(p.id, 'overview');
  return p;
}
async function addApps(pid) {
  const i = info[pid]; if (!i) return;
  const fresh = (i.apps || []).filter(a => !projectByDir(a));
  for (const a of fresh) { const x = await invoke('project_info', {dir: a}).catch(() => ({})); const p = newProject(a, x); const parent = projectOf(pid);
    if (parent) Object.assign(p, {level: parent.level, auto: parent.auto, web: parent.web, rules: parent.rules});
    cfg.projects.push(p); info[p.id] = x; infoAt[p.id] = Date.now(); }
  saveCfg(); toast(tn('p.addedN', fresh.length)); renderAll();
}

/* ================= Clicks of the project views ================= */
async function projectClick(b, e) {
  const d = b.dataset, p = curProject();
  if (d.proj) { openProject(d.proj); return true; }
  if (d.ptab) { settings.ptab = d.ptab; saveSettings(); if (p) { if (d.ptab === 'changes') delete diffs[p.id]; loadInfo(p, true).then(() => renderView()); } renderView(); return true; }
  if (d.popen) { invoke && invoke('open_path', {path: d.popen}).catch(err => toast(String(err))); return true; }
  if (d.doc) { showDoc(d.doc); return true; }
  if (d.addApps) { addApps(d.addApps); return true; }
  if (d.addApp) { const parent = p; const x = await addProject(d.addApp); if (x && parent) { Object.assign(x, {level: parent.level, auto: parent.auto, web: parent.web, rules: parent.rules}); saveCfg(); } return true; }
  if (!p) return false;
  if (d.work) { pDraft[p.id].work = d.work; renderView(); const el = $('#workIn'); if (el) el.focus(); return true; }
  if (d.workPlan !== undefined) { pDraft[p.id].plan = !pDraft[p.id].plan; renderView(); return true; }
  if (d.workGo) { startWork(d.workGo); return true; }
  if (d.jobRun) { runJob(p, d.jobRun); return true; }
  if (d.jobStop) { invoke('job_stop', {id: d.jobStop}); return true; }
  if (d.jobShow) { jobSel[p.id] = d.jobShow; renderView(); return true; }
  if (d.jobFix) { fixJob(d.jobFix); return true; }
  if (d.cmdAdd !== undefined || d.cmdOnce !== undefined) {
    const cmd = ($('#cmdNew').value || '').trim(); if (!cmd) { $('#cmdNew').focus(); return true; }
    if (d.cmdAdd !== undefined) { p.cmds = [...(p.cmds || []).filter(c => c.cmd !== cmd), {kind: $('#cmdKind').value, cmd}]; p.hiddenCmds = (p.hiddenCmds || []).filter(c => c !== cmd); saveCfg(); }
    runJob(p, cmd); return true;
  }
  if (d.cmdDel) { p.cmds = (p.cmds || []).filter(c => c.cmd !== d.cmdDel); p.hiddenCmds = [...new Set([...(p.hiddenCmds || []), d.cmdDel])]; saveCfg(); renderView(); return true; }
  if (d.pdiffReload !== undefined) { delete diffs[p.id]; await loadInfo(p, true); renderView(); return true; }
  if (d.pwork) { changeWork(p, d.pwork); return true; }
  if (d.pgitInit) { runJob(p, 'git init'); return true; }
  if (d.ptodo) { p.todo = d.ptodo; saveCfg(); await loadTodo(p.todo); renderView(); return true; }
  if (d.ptodoPick !== undefined) { const paths = invoke ? await invoke('pick_files', {filterName: 'Markdown', exts: ['md', 'markdown', 'txt']}) : []; if (paths[0]) { p.todo = paths[0]; saveCfg(); await loadTodo(p.todo); renderView(); } return true; }
  if (d.ptodoOff !== undefined) { p.todo = ''; saveCfg(); renderView(); return true; }
  if (d.pcolor) { p.color = d.pcolor; saveCfg(); renderAll(); return true; }
  if (d.pdir !== undefined) { const dir = invoke && await invoke('pick_folder'); if (dir) { p.dir = dir; delete info[p.id]; saveCfg(); await loadInfo(p, true); renderAll(); } return true; }
  if (d.ppin !== undefined) { p.pinned = !p.pinned; saveCfg(); renderAll(); return true; }
  if (d.premove !== undefined) {
    if (!await ask(t('p.removeTitle'), t('p.removeAsk', {name: p.name}), t('p.remove'))) return true;
    cfg.projects = cfg.projects.filter(x => x.id !== p.id); saveCfg(); setView('home'); return true;
  }
  return false;
}
async function showDoc(path) {
  const text = invoke ? await invoke('read_text', {path}).catch(e => String(e)) : '';
  modal(baseName(path), `<div class="answer doc">${md(text)}</div>`);
  $('.modal').classList.add('wide');
  $('#modalFoot').hidden = false;
  $('#modalFoot').innerHTML = `<span class="meta">${esc(path)}</span><span class="flex"></span><button class="btn-sm" id="docOpen">${esc(t('p.openFile'))}</button>`;
  $('#docOpen').onclick = () => invoke('open_path', {path}).catch(err => toast(String(err)));
}

/* ================= Moving the setup of 0.x over ================= */
/* 0.x had linked to-do lists with their own folders and rights; 1.0 makes a project of each folder they worked in. */
function migrate(c) {
  if ((c.version || 1) >= 2 && Array.isArray(c.projects)) return c;
  c.projects = Array.isArray(c.projects) ? c.projects : [];
  const add = (dir, prof, todo, tick) => {
    if (!dir) return null;
    let p = c.projects.find(x => x.dir === dir);
    if (!p) {
      p = {id: uid(), name: baseName(dir), dir, extraDirs: (prof.dirs || []).slice(1), color: COLORS[c.projects.length % COLORS.length], todo: '', tick: false,
        level: prof.level || 'edit', commands: prof.commands || '', web: !!prof.web, auto: !!prof.auto, noDelete: prof.noDelete !== false, maxMinutes: prof.maxMinutes || 30,
        model: prof.model || '', rules: prof.prompt || '', cmds: [], created: Date.now(), opened: 0};
      c.projects.push(p);
    }
    if (todo && !p.todo) { p.todo = todo; p.tick = !!tick; }
    return p;
  };
  for (const f of c.todoFiles || []) {
    const prof = f.own ? f : (c.todo || {});
    add((prof.dirs || [])[0] || dirOf(f.path), prof, f.path, f.tick);
  }
  for (const r of c.routines || []) {
    const p = (r.dirs || [])[0] && (c.projects.find(x => x.dir === r.dirs[0]) || null);
    if (p) r.project = p.id;
  }
  c.version = 2;
  return c;
}
