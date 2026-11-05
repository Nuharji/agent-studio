//! Projects: the apps you build with AI. A project is a folder (usually a git repository); Agent Studio reads what it
//! is (name, version, stack), which commands build / test / start it, its git state (branch, changes, commits ahead,
//! last tag) and its to-do, status and changelog files — and finds the apps inside a folder that holds several.

use serde::Serialize;
use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Serialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Cmd {
    /// "build", "test", "run", "lint", "check" or a script name.
    pub kind: String,
    pub cmd: String,
}

#[derive(Serialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Git {
    pub branch: String,
    /// Changed, new or deleted files not committed yet ("M src/main.rs", "?? new.txt"), at most 200.
    pub changes: Vec<String>,
    pub ahead: u32,
    pub behind: u32,
    pub upstream: bool,
    /// The last commits: "abc1234|message|when (relative)".
    pub log: Vec<String>,
    pub tag: String,
    pub remote: String,
}

#[derive(Serialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Info {
    pub dir: String,
    pub exists: bool,
    pub name: String,
    pub version: String,
    pub description: String,
    /// "tauri", "rust", "node", "web", "python", "go", "swift" …
    pub stack: Vec<String>,
    pub commands: Vec<Cmd>,
    pub git: Option<Git>,
    /// To-do lists, status and changelog files found in the folder (full paths).
    pub todo: Vec<String>,
    pub status: String,
    pub changelog: String,
    pub readme: String,
    pub icon: String,
    /// Apps in direct subfolders (when this folder holds several, like a workspace).
    pub apps: Vec<String>,
}

fn read(p: &Path) -> Option<String> { std::fs::read_to_string(p).ok() }
fn json(p: &Path) -> Option<Value> { read(p).and_then(|s| serde_json::from_str(&s).ok()) }

/// `key = "value"` in the [package] part of a Cargo.toml (no full TOML parser needed for name / version / description).
fn cargo_field(toml: &str, key: &str) -> Option<String> {
    let mut in_pkg = false;
    for line in toml.lines() {
        let l = line.trim();
        if l.starts_with('[') { in_pkg = l == "[package]"; continue; }
        if !in_pkg { continue; }
        if let Some(rest) = l.strip_prefix(key) {
            let rest = rest.trim_start();
            if let Some(v) = rest.strip_prefix('=') { return Some(v.trim().trim_matches('"').to_string()); }
        }
    }
    None
}

fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let o = crate::runner::command("git").arg("-C").arg(dir).args(args).output().ok()?;
    o.status.success().then(|| String::from_utf8_lossy(&o.stdout).trim_end().to_string())
}

pub fn git_info(dir: &Path) -> Option<Git> {
    git(dir, &["rev-parse", "--is-inside-work-tree"])?;
    let branch = git(dir, &["rev-parse", "--abbrev-ref", "HEAD"]).unwrap_or_default();
    let changes: Vec<String> = git(dir, &["status", "--porcelain", "--untracked-files=normal"]).unwrap_or_default()
        .lines().filter(|l| l.len() > 3).take(200).map(|l| l.trim_end().to_string()).collect();
    let (ahead, behind, upstream) = match git(dir, &["rev-list", "--left-right", "--count", "HEAD...@{upstream}"]) {
        Some(s) => { let mut it = s.split_whitespace().map(|x| x.parse().unwrap_or(0)); (it.next().unwrap_or(0), it.next().unwrap_or(0), true) }
        None => (0, 0, false),
    };
    let log = git(dir, &["log", "-8", "--format=%h|%s|%cr"]).unwrap_or_default().lines().map(str::to_string).collect();
    let tag = git(dir, &["describe", "--tags", "--abbrev=0"]).unwrap_or_default();
    let remote = git(dir, &["remote", "get-url", "origin"]).unwrap_or_default();
    Some(Git { branch, changes, ahead, behind, upstream, log, tag, remote })
}

/// The commands that build, test and start the project, from its files.
pub fn commands(dir: &Path) -> (Vec<String>, Vec<Cmd>) {
    let mut stack: Vec<String> = Vec::new();
    let mut cmds: Vec<Cmd> = Vec::new();
    let mut add = |kind: &str, cmd: String| { if !cmds.iter().any(|c| c.cmd == cmd) { cmds.push(Cmd { kind: kind.into(), cmd }); } };
    let tauri = dir.join("src-tauri/tauri.conf.json").is_file() || dir.join("tauri.conf.json").is_file();
    if tauri { stack.push("tauri".into()); }
    if let Some(pkg) = json(&dir.join("package.json")) {
        stack.push("node".into());
        let scripts = pkg.get("scripts").and_then(Value::as_object).cloned().unwrap_or_default();
        let runner = if dir.join("pnpm-lock.yaml").is_file() { "pnpm" } else if dir.join("yarn.lock").is_file() { "yarn" } else if dir.join("bun.lockb").is_file() || dir.join("bun.lock").is_file() { "bun" } else { "npm" };
        for (k, kind) in [("dev", "run"), ("start", "run"), ("build", "build"), ("test", "test"), ("lint", "lint"), ("typecheck", "check"), ("check", "check")] {
            if scripts.contains_key(k) { add(kind, if runner == "npm" && k != "start" && k != "test" { format!("npm run {k}") } else { format!("{runner} {}{k}", if runner == "npm" { "" } else { "run " }) }); }
        }
    }
    let cargo_dir = if dir.join("Cargo.toml").is_file() { Some(String::new()) } else if dir.join("src-tauri/Cargo.toml").is_file() { Some("src-tauri".to_string()) } else { None };
    if let Some(sub) = cargo_dir {
        stack.push("rust".into());
        let pre = if sub.is_empty() { String::new() } else { format!("cd {sub} && ") };
        add("build", format!("{pre}cargo build"));
        add("test", format!("{pre}cargo test"));
        add("lint", format!("{pre}cargo clippy"));
        if tauri { add("run", "npx @tauri-apps/cli@2 dev".into()); }
        else if read(&dir.join(format!("{sub}/Cargo.toml").trim_start_matches('/'))).is_some_and(|t| !t.contains("[lib]")) { add("run", format!("{pre}cargo run")); }
    }
    if dir.join("pyproject.toml").is_file() || dir.join("requirements.txt").is_file() || dir.join("setup.py").is_file() {
        stack.push("python".into());
        add("test", "python3 -m pytest".into());
    }
    if dir.join("go.mod").is_file() { stack.push("go".into()); add("build", "go build ./...".into()); add("test", "go test ./...".into()); }
    if dir.join("Package.swift").is_file() { stack.push("swift".into()); add("build", "swift build".into()); add("test", "swift test".into()); }
    if dir.join("Makefile").is_file() { add("build", "make".into()); }
    if stack.is_empty() && (dir.join("index.html").is_file() || dir.join("ui/index.html").is_file()) { stack.push("web".into()); }
    (stack, cmds)
}

const TODO_NAMES: [&str; 6] = ["TODO.md", "todo.md", "Todo.md", "TODO.markdown", "docs/TODO.md", "TASKS.md"];

fn first(dir: &Path, names: &[&str]) -> String {
    names.iter().map(|n| dir.join(n)).find(|p| p.is_file()).map(|p| p.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Whether a folder looks like an app of its own.
fn is_app(d: &Path) -> bool {
    d.join(".git").exists() || ["package.json", "Cargo.toml", "src-tauri", "pyproject.toml", "go.mod", "Package.swift"].iter().any(|n| d.join(n).exists())
}

pub fn info(dir: &str) -> Info {
    let d = PathBuf::from(dir);
    if !d.is_dir() { return Info { dir: dir.into(), name: d.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(), ..Default::default() }; }
    let (stack, commands) = commands(&d);
    let tauri = json(&d.join("src-tauri/tauri.conf.json")).or_else(|| json(&d.join("tauri.conf.json")));
    let pkg = json(&d.join("package.json"));
    let cargo = read(&d.join("Cargo.toml")).or_else(|| read(&d.join("src-tauri/Cargo.toml")));
    let s = |v: Option<&Value>, k: &str| v.and_then(|x| x.get(k)).and_then(Value::as_str).map(str::to_string).filter(|x| !x.is_empty());
    let name = s(tauri.as_ref(), "productName").or_else(|| s(pkg.as_ref(), "name")).or_else(|| cargo.as_deref().and_then(|c| cargo_field(c, "name")))
        .unwrap_or_else(|| d.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default());
    let version = s(tauri.as_ref(), "version").or_else(|| cargo.as_deref().and_then(|c| cargo_field(c, "version"))).or_else(|| s(pkg.as_ref(), "version")).unwrap_or_default();
    let description = cargo.as_deref().and_then(|c| cargo_field(c, "description")).or_else(|| s(pkg.as_ref(), "description")).unwrap_or_default();
    let todo: Vec<String> = TODO_NAMES.iter().map(|n| d.join(n)).filter(|p| p.is_file()).map(|p| p.to_string_lossy().into_owned()).collect();
    let icon = first(&d, &["app-icon.png", "icon.png", "src-tauri/icons/128x128.png", "public/favicon.png", "assets/icon.png"]);
    let mut apps: Vec<String> = Vec::new();
    if !is_app(&d) || !d.join(".git").exists() {
        if let Ok(rd) = std::fs::read_dir(&d) {
            for e in rd.flatten() {
                let p = e.path();
                let n = e.file_name().to_string_lossy().into_owned();
                if p.is_dir() && !n.starts_with('.') && n != "node_modules" && n != "target" && is_app(&p) { apps.push(p.to_string_lossy().into_owned()); }
            }
        }
        apps.sort_by_key(|a| a.to_lowercase());
    }
    Info {
        dir: dir.into(), exists: true, name, version, description, stack, commands, git: git_info(&d), todo,
        status: first(&d, &["STATUS.md", "Status.md", "status.md"]), changelog: first(&d, &["CHANGELOG.md", "Changelog.md", "CHANGES.md"]),
        readme: first(&d, &["README.md", "readme.md", "Readme.md"]), icon, apps,
    }
}

/// What is not committed yet: the patch against HEAD and the new files (for "Changes").
pub fn worktree_diff(dir: &str) -> String {
    let d = Path::new(dir);
    let mut out = git(d, &["diff", "HEAD", "--stat", "--patch"]).or_else(|| git(d, &["diff", "--stat", "--patch"])).unwrap_or_default();
    let new: Vec<String> = git(d, &["ls-files", "--others", "--exclude-standard"]).unwrap_or_default().lines().map(str::to_string).collect();
    for f in new.iter().take(200) { out += &format!("\n+++ {f} (new file)\n"); }
    if out.len() > 600_000 { out.truncate(600_000); out += "\n…"; }
    out
}

/// Makes the folder of a new app (it must not exist yet, or be empty).
pub fn make_dir(path: &str) -> Result<String, String> {
    let p = PathBuf::from(path.trim());
    if !p.is_absolute() { return Err(format!("Not a full path: {path}")); }
    if p.exists() && std::fs::read_dir(&p).map(|mut d| d.next().is_some()).unwrap_or(true) { return Err(format!("{path} exists and is not empty")); }
    std::fs::create_dir_all(&p).map_err(|e| e.to_string())?;
    Ok(p.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(n: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("agent-proj-{n}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn reads_a_tauri_app() {
        let d = tmp("tauri");
        std::fs::create_dir_all(d.join("src-tauri")).unwrap();
        std::fs::write(d.join("src-tauri/Cargo.toml"), "[package]\nname = \"demo-app\"\nversion = \"0.3.1\"\ndescription = \"A demo\"\n\n[dependencies]\nversion = \"9\"\n").unwrap();
        std::fs::write(d.join("src-tauri/tauri.conf.json"), r#"{"productName": "Demo Studio", "version": "0.3.2"}"#).unwrap();
        std::fs::write(d.join("TODO.md"), "- [ ] x").unwrap();
        std::fs::write(d.join("CHANGELOG.md"), "").unwrap();
        let i = info(&d.to_string_lossy());
        assert_eq!((i.name.as_str(), i.version.as_str(), i.description.as_str()), ("Demo Studio", "0.3.2", "A demo"));
        assert_eq!(i.stack, vec!["tauri", "rust"]);
        assert!(i.commands.iter().any(|c| c.kind == "test" && c.cmd == "cd src-tauri && cargo test"));
        assert!(i.commands.iter().any(|c| c.kind == "run" && c.cmd.contains("tauri")));
        assert!(i.todo[0].ends_with("TODO.md") && i.changelog.ends_with("CHANGELOG.md") && i.status.is_empty());
        assert!(i.git.is_none() || i.git.as_ref().is_some_and(|g| !g.branch.is_empty()), "outside a repository there is no git state of its own");
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn reads_node_scripts_and_finds_apps() {
        let d = tmp("ws");
        std::fs::create_dir_all(d.join("web")).unwrap();
        std::fs::write(d.join("web/package.json"), r#"{"name": "web", "version": "1.0.0", "scripts": {"dev": "vite", "build": "vite build", "test": "vitest"}}"#).unwrap();
        std::fs::write(d.join("web/yarn.lock"), "").unwrap();
        std::fs::create_dir_all(d.join("notes")).unwrap();
        std::fs::create_dir_all(d.join("node_modules/x")).unwrap();
        let i = info(&d.to_string_lossy());
        assert_eq!(i.apps.len(), 1);
        assert!(i.apps[0].ends_with("web"));
        let w = info(&i.apps[0]);
        assert_eq!((w.name.as_str(), w.version.as_str()), ("web", "1.0.0"));
        assert_eq!(w.commands.iter().map(|c| c.cmd.as_str()).collect::<Vec<_>>(), vec!["yarn run dev", "yarn run build", "yarn run test"]);
        assert!(!info(&d.join("missing").to_string_lossy()).exists);
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn git_state_and_changes() {
        let d = tmp("git");
        let g = |a: &[&str]| assert!(crate::runner::command("git").arg("-C").arg(&d).args(["-c", "user.name=t", "-c", "user.email=t@t"]).args(a).output().unwrap().status.success(), "{a:?}");
        g(&["init", "-q", "-b", "main"]);
        std::fs::write(d.join("a.txt"), "1\n").unwrap();
        g(&["add", "a.txt"]);
        g(&["commit", "-qm", "first"]);
        g(&["tag", "v0.1.0"]);
        std::fs::write(d.join("a.txt"), "2\n").unwrap();
        std::fs::write(d.join("new.txt"), "n").unwrap();
        let gi = git_info(&d).unwrap();
        assert_eq!((gi.branch.as_str(), gi.tag.as_str(), gi.upstream), ("main", "v0.1.0", false));
        assert_eq!(gi.changes, vec![" M a.txt".to_string(), "?? new.txt".to_string()]);
        assert!(gi.log[0].starts_with(&gi.log[0][..7]) && gi.log[0].contains("|first|"));
        let df = worktree_diff(&d.to_string_lossy());
        assert!(df.contains("+2") && df.contains("new.txt (new file)"));
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn new_app_folder() {
        let d = tmp("new");
        let p = d.join("My App");
        assert!(make_dir(&p.to_string_lossy()).is_ok() && p.is_dir());
        assert!(make_dir(&p.to_string_lossy()).is_ok(), "an empty folder is fine");
        std::fs::write(p.join("x"), "").unwrap();
        assert!(make_dir(&p.to_string_lossy()).is_err());
        assert!(make_dir("relative/x").is_err());
        let _ = std::fs::remove_dir_all(d);
    }
}
