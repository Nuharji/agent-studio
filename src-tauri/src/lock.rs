//! Two runs never change the same file at the same time. Every Claude run gets a PreToolUse hook (in its --settings)
//! for Edit, Write, MultiEdit and NotebookEdit that starts this program with `--file-lock`: the first run that changes
//! a file holds it until it ends; another run that wants to change it is stopped with a note (exit code 2, Claude reads
//! it) and works on something else first. The locks are files in the app's data folder (`locks/`), one per file.

use serde_json::{json, Value};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

/// Environment variables a run passes on to its hook.
pub const ENV_DIR: &str = "AGENT_STUDIO_LOCKS";
pub const ENV_RUN: &str = "AGENT_STUDIO_RUN";
pub const ENV_TITLE: &str = "AGENT_STUDIO_TITLE";

pub fn wanted() -> bool { std::env::args().skip(1).any(|a| a == "--file-lock") }

pub fn dir(data: &Path) -> PathBuf { data.join("locks") }

/// The hook for a run's settings: this program with --file-lock before every change of a file.
pub fn hook_settings(exe: &str) -> Value {
    json!({"PreToolUse": [{"matcher": "Edit|Write|MultiEdit|NotebookEdit", "hooks": [{"type": "command", "command": format!("\"{exe}\" --file-lock"), "timeout": 10}]}]})
}

/// The same file always gives the same key: absolute, with the real folder, and on macOS and Windows in small letters
/// (their file systems don't tell "A.md" from "a.md").
pub fn key(file: &str, cwd: &str) -> String {
    let p = Path::new(file);
    let p = if p.is_absolute() { p.to_path_buf() } else { Path::new(cwd).join(p) };
    let real = std::fs::canonicalize(&p).ok().or_else(|| {
        let parent = std::fs::canonicalize(p.parent()?).ok()?;
        Some(parent.join(p.file_name()?))
    }).unwrap_or(p);
    let s = real.to_string_lossy().into_owned();
    if cfg!(any(target_os = "macos", windows)) { s.to_lowercase() } else { s }
}

/// FNV-1a: the lock file's name for a key.
fn hash(s: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() { h ^= b as u64; h = h.wrapping_mul(0x0100_0000_01b3); }
    format!("{h:016x}")
}

#[derive(Debug, PartialEq)]
pub enum Claim {
    /// The file is this run's now (or was already).
    Mine,
    /// Another run holds it: its title.
    Taken(String),
}

/// Claims `file` for run `run` (creating the lock only when there is none, so two runs can't both get it).
pub fn claim(locks: &Path, file: &str, cwd: &str, run: &str, title: &str) -> Claim {
    let k = key(file, cwd);
    let _ = std::fs::create_dir_all(locks);
    let p = locks.join(format!("{}.json", hash(&k)));
    for _ in 0..2 {
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&p) {
            Ok(mut f) => {
                let _ = f.write_all(json!({"path": k, "run": run, "title": title}).to_string().as_bytes());
                return Claim::Mine;
            }
            Err(_) => {
                let held: Value = std::fs::read_to_string(&p).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(Value::Null);
                match held.get("run").and_then(Value::as_str) {
                    Some(r) if r == run => return Claim::Mine,
                    Some(_) => return Claim::Taken(held.get("title").and_then(Value::as_str).unwrap_or("").to_string()),
                    // Being written right now, or broken: try once more after a moment.
                    None => std::thread::sleep(std::time::Duration::from_millis(50)),
                }
            }
        }
    }
    Claim::Taken(String::new())
}

/// Frees every file run `run` holds (when it ends).
pub fn release(locks: &Path, run: &str) {
    for e in std::fs::read_dir(locks).into_iter().flatten().flatten() {
        let held: Value = std::fs::read_to_string(e.path()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(Value::Null);
        if held.get("run").and_then(Value::as_str) == Some(run) { let _ = std::fs::remove_file(e.path()); }
    }
}

/// Frees all files (when the app starts: no run is going then).
pub fn release_all(locks: &Path) {
    for e in std::fs::read_dir(locks).into_iter().flatten().flatten() {
        if e.path().extension().is_some_and(|x| x == "json") { let _ = std::fs::remove_file(e.path()); }
    }
}

/// The hook: reads Claude Code's JSON from stdin; exit code 2 with a note when another run holds the file.
pub fn run() -> i32 {
    let (Some(locks), Ok(run)) = (std::env::var_os(ENV_DIR), std::env::var(ENV_RUN)) else { return 0 };
    let mut input = String::new();
    let _ = std::io::stdin().read_to_string(&mut input);
    let v: Value = serde_json::from_str(&input).unwrap_or(Value::Null);
    let ti = v.get("tool_input").cloned().unwrap_or(Value::Null);
    let file = ti.get("file_path").or_else(|| ti.get("notebook_path")).and_then(Value::as_str).unwrap_or("");
    if file.is_empty() { return 0; }
    let cwd = v.get("cwd").and_then(Value::as_str).map(str::to_string)
        .or_else(|| std::env::current_dir().ok().map(|d| d.to_string_lossy().into_owned())).unwrap_or_default();
    let title = std::env::var(ENV_TITLE).unwrap_or_default();
    match claim(Path::new(&locks), file, &cwd, &run, &title) {
        Claim::Mine => 0,
        Claim::Taken(other) => {
            let de = std::env::var("AGENT_STUDIO_LANG").is_ok_and(|l| l == "de");
            let who = if other.is_empty() { String::new() } else if de { format!(" („{other}“)") } else { format!(" (\"{other}\")") };
            eprintln!("{}", if de {
                format!("{file} wird gerade von einem anderen Lauf in Agent Studio geändert{who}. Ändere die Datei jetzt nicht und versuche nicht, das zu umgehen (auch nicht mit Bash): Mach zuerst anderes und versuche es später noch einmal, sonst nenne die Datei in deiner Zusammenfassung als offen.")
            } else {
                format!("{file} is being changed by another run in Agent Studio{who}. Don't change the file now and don't work around it (not with Bash either): do other things first and try again later, otherwise name the file as open in your summary.")
            });
            2
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_run_per_file() {
        let d = std::env::temp_dir().join(format!("agent-lock-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let (locks, work) = (d.join("locks"), d.join("work"));
        std::fs::create_dir_all(&work).unwrap();
        std::fs::write(work.join("README.md"), "x").unwrap();
        let cwd = work.to_string_lossy().into_owned();
        let abs = work.join("README.md").to_string_lossy().into_owned();
        assert_eq!(key("README.md", &cwd), key(&abs, "/elsewhere"), "relative and absolute are the same file");
        assert_eq!(key("new.md", &cwd), key(&work.join("new.md").to_string_lossy(), ""), "a file not made yet");

        assert_eq!(claim(&locks, "README.md", &cwd, "r1", "Feature A"), Claim::Mine);
        assert_eq!(claim(&locks, &abs, "/", "r1", "Feature A"), Claim::Mine, "the same run again");
        assert_eq!(claim(&locks, &abs, "/", "r2", "Fix B"), Claim::Taken("Feature A".into()));
        assert_eq!(claim(&locks, "other.md", &cwd, "r2", "Fix B"), Claim::Mine, "other files are free");
        release(&locks, "r1");
        assert_eq!(claim(&locks, &abs, "/", "r2", "Fix B"), Claim::Mine, "free when the run has ended");
        assert_eq!(claim(&locks, &abs, "/", "r1", "Feature A"), Claim::Taken("Fix B".into()));
        release_all(&locks);
        assert_eq!(claim(&locks, &abs, "/", "r1", "Feature A"), Claim::Mine);

        let h = hook_settings("/Applications/Agent Studio.app/Contents/MacOS/agent-studio");
        assert_eq!(h.pointer("/PreToolUse/0/hooks/0/command").and_then(Value::as_str), Some("\"/Applications/Agent Studio.app/Contents/MacOS/agent-studio\" --file-lock"));
        let _ = std::fs::remove_dir_all(d);
    }
}
