// Hide the console window in release builds on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod agentlink;
mod codex;
mod link;
mod runner;
mod runs;
mod shared;
mod slash;
mod todo;

use serde::{Deserialize, Serialize};
use std::{fs, io::BufRead, path::Path};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tauri_plugin_opener::OpenerExt;

/// The app's texts in English and German (i18n/*.json, built into the program).
#[tauri::command]
fn texts() -> serde_json::Value {
    let parse = |s: &str| serde_json::from_str::<serde_json::Value>(s).unwrap_or_default();
    serde_json::json!({"en": parse(include_str!("../i18n/en.json")), "de": parse(include_str!("../i18n/de.json"))})
}

/// The setup (routines, linked to-do lists, rules, limits) — JSON kept by the UI in the app's data folder
/// (Windows: %APPDATA%\com.agentstudio.app, macOS: ~/Library/Application Support/com.agentstudio.app).
#[tauri::command]
fn config_load(app: AppHandle) -> Option<String> {
    fs::read_to_string(runs::dir(&app).join("config.json")).ok()
}

#[tauri::command]
fn config_save(app: AppHandle, content: String, claude: String, codex: Option<String>) -> Result<(), String> {
    *app.state::<runs::Agent>().claude.lock().unwrap() = claude;
    *app.state::<runs::Agent>().codex.lock().unwrap() = codex.unwrap_or_default();
    let p = runs::dir(&app).join("config.json");
    let tmp = p.with_extension("json.tmp");
    fs::write(&tmp, content).map_err(|e| e.to_string())?;
    if p.exists() { let _ = fs::copy(&p, p.with_extension("bak")); }
    fs::rename(&tmp, &p).map_err(|e| e.to_string())
}

#[derive(Serialize)]
struct ClaudeInfo {
    path: Option<String>,
    version: String,
    status: serde_json::Value,
}

/// Where Claude Code is, its version and whether someone is signed in (`claude auth status`).
#[tauri::command]
async fn claude_status(own: String) -> ClaudeInfo {
    let Some(p) = runner::find_claude(Some(&own)).or_else(|| if own.trim().is_empty() { runner::find_claude(None) } else { None }) else {
        return ClaudeInfo { path: None, version: String::new(), status: serde_json::Value::Null };
    };
    let run = |args: &[&str]| {
        let mut c = runner::command(&p);
        c.args(args);
        runner::clean_env(&mut c);
        c.output().ok().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default()
    };
    let version = run(&["--version"]);
    let status = serde_json::from_str(&run(&["auth", "status"])).unwrap_or(serde_json::Value::Null);
    ClaudeInfo { path: Some(p.to_string_lossy().into_owned()), version, status }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CodexInfo {
    path: Option<String>,
    version: String,
    logged_in: bool,
    how: String,
}

/// Where Codex (for ChatGPT) is, its version and whether someone is signed in (`codex login status`).
#[tauri::command]
async fn codex_status(own: String) -> CodexInfo {
    let Some(p) = codex::find(Some(&own)).or_else(|| if own.trim().is_empty() { codex::find(None) } else { None }) else {
        return CodexInfo { path: None, version: String::new(), logged_in: false, how: String::new() };
    };
    let version = runner::command(&p).arg("--version").output().ok().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
    let (logged_in, how) = runner::command(&p).args(["login", "status"]).output().ok()
        .map(|o| codex::status_of(o.status.success(), &format!("{}\n{}", String::from_utf8_lossy(&o.stderr), String::from_utf8_lossy(&o.stdout))))
        .unwrap_or_default();
    CodexInfo { path: Some(p.to_string_lossy().into_owned()), version, logged_in, how }
}

/// Signs in to ChatGPT (`codex login`: opens the browser), shown like the sign-in to Claude.
#[tauri::command]
fn codex_login(app: AppHandle, own: String) -> Result<(), String> {
    let p = codex::find(Some(&own)).or_else(|| codex::find(None)).ok_or("Codex not found")?;
    sign_in(app, &p, &["login"])
}

/// Signs in to Claude (`claude auth login`: opens the browser). Its output reaches the UI as "login-output" lines,
/// the end as "login-done"; a code asked for is typed in with `login_input`.
#[tauri::command]
fn login_start(app: AppHandle, own: String) -> Result<(), String> {
    let p = runner::find_claude(Some(&own)).or_else(|| runner::find_claude(None)).ok_or("Claude Code not found")?;
    sign_in(app, &p, &["auth", "login"])
}

fn sign_in(app: AppHandle, program: &Path, args: &[&str]) -> Result<(), String> {
    let mut c = runner::command(program);
    c.args(args).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    runner::clean_env(&mut c);
    let mut child = c.spawn().map_err(|e| e.to_string())?;
    *app.state::<runs::Agent>().login.lock().unwrap() = child.stdin.take();
    for pipe in [child.stdout.take().map(|o| Box::new(o) as Box<dyn std::io::Read + Send>), child.stderr.take().map(|o| Box::new(o) as Box<dyn std::io::Read + Send>)].into_iter().flatten() {
        let app2 = app.clone();
        std::thread::spawn(move || {
            for line in std::io::BufReader::new(pipe).lines().map_while(Result::ok) { let _ = app2.emit("login-output", line); }
        });
    }
    std::thread::spawn(move || {
        let ok = child.wait().map(|s| s.success()).unwrap_or(false);
        *app.state::<runs::Agent>().login.lock().unwrap() = None;
        let _ = app.emit("login-done", ok);
    });
    Ok(())
}

#[tauri::command]
fn login_input(app: AppHandle, text: String) -> Result<(), String> {
    use std::io::Write;
    let st = app.state::<runs::Agent>();
    let mut g = st.login.lock().unwrap();
    let w = g.as_mut().ok_or("no sign-in running")?;
    writeln!(w, "{}", text.trim()).map_err(|e| e.to_string())
}

#[tauri::command]
fn run_start(app: AppHandle, spec: runner::Spec) -> Result<String, String> { runs::start(&app, spec) }

#[tauri::command]
fn run_stop(app: AppHandle, id: String) -> bool { runs::stop(&app, &id) }

/// Stops every run (the emergency stop).
#[tauri::command]
fn run_stop_all(app: AppHandle) -> usize {
    let ids = runs::running(&app);
    ids.iter().filter(|id| runs::stop(&app, id)).count()
}

#[derive(Serialize)]
struct Runs {
    runs: Vec<runs::Run>,
    inbox: Vec<runs::Item>,
    running: Vec<String>,
}

#[tauri::command]
fn runs_get(app: AppHandle, state: State<runs::Agent>) -> Runs {
    let d = state.data.lock().unwrap();
    Runs { runs: d.runs.clone(), inbox: d.inbox.clone(), running: runs::running(&app) }
}

#[tauri::command]
fn run_log(app: AppHandle, id: String) -> Vec<runner::LogLine> { runs::read_log(&app, &id) }

#[tauri::command]
async fn run_diff(app: AppHandle, id: String) -> String {
    let repos = runs::with_run(&app, &id, |r| r.repos.clone()).unwrap_or_default();
    runner::diff(&repos)
}

/// Takes back the commits of a run (when they are not pushed).
#[tauri::command]
fn run_undo(app: AppHandle, id: String) -> Result<usize, String> {
    let n = runs::with_run(&app, &id, |r| {
        let n = runner::undo(&mut r.repos)?;
        r.undone = true;
        Ok::<usize, String>(n)
    }).ok_or("run not found")??;
    runs::touched(&app);
    Ok(n)
}

/// Goes on with a run (a message in its chat).
#[tauri::command]
fn run_message(app: AppHandle, id: String, text: String, opts: Option<runs::MsgOpts>) -> Result<String, String> {
    runs::continue_with(&app, &id, text, vec![], opts.unwrap_or_default())
}

/// The person's own slash commands and skills for a run in these folders (~/.claude and <dir>/.claude).
#[tauri::command]
fn slash_list(dirs: Vec<String>) -> Vec<slash::Cmd> {
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(std::path::PathBuf::from).unwrap_or_default();
    slash::list(&home, &dirs)
}

/// The text sent to Claude for "/name args".
#[tauri::command]
fn slash_expand(path: String, kind: String, name: String, args: String, lang: String) -> Result<String, String> {
    slash::expand(&path, &kind, &name, &args, lang == "de")
}

#[tauri::command]
fn inbox_answer(app: AppHandle, id: String, text: String, approve: Option<bool>) -> Result<Option<String>, String> { runs::answer(&app, &id, text, approve) }

/// Removes notes from the box (done / problem) without an answer.
#[tauri::command]
fn inbox_dismiss(app: AppHandle, ids: Vec<String>) {
    app.state::<runs::Agent>().data.lock().unwrap().inbox.retain(|x| !ids.contains(&x.id));
    runs::touched(&app);
}

#[tauri::command]
fn runs_clear(app: AppHandle) {
    {
        let st = app.state::<runs::Agent>();
        let mut d = st.data.lock().unwrap();
        let keep: Vec<String> = d.inbox.iter().map(|i| i.run_id.clone()).collect();
        let running = runs::running(&app);
        d.runs.retain(|r| keep.contains(&r.id) || running.contains(&r.id));
    }
    runs::touched(&app);
}

/// The tasks other Studio apps sent (link.rs); the UI starts them as runs.
#[tauri::command]
fn link_take(app: AppHandle) -> Vec<serde_json::Value> { link::take(&app) }

/// A task that is not started (taking tasks is off, the daily limit …): the app that sent it is told why.
#[tauri::command]
fn link_decline(app: AppHandle, task: serde_json::Value, why: String) { link::decline(&app, &task, &why) }

#[derive(Serialize)]
struct TodoFile {
    items: Vec<todo::Item>,
    sections: Vec<String>,
    stamp: u64,
    error: Option<String>,
}

fn stamp(p: &Path) -> u64 {
    fs::metadata(p).and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_millis() as u64).unwrap_or(0)
}

#[tauri::command]
fn todo_read(path: String) -> TodoFile {
    let p = Path::new(&path);
    match fs::read_to_string(p) {
        Ok(s) => TodoFile { items: todo::parse(&s), sections: todo::sections(&s), stamp: stamp(p), error: None },
        Err(e) => TodoFile { items: vec![], sections: vec![], stamp: 0, error: Some(e.to_string()) },
    }
}

/// When the to-do lists last changed (to notice edits).
#[tauri::command]
fn todo_stamps(paths: Vec<String>) -> Vec<u64> { paths.iter().map(|p| stamp(Path::new(p))).collect() }

/// Ticks one item (only when the line still holds that item).
#[tauri::command]
fn todo_tick(path: String, line: usize, text: String) -> Result<bool, String> {
    let s = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let Some(out) = todo::tick(&s, line, &text) else { return Ok(false) };
    fs::write(&path, out).map_err(|e| e.to_string())?;
    Ok(true)
}

/// Adds a new item at the end of a section; returns its line.
#[tauri::command]
fn todo_add(path: String, section: String, text: String) -> Result<usize, String> {
    let s = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let (out, line) = todo::add(&s, &section, &text).ok_or("empty")?;
    fs::write(&path, out).map_err(|e| e.to_string())?;
    Ok(line)
}

#[tauri::command]
async fn pick_folder(app: AppHandle) -> Option<String> {
    shared::file_dialog(&app).blocking_pick_folder().and_then(|f| f.into_path().ok()).map(|p| p.to_string_lossy().into_owned())
}

#[tauri::command]
async fn pick_file(app: AppHandle, filter_name: String, exts: Vec<String>) -> Option<String> {
    let exts: Vec<&str> = exts.iter().map(String::as_str).collect();
    let d = shared::file_dialog(&app);
    let d = if exts.is_empty() { d } else { d.add_filter(&filter_name, &exts) };
    d.blocking_pick_file().and_then(|f| f.into_path().ok()).map(|p| p.to_string_lossy().into_owned())
}

/// Picks several files (to-do lists).
#[tauri::command]
async fn pick_files(app: AppHandle, filter_name: String, exts: Vec<String>) -> Vec<String> {
    let exts: Vec<&str> = exts.iter().map(String::as_str).collect();
    shared::file_dialog(&app).add_filter(&filter_name, &exts).blocking_pick_files().unwrap_or_default()
        .into_iter().filter_map(|f| f.into_path().ok()).map(|p| p.to_string_lossy().into_owned()).collect()
}

/// Shows a file or folder in Finder / Explorer.
#[tauri::command]
fn reveal(app: AppHandle, path: String) -> Result<(), String> {
    app.opener().reveal_item_in_dir(&path).map_err(|e| e.to_string())
}

#[tauri::command]
fn open_url(app: AppHandle, url: String) -> Result<(), String> {
    if !["https://", "http://", "mailto:"].iter().any(|p| url.starts_with(p)) { return Err(format!("Refusing to open {url}")); }
    app.opener().open_url(url, None::<&str>).map_err(|e| e.to_string())
}

/// The number on the app's icon (macOS dock, Linux); on Windows nothing.
#[tauri::command]
fn set_badge(app: AppHandle, n: i64) {
    #[cfg(not(windows))]
    if let Some(w) = app.get_webview_window("main") { let _ = w.set_badge_count(if n > 0 { Some(n) } else { None }); }
    #[cfg(windows)]
    let _ = (app, n);
}

#[tauri::command]
async fn confirm(app: AppHandle, title: String, message: String, ok_label: String, cancel_label: String) -> bool {
    app.dialog().message(message).title(title).kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::OkCancelCustom(ok_label, cancel_label)).blocking_show()
}

/// One entry of a native context menu; an entry without `id` is a separator.
#[derive(Deserialize)]
struct MenuEntry {
    id: Option<String>,
    label: Option<String>,
    enabled: Option<bool>,
    checked: Option<bool>,
    accel: Option<String>,
}

/// Shows a native (OS) context menu at the mouse position. The chosen item's id
/// reaches the UI as a "context-menu" event (see `on_menu_event` in `main`).
#[tauri::command]
fn context_menu(app: AppHandle, window: tauri::Window, items: Vec<MenuEntry>) -> Result<(), String> {
    use tauri::menu::{CheckMenuItemBuilder, MenuBuilder, MenuItemBuilder};
    let mut menu = MenuBuilder::new(&app);
    for entry in items {
        let Some(id) = entry.id else {
            menu = menu.separator();
            continue;
        };
        let label = entry.label.unwrap_or_default();
        let enabled = entry.enabled.unwrap_or(true);
        if let Some(checked) = entry.checked {
            let item = CheckMenuItemBuilder::with_id(id, label).enabled(enabled).checked(checked).build(&app).map_err(|e| e.to_string())?;
            menu = menu.item(&item);
            continue;
        }
        let plain = || MenuItemBuilder::with_id(id.clone(), label.clone()).enabled(enabled);
        // The shortcut is only shown as a hint; leave it out if the OS can't display it.
        let item = match entry.accel.as_deref() {
            Some(a) => plain().accelerator(a).build(&app).or_else(|_| plain().build(&app)),
            None => plain().build(&app),
        }
        .map_err(|e| e.to_string())?;
        menu = menu.item(&item);
    }
    let menu = menu.build().map_err(|e| e.to_string())?;
    window.popup_menu(&menu).map_err(|e| e.to_string())
}

#[tauri::command]
fn clipboard_read(app: AppHandle) -> String {
    app.clipboard().read_text().unwrap_or_default()
}

#[tauri::command]
fn clipboard_write(app: AppHandle, text: String) -> Result<(), String> {
    app.clipboard().write_text(text).map_err(|e| e.to_string())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_notification::init())
        .manage(runs::Agent::default())
        .setup(|app| {
            let h = app.handle();
            let data = runs::load(h);
            runs::save(h, &data);
            *app.state::<runs::Agent>().data.lock().unwrap() = data;
            // The heartbeat in status.json: the other apps see that Agent Studio is open; a task they send wakes the UI.
            let h = h.clone();
            std::thread::spawn(move || {
                let mut n = 0u32;
                loop {
                    if n % 10 == 0 { link::write_status(&h, &h.state::<runs::Agent>().data.lock().unwrap()); }
                    if agentlink::dir(&h).is_some_and(|d| std::fs::read_dir(d.join("inbox")).into_iter().flatten().flatten().next().is_some()) {
                        let _ = h.emit("link-task", ());
                    }
                    n = n.wrapping_add(1);
                    std::thread::sleep(std::time::Duration::from_secs(2));
                }
            });
            Ok(())
        })
        .on_menu_event(|app, event| {
            let _ = app.emit("context-menu", event.id().as_ref());
        })
        .invoke_handler(tauri::generate_handler![
            shared::shared_get,
            shared::shared_set,
            agentlink::agent_poll,
            agentlink::agent_send,
            agentlink::agent_ack,
            agentlink::agent_open,
            agentlink::agent_open_file,
            link_take,
            link_decline,
            texts,
            config_load,
            config_save,
            claude_status,
            codex_status,
            codex_login,
            login_start,
            login_input,
            run_start,
            run_stop,
            run_stop_all,
            runs_get,
            run_log,
            run_diff,
            run_undo,
            run_message,
            slash_list,
            slash_expand,
            inbox_answer,
            inbox_dismiss,
            runs_clear,
            todo_read,
            todo_stamps,
            todo_tick,
            todo_add,
            pick_folder,
            pick_file,
            pick_files,
            reveal,
            open_url,
            set_badge,
            confirm,
            context_menu,
            clipboard_read,
            clipboard_write
        ])
        .run(tauri::generate_context!())
        .expect("error while running Agent Studio");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texts_parse_and_match() {
        let t = texts();
        let (en, de) = (t["en"].as_object().unwrap(), t["de"].as_object().unwrap());
        assert!(en.len() > 80);
        assert!(en.keys().all(|k| de.contains_key(k)), "every English text has a German one: {:?}", en.keys().filter(|k| !de.contains_key(*k)).collect::<Vec<_>>());
        assert!(de.keys().all(|k| en.contains_key(k)), "every German text has an English one: {:?}", de.keys().filter(|k| !en.contains_key(*k)).collect::<Vec<_>>());
    }
}
