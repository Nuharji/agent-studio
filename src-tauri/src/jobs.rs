//! Commands of a project that Agent Studio runs itself, without the AI: build, test, start the app, or any command
//! the person types. They run in the login shell (so the PATH of the terminal counts) in the project's folder; their
//! output reaches the UI line by line ("job-output") and the end with the exit code ("job-done"). When one fails,
//! the UI offers to let Claude fix it with the output.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read};
use std::process::Stdio;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter};

static RUNNING: Mutex<Option<HashMap<String, u32>>> = Mutex::new(None);

fn running() -> std::sync::MutexGuard<'static, Option<HashMap<String, u32>>> { RUNNING.lock().unwrap() }

/// The program and arguments that run `cmd` in a shell.
pub fn shell(cmd: &str) -> (String, Vec<String>) {
    if cfg!(windows) { ("cmd".into(), vec!["/C".into(), cmd.into()]) } else {
        (std::env::var("SHELL").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| "/bin/sh".into()), vec!["-lc".into(), cmd.into()])
    }
}

/// Starts `cmd` in `dir`; returns at once, the output comes as events.
pub fn start(app: &AppHandle, id: &str, dir: &str, cmd: &str) -> Result<(), String> {
    if !std::path::Path::new(dir).is_dir() { return Err(format!("{dir} is not a folder")); }
    let (prog, args) = shell(cmd);
    let mut c = crate::runner::command(prog);
    c.args(args).current_dir(dir).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    crate::runner::clean_env(&mut c);
    // Colors and progress bars make no sense in the panel.
    c.env("NO_COLOR", "1").env("CARGO_TERM_COLOR", "never").env("FORCE_COLOR", "0").env("CI", "1");
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        c.process_group(0);
    }
    let mut child = c.spawn().map_err(|e| e.to_string())?;
    running().get_or_insert_with(HashMap::new).insert(id.to_string(), child.id());
    let pipes: Vec<(Box<dyn Read + Send>, bool)> = [child.stdout.take().map(|o| (Box::new(o) as Box<dyn Read + Send>, false)), child.stderr.take().map(|o| (Box::new(o) as Box<dyn Read + Send>, true))].into_iter().flatten().collect();
    let mut readers = Vec::new();
    for (pipe, err) in pipes {
        let (app, id) = (app.clone(), id.to_string());
        readers.push(std::thread::spawn(move || {
            for line in BufReader::new(pipe).lines().map_while(Result::ok) {
                let _ = app.emit("job-output", serde_json::json!({"id": id, "line": line, "err": err}));
            }
        }));
    }
    let (app, id) = (app.clone(), id.to_string());
    std::thread::spawn(move || {
        let code = child.wait().ok().and_then(|s| s.code());
        for r in readers { let _ = r.join(); }
        let stopped = running().get_or_insert_with(HashMap::new).remove(&id).is_none();
        let _ = app.emit("job-done", serde_json::json!({"id": id, "code": code, "stopped": stopped}));
    });
    Ok(())
}

/// Stops a command (and what it started).
pub fn stop(id: &str) -> bool {
    let Some(pid) = running().get_or_insert_with(HashMap::new).remove(id) else { return false };
    let _ = if cfg!(windows) {
        crate::runner::command("taskkill").args(["/PID", &pid.to_string(), "/T", "/F"]).output()
    } else {
        crate::runner::command("kill").args(["-TERM", &format!("-{pid}")]).output()
    };
    true
}

#[tauri::command]
pub fn job_start(app: AppHandle, id: String, dir: String, cmd: String) -> Result<(), String> { start(&app, &id, &dir, &cmd) }

#[tauri::command]
pub fn job_stop(id: String) -> bool { stop(&id) }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_in_a_shell() {
        let (p, a) = shell("cargo test && echo ok");
        if cfg!(windows) { assert_eq!((p.as_str(), a[0].as_str()), ("cmd", "/C")); } else { assert_eq!(a[0], "-lc"); assert!(!p.is_empty()); }
        assert_eq!(a[1], "cargo test && echo ok");
        assert!(!stop("nothing"));
    }
}
