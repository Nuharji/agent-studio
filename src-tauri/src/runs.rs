//! The runs (history with logs) and the "Needs you" box. Kept in the app's data folder: state.json (runs and the
//! box) and runs/<id>.jsonl (the log of each run). One run = one `claude` process (runner.rs).

use crate::runner::{self, Denial, LogLine, Needs, Repo, Spec};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Run {
    pub id: String,
    pub spec: Spec,
    pub started: u64,
    pub ended: u64,
    /// "running", "done", "needsYou", "failed", "stopped", "timeout".
    pub status: String,
    pub session_id: String,
    pub summary: String,
    pub needs: Option<Needs>,
    pub denials: Vec<Denial>,
    pub cost: f64,
    pub repos: Vec<Repo>,
    pub error: String,
    pub undone: bool,
    /// The last step (shown in the lists while it runs).
    pub last: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Item {
    pub id: String,
    pub run_id: String,
    /// "question", "approval", "done" or "problem".
    pub kind: String,
    pub title: String,
    pub text: String,
    pub options: Vec<String>,
    /// For an approval: the rules that would allow what was refused.
    pub rules: Vec<String>,
    pub denials: Vec<Denial>,
    pub created: u64,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Data {
    pub runs: Vec<Run>,
    pub inbox: Vec<Item>,
}

#[derive(Default)]
pub struct Agent {
    pub data: Mutex<Data>,
    pids: Mutex<HashMap<String, u32>>,
    stopping: Mutex<HashSet<String>>,
    pub claude: Mutex<String>,
    pub login: Mutex<Option<std::process::ChildStdin>>,
}

const KEEP: usize = 400;

pub fn now() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0) }
fn new_id() -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    static N: AtomicU32 = AtomicU32::new(0);
    format!("{:x}{:03x}", now(), N.fetch_add(1, Ordering::Relaxed) % 4096)
}

pub fn dir(app: &AppHandle) -> PathBuf {
    let d = app.path().app_data_dir().unwrap_or_else(|_| std::env::temp_dir().join("agent-studio"));
    let _ = std::fs::create_dir_all(d.join("runs"));
    d
}
fn log_file(app: &AppHandle, id: &str) -> PathBuf { dir(app).join("runs").join(format!("{id}.jsonl")) }

pub fn load(app: &AppHandle) -> Data {
    let mut d: Data = std::fs::read_to_string(dir(app).join("state.json")).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
    // Runs that were going when the app was closed have ended.
    for r in d.runs.iter_mut().filter(|r| r.status == "running") { r.status = "stopped".into(); if r.ended == 0 { r.ended = r.started; } }
    d
}

pub fn save(app: &AppHandle, d: &Data) {
    let p = dir(app).join("state.json");
    let tmp = p.with_extension("json.tmp");
    if let Ok(s) = serde_json::to_string(d) {
        if std::fs::write(&tmp, s).is_ok() { let _ = std::fs::rename(&tmp, &p); }
    }
}

fn changed(app: &AppHandle) {
    let st = app.state::<Agent>();
    let d = st.data.lock().unwrap();
    save(app, &d);
    drop(d);
    let _ = app.emit("runs-changed", ());
}

fn update(app: &AppHandle, id: &str, f: impl FnOnce(&mut Run)) {
    let st = app.state::<Agent>();
    let mut d = st.data.lock().unwrap();
    if let Some(r) = d.runs.iter_mut().find(|r| r.id == id) { f(r); }
}

fn log(app: &AppHandle, id: &str, kind: &str, text: String) {
    let line = LogLine { t: now(), kind: kind.into(), text };
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(log_file(app, id)) {
        let _ = writeln!(f, "{}", serde_json::to_string(&line).unwrap_or_default());
    }
    if kind == "tool" || kind == "text" { update(app, id, |r| r.last = line.text.chars().take(200).collect()); }
    let _ = app.emit("run-log", serde_json::json!({"id": id, "line": line}));
}

pub fn read_log(app: &AppHandle, id: &str) -> Vec<LogLine> {
    std::fs::read_to_string(log_file(app, id)).unwrap_or_default().lines().filter_map(|l| serde_json::from_str(l).ok()).collect()
}

/// Starts a run in the background; returns its id.
pub fn start(app: &AppHandle, spec: Spec) -> Result<String, String> {
    let first = spec.dirs.first().ok_or_else(|| if spec.lang == "de" { "Kein Ordner gewählt".to_string() } else { "No folder chosen".to_string() })?;
    if !Path::new(first).is_dir() { return Err(format!("{first}: {}", if spec.lang == "de" { "Ordner nicht gefunden" } else { "folder not found" })); }
    if spec.prompt.trim().is_empty() { return Err(if spec.lang == "de" { "Kein Auftrag".into() } else { "No task".into() }); }
    let own = app.state::<Agent>().claude.lock().unwrap().clone();
    let exe = runner::find_claude(Some(&own)).or_else(|| runner::find_claude(None))
        .ok_or_else(|| if spec.lang == "de" { "Claude Code nicht gefunden — in den Einstellungen festlegen".to_string() } else { "Claude Code not found — set it in Settings".to_string() })?;
    let id = new_id();
    let run = Run { id: id.clone(), repos: runner::snapshot(&spec.dirs), spec, started: now(), status: "running".into(), ..Default::default() };
    {
        let st = app.state::<Agent>();
        let mut d = st.data.lock().unwrap();
        d.runs.insert(0, run.clone());
        if d.runs.len() > KEEP {
            for old in d.runs.split_off(KEEP) { let _ = std::fs::remove_file(log_file(app, &old.id)); }
        }
    }
    changed(app);
    let app2 = app.clone();
    std::thread::spawn(move || execute(app2, run, exe));
    Ok(id)
}

fn execute(app: AppHandle, run: Run, exe: PathBuf) {
    let id = run.id.clone();
    let spec = run.spec.clone();
    let de = spec.lang == "de";
    log(&app, &id, "info", format!("{} {}", if de { "Start in" } else { "Starting in" }, spec.dirs.join(", ")));
    let mut cmd = runner::command(&exe);
    cmd.args(runner::args(&spec)).current_dir(&spec.dirs[0])
        .stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    runner::clean_env(&mut cmd);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => return finish(&app, &id, None, format!("{}: {e}", exe.display()), false),
    };
    app.state::<Agent>().pids.lock().unwrap().insert(id.clone(), child.id());
    if let Some(mut stdin) = child.stdin.take() { let _ = stdin.write_all(spec.prompt.as_bytes()); }
    let mut stderr = child.stderr.take();
    let err_thread = std::thread::spawn(move || {
        let mut s = String::new();
        if let Some(e) = stderr.as_mut() { let _ = e.read_to_string(&mut s); }
        s
    });
    // Time limit.
    let done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    if spec.max_minutes > 0 {
        let (app3, id3, done3, limit) = (app.clone(), id.clone(), done.clone(), spec.max_minutes as u64 * 60);
        std::thread::spawn(move || {
            for _ in 0..limit {
                std::thread::sleep(std::time::Duration::from_secs(1));
                if done3.load(std::sync::atomic::Ordering::Relaxed) { return; }
            }
            app3.state::<Agent>().stopping.lock().unwrap().insert(format!("{id3}|timeout"));
            kill(&app3, &id3);
        });
    }
    let mut result = None;
    if let Some(out) = child.stdout.take() {
        for line in BufReader::new(out).lines().map_while(Result::ok) {
            for ev in runner::parse_line(&line) {
                match ev {
                    runner::Ev::Session(s) => update(&app, &id, |r| r.session_id = s),
                    runner::Ev::Log(k, t) => log(&app, &id, k, t),
                    r @ runner::Ev::Result { .. } => result = Some(r),
                }
            }
        }
    }
    let status = child.wait();
    done.store(true, std::sync::atomic::Ordering::Relaxed);
    app.state::<Agent>().pids.lock().unwrap().remove(&id);
    let err = err_thread.join().unwrap_or_default();
    let ok = status.map(|s| s.success()).unwrap_or(false);
    let tail: String = err.trim().chars().rev().take(1500).collect::<Vec<_>>().into_iter().rev().collect();
    finish(&app, &id, result, tail, ok);
}

fn finish(app: &AppHandle, id: &str, result: Option<runner::Ev>, err: String, exit_ok: bool) {
    let st = app.state::<Agent>();
    let (stopped, timeout) = {
        let mut s = st.stopping.lock().unwrap();
        (s.remove(id), s.remove(&format!("{id}|timeout")))
    };
    let mut d = st.data.lock().unwrap();
    let Some(r) = d.runs.iter_mut().find(|r| r.id == id) else { return };
    r.ended = now();
    runner::after(&mut r.repos);
    let de = r.spec.lang == "de";
    match result {
        Some(runner::Ev::Result { text, error, cost, denials }) => {
            let (summary, needs) = runner::needs(&text);
            r.summary = summary;
            r.needs = needs;
            r.denials = denials;
            r.cost = cost;
            if error { r.error = if err.is_empty() { r.summary.clone() } else { err.clone() }; }
            r.status = if timeout { "timeout" } else if stopped { "stopped" } else if error { "failed" } else if r.needs.is_some() || !r.denials.is_empty() { "needsYou" } else { "done" }.into();
        }
        _ => {
            r.status = if timeout { "timeout" } else if stopped { "stopped" } else { "failed" }.into();
            r.error = if !err.is_empty() { err.clone() } else if exit_ok { String::new() } else if de { "Claude Code hat ohne Ergebnis aufgehört".into() } else { "Claude Code stopped without a result".into() };
        }
    }
    // "Not logged in" comes back as an answer, not as an error.
    if r.summary.contains("Not logged in") || r.summary.contains("/login") || r.error.contains("Not logged in") {
        r.status = "failed".into();
        r.error = if de { "Nicht bei Claude angemeldet — in den Einstellungen anmelden".into() } else { "Not signed in to Claude — sign in in Settings".into() };
        r.summary.clear();
    }
    let r = r.clone();
    let item = inbox_item(&r);
    if let Some(it) = item.clone() { d.inbox.insert(0, it); }
    drop(d);
    if let Some(it) = item { notify(app, &it); }
    let line = match r.status.as_str() {
        "done" => if de { "Fertig" } else { "Done" },
        "needsYou" => if de { "Braucht dich" } else { "Needs you" },
        "stopped" => if de { "Angehalten" } else { "Stopped" },
        "timeout" => if de { "Zeitlimit erreicht — angehalten" } else { "Time limit reached — stopped" },
        _ => if de { "Fehlgeschlagen" } else { "Failed" },
    };
    log(app, id, if r.status == "failed" { "error" } else { "info" }, if r.error.is_empty() { line.to_string() } else { format!("{line}: {}", r.error) });
    changed(app);
}

/// What a finished run puts into the "Needs you" box (a run stopped by hand: nothing).
fn inbox_item(r: &Run) -> Option<Item> {
    let de = r.spec.lang == "de";
    let base = Item { id: new_id(), run_id: r.id.clone(), title: r.spec.title.clone(), created: now(), ..Default::default() };
    match r.status.as_str() {
        "needsYou" => {
            let n = r.needs.clone().unwrap_or_default();
            let kind = if !r.denials.is_empty() { "approval" } else if n.kind == "approval" { "approval" } else { "question" };
            let text = if !n.text.is_empty() { n.text } else if de { "Claude möchte etwas tun, das dieser Lauf nicht darf:".into() } else { "Claude wants to do something this run may not:".into() };
            Some(Item { kind: kind.into(), text, options: n.options, rules: r.denials.iter().map(|d| d.rule.clone()).collect(), denials: r.denials.clone(), ..base })
        }
        "done" => Some(Item { kind: "done".into(), text: r.summary.clone(), ..base }),
        "failed" | "timeout" => Some(Item { kind: "problem".into(), text: if r.error.is_empty() { r.summary.clone() } else { r.error.clone() }, ..base }),
        _ => None,
    }
}

fn notify(app: &AppHandle, it: &Item) {
    use tauri_plugin_notification::NotificationExt;
    let de = app.state::<Agent>().data.lock().unwrap().runs.iter().find(|r| r.id == it.run_id).map(|r| r.spec.lang == "de").unwrap_or(false);
    let head = match (it.kind.as_str(), de) {
        ("question", true) => "Frage", ("question", false) => "Question",
        ("approval", true) => "Freigabe nötig", ("approval", false) => "Approval needed",
        ("done", true) => "Fertig", ("done", false) => "Done",
        (_, true) => "Problem", (_, false) => "Problem",
    };
    let body: String = it.text.chars().take(180).collect();
    let _ = app.notification().builder().title(format!("{head}: {}", it.title)).body(body).show();
}

pub fn kill(app: &AppHandle, id: &str) -> bool {
    let Some(pid) = app.state::<Agent>().pids.lock().unwrap().get(id).copied() else { return false };
    #[cfg(unix)]
    let _ = runner::command("kill").args(["-TERM", &format!("-{pid}")]).status();
    #[cfg(windows)]
    let _ = runner::command("taskkill").args(["/T", "/F", "/PID", &pid.to_string()]).status();
    true
}

pub fn stop(app: &AppHandle, id: &str) -> bool {
    app.state::<Agent>().stopping.lock().unwrap().insert(id.to_string());
    kill(app, id)
}

pub fn running(app: &AppHandle) -> Vec<String> { app.state::<Agent>().pids.lock().unwrap().keys().cloned().collect() }

/// What a message from the chat box changes for the next step (the slash commands /clear, /model and /plan).
#[derive(Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct MsgOpts {
    /// A new conversation (/clear): not the same Claude Code session, only this message as the prompt.
    pub fresh: bool,
    /// Another model from now on ("" = the standard one).
    pub model: Option<String>,
    /// Only plan this one step (/plan); the next message works normally again.
    pub plan: bool,
}

/// Goes on with a run: an answer, an approval or a further message. The same Claude Code session when there is one.
pub fn continue_run(app: &AppHandle, run_id: &str, prompt: String, allow: Vec<String>) -> Result<String, String> {
    continue_with(app, run_id, prompt, allow, MsgOpts::default())
}

pub fn continue_with(app: &AppHandle, run_id: &str, prompt: String, allow: Vec<String>, o: MsgOpts) -> Result<String, String> {
    let r = app.state::<Agent>().data.lock().unwrap().runs.iter().find(|r| r.id == run_id).cloned().ok_or("run not found")?;
    start(app, next_spec(&r, prompt, allow, o))
}

/// The spec of the run that goes on from `r`.
pub fn next_spec(r: &Run, prompt: String, allow: Vec<String>, o: MsgOpts) -> Spec {
    let mut spec = r.spec.clone();
    spec.continues = Some(r.id.clone());
    spec.allow_once.extend(allow);
    spec.allow_once.dedup();
    if spec.plan_once { spec.plan_only = false; spec.plan_once = false; }
    if o.plan && !spec.plan_only { spec.plan_only = true; spec.plan_once = true; }
    if let Some(m) = o.model { spec.model = m.trim().to_string(); }
    if o.fresh {
        spec.resume = None;
        spec.allow_once.clear();
        spec.prompt = prompt;
    } else if r.session_id.is_empty() {
        spec.resume = None;
        spec.prompt = format!("{}\n\n{}", r.spec.prompt, prompt);
    } else {
        spec.resume = Some(r.session_id.clone());
        spec.prompt = prompt;
    }
    spec
}

/// The answer to an item of the box: `approve` for approvals (true = allow, false = refuse), `text` the answer or
/// an addition. Returns the id of the run that goes on (none for a dismissed note).
pub fn answer(app: &AppHandle, item_id: &str, text: String, approve: Option<bool>) -> Result<Option<String>, String> {
    let it = {
        let st = app.state::<Agent>();
        let mut d = st.data.lock().unwrap();
        let i = d.inbox.iter().position(|x| x.id == item_id).ok_or("item not found")?;
        d.inbox.remove(i)
    };
    let de = app.state::<Agent>().data.lock().unwrap().runs.iter().find(|r| r.id == it.run_id).map(|r| r.spec.lang == "de").unwrap_or(false);
    let text = text.trim().to_string();
    let res = match (it.kind.as_str(), approve) {
        ("approval", Some(true)) => {
            let what = if it.rules.is_empty() { it.text.clone() } else { it.rules.join(", ") };
            let p = if de { format!("Freigegeben: {what}. Mach jetzt damit weiter.") } else { format!("Approved: {what}. Go on with it now.") };
            continue_run(app, &it.run_id, if text.is_empty() { p } else { format!("{p}\n{text}") }, it.rules.clone()).map(Some)
        }
        ("approval", Some(false)) => {
            let p = if de { "Nicht freigegeben. Mach ohne das weiter, wenn es geht — sonst hör auf und fasse zusammen." } else { "Not approved. Go on without it if you can — else stop and sum up." };
            continue_run(app, &it.run_id, if text.is_empty() { p.to_string() } else { format!("{p}\n{text}") }, vec![]).map(Some)
        }
        _ if !text.is_empty() => continue_run(app, &it.run_id, text, vec![]).map(Some),
        _ => Ok(None),
    };
    changed(app);
    res
}

pub fn with_run<T>(app: &AppHandle, id: &str, f: impl FnOnce(&mut Run) -> T) -> Option<T> {
    let st = app.state::<Agent>();
    let mut d = st.data.lock().unwrap();
    d.runs.iter_mut().find(|r| r.id == id).map(f)
}

pub fn touched(app: &AppHandle) { changed(app); }

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn box_items_follow_the_result() {
        let mut r = Run { id: "r".into(), status: "needsYou".into(), spec: Spec { lang: "de".into(), title: "T".into(), ..Default::default() }, ..Default::default() };
        r.denials = vec![Denial { tool: "Bash".into(), rule: "Bash(git push)".into(), what: "Bash: git push".into() }];
        let it = inbox_item(&r).unwrap();
        assert_eq!((it.kind.as_str(), it.rules.clone()), ("approval", vec!["Bash(git push)".to_string()]));
        r.denials.clear();
        r.needs = Some(Needs { kind: "question".into(), text: "Welche?".into(), options: vec!["A".into()] });
        let it = inbox_item(&r).unwrap();
        assert_eq!((it.kind.as_str(), it.text.as_str(), it.options.len()), ("question", "Welche?", 1));
        r.status = "stopped".into();
        assert!(inbox_item(&r).is_none());
        r.status = "done".into();
        r.summary = "Alles gut".into();
        assert_eq!(inbox_item(&r).unwrap().kind, "done");
    }

    #[test]
    fn a_message_goes_on_as_wanted() {
        let r = Run { id: "r1".into(), session_id: "s1".into(), spec: Spec { prompt: "Erst".into(), model: "opus".into(), allow_once: vec!["Write".into()], ..Default::default() }, ..Default::default() };
        let s = next_spec(&r, "Weiter".into(), vec![], MsgOpts::default());
        assert_eq!((s.resume.as_deref(), s.prompt.as_str(), s.model.as_str(), s.continues.as_deref()), (Some("s1"), "Weiter", "opus", Some("r1")));
        let s = next_spec(&r, "Neu".into(), vec![], MsgOpts { fresh: true, model: Some("haiku".into()), plan: false });
        assert_eq!((s.resume, s.prompt.as_str(), s.model.as_str()), (None, "Neu", "haiku"));
        assert!(s.allow_once.is_empty(), "/clear forgets approvals of the old conversation");
        // /plan only for one step.
        let s = next_spec(&r, "Plane".into(), vec![], MsgOpts { plan: true, ..Default::default() });
        assert!(s.plan_only && s.plan_once);
        let after = next_spec(&Run { spec: s, ..r.clone() }, "Mach".into(), vec![], MsgOpts::default());
        assert!(!after.plan_only && !after.plan_once);
        // A run that only plans anyway stays so.
        let mut p = r.clone();
        p.spec.plan_only = true;
        let s = next_spec(&p, "x".into(), vec![], MsgOpts { plan: true, ..Default::default() });
        assert!(s.plan_only && !s.plan_once);
        assert!(next_spec(&p, "x".into(), vec![], MsgOpts::default()).plan_only);
    }
}
