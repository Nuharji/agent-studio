/* Agent Studio — Claude as an assistant of the Studio family: routines, a linked Markdown to-do list and the
   "Needs you" box, under the person's control. Same chrome as the other apps (menus from MENUS with customizable
   shortcuts, option bar, tool rail, panel, status bar, settings dialog).
   Rust runs Claude Code (runner.rs / runs.rs) and keeps the runs, their logs and the box; this file keeps the setup
   (config.json: routines, to-do lists, rules, limits), plans the routines and draws everything. */
'use strict';
const $ = s => document.querySelector(s), $$ = s => [...document.querySelectorAll(s)];
const STORE = 'agent-studio.v1';
const invoke = window.__TAURI__ ? window.__TAURI__.core.invoke : null;
const listen = window.__TAURI__ ? window.__TAURI__.event.listen : () => {};
const esc = s => String(s ?? '').replace(/[&<>"']/g, c => ({'&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;'}[c]));
const uid = () => Math.random().toString(36).slice(2, 10);

/* ================= Texts ================= */
let I18N = {en: {}, de: {}}, lang = 'en';
function t(key, vars) {
  let s = (I18N[lang] && I18N[lang][key]) ?? I18N.en[key] ?? key;
  if (vars) for (const k in vars) s = s.split('{' + k + '}').join(vars[k]);
  return s;
}
const fmtKey = k => lang === 'de' ? k.replace(/Ctrl/g, 'Strg').replace(/Shift/g, 'Umschalt') : k;
async function loadTexts() {
  if (invoke) { I18N = await invoke('texts'); return; }
  const get = l => fetch(`../src-tauri/i18n/${l}.json`).then(r => r.json()).catch(() => ({}));
  I18N = {en: await get('en'), de: await get('de')};
}
/* A text with a number: its own wording for 1 (key + '1'). */
const tn = (key, n, vars = {}) => t(n === 1 && I18N.en[key + '1'] ? key + '1' : key, {n, ...vars});
const locale = () => lang === 'de' ? 'de-DE' : 'en-GB';

/* ================= Times ================= */
const pad = n => String(n).padStart(2, '0');
const dayStart = (d = new Date()) => new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
const fmtWhen = ms => {
  const d = new Date(ms), today = dayStart();
  const clock = d.toLocaleTimeString(locale(), {hour: '2-digit', minute: '2-digit'});
  const day = 86400000;
  if (ms >= today && ms < today + day) return t('time.today', {t: clock});
  if (ms >= today + day && ms < today + 2 * day) return t('time.tomorrow', {t: clock});
  if (ms >= today - day && ms < today) return t('time.yesterday', {t: clock});
  return d.toLocaleDateString(locale(), {weekday: 'short', day: 'numeric', month: 'short'}) + ' ' + clock;
};
const fmtDur = ms => {
  const s = Math.max(0, Math.round(ms / 1000));
  if (s < 60) return t('time.sec', {n: s});
  const m = Math.round(s / 60);
  return m < 60 ? t('time.min', {n: m}) : t('time.hmin', {h: Math.floor(m / 60), m: m % 60});
};
const dayName = (i, style = 'long') => new Date(2024, 0, 1 + i).toLocaleDateString(locale(), {weekday: style});
const lines = s => String(s || '').split('\n').map(x => x.trim()).filter(Boolean);
const list = s => String(s || '').split(/[\n,]/).map(x => x.trim()).filter(Boolean);
const baseName = p => String(p || '').split(/[\\/]/).filter(Boolean).pop() || p;

/* ================= State ================= */
const THEMES = ['dark', 'light', 'github', 'github-dark'];
const LEVELS = ['read', 'edit', 'commit', 'publish'];
const DEF = {keys: {}, autoHide: false, lang: 'auto', theme: 'dark', dock: true, view: 'inbox', histFilter: 'all'};
let settings = {...DEF}, booted = false;
const newCfg = () => ({version: 1, routines: [], todoFiles: [],
  todo: {dirs: [], level: 'edit', commands: '', web: false, noDelete: true, maxMinutes: 30, prompt: ''},
  rules: '', dailyMinutes: 0, claude: '', model: '', ticked: [], doneNotes: true});
let cfg = newCfg();
let data = {runs: [], inbox: [], running: []};
let sel = null;          // {type: 'run' | 'routine' | 'todo', id}
let claude = {path: null, version: '', status: null, checked: false};
const todos = {};        // path → {items, stamp, error}
const bootAt = Date.now();
const effectiveLang = () => settings.lang === 'auto' ? ((navigator.language || 'en').toLowerCase().startsWith('de') ? 'de' : 'en') : settings.lang;
function saveSettings() { try { localStorage.setItem(STORE, JSON.stringify({settings, cfg: invoke ? undefined : cfg})); } catch (e) {} }
let cfgT = 0;
function saveCfg() {
  clearTimeout(cfgT);
  cfgT = setTimeout(() => {
    if (invoke) invoke('config_save', {content: JSON.stringify(cfg, null, 1), claude: cfg.claude || ''}).catch(e => toast(t('toast.saveFail', {e})));
    else saveSettings();
  }, 300);
}
const runOf = id => data.runs.find(r => r.id === id);
const isRunning = id => data.running.includes(id);
const routineOf = id => cfg.routines.find(r => r.id === id);
const routineRunning = id => data.runs.some(r => r.spec.routineId === id && isRunning(r.id));
const needItems = () => data.inbox.filter(i => i.kind === 'question' || i.kind === 'approval');
/* Minutes Claude worked today (all runs). */
const usedToday = () => data.runs.filter(r => r.started >= dayStart()).reduce((n, r) => n + ((r.ended || Date.now()) - r.started), 0) / 60000;
const overLimit = () => +cfg.dailyMinutes > 0 && usedToday() >= +cfg.dailyMinutes;
/* The run that goes on from this one (an answer, an approval, a message). */
/* What a run's status is now: a question / approval that was answered goes on in the next run. */
const stOf = r => isRunning(r.id) ? 'running' : r.status === 'needsYou' && nextOf(r.id) ? 'answered' : r.status;
const nextOf = id => data.runs.find(r => r.spec.continues === id);
const firstOf = r => { let x = r; for (let i = 0; i < 50 && x && x.spec.continues && runOf(x.spec.continues); i++) x = runOf(x.spec.continues); return x; };

/* ================= Runs ================= */
function specFrom(o, extra = {}) {
  return {title: o.name || o.title || '', prompt: o.prompt || '', dirs: (o.dirs || []).filter(Boolean), level: LEVELS.includes(o.level) ? o.level : 'read',
    commands: list(o.commands), web: !!o.web, noDelete: o.noDelete !== false, planOnly: !!o.planOnly, maxMinutes: Math.max(0, +o.maxMinutes || 0),
    model: o.model || cfg.model || '', rules: [...lines(cfg.rules), ...lines(o.rules)], lang, routineId: o.id || null, ...extra};
}
async function startRun(spec, {show = true, manual = true} = {}) {
  if (!invoke) { toast(t('ui.appOnly')); return null; }
  if (manual && overLimit() && !await ask(t('limit.title'), t('limit.ask', {n: cfg.dailyMinutes}), t('limit.anyway'))) return null;
  try {
    const id = await invoke('run_start', {spec});
    await refresh();
    if (show) { sel = {type: 'run', id}; renderAll(); }
    return id;
  } catch (e) { toast(String(e)); return null; }
}
async function startRoutine(r, manual = true) {
  if (!(r.dirs || []).length) { toast(t('rt.noDirs')); sel = {type: 'routine', id: r.id}; setView('routines'); return; }
  if (!String(r.prompt || '').trim()) { toast(t('rt.noPrompt')); sel = {type: 'routine', id: r.id}; setView('routines'); return; }
  r.lastRun = Date.now(); saveCfg();
  const id = await startRun(specFrom(r), {show: manual, manual});
  if (id && !manual) toast(t('rt.started', {name: r.name}));
}
async function stopRun(id) { if (invoke) { await invoke('run_stop', {id}); toast(t('run.stopping')); } }
async function stopAll() {
  if (!invoke) return;
  const n = await invoke('run_stop_all');
  toast(n ? tn('run.stoppedAll', n) : t('run.noneRunning'));
}
let refreshT = 0;
async function refresh() {
  if (!invoke) return;
  data = await invoke('runs_get').catch(() => data);
  await afterRuns();
  if (booted) renderAll();
}
const refreshSoon = () => { clearTimeout(refreshT); refreshT = setTimeout(refresh, 120); };
/* After the runs changed: tick finished to-do items, the badge, "done" notes when they are not wanted. */
async function afterRuns() {
  for (const r of data.runs) {
    const td = r.spec.todo;
    if (!td || r.status !== 'done' || cfg.ticked.includes(r.id)) continue;
    const f = cfg.todoFiles.find(x => x.path === td.path);
    if (!f || !f.tick) continue;
    cfg.ticked = [r.id, ...cfg.ticked].slice(0, 500);
    const ok = await invoke('todo_tick', {path: td.path, line: td.line, text: td.text}).catch(() => false);
    if (ok) { await loadTodo(td.path); ownChange(td.path); toast(t('todo.ticked', {text: td.text})); }
    saveCfg();
  }
  if (!cfg.doneNotes) {
    const ids = data.inbox.filter(i => i.kind === 'done').map(i => i.id);
    if (ids.length) { await invoke('inbox_dismiss', {ids}); data.inbox = data.inbox.filter(i => !ids.includes(i.id)); }
  }
  const n = needItems().length;
  invoke('set_badge', {n}).catch(() => {});
  document.title = n ? `Agent Studio (${n})` : 'Agent Studio';
}

/* ================= To-do lists ================= */
async function loadTodo(path) {
  todos[path] = invoke ? await invoke('todo_read', {path}).catch(e => ({items: [], stamp: 0, error: String(e)})) : {items: [], stamp: 0, error: t('ui.appOnly')};
}
const todoKey = (path, text) => path + '\u0000' + text;
/* The newest run for an item of a list. */
function runForItem(path, text) {
  return data.runs.find(r => r.spec.todo && r.spec.todo.path === path && r.spec.todo.text === text);
}
function todoProfileOk() {
  if ((cfg.todo.dirs || []).length) return true;
  toast(t('todo.needDirs')); sel = {type: 'todo'}; setView('todos'); return false;
}
async function doItem(path, it) {
  if (!todoProfileOk()) return;
  const where = it.section ? t('todo.inSection', {s: it.section}) : '';
  const prompt = t('todo.prompt', {path, where, text: it.text}) + (cfg.todo.prompt ? '\n\n' + cfg.todo.prompt : '');
  await startRun(specFrom({...cfg.todo, title: it.text}, {prompt, todo: {path, line: it.line, text: it.text}}));
}
async function doList(path) {
  if (!todoProfileOk()) return;
  const prompt = t('todo.promptAll', {path}) + (cfg.todo.prompt ? '\n\n' + cfg.todo.prompt : '');
  await startRun(specFrom({...cfg.todo, title: t('todo.allTitle', {name: baseName(path)})}, {prompt}));
}
async function linkTodo() {
  if (!invoke) return toast(t('ui.appOnly'));
  const path = await invoke('pick_file', {filterName: 'Markdown', exts: ['md', 'markdown', 'txt']});
  if (!path) return;
  if (cfg.todoFiles.some(f => f.path === path)) return toast(t('todo.already'));
  cfg.todoFiles.push({path, tick: false}); saveCfg();
  await loadTodo(path);
  setView('todos');
  toast(tn('todo.linked', todos[path].items.filter(i => !i.done).length));
}
/* Changes of the lists: re-read them, and start routines that wait for a change (after a minute of quiet). */
const seen = {};         // path → stamp seen
const ownStamp = {};     // path → true while the change is ours (a tick)
function ownChange(path) { ownStamp[path] = true; }
async function watchTodos() {
  if (!invoke) return;
  const paths = [...new Set([...cfg.todoFiles.map(f => f.path), ...cfg.routines.filter(r => r.schedule && r.schedule.kind === 'todo' && r.todoFile).map(r => r.todoFile)])];
  if (!paths.length) return;
  const stamps = await invoke('todo_stamps', {paths}).catch(() => []);
  let redraw = false;
  paths.forEach((p, i) => {
    const s = stamps[i];
    if (seen[p] === undefined) { seen[p] = s; return; }
    if (s === seen[p]) return;
    seen[p] = s; redraw = true;
    if (ownStamp[p]) { delete ownStamp[p]; for (const r of cfg.routines) if (r.todoFile === p) r.todoStamp = s; return; }
    for (const r of cfg.routines) if (r.todoFile === p && r.schedule && r.schedule.kind === 'todo') r.todoChanged = Date.now();
  });
  if (redraw) { await Promise.all(cfg.todoFiles.map(f => loadTodo(f.path))); if (settings.view === 'todos') renderView(); }
}

/* ================= Planning the routines ================= */
const SCHEDULES = ['manual', 'daily', 'weekdays', 'weekly', 'start', 'todo'];
const atTime = (d, hm) => { const [h, m] = String(hm || '07:00').split(':').map(Number); return new Date(d.getFullYear(), d.getMonth(), d.getDate(), h || 0, m || 0).getTime(); };
/* The newest time the routine should have run (≤ now), or null. */
function lastSlot(r, now = new Date()) {
  const s = r.schedule || {};
  if (!['daily', 'weekdays', 'weekly'].includes(s.kind)) return null;
  for (let i = 0; i < 8; i++) {
    const d = new Date(now.getFullYear(), now.getMonth(), now.getDate() - i), wd = (d.getDay() + 6) % 7;
    if (s.kind === 'weekdays' && wd > 4) continue;
    if (s.kind === 'weekly' && wd !== (+s.day || 0)) continue;
    const at = atTime(d, s.time);
    if (at <= now.getTime()) return at;
  }
  return null;
}
function nextSlot(r, now = new Date()) {
  const s = r.schedule || {};
  if (!['daily', 'weekdays', 'weekly'].includes(s.kind)) return null;
  for (let i = 0; i < 9; i++) {
    const d = new Date(now.getFullYear(), now.getMonth(), now.getDate() + i), wd = (d.getDay() + 6) % 7;
    if (s.kind === 'weekdays' && wd > 4) continue;
    if (s.kind === 'weekly' && wd !== (+s.day || 0)) continue;
    const at = atTime(d, s.time);
    if (at > now.getTime()) return at;
  }
  return null;
}
const startedNow = new Set();
let limitNoted = false;
async function planTick() {
  if (!invoke || !booted) return;
  for (const r of cfg.routines) {
    if (!r.enabled || routineRunning(r.id)) continue;
    const s = r.schedule || {};
    let due = false;
    if (s.kind === 'start') due = !startedNow.has(r.id) && Date.now() - bootAt > 15000;
    else if (s.kind === 'todo') due = r.todoChanged && Date.now() - r.todoChanged > 60000;
    else {
      // Missed slots (computer off, app closed) are caught up once when the app runs again.
      const slot = lastSlot(r);
      due = slot && (r.lastRun || 0) < slot && (r.created || 0) < slot;
    }
    if (!due) continue;
    if (overLimit()) { if (!limitNoted) { limitNoted = true; toast(t('limit.reached')); updateStatus(); } continue; }
    startedNow.add(r.id);
    if (s.kind === 'todo') { r.todoChanged = 0; r.todoStamp = seen[r.todoFile]; }
    await startRoutine(r, false);
  }
}

/* ================= Routines: templates ================= */
const TEMPLATES = ['todo', 'bugs', 'builds', 'week', 'notes', 'deps', 'empty'];
function fromTemplate(k) {
  const todoPath = (cfg.todoFiles[0] || {}).path || 'TODO.md';
  const base = {id: uid(), created: Date.now(), name: t(`tpl.${k}`), prompt: t(`tpl.${k}.prompt`, {todo: todoPath}), dirs: [], level: 'read', commands: '',
    web: false, noDelete: true, planOnly: false, maxMinutes: 30, model: '', rules: '', enabled: true, schedule: {kind: 'manual', time: '07:00', day: 0}};
  const over = {
    todo: {level: 'commit', schedule: {kind: 'daily', time: '07:00', day: 0}, maxMinutes: 60},
    bugs: {schedule: {kind: 'daily', time: '18:00', day: 0}},
    builds: {schedule: {kind: 'start', time: '07:00', day: 0}, maxMinutes: 10},
    week: {schedule: {kind: 'weekly', time: '16:00', day: 4}},
    notes: {level: 'edit', schedule: {kind: 'weekly', time: '17:00', day: 6}},
    deps: {web: true, schedule: {kind: 'weekly', time: '08:00', day: 0}},
    empty: {prompt: '', name: t('rt.newName')}
  }[k] || {};
  return {...base, ...over};
}
function newRoutine() {
  modal(t('rt.newTitle'), `<p class="hint">${esc(t('rt.newHint'))}</p><div class="tpl-grid">${TEMPLATES.map(k =>
    `<button data-tpl="${k}"><b>${esc(t('tpl.' + k))}</b><span>${esc(t('tpl.' + k + '.desc'))}</span></button>`).join('')}</div>`);
}
$('#modalBody').addEventListener('click', e => {
  const b = e.target.closest('[data-tpl]');
  if (!b) return;
  const r = fromTemplate(b.dataset.tpl);
  if (r.schedule.kind === 'todo' || b.dataset.tpl === 'todo') r.todoFile = (cfg.todoFiles[0] || {}).path || '';
  if (!r.dirs.length && cfg.todo.dirs.length) r.dirs = [...cfg.todo.dirs];
  cfg.routines.push(r); saveCfg(); closeModal();
  sel = {type: 'routine', id: r.id}; setView('routines');
});

/* ================= Views ================= */
const VIEWS = ['inbox', 'todos', 'routines', 'history'];
function setView(v) { settings.view = v; saveSettings(); renderAll(); }
function renderAll() { renderView(); renderDetails(); updateStatus(); syncTools(); }
function renderView() {
  const box = $('#view');
  const keep = box.scrollTop;
  box.innerHTML = {inbox: viewInbox, todos: viewTodos, routines: viewRoutines, history: viewHistory}[settings.view]();
  box.scrollTop = keep;
}
const statusBadge = st => `<span class="badge ${({running: 'blue', done: 'ok', needsYou: 'warn', failed: 'red', timeout: 'red'})[st] || ''}">${esc(t('st.' + st))}</span>`;
const levelBadge = l => `<span class="badge${l === 'publish' ? ' warn' : ''}">${esc(t('lv.' + l))}</span>`;
function emptyBig(title, text, btns = '') {
  return `<div class="empty-big"><div class="big-tile">Ag</div><h2>${esc(title)}</h2><p>${esc(text)}</p>${btns ? `<div class="it-actions">${btns}</div>` : ''}</div>`;
}
function signInHint() {
  if (!invoke || !claude.checked) return '';
  if (!claude.path) return `<div class="pcard problem"><div class="it-head"><b>${esc(t('cl.notFound'))}</b></div><div class="it-text">${esc(t('cl.notFoundHint'))}</div>
    <div class="it-actions"><button class="btn-sm primary" data-go="claude">${esc(t('cl.open'))}</button></div></div>`;
  if (claude.status && claude.status.loggedIn === false) return `<div class="pcard approval"><div class="it-head"><b>${esc(t('cl.signIn'))}</b></div><div class="it-text">${esc(t('cl.signInHint'))}</div>
    <div class="it-actions"><button class="btn-sm primary" data-go="login">${esc(t('cl.login'))}</button></div></div>`;
  return '';
}
function viewInbox() {
  const running = data.runs.filter(r => isRunning(r.id));
  const notes = data.inbox.filter(i => i.kind === 'done' || i.kind === 'problem');
  let h = `<div class="view-title"><h1>${esc(t('v.inbox'))}</h1><span class="sub">${esc(needItems().length ? tn('inbox.count', needItems().length) : t('inbox.none'))}</span><span class="flex"></span>
    ${notes.length ? `<button class="btn-sm" data-act="dismissDone">${esc(t('a.dismissDone'))}</button>` : ''}</div>`;
  h += signInHint();
  if (running.length) {
    h += `<div class="group-h">${esc(t('inbox.running'))}</div>`;
    h += running.map(r => `<div class="pcard running${sel && sel.id === r.id ? ' sel' : ''}" data-run="${esc(r.id)}"><div class="it-head"><span class="dot running"></span><b>${esc(r.spec.title)}</b>
      <span class="meta">${esc(t('run.since', {d: fmtDur(Date.now() - r.started)}))}</span><span class="flex"></span><button class="btn-sm" data-stop="${esc(r.id)}">${esc(t('run.stop'))}</button></div>
      <div class="it-text short meta">${esc(r.last || t('run.starting'))}</div></div>`).join('');
  }
  const order = ['approval', 'question', 'problem', 'done'];
  const items = [...data.inbox].sort((a, b) => order.indexOf(a.kind) - order.indexOf(b.kind) || b.created - a.created);
  if (!items.length && !running.length) return h + emptyBig(t('inbox.emptyTitle'), t('inbox.empty'),
    `<button class="btn-sm primary" data-act="newTask">${esc(t('a.newTask'))}</button><button class="btn-sm" data-act="viewTodos">${esc(t('a.viewTodos'))}</button><button class="btn-sm" data-act="newRoutine">${esc(t('a.newRoutine'))}</button>`);
  if (items.length && running.length) h += `<div class="group-h">${esc(t('inbox.forYou'))}</div>`;
  return h + items.map(itemCard).join('');
}
function itemCard(it) {
  const r = runOf(it.runId), when = fmtWhen(it.created);
  const head = `<div class="it-head"><b>${esc(t('kind.' + it.kind))}</b><span class="meta">${esc(it.title || '')} · ${esc(when)}</span><span class="flex"></span>
    ${r ? `<button class="btn-sm" data-open-run="${esc(r.id)}">${esc(t('inbox.open'))}</button>` : ''}</div>`;
  const text = `<div class="it-text${it.kind === 'done' ? ' short' : ''}">${esc(it.text)}</div>`;
  let body = '';
  if (it.kind === 'approval') {
    body = (it.denials || []).length ? `<ul class="denials">${it.denials.map(d => `<li>${esc(d.what || d.rule)}</li>`).join('')}</ul>` : '';
    body += `<div class="it-actions"><button class="btn-sm primary" data-approve="${esc(it.id)}">${esc(t('inbox.approve'))}</button>
      <button class="btn-sm" data-reject="${esc(it.id)}">${esc(t('inbox.reject'))}</button>
      <input class="field-in" data-note="${esc(it.id)}" placeholder="${esc(t('inbox.notePh'))}"></div>`;
  } else if (it.kind === 'question') {
    body = `<div class="it-actions">${(it.options || []).map((o, i) => `<button class="btn-sm${i ? '' : ' primary'}" data-option="${esc(it.id)}" data-val="${esc(o)}">${esc(o)}</button>`).join('')}
      <input class="field-in" data-note="${esc(it.id)}" placeholder="${esc(t('inbox.answerPh'))}"><button class="btn-sm${(it.options || []).length ? '' : ' primary'}" data-answer="${esc(it.id)}">${esc(t('inbox.answer'))}</button>
      <button class="btn-sm" data-dismiss="${esc(it.id)}">${esc(t('inbox.dismiss'))}</button></div>`;
  } else {
    const files = r ? r.repos.reduce((n, x) => n + x.files.length, 0) : 0;
    body = `<div class="it-actions">${it.kind === 'problem' && r ? `<button class="btn-sm primary" data-again="${esc(r.id)}">${esc(t('run.again'))}</button>` : ''}
      <button class="btn-sm${it.kind === 'done' ? ' primary' : ''}" data-dismiss="${esc(it.id)}">${esc(t('inbox.seen'))}</button>
      ${files ? `<span class="meta">${esc(tn('run.filesN', files))}</span>` : ''}
      <input class="field-in" data-note="${esc(it.id)}" placeholder="${esc(t('inbox.morePh'))}"><button class="btn-sm" data-answer="${esc(it.id)}">${esc(t('inbox.send'))}</button></div>`;
  }
  return `<div class="pcard ${esc(it.kind)}">${head}${text}${body}</div>`;
}
function viewTodos() {
  const p = cfg.todo;
  let h = `<div class="view-title"><h1>${esc(t('v.todos'))}</h1><span class="sub">${esc(t('todo.sub'))}</span><span class="flex"></span>
    <button class="btn-sm" data-act="linkTodo">${esc(t('a.linkTodo'))}</button></div>`;
  h += `<div class="pcard${sel && sel.type === 'todo' ? ' sel' : ''}" data-todo-profile><div class="it-head"><b>${esc(t('todo.how'))}</b><span class="flex"></span><button class="btn-sm" data-todo-edit>${esc(t('todo.edit'))}</button></div>
    <div class="meta">${(p.dirs || []).length ? esc(t('todo.howText', {dirs: p.dirs.map(baseName).join(', '), lv: t('lv.' + p.level)})) : esc(t('todo.howNone'))}</div></div>`;
  if (!cfg.todoFiles.length) return h + emptyBig(t('todo.emptyTitle'), t('todo.empty'), `<button class="btn-sm primary" data-act="linkTodo">${esc(t('a.linkTodo'))}</button>`);
  for (const f of cfg.todoFiles) {
    const d = todos[f.path] || {items: []};
    const open = d.items.filter(i => !i.done).length;
    h += `<div class="pcard"><h2><span title="${esc(f.path)}">${esc(baseName(f.path))}</span><span class="badge">${esc(tn('todo.openN', open))}</span><span class="flex"></span>
      <label class="switch" style="margin:0;text-transform:none;letter-spacing:0;font-weight:400"><input type="checkbox" data-tick="${esc(f.path)}"${f.tick ? ' checked' : ''}><span></span>${esc(t('todo.tick'))}</label>
      <button class="btn-sm" data-doall="${esc(f.path)}"${open ? '' : ' disabled'}>${esc(t('todo.doAll'))}</button>
      <button class="btn-sm" data-reveal="${esc(f.path)}">${esc(t('ui.show'))}</button><button class="btn-sm" data-unlink="${esc(f.path)}">${esc(t('todo.unlink'))}</button></h2>`;
    if (d.error) h += `<div class="empty-s" style="color:var(--danger)">${esc(d.error)}</div>`;
    else if (!d.items.length) h += `<div class="empty-s">${esc(t('todo.noItems'))}</div>`;
    let sect = null;
    for (const it of d.items) {
      if (it.section !== sect) { sect = it.section; if (sect) h += `<div class="group-h">${esc(sect)}</div>`; }
      const r = runForItem(f.path, it.text), st = r ? (stOf(r)) : '';
      h += `<div class="row d${Math.min(2, it.depth)}${it.done ? ' done' : ''}${r && sel && sel.id === r.id ? ' sel' : ''}"${r ? ` data-run="${esc(r.id)}"` : ''}>
        <input type="checkbox" disabled${it.done ? ' checked' : ''}><span class="grow" title="${esc(it.text)}">${esc(it.text)}</span>
        ${st ? statusBadge(st) : ''}
        ${!it.done && st !== 'running' ? `<button class="btn-sm${st ? '' : ' primary'}" data-doitem="${esc(f.path)}" data-line="${it.line}">${esc(st ? t('todo.again') : t('todo.do'))}</button>` : ''}</div>`;
    }
    h += '</div>';
  }
  return h;
}
function scheduleText(r) {
  const s = r.schedule || {};
  if (s.kind === 'daily') return t('sch.dailyAt', {t: s.time});
  if (s.kind === 'weekdays') return t('sch.weekdaysAt', {t: s.time});
  if (s.kind === 'weekly') return t('sch.weeklyAt', {d: dayName(+s.day || 0), t: s.time});
  if (s.kind === 'todo') return t('sch.todoOf', {f: baseName(r.todoFile || '?')});
  return t('sch.' + (s.kind || 'manual'));
}
function viewRoutines() {
  let h = `<div class="view-title"><h1>${esc(t('v.routines'))}</h1><span class="sub">${esc(t('rt.sub'))}</span><span class="flex"></span>
    <button class="btn-sm primary" data-act="newRoutine">${esc(t('a.newRoutine'))}</button></div>`;
  if (!cfg.routines.length) return h + emptyBig(t('rt.emptyTitle'), t('rt.empty'), `<button class="btn-sm primary" data-act="newRoutine">${esc(t('a.newRoutine'))}</button>`);
  return h + cfg.routines.map(r => {
    const last = data.runs.find(x => x.spec.routineId === r.id), next = r.enabled ? nextSlot(r) : null, run = routineRunning(r.id);
    return `<div class="pcard${sel && sel.type === 'routine' && sel.id === r.id ? ' sel' : ''}" data-routine="${esc(r.id)}">
      <div class="it-head"><label class="switch" style="margin:0" title="${esc(t('rt.enabled'))}"><input type="checkbox" data-enable="${esc(r.id)}"${r.enabled ? ' checked' : ''}><span></span></label>
        <b>${esc(r.name || t('rt.newName'))}</b>${levelBadge(r.level)}${r.planOnly ? `<span class="badge">${esc(t('rt.planOnly'))}</span>` : ''}<span class="flex"></span>
        ${run ? `<button class="btn-sm" data-stop-routine="${esc(r.id)}">${esc(t('run.stop'))}</button>` : `<button class="btn-sm primary" data-start="${esc(r.id)}">${esc(t('rt.start'))}</button>`}</div>
      <div class="meta">${esc(scheduleText(r))}${next ? ' · ' + esc(t('rt.next', {w: fmtWhen(next)})) : ''} · ${esc((r.dirs || []).map(baseName).join(', ') || t('rt.noDirsShort'))}</div>
      ${last ? `<div class="meta" style="margin-top:4px">${statusBadge(stOf(last))} ${esc(t('rt.last', {w: fmtWhen(last.started)}))}</div>` : ''}</div>`;
  }).join('');
}
function viewHistory() {
  const f = settings.histFilter;
  const runs = data.runs.filter(r => f === 'all' || (f === 'problems' ? ['failed', 'timeout'].includes(r.status) : f === 'needsYou' ? r.status === 'needsYou' : f === 'changes' ? r.repos.some(x => x.files.length || x.commits.length) : true));
  let h = `<div class="view-title"><h1>${esc(t('v.history'))}</h1><span class="sub">${esc(t('hist.sub', {n: data.runs.length, m: Math.round(usedToday())}))}</span><span class="flex"></span>
    <div class="seg small">${['all', 'changes', 'needsYou', 'problems'].map(k => `<button class="${f === k ? 'active' : ''}" data-hist="${k}">${esc(t('hist.' + k))}</button>`).join('')}</div>
    <button class="btn-sm" data-act="clearHistory"${data.runs.length ? '' : ' disabled'}>${esc(t('a.clearHistory'))}</button></div>`;
  if (!runs.length) return h + `<div class="empty-s">${esc(t('hist.empty'))}</div>`;
  let day = null;
  h += '<div class="pcard">';
  for (const r of runs) {
    const d = new Date(r.started).toDateString();
    if (d !== day) { day = d; h += `<div class="group-h">${esc(new Date(r.started).toLocaleDateString(locale(), {weekday: 'long', day: 'numeric', month: 'long'}))}</div>`; }
    const st = stOf(r), files = r.repos.reduce((n, x) => n + x.files.length, 0), commits = r.repos.reduce((n, x) => n + x.commits.length, 0);
    h += `<div class="row${sel && sel.id === r.id ? ' sel' : ''}" data-run="${esc(r.id)}"><span class="dot ${esc(st)}"></span>
      <span class="meta" style="width:44px">${esc(new Date(r.started).toLocaleTimeString(locale(), {hour: '2-digit', minute: '2-digit'}))}</span>
      <span class="grow">${r.spec.continues ? '↳ ' : ''}${esc(r.spec.title)}</span>
      ${commits ? `<span class="meta">${esc(tn('run.commitsN', commits))}</span>` : files ? `<span class="meta">${esc(tn('run.filesN', files))}</span>` : ''}
      <span class="meta">${esc(fmtDur((r.ended || Date.now()) - r.started))}</span>${statusBadge(st)}</div>`;
  }
  return h + '</div>';
}

/* ================= Details panel ================= */
let logFor = null;
function renderDetails() {
  const box = $('#details');
  logFor = null;
  if (sel && sel.type === 'run' && runOf(sel.id)) return runDetails(box, runOf(sel.id));
  if (sel && sel.type === 'routine' && routineOf(sel.id)) return routineEditor(box, routineOf(sel.id));
  if (sel && sel.type === 'todo') return profileEditor(box);
  box.innerHTML = `<div class="empty-s">${esc(t('det.none'))}</div>
    <div class="group-h">${esc(t('det.levels'))}</div>${LEVELS.map(l => `<div class="kv"><span>${esc(t('lv.' + l))}</span></div><div class="meta" style="margin:-2px 0 6px">${esc(t('lv.' + l + '.hint'))}</div>`).join('')}`;
}
function runDetails(box, r) {
  const st = stOf(r);
  const commits = r.repos.flatMap(x => x.commits), files = r.repos.flatMap(x => x.files.map(f => ({dir: x.dir, f})));
  const canUndo = !r.undone && commits.length && !r.repos.some(x => x.pushed);
  const prev = r.spec.continues && runOf(r.spec.continues), next = nextOf(r.id);
  let h = `<div class="it-head"><b>${esc(r.spec.title)}</b>${statusBadge(st)}</div>
    <div class="kv"><span>${esc(t('det.started'))}</span><b>${esc(fmtWhen(r.started))}</b></div>
    <div class="kv"><span>${esc(t('det.duration'))}</span><b>${esc(fmtDur((r.ended || Date.now()) - r.started))}</b></div>
    <div class="kv"><span>${esc(t('det.rights'))}</span><b>${esc(t('lv.' + r.spec.level))}${r.spec.planOnly ? ' · ' + esc(t('rt.planOnly')) : ''}</b></div>
    <div class="kv"><span>${esc(t('det.dirs'))}</span><b title="${esc(r.spec.dirs.join('\n'))}">${esc(r.spec.dirs.map(baseName).join(', '))}</b></div>
    ${prev ? `<div class="kv"><span>${esc(t('det.continues'))}</span><b><a href="#" data-open-run="${esc(prev.id)}">${esc(fmtWhen(prev.started))}</a></b></div>` : ''}
    ${next ? `<div class="kv"><span>${esc(t('det.continued'))}</span><b><a href="#" data-open-run="${esc(next.id)}">${esc(fmtWhen(next.started))}</a></b></div>` : ''}
    <div class="d-actions">${st === 'running' ? `<button class="btn-sm" data-stop="${esc(r.id)}" style="color:var(--danger)">${esc(t('run.stop'))}</button>` : `<button class="btn-sm" data-again="${esc(r.id)}">${esc(t('run.again'))}</button>`}
      ${r.repos.length ? `<button class="btn-sm" data-diff="${esc(r.id)}">${esc(t('run.diff'))}</button>` : ''}
      ${canUndo ? `<button class="btn-sm" data-undo="${esc(r.id)}">${esc(t('run.undo'))}</button>` : ''}</div>`;
  if (r.error) h += `<div class="group-h" style="color:var(--danger)">${esc(t('det.problem'))}</div><div class="it-text">${esc(r.error)}</div>`;
  if (r.summary) h += `<div class="group-h">${esc(t('det.summary'))}</div><div class="it-text">${esc(r.summary)}</div>`;
  if (r.needs) h += `<div class="group-h" style="color:var(--warn)">${esc(t('kind.' + (r.needs.kind === 'approval' ? 'approval' : 'question')))}</div><div class="it-text">${esc(r.needs.text)}</div>`;
  if (r.denials.length) h += `<div class="group-h" style="color:var(--warn)">${esc(t('det.denied'))}</div><ul class="denials">${r.denials.map(d => `<li>${esc(d.what || d.rule)}</li>`).join('')}</ul>`;
  if (commits.length) h += `<div class="group-h">${esc(t('det.commits'))}${r.undone ? ' · ' + esc(t('run.undone')) : ''}${r.repos.some(x => x.pushed) ? ' · ' + esc(t('run.pushed')) : ''}</div><ul class="files">${commits.map(c => `<li>${esc(c)}</li>`).join('')}</ul>`;
  if (files.length) h += `<div class="group-h">${esc(tn('det.files', files.length))}</div><ul class="files">${files.slice(0, 80).map(x => `<li data-reveal="${esc(x.dir + '/' + x.f)}" title="${esc(x.dir)}">${esc(x.f)}</li>`).join('')}</ul>`;
  h += `<div class="group-h">${esc(t('det.log'))}</div><div class="log" id="runLog"><div class="info ph">…</div></div>`;
  if (r.sessionId || st !== 'running') h += `<div class="group-h">${esc(t('det.chat'))}</div><div class="chat"><textarea id="chatIn" placeholder="${esc(t('det.chatPh'))}"></textarea>
    <div class="d-actions" style="margin:0"><button class="btn-sm primary" data-chat="${esc(r.id)}"${st === 'running' ? ' disabled' : ''}>${esc(t('det.send'))}</button><span class="meta">${esc(t('det.chatHint'))}</span></div></div>`;
  box.innerHTML = h;
  logFor = r.id;
  if (invoke) invoke('run_log', {id: r.id}).then(ls => { if (logFor === r.id) { $('#runLog').innerHTML = ls.map(logLine).join('') || `<div class="info ph">${esc(t('det.noLog'))}</div>`; scrollLog(true); } });
}
const logLine = l => `<div class="${esc(l.kind)}"><time>${esc(new Date(l.t).toLocaleTimeString(locale()))}</time>${esc(l.text)}</div>`;
function scrollLog(force) { const el = $('#runLog'); if (el && (force || el.scrollHeight - el.scrollTop - el.clientHeight < 60)) el.scrollTop = el.scrollHeight; }
function dirsField(dirs, key) {
  return `<div class="chips">${dirs.map((d, i) => `<span class="fchip" title="${esc(d)}"><span>&lrm;${esc(d)}&lrm;</span><button class="x" data-dir-del="${key}|${i}">&#10005;</button></span>`).join('')}
    <button class="btn-sm" data-dir-add="${key}">${esc(t('ed.addDir'))}</button></div>`;
}
function rightsFields(o, key) {
  return `<div class="ed-field"><label>${esc(t('ed.dirs'))}</label>${dirsField(o.dirs || [], key)}<div class="hint">${esc(t('ed.dirsHint'))}</div></div>
    <div class="ed-field"><label>${esc(t('ed.level'))}</label><div class="lvl">${LEVELS.map((l, i) => `<button class="${o.level === l ? 'on l' + i : ''}" data-level="${key}|${l}">${esc(t('lv.' + l + '.short'))}</button>`).join('')}</div>
      <div class="hint">${esc(t('lv.' + o.level + '.hint'))}</div></div>
    ${LEVELS.indexOf(o.level) >= 1 ? `<div class="ed-field"><label>${esc(t('ed.commands'))}</label><input class="field-in" data-f="${key}|commands" value="${esc(o.commands || '')}" placeholder="cargo test, npm run build"><div class="hint">${esc(t('ed.commandsHint'))}</div></div>` : ''}
    <label class="switch"><input type="checkbox" data-sw="${key}|web"${o.web ? ' checked' : ''}><span></span>${esc(t('ed.web'))}</label>
    <label class="switch"><input type="checkbox" data-sw="${key}|noDelete"${o.noDelete !== false ? ' checked' : ''}><span></span>${esc(t('ed.noDelete'))}</label>
    <label class="switch"><input type="checkbox" data-sw="${key}|planOnly"${o.planOnly ? ' checked' : ''}><span></span>${esc(t('ed.planOnly'))}</label>
    <div class="ed-field"><label>${esc(t('ed.limit'))}</label><div class="two"><input type="number" min="0" max="600" step="5" data-f="${key}|maxMinutes" value="${esc(o.maxMinutes || 0)}">
      <select data-f="${key}|model">${['', 'opus', 'sonnet', 'haiku'].map(m => `<option value="${m}"${(o.model || '') === m ? ' selected' : ''}>${esc(m ? t('model.' + m) : t('model.default'))}</option>`).join('')}</select></div>
      <div class="hint">${esc(t('ed.limitHint'))}</div></div>`;
}
function routineEditor(box, r) {
  const s = r.schedule || (r.schedule = {kind: 'manual', time: '07:00', day: 0});
  const key = 'r:' + r.id;
  box.innerHTML = `<div class="ed-field"><label>${esc(t('ed.name'))}</label><input class="field-in" data-f="${key}|name" value="${esc(r.name)}"></div>
    <div class="ed-field"><label>${esc(t('ed.prompt'))}</label><textarea data-f="${key}|prompt" placeholder="${esc(t('ed.promptPh'))}">${esc(r.prompt)}</textarea></div>
    <div class="ed-field"><label>${esc(t('ed.when'))}</label><select data-sched="kind">${SCHEDULES.map(k => `<option value="${k}"${s.kind === k ? ' selected' : ''}>${esc(t('sch.' + k))}</option>`).join('')}</select>
      ${['daily', 'weekdays', 'weekly'].includes(s.kind) ? `<div class="two" style="margin-top:6px">${s.kind === 'weekly' ? `<select data-sched="day">${[0, 1, 2, 3, 4, 5, 6].map(d => `<option value="${d}"${+s.day === d ? ' selected' : ''}>${esc(dayName(d))}</option>`).join('')}</select>` : '<span></span>'}
        <input type="time" data-sched="time" value="${esc(s.time || '07:00')}"></div>` : ''}
      ${s.kind === 'todo' ? `<select data-f="${key}|todoFile" style="margin-top:6px"><option value="">${esc(t('ed.pickTodo'))}</option>${cfg.todoFiles.map(f => `<option value="${esc(f.path)}"${r.todoFile === f.path ? ' selected' : ''}>${esc(baseName(f.path))}</option>`).join('')}</select>` : ''}
      <div class="hint">${esc(t('sch.' + s.kind + '.hint'))}</div></div>
    ${rightsFields(r, key)}
    <div class="ed-field"><label>${esc(t('ed.rules'))}</label><textarea class="small" data-f="${key}|rules" placeholder="${esc(t('ed.rulesPh'))}">${esc(r.rules || '')}</textarea></div>
    <div class="d-actions"><button class="btn-sm primary" data-start="${esc(r.id)}"${routineRunning(r.id) ? ' disabled' : ''}>${esc(t('rt.start'))}</button>
      <button class="btn-sm" data-dup="${esc(r.id)}">${esc(t('rt.dup'))}</button><button class="btn-sm" data-del-routine="${esc(r.id)}" style="color:var(--danger)">${esc(t('rt.del'))}</button></div>`;
}
function profileEditor(box) {
  const p = cfg.todo;
  box.innerHTML = `<p class="meta" style="margin-top:0">${esc(t('todo.profileHint'))}</p>${rightsFields(p, 'todo')}
    <div class="ed-field"><label>${esc(t('todo.extra'))}</label><textarea class="small" data-f="todo|prompt" placeholder="${esc(t('todo.extraPh'))}">${esc(p.prompt || '')}</textarea></div>`;
}
function target(key) { return key === 'todo' ? cfg.todo : routineOf(key.slice(2)); }
$('#details').addEventListener('input', e => {
  const d = e.target.dataset;
  if (d.f) {
    const [key, f] = d.f.split('|'), o = target(key);
    if (!o) return;
    o[f] = f === 'maxMinutes' ? Math.max(0, +e.target.value || 0) : e.target.value;
    saveCfg();
    if (f === 'name' || f === 'todoFile') renderView();
  }
});
$('#details').addEventListener('change', e => {
  const d = e.target.dataset;
  if (d.sw) { const [key, f] = d.sw.split('|'), o = target(key); if (o) { o[f] = e.target.checked; saveCfg(); renderView(); } }
  if (d.sched && sel && sel.type === 'routine') {
    const r = routineOf(sel.id);
    r.schedule[d.sched] = d.sched === 'day' ? +e.target.value : e.target.value;
    if (d.sched === 'kind' && e.target.value === 'todo' && !r.todoFile) r.todoFile = (cfg.todoFiles[0] || {}).path || '';
    if (d.sched === 'kind' || d.sched === 'time' || d.sched === 'day') r.created = Math.max(r.created || 0, Date.now() - 1);   // a new time counts from now on
    saveCfg(); renderAll();
  }
  if (d.f && e.target.tagName === 'SELECT') renderAll();
});
$('#details').addEventListener('click', async e => {
  const b = e.target.closest('button,[data-reveal],a');
  if (!b) return;
  const d = b.dataset;
  if (d.level) { const [key, l] = d.level.split('|'); target(key).level = l; saveCfg(); renderAll(); return; }
  if (d.dirAdd) {
    if (!invoke) return toast(t('ui.appOnly'));
    const p = await invoke('pick_folder');
    if (p) { const o = target(d.dirAdd); o.dirs = [...new Set([...(o.dirs || []), p])]; saveCfg(); renderAll(); }
    return;
  }
  if (d.dirDel) { const [key, i] = d.dirDel.split('|'); target(key).dirs.splice(+i, 1); saveCfg(); renderAll(); return; }
  if (d.chat) {
    const text = $('#chatIn').value.trim();
    if (!text) return;
    try { const id = await invoke('run_message', {id: d.chat, text}); await refresh(); sel = {type: 'run', id}; renderAll(); } catch (err) { toast(String(err)); }
    return;
  }
  if (d.dup) { const r = routineOf(d.dup), c = {...JSON.parse(JSON.stringify(r)), id: uid(), name: r.name + ' ' + t('rt.copy'), created: Date.now(), lastRun: 0}; cfg.routines.push(c); saveCfg(); sel = {type: 'routine', id: c.id}; renderAll(); return; }
  if (d.delRoutine) {
    const r = routineOf(d.delRoutine);
    if (!await ask(t('rt.delTitle'), t('rt.delAsk', {name: r.name}), t('rt.del'))) return;
    cfg.routines = cfg.routines.filter(x => x.id !== r.id); sel = null; saveCfg(); renderAll(); return;
  }
  common(b, e);
});
/* Buttons that exist in the views and in the panel. */
async function common(b, e) {
  const d = b.dataset;
  if (d.openRun) { e && e.preventDefault(); sel = {type: 'run', id: d.openRun}; renderAll(); return true; }
  if (d.stop) { stopRun(d.stop); return true; }
  if (d.start) { const r = routineOf(d.start); if (r) startRoutine(r, true); return true; }
  if (d.again) { const r = runOf(d.again), f = firstOf(r); startRun({...f.spec, resume: null, continues: null, allowOnce: []}); return true; }
  if (d.diff) { showDiff(d.diff); return true; }
  if (d.undo) {
    if (!await ask(t('run.undoTitle'), t('run.undoAsk'), t('run.undo'))) return true;
    try { const n = await invoke('run_undo', {id: d.undo}); toast(tn('run.undoneN', n)); await refresh(); } catch (err) { toast(t('run.undoFail', {e: err})); }
    return true;
  }
  if (d.reveal) { if (invoke) invoke('reveal', {path: d.reveal}).catch(err => toast(String(err))); return true; }
  return false;
}
async function showDiff(id) {
  const text = invoke ? await invoke('run_diff', {id}) : '';
  modal(t('run.diff'), '');
  $('.modal').classList.add('wide');
  const html = (text || t('run.noDiff')).split('\n').map(l => {
    const c = l.startsWith('### ') ? 'f' : l.startsWith('+++') || l.startsWith('---') || l.startsWith('diff ') ? 'h' : l.startsWith('@@') ? 'h' : l.startsWith('+') ? 'a' : l.startsWith('-') ? 'r' : '';
    return c ? `<span class="${c}">${esc(l)}</span>` : esc(l);
  }).join('\n');
  $('#modalBody').innerHTML = `<pre class="diff">${html}</pre>`;
}

/* ================= View clicks ================= */
$('#view').addEventListener('click', async e => {
  const b = e.target.closest('button,[data-run],[data-routine],[data-todo-profile],a');
  if (!b) return;
  const d = b.dataset;
  if (b.tagName === 'BUTTON' && await common(b, e)) return;
  const note = id => { const el = $(`[data-note="${id}"]`); return el ? el.value : ''; };
  const answer = async (id, text, approve) => {
    try {
      const run = await invoke('inbox_answer', {id, text, approve: approve ?? null});
      await refresh();
      if (run) { sel = {type: 'run', id: run}; renderAll(); toast(t('inbox.going')); }
    } catch (err) { toast(String(err)); }
  };
  if (d.approve) return answer(d.approve, note(d.approve), true);
  if (d.reject) return answer(d.reject, note(d.reject), false);
  if (d.option) return answer(d.option, d.val + (note(d.option) ? '\n' + note(d.option) : ''));
  if (d.answer) { const v = note(d.answer).trim(); if (!v) { const el = $(`[data-note="${d.answer}"]`); el && el.focus(); return; } return answer(d.answer, v); }
  if (d.dismiss) { await invoke('inbox_dismiss', {ids: [d.dismiss]}); return refresh(); }
  if (d.go === 'claude') return openSettings('claude');
  if (d.go === 'login') { openSettings('claude'); return login(); }
  if (d.hist) { settings.histFilter = d.hist; saveSettings(); return renderView(); }
  if (d.stopRoutine) { const r = data.runs.find(x => x.spec.routineId === d.stopRoutine && isRunning(x.id)); if (r) stopRun(r.id); return; }
  if (d.doitem) { const it = (todos[d.doitem].items || []).find(i => i.line === +d.line); if (it) doItem(d.doitem, it); return; }
  if (d.doall) return doList(d.doall);
  if (d.unlink) { cfg.todoFiles = cfg.todoFiles.filter(f => f.path !== d.unlink); saveCfg(); return renderView(); }
  if (d.todoEdit !== undefined || d.todoProfile !== undefined) { sel = {type: 'todo'}; return renderAll(); }
  if (d.run) { sel = {type: 'run', id: d.run}; return renderAll(); }
  if (d.routine) { sel = {type: 'routine', id: d.routine}; return renderAll(); }
});
$('#view').addEventListener('change', e => {
  const d = e.target.dataset;
  if (d.tick) { const f = cfg.todoFiles.find(x => x.path === d.tick); f.tick = e.target.checked; saveCfg(); }
  if (d.enable) { const r = routineOf(d.enable); r.enabled = e.target.checked; if (r.enabled) r.created = Math.max(r.created || 0, Date.now() - 1); saveCfg(); renderAll(); }
});
$('#view').addEventListener('keydown', e => {
  const n = e.target.dataset && e.target.dataset.note;
  if (n && e.key === 'Enter') { const b = $(`[data-answer="${n}"]`) || $(`[data-approve="${n}"]`); if (b) b.click(); }
});

/* ================= New task ================= */
let lastTask = null;
function newTask() {
  const o = lastTask || {title: '', prompt: '', dirs: [...(cfg.todo.dirs || [])], level: 'edit', commands: '', web: false, noDelete: true, planOnly: false, maxMinutes: 30, model: ''};
  lastTask = o;
  modal(t('task.title'), `<div class="task-form"><div class="ed-field"><label>${esc(t('task.what'))}</label><textarea id="taskPrompt" placeholder="${esc(t('task.whatPh'))}">${esc(o.prompt)}</textarea></div>
    <div id="taskRights"></div></div>`);
  const draw = () => { $('#taskRights').innerHTML = rightsFields(o, 'task'); };
  draw();
  $('#modalFoot').hidden = false;
  $('#modalFoot').innerHTML = `<span class="flex"></span><button class="btn-sm" id="taskSave">${esc(t('task.asRoutine'))}</button><button class="btn-sm primary" id="taskGo">${esc(t('task.go'))}</button>`;
  const form = $('#modalBody');
  form.oninput = e => {
    if (e.target.id === 'taskPrompt') o.prompt = e.target.value;
    const f = e.target.dataset.f; if (f) { const k = f.split('|')[1]; o[k] = k === 'maxMinutes' ? Math.max(0, +e.target.value || 0) : e.target.value; }
  };
  form.onchange = e => { const s = e.target.dataset.sw; if (s) { o[s.split('|')[1]] = e.target.checked; } };
  form.onclick = async e => {
    const b = e.target.closest('button'); if (!b) return;
    const d = b.dataset;
    if (d.level) { o.level = d.level.split('|')[1]; draw(); }
    if (d.dirAdd) { if (!invoke) return toast(t('ui.appOnly')); const p = await invoke('pick_folder'); if (p) { o.dirs = [...new Set([...o.dirs, p])]; draw(); } }
    if (d.dirDel) { o.dirs.splice(+d.dirDel.split('|')[1], 1); draw(); }
  };
  const valid = () => {
    if (!o.prompt.trim()) { toast(t('rt.noPrompt')); $('#taskPrompt').focus(); return false; }
    if (!o.dirs.length) { toast(t('rt.noDirs')); return false; }
    return true;
  };
  $('#taskGo').onclick = async () => {
    if (!valid()) return;
    const title = o.prompt.trim().split('\n')[0].slice(0, 70);
    closeModal();
    if (await startRun(specFrom({...o, title}))) lastTask = null;
  };
  $('#taskSave').onclick = () => {
    if (!valid()) return;
    const r = {...fromTemplate('empty'), ...JSON.parse(JSON.stringify(o)), id: uid(), name: o.prompt.trim().split('\n')[0].slice(0, 50), created: Date.now(), enabled: true, schedule: {kind: 'manual', time: '07:00', day: 0}};
    cfg.routines.push(r); saveCfg(); lastTask = null; closeModal();
    sel = {type: 'routine', id: r.id}; setView('routines');
  };
  setTimeout(() => $('#taskPrompt').focus(), 0);
}

/* ================= Status bar, title ================= */
function updateStatus() {
  const run = data.running.length, need = needItems().length;
  const s = $('#stRun');
  s.className = 'st-run' + (run ? ' busy' : need ? ' need' : '');
  s.textContent = run ? t('stat.running', {n: run}) : need ? tn('stat.need', need) : t('stat.idle');
  const used = Math.round(usedToday());
  $('#stUsage').textContent = +cfg.dailyMinutes > 0 ? t('stat.usageOf', {n: used, m: cfg.dailyMinutes}) : t('stat.usage', {n: used});
  if (overLimit()) $('#stUsage').textContent += ' · ' + t('stat.limit');
  const nexts = cfg.routines.filter(r => r.enabled).map(r => ({r, at: nextSlot(r)})).filter(x => x.at).sort((a, b) => a.at - b.at);
  $('#stNext').textContent = nexts.length ? t('stat.next', {name: nexts[0].r.name, w: fmtWhen(nexts[0].at)}) : '';
  const st = claude.status;
  $('#stClaude').textContent = !invoke ? t('ui.browser') : !claude.checked ? '' : !claude.path ? t('cl.notFound')
    : st && st.loggedIn ? t('cl.ok', {who: st.email || st.account || st.orgName || 'Claude'}) : t('cl.signIn');
  const stop = $('#stopAll'); if (stop) stop.disabled = !run;
  const c = $('#toolInboxCount'); if (c) { c.textContent = need; c.hidden = !need; }
  $('#ctxBar').innerHTML = run ? esc(t('ctx.running', {n: run})) : '';
}
function syncTools() { $$('#tools .view').forEach(b => b.classList.toggle('active', b.dataset.act === 'view' + settings.view[0].toUpperCase() + settings.view.slice(1))); syncDisabled(); }

/* ================= Claude Code: where, sign-in ================= */
async function checkClaude() {
  if (!invoke) return;
  claude = {...await invoke('claude_status', {own: cfg.claude || ''}).catch(() => ({path: null, version: '', status: null})), checked: true};
  if (booted) { updateStatus(); if (settings.view === 'inbox') renderView(); if (settingsOpen) renderSettings(); }
}
let loginOut = '', loginBusy = false;
async function login() {
  if (!invoke) return;
  loginOut = ''; loginBusy = true; if (settingsOpen) renderSettings();
  try { await invoke('login_start', {own: cfg.claude || ''}); } catch (e) { loginBusy = false; loginOut = String(e); if (settingsOpen) renderSettings(); }
}
listen('login-output', e => {
  loginOut += e.payload + '\n';
  const el = $('#loginOut'); if (el) { el.textContent = loginOut; el.scrollTop = el.scrollHeight; } else if (settingsOpen) renderSettings();
  // The sign-in page — opened by Claude Code itself; the link stays here in case no browser opened.
});
listen('login-done', e => { loginBusy = false; toast(e.payload ? t('cl.loginOk') : t('cl.loginFail')); checkClaude(); if (settingsOpen) renderSettings(); });

/* ================= Menus, toolbars ================= */
const A = {
  newTask, newRoutine, linkTodo, stopAll,
  runSel: () => { if (sel && sel.type === 'routine') startRoutine(routineOf(sel.id)); else if (sel && sel.type === 'run' && runOf(sel.id)) startRun({...firstOf(runOf(sel.id)).spec, resume: null, continues: null, allowOnce: []}); },
  dismissDone: async () => { const ids = data.inbox.filter(i => i.kind === 'done' || i.kind === 'problem').map(i => i.id); if (ids.length) { await invoke('inbox_dismiss', {ids}); refresh(); } },
  clearHistory: async () => { if (await ask(t('a.clearHistory'), t('hist.clearAsk'), t('a.clearHistory'))) { await invoke('runs_clear'); sel = null; refresh(); } },
  viewInbox: () => setView('inbox'), viewTodos: () => setView('todos'), viewRoutines: () => setView('routines'), viewHistory: () => setView('history'),
  togglePanels: () => { settings.dock = !settings.dock; applySettings(); },
  toggleAutoHide: () => { settings.autoHide = !settings.autoHide; applySettings(); },
  settings: () => openSettings(), claudeSettings: () => openSettings('claude'), controlSettings: () => openSettings('control'),
  langAuto: () => setLang('auto'), langEn: () => setLang('en'), langDe: () => setLang('de'),
  themeDark: () => setTheme('dark'), themeLight: () => setTheme('light'), themeGithub: () => setTheme('github'), themeGithubDark: () => setTheme('github-dark'),
  shortcuts: () => {
    let h = '';
    MENUS.forEach(([m, items]) => {
      const rows = items.filter(it => Array.isArray(it) && it[1]);
      if (rows.length) h += `<h4>${esc(t('menu.' + m))}</h4><table>` + rows.map(it => `<tr><td>${esc(t('a.' + it[0]))}</td><td><kbd>${esc(fmtKey(it[1]))}</kbd></td></tr>`).join('') + '</table>';
    });
    modal(t('a.shortcuts'), h);
  },
  about: async () => {
    const v = invoke ? await window.__TAURI__.app.getVersion().catch(() => '') : '';
    modal(t('a.about'), `${v ? `<p><b>${esc(t('about.version', {v}))}</b></p>` : ''}<p>${esc(t('about.p1'))}</p><p>${esc(t('about.p2'))}</p><p>${esc(t('about.p3'))}</p>`);
  }
};
// Menu items: [action, shortcut, checked(), fixed]; '-' is a separator, '#key' a section label (as in every Studio app).
const MENUS = [
  ['agent', [['newTask', 'Ctrl+N'], ['newRoutine', 'Ctrl+Shift+N'], ['linkTodo', 'Ctrl+L'], '-', ['runSel', 'Ctrl+R'], ['stopAll', 'Ctrl+.'], '-', ['dismissDone'], ['clearHistory']]],
  ['view', [['viewInbox', 'Alt+1', () => settings.view === 'inbox'], ['viewTodos', 'Alt+2', () => settings.view === 'todos'], ['viewRoutines', 'Alt+3', () => settings.view === 'routines'],
    ['viewHistory', 'Alt+4', () => settings.view === 'history'], '-', ['togglePanels', 'Ctrl+\\', () => settings.dock], ['toggleAutoHide', null, () => settings.autoHide]]],
  ['settings', [['settings', 'Ctrl+,'], ['claudeSettings'], ['controlSettings'], '-', '#menu.language',
    ['langAuto', null, () => settings.lang === 'auto'], ['langEn', null, () => settings.lang === 'en'], ['langDe', null, () => settings.lang === 'de'],
    '-', '#menu.theme',
    ['themeDark', null, () => settings.theme === 'dark'], ['themeLight', null, () => settings.theme === 'light'],
    ['themeGithub', null, () => settings.theme === 'github'], ['themeGithubDark', null, () => settings.theme === 'github-dark']]],
  ['help', [['shortcuts', 'F1'], '-', ['about']]]
];
const allowed = act => (act !== 'stopAll' || data.running.length > 0) && (act !== 'runSel' || !!(sel && (sel.type === 'routine' || sel.type === 'run')))
  && (act !== 'dismissDone' || data.inbox.some(i => i.kind === 'done' || i.kind === 'problem')) && (act !== 'clearHistory' || data.runs.length > 0);
function run(act) { if (A[act] && allowed(act)) A[act](); }
function syncDisabled() { $$('[data-act]').forEach(b => b.disabled = !allowed(b.dataset.act)); }
const KEYMAP = {};
function buildKeymap() {
  for (const k in KEYMAP) delete KEYMAP[k];
  MENUS.forEach(([, items]) => items.forEach(it => { if (Array.isArray(it) && !it[3]) [it[1], ...(it[4] || [])].forEach(k => { if (k) KEYMAP[k] = it[0]; }); }));
}
let menuOpen = null;
function openMenu(m, items) {
  closeMenus();
  const dd = m.querySelector('.dropdown'); dd.innerHTML = '';
  items.forEach(it => {
    if (it === '-') return dd.append(document.createElement('hr'));
    if (typeof it === 'string') return dd.append(Object.assign(document.createElement('div'), {className: 'dd-label', textContent: t(it.slice(1))}));
    const [act, key, check] = it, b = document.createElement('button');
    b.innerHTML = `<span class="chk">${check && check() ? '&#10003;' : ''}</span><span>${esc(t('a.' + act))}</span><kbd>${esc(key ? fmtKey(key) : '')}</kbd>`;
    b.disabled = !allowed(act);
    b.onmousedown = e => e.preventDefault();
    b.onclick = () => { closeMenus(); run(act); };
    dd.append(b);
  });
  m.classList.add('open'); menuOpen = m;
}
function closeMenus() { $$('.menu.open').forEach(x => x.classList.remove('open')); menuOpen = null; }
function buildMenubar() {
  const nav = $('#menus'); nav.innerHTML = ''; menuOpen = null;
  MENUS.forEach(([name, items]) => {
    const m = document.createElement('div'), b = document.createElement('button'), dd = document.createElement('div');
    m.className = 'menu'; dd.className = 'dropdown'; b.textContent = t('menu.' + name);
    b.onmousedown = e => e.preventDefault();
    b.onclick = () => menuOpen === m ? closeMenus() : openMenu(m, items);
    b.onmouseenter = () => { if (menuOpen && menuOpen !== m) openMenu(m, items); };
    m.append(b, dd); nav.append(m);
  });
}
document.addEventListener('mousedown', e => { if (menuOpen && !e.target.closest('.menu')) closeMenus(); });
const keyOf = act => { for (const [, items] of MENUS) for (const it of items) if (Array.isArray(it) && it[0] === act && it[1]) return it[1]; return ''; };
const tip = act => t('a.' + act) + (keyOf(act) ? ` (${fmtKey(keyOf(act))})` : '');

const SV = p => `<svg viewBox="0 0 24 24">${p}</svg>`;
const ICONS = {
  inbox: SV('<path d="M22 12h-6l-2 3h-4l-2-3H2"/><path d="M5.5 5h13l3.5 7v6a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2v-6z"/>'),
  todos: SV('<path d="m4 7 2 2 4-4M4 17l2 2 4-4M13 7h8M13 17h8"/>'),
  routines: SV('<path d="M21 12a9 9 0 1 1-3-6.7L21 8"/><path d="M21 3v5h-5M12 7v5l3 2"/>'),
  history: SV('<path d="M8 6h13M8 12h13M8 18h13M3 6h.01M3 12h.01M3 18h.01"/>'),
  task: SV('<path d="M12 5v14M5 12h14"/>'),
  routine: SV('<rect x="3" y="4" width="18" height="17" rx="2"/><path d="M3 9h18M8 2v4M16 2v4M12 12v5M9.5 14.5h5"/>'),
  link: SV('<path d="M10 13a5 5 0 0 0 7.5.5l3-3a5 5 0 0 0-7-7l-1.7 1.7"/><path d="M14 11a5 5 0 0 0-7.5-.5l-3 3a5 5 0 0 0 7 7l1.7-1.7"/>'),
  play: SV('<path d="M7 4v16l13-8z"/>'),
  panel: SV('<rect x="3" y="3" width="18" height="18" rx="2"/><path d="M15 3v18"/>'),
  help: SV('<circle cx="12" cy="12" r="9"/><path d="M9.5 9a2.5 2.5 0 1 1 3.5 2.3c-.6.3-1 .9-1 1.6V14M12 17.5v.01"/>'),
  gear: SV('<circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z"/>')
};
function makeBtn(act, html, cls = '') {
  const b = document.createElement('button');
  b.className = 'ibtn' + (cls ? ' ' + cls : ''); b.innerHTML = html; b.title = tip(act); b.dataset.act = act;
  return b;
}
function buildToolbars() {
  const fill = (id, list) => { const box = $(id); box.innerHTML = ''; list.forEach(([a, i]) => box.append(makeBtn(a, i))); };
  fill('#fileBtns', [['newTask', ICONS.task], ['newRoutine', ICONS.routine], ['linkTodo', ICONS.link], ['runSel', ICONS.play]]);
  fill('#viewBtns', [['togglePanels', ICONS.panel], ['settings', ICONS.gear], ['shortcuts', ICONS.help]]);
  const stop = Object.assign(document.createElement('button'), {className: 'stop-btn', id: 'stopAll', textContent: t('ui.stopAll'), title: tip('stopAll')});
  stop.dataset.act = 'stopAll';
  $('#viewBtns').prepend(stop);
  const tools = $('#tools'); tools.innerHTML = '';
  VIEWS.forEach(v => {
    const b = makeBtn('view' + v[0].toUpperCase() + v.slice(1), ICONS[v], 'view');
    if (v === 'inbox') b.insertAdjacentHTML('beforeend', '<span class="count" id="toolInboxCount" hidden></span>');
    tools.append(b);
  });
  $$('[data-act="togglePanels"]').forEach(b => b.classList.toggle('active', settings.dock));
  syncDisabled();
}
document.addEventListener('mousedown', e => { if (e.target.closest('.ibtn,.cta')) e.preventDefault(); });
document.addEventListener('click', e => { const b = e.target.closest('[data-act]'); if (b && !b.disabled) run(b.dataset.act); });

/* ================= Modal, settings ================= */
let toastT;
function toast(msg) { const n = $('#toast'); n.textContent = msg; n.classList.add('show'); clearTimeout(toastT); toastT = setTimeout(() => n.classList.remove('show'), 3200); }
async function ask(title, message, ok) {
  if (invoke) return invoke('confirm', {title, message, okLabel: ok, cancelLabel: t('ui.cancel')}).catch(() => false);
  return window.confirm(message);
}
let settingsOpen = false, prefsSection = 'general';
function modal(title, html) {
  $('.modal').classList.remove('wide'); $('#modalFoot').hidden = true; $('#modalFoot').innerHTML = ''; settingsOpen = false;
  const b = $('#modalBody'); b.oninput = b.onchange = b.onclick = null;
  $('#modalTitle').textContent = title; b.innerHTML = html; $('#modal').classList.add('show');
}
function closeModal() { $('#modal').classList.remove('show'); settingsOpen = false; }
$('#modalClose').onclick = closeModal;
$('#modal').onmousedown = e => { if (e.target.id === 'modal') closeModal(); };
function openSettings(section) {
  modal(t('menu.settings'), '');
  if (section) prefsSection = section;
  $('.modal').classList.add('wide');
  settingsOpen = true; renderSettings();
  if (section === 'claude' || !claude.checked) checkClaude();
}
function renderSettings() {
  const opt = (v, label, c) => `<option value="${v}"${String(v) === String(c) ? ' selected' : ''}>${esc(label)}</option>`;
  const sw = (name, label, on) => `<label class="switch"><input type="checkbox" data-set="${name}"${on ? ' checked' : ''}><span></span>${esc(label)}</label>`;
  const mock = th => `<div class="tc-mock" data-theme="${th}"><div class="tc-bar"><i></i><i></i><i></i><b></b></div>
    <div class="tc-body" style="grid-template-columns:14px 1fr 38px"><div class="tc-rail"></div>
    <div class="tc-pv" style="padding:10px 12px"><div class="tc-paper" style="border-radius:3px;border-bottom:var(--paper-border)"><i></i><i style="width:70%"></i><i style="width:50%"></i></div></div><div class="tc-dock"></div></div></div>`;
  const st = claude.status || {};
  const used = Math.round(usedToday());
  const body = {
    general: `<h3>${esc(t('set.language'))}</h3><p class="hint">${esc(t('set.languageHint'))}</p>
      <select class="wide-sel" data-set="lang">${opt('auto', t('a.langAuto'), settings.lang)}${opt('en', 'English', settings.lang)}${opt('de', 'Deutsch', settings.lang)}</select>
      <h3>${esc(t('set.layout'))}</h3>${sw('dock', t('set.panels'), settings.dock)}${sw('autoHide', t('set.autoHide'), settings.autoHide)}
      <h3>${esc(t('set.notes'))}</h3>${sw('doneNotes', t('set.doneNotes'), cfg.doneNotes)}<p class="hint">${esc(t('set.doneNotesHint'))}</p>`,
    claude: `<h3>${esc(t('cl.program'))}</h3><p class="hint">${esc(t('cl.programHint'))}</p>
      ${!invoke ? `<p class="hint">${esc(t('ui.appOnly'))}</p>` : `
      <p>${claude.checked ? (claude.path ? `<b>${esc(t('cl.found'))}</b> ${esc(claude.version)}<br><span class="meta">${esc(claude.path)}</span>` : `<b style="color:var(--danger)">${esc(t('cl.notFound'))}</b><br><span class="meta">${esc(t('cl.notFoundHint'))}</span>`) : esc(t('cl.checking'))}</p>
      <div class="ed-field" style="max-width:560px"><label>${esc(t('cl.own'))}</label><input class="field-in" data-set="claude" value="${esc(cfg.claude || '')}" placeholder="${esc(t('cl.auto'))}" style="width:100%"></div>
      <p><button class="btn-sm" data-cl="pick">${esc(t('cl.pick'))}</button> <button class="btn-sm" data-cl="check">${esc(t('cl.check'))}</button></p>
      <h3>${esc(t('cl.account'))}</h3>
      ${claude.path ? (st.loggedIn ? `<p>${esc(t('cl.ok', {who: st.email || st.account || st.orgName || 'Claude'}))}${st.authMethod ? `<br><span class="meta">${esc(st.authMethod)}${st.subscriptionType ? ' · ' + esc(st.subscriptionType) : ''}</span>` : ''}</p>`
        : `<p class="hint">${esc(t('cl.signInHint'))}</p><p><button class="btn-sm primary" data-cl="login"${loginBusy ? ' disabled' : ''}>${esc(loginBusy ? t('cl.loginBusy') : t('cl.login'))}</button></p>`) : ''}
      ${loginOut || loginBusy ? `<div class="login-out" id="loginOut">${esc(loginOut || '…')}</div>
        ${loginBusy ? `<div class="ed-field" style="max-width:560px;margin-top:8px"><label>${esc(t('cl.code'))}</label><div style="display:flex;gap:6px"><input class="field-in" id="loginCode" style="flex:1" autocomplete="off"><button class="btn-sm" data-cl="code">${esc(t('cl.sendCode'))}</button></div></div>` : ''}` : ''}
      <p class="hint">${esc(t('cl.privacy'))}</p>
      <h3>${esc(t('cl.model'))}</h3><select class="wide-sel" data-set="model">${['', 'opus', 'sonnet', 'haiku'].map(m => opt(m, m ? t('model.' + m) : t('model.default'), cfg.model || '')).join('')}</select>`}`,
    control: `<h3>${esc(t('ctl.rules'))}</h3><p class="hint">${esc(t('ctl.rulesHint'))}</p>
      <div class="ed-field" style="max-width:620px"><textarea data-set="rules" style="height:140px" placeholder="${esc(t('ctl.rulesPh'))}">${esc(cfg.rules || '')}</textarea></div>
      <h3>${esc(t('ctl.limit'))}</h3><p class="hint">${esc(t('ctl.limitHint'))}</p>
      <select class="wide-sel" data-set="dailyMinutes">${[0, 30, 60, 120, 180, 300, 480].map(n => opt(n, n ? t('ctl.limitN', {n}) : t('ctl.limit0'), cfg.dailyMinutes || 0)).join('')}</select>
      <p class="hint">${esc(t('stat.usage', {n: used}))}</p>${+cfg.dailyMinutes > 0 ? `<div class="usage"><i style="width:${Math.min(100, used / cfg.dailyMinutes * 100)}%"></i></div>` : ''}
      <h3>${esc(t('ctl.safety'))}</h3><p class="hint">${esc(t('ctl.safetyHint'))}</p>`,
    appearance: `<h3>${esc(t('set.theme'))}</h3><div class="theme-grid">` + THEMES.map(th =>
      `<button class="theme-card${settings.theme === th ? ' active' : ''}" data-theme-pick="${th}">${mock(th)}
       <div class="theme-name">${esc(t('theme.' + th))}<span>${settings.theme === th ? '&#10003;' : ''}</span></div></button>`).join('') + '</div>',
    keys: '<div id="keysBox"></div>'
  };
  const sections = ['general', 'claude', 'control', 'appearance', 'keys'];
  $('#modalBody').innerHTML = `<div class="prefs"><nav>${sections.map(s => `<button data-sec="${s}" class="${s === prefsSection ? 'active' : ''}">${esc(t('set.' + s))}</button>`).join('')}</nav>
    <section>${body[prefsSection]}</section></div>`;
  if (prefsSection === 'keys') Shortcuts.editor($('#keysBox'), {MENUS, keys: settings.keys || {}, t, fmtKey, comboOf, label: a => t('a.' + a),
    onChange: keys => { settings.keys = keys; applySettings(); }});
  const lo = $('#loginOut'); if (lo) lo.scrollTop = lo.scrollHeight;
  const b = $('#modalBody');
  b.onclick = async e => {
    const el = e.target.closest('button'); if (!el) return;
    if (el.dataset.sec) { prefsSection = el.dataset.sec; renderSettings(); return; }
    if (el.dataset.themePick) { setTheme(el.dataset.themePick); renderSettings(); return; }
    const c = el.dataset.cl;
    if (c === 'check') checkClaude();
    if (c === 'login') login();
    if (c === 'pick') { const p = await invoke('pick_file', {filterName: 'Claude Code', exts: []}); if (p) { cfg.claude = p; saveCfg(); checkClaude(); renderSettings(); } }
    if (c === 'code') { const v = $('#loginCode').value; if (v.trim()) invoke('login_input', {text: v}).then(() => { $('#loginCode').value = ''; }).catch(err => toast(String(err))); }
  };
  b.onchange = e => {
    const k = e.target.dataset.set; if (!k) return;
    if (k === 'lang') { setLang(e.target.value); renderSettings(); return; }
    if (k === 'dock' || k === 'autoHide') { settings[k] = e.target.checked; applySettings(); return; }
    if (k === 'doneNotes') { cfg.doneNotes = e.target.checked; saveCfg(); refresh(); return; }
    if (k === 'dailyMinutes') { cfg.dailyMinutes = +e.target.value; limitNoted = false; saveCfg(); updateStatus(); renderSettings(); return; }
    if (k === 'model') { cfg.model = e.target.value; saveCfg(); return; }
    if (k === 'claude') { cfg.claude = e.target.value.trim(); saveCfg(); checkClaude(); }
  };
  b.oninput = e => { if (e.target.dataset.set === 'rules') { cfg.rules = e.target.value; saveCfg(); } };
}
function setLang(l) { settings.lang = l; applySettings(); }
function setTheme(th) { settings.theme = th; applySettings(); }
function applyLanguage() {
  document.documentElement.lang = lang;
  $$('[data-i18n]').forEach(n => n.textContent = t(n.dataset.i18n));
  buildMenubar(); buildToolbars();
  if (booted) renderAll();
}
let appliedKeys = null;
function applySettings() {
  if (window.AutoHide) AutoHide.set(settings.autoHide);
  if (!THEMES.includes(settings.theme)) settings.theme = 'dark';
  if (!VIEWS.includes(settings.view)) settings.view = 'inbox';
  if (window.Shared) Shared.push({theme: settings.theme, lang: settings.lang});
  document.documentElement.dataset.theme = settings.theme;
  if (JSON.stringify(settings.keys || {}) !== appliedKeys) { appliedKeys = JSON.stringify(settings.keys || {}); Shortcuts.apply(MENUS, settings.keys || {}); buildKeymap(); buildToolbars(); }
  const nl = effectiveLang();
  if (nl !== lang) { lang = nl; applyLanguage(); }
  $('#dock').classList.toggle('hidden', !settings.dock);
  $$('[data-act="togglePanels"]').forEach(b => b.classList.toggle('active', settings.dock));
  $$('#dockTabs button').forEach(b => b.classList.add('active'));
  if (booted) renderAll();
  saveSettings();
}

/* ================= Keyboard ================= */
function comboOf(e) {
  let k = e.key;
  if (/^Digit\d$/.test(e.code)) k = e.code.slice(5);
  else if (e.code === 'Comma') k = ',';
  else if (e.code === 'Period') k = '.';
  else if (e.code === 'Backslash' || e.code === 'IntlBackslash') k = '\\';
  else if (k.length === 1) k = k.toUpperCase();
  return (e.ctrlKey || e.metaKey ? 'Ctrl+' : '') + (e.shiftKey ? 'Shift+' : '') + (e.altKey ? 'Alt+' : '') + k;
}
const typing = () => { const a = document.activeElement; return a && (a.tagName === 'INPUT' || a.tagName === 'TEXTAREA' || a.tagName === 'SELECT' || a.isContentEditable); };
window.addEventListener('keydown', e => {
  if (window.Shortcuts && Shortcuts.recording) return;
  if (e.key === 'Escape') {
    if ($('#modal').classList.contains('show')) closeModal();
    else if (menuOpen) closeMenus();
    else if (sel) { sel = null; renderAll(); }
    return;
  }
  if (e.ctrlKey && e.altKey) return;
  const combo = comboOf(e);
  if (typing() && ['Ctrl+Z', 'Ctrl+Y', 'Ctrl+Shift+Z', 'Delete', 'Backspace', 'Ctrl+A'].includes(combo)) return;
  if ($('#modal').classList.contains('show')) return;
  const act = KEYMAP[combo];
  if (act) { e.preventDefault(); e.stopPropagation(); run(act); }
}, true);

/* Native right-click menus (native.js) */
{
  const item = act => ({id: act, label: t('a.' + act), accel: keyOf(act) || null, enabled: allowed(act)});
  Native.setProvider(e => {
    if (e.target.closest('#view') && !e.target.closest('input,textarea')) return [item('newTask'), item('newRoutine'), item('linkTodo'), null, item('stopAll')];
    return null;
  });
  Native.setHandler(id => run(id));
}

/* Test hook (like Plan Studio's __pl). */
window.__ag = {get cfg() { return cfg; }, get data() { return data; }, get sel() { return sel; }, set sel(v) { sel = v; renderAll(); }, A, run, setView, specFrom, lastSlot, nextSlot, planTick, refresh};

/* ================= Boot ================= */
(async function boot() {
  await loadTexts();
  let saved = null;
  try { saved = JSON.parse(localStorage.getItem(STORE)); } catch (e) {}
  if (saved && saved.settings) settings = {...DEF, ...saved.settings};
  // Theme and language shared by all Studio apps (shared.js); the shared values win over this app's own.
  if (window.Shared) await Shared.init({apply: sh => {
    if (sh.theme) settings.theme = sh.theme;
    if (sh.lang) settings.lang = sh.lang;
    if (booted) applySettings();
  }});
  lang = effectiveLang();
  applyLanguage();
  try {
    const text = invoke ? await invoke('config_load') : saved && saved.cfg ? JSON.stringify(saved.cfg) : null;
    const c = text ? JSON.parse(text) : null;
    cfg = {...newCfg(), ...(c || {})};
    cfg.todo = {...newCfg().todo, ...(cfg.todo || {})};
    if (invoke) invoke('config_save', {content: JSON.stringify(cfg, null, 1), claude: cfg.claude || ''}).catch(() => {});
  } catch (e) { cfg = newCfg(); toast(t('toast.loadFail', {e})); }
  await Promise.all(cfg.todoFiles.map(f => loadTodo(f.path)));
  await refresh();
  booted = true;
  applySettings();
  renderAll();
  checkClaude();
  if (invoke) {
    listen('runs-changed', refreshSoon);
    listen('run-log', e => {
      const {id, line} = e.payload;
      const r = runOf(id); if (r && (line.kind === 'tool' || line.kind === 'text')) r.last = line.text.slice(0, 200);
      if (logFor === id) { const el = $('#runLog'); if (el) { el.querySelectorAll('.ph').forEach(x => x.remove()); el.insertAdjacentHTML('beforeend', logLine(line)); scrollLog(); } }
      if (settings.view === 'inbox' && !typing()) { const c = $(`[data-run="${CSS.escape(id)}"] .it-text`); if (c && r) c.textContent = r.last; }
    });
  }
  setInterval(watchTodos, 4000);
  setInterval(planTick, 30000);
  setTimeout(planTick, 16000);
  // Durations and "in 5 min" go on.
  setInterval(() => { if (!typing() && booted) { updateStatus(); if (data.running.length && settings.view === 'inbox') renderView(); } }, 30000);
})();
