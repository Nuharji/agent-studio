//! Runs Claude Code without a window (`claude -p`, output as stream-json) with fixed rights: what a run may do comes
//! only from Agent Studio (--setting-sources "" + --settings), anything else is refused at once (--permission-mode
//! dontAsk) and comes back as a request for approval. The person's sign-in is Claude Code's own — Agent Studio stores
//! no key or password.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Deserialize, Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct TodoRef {
    pub path: String,
    pub line: usize,
    pub text: String,
}

/// What to run and with which rights (built by the UI from a routine, a to-do item or a task).
#[derive(Deserialize, Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Spec {
    pub title: String,
    pub prompt: String,
    /// The first folder is where Claude starts; the others may be used too.
    pub dirs: Vec<String>,
    /// "read", "edit", "commit" or "publish".
    pub level: String,
    /// Commands allowed from the level "edit" on (e.g. "cargo test", "npm run build").
    pub commands: Vec<String>,
    pub web: bool,
    pub no_delete: bool,
    /// Only plan: say what would be done, change nothing.
    pub plan_only: bool,
    pub max_minutes: u32,
    pub model: String,
    pub rules: Vec<String>,
    pub lang: String,
    pub routine_id: Option<String>,
    pub todo: Option<TodoRef>,
    /// Continue this Claude Code session (an answer or an approval).
    pub resume: Option<String>,
    /// Rules allowed for this run only (an approval).
    pub allow_once: Vec<String>,
    /// The run this one continues.
    pub continues: Option<String>,
}

const LEVELS: [&str; 4] = ["read", "edit", "commit", "publish"];
pub fn level_no(l: &str) -> usize { LEVELS.iter().position(|x| *x == l).unwrap_or(0) }

/// "git status" → Bash(git status) and Bash(git status *).
fn bash(cmd: &str) -> [String; 2] { [format!("Bash({cmd})"), format!("Bash({cmd} *)")] }

/// The allowed and the refused rules of a run.
pub fn rules(s: &Spec) -> (Vec<String>, Vec<String>) {
    let lv = level_no(&s.level);
    let mut allow: Vec<String> = ["Read", "Glob", "Grep", "LS", "TodoWrite", "Task", "Agent"].map(String::from).to_vec();
    for c in ["git status", "git log", "git diff", "git show", "git branch", "git rev-parse", "git ls-files", "gh run list", "gh run view",
        "gh release list", "gh release view", "gh pr list", "gh pr view", "gh issue list", "gh issue view", "pwd", "ls"] {
        allow.extend(bash(c));
    }
    if lv >= 1 {
        allow.extend(["Edit", "Write", "MultiEdit", "NotebookEdit"].map(String::from));
        for c in s.commands.iter().map(|c| c.trim()).filter(|c| !c.is_empty()) { allow.extend(bash(c)); }
    }
    if lv >= 2 { for c in ["git add", "git commit", "git mv", "git restore --staged", "git stash list"] { allow.extend(bash(c)); } }
    if lv >= 3 { for c in ["git push", "git tag", "gh release create", "gh release upload", "gh pr create", "gh workflow run", "gh run rerun"] { allow.extend(bash(c)); } }
    if s.web { allow.extend(["WebFetch", "WebSearch"].map(String::from)); }
    allow.extend(s.allow_once.iter().cloned());
    let mut deny: Vec<String> = Vec::new();
    for c in ["git push --force", "git push -f", "git push --force-with-lease", "git reset --hard", "git clean", "sudo"] { deny.extend(bash(c)); }
    if s.no_delete { for c in ["rm", "rmdir", "git rm", "del", "rd", "Remove-Item", "unlink", "find . -delete"] { deny.extend(bash(c)); } }
    // An approval of one command wins over a refusal it falls under (e.g. one "rm build/x" while deleting is off).
    let once: Vec<String> = s.allow_once.iter().filter_map(|a| a.strip_prefix("Bash(").and_then(|x| x.strip_suffix(')'))).map(str::to_string).collect();
    deny.retain(|d| {
        let pre = d.strip_prefix("Bash(").and_then(|x| x.strip_suffix(')')).map(|x| x.trim_end_matches(" *"));
        !pre.is_some_and(|p| once.iter().any(|o| o == p || o.starts_with(&format!("{p} "))))
    });
    (allow, deny)
}

fn level_text(s: &Spec, de: bool) -> String {
    let lv = level_no(&s.level);
    let what = if de {
        ["nur lesen (Dateien ansehen, git status/log/diff)", "Dateien in den Ordnern ändern", "Dateien ändern und committen", "ändern, committen und pushen / veröffentlichen"][lv]
    } else {
        ["read only (look at files, git status/log/diff)", "change files in the folders", "change files and commit", "change, commit and push / publish"][lv]
    };
    let mut t = what.to_string();
    if lv >= 1 && !s.commands.is_empty() { t += &format!("{}{}", if de { "; erlaubte Befehle: " } else { "; allowed commands: " }, s.commands.join(", ")); }
    if s.web { t += if de { "; Internet erlaubt" } else { "; internet allowed" }; }
    if s.no_delete { t += if de { "; nichts löschen" } else { "; never delete" }; }
    t
}

/// Explains to Claude where it runs and how it asks for something (the block Agent Studio reads, see `needs`).
pub fn system_prompt(s: &Spec) -> String {
    let de = s.lang == "de";
    let rules = s.rules.iter().map(|r| r.trim()).filter(|r| !r.is_empty()).map(|r| format!("- {r}")).collect::<Vec<_>>().join("\n");
    let todo = s.todo.as_ref().map(|t| if de {
        format!("\nDieser Lauf ist für einen Punkt der To-do-Liste {} (Zeile {}): „{}“. Hake ihn nicht selbst ab — das macht Agent Studio.", t.path, t.line + 1, t.text)
    } else {
        format!("\nThis run is for an item of the to-do list {} (line {}): \"{}\". Don't tick it yourself — Agent Studio does.", t.path, t.line + 1, t.text)
    }).unwrap_or_default();
    if de {
        format!("Du arbeitest in Agent Studio für die Person, die dich eingerichtet hat — ohne Fenster: Während des Laufs kann dir niemand antworten.
Deine Rechte in diesem Lauf: {}. Was darüber hinausgeht, wird automatisch abgelehnt und der Person zur Freigabe vorgelegt — versuche dann nicht, es zu umgehen.
{}
Wenn du etwas von der Person brauchst (eine Frage, eine Entscheidung oder eine Freigabe für etwas, das du nicht darfst), höre an dieser Stelle auf und schreibe am Ende deiner Antwort genau einen Block:
```agent-studio
{{\"needs\": \"question\", \"text\": \"deine Frage\", \"options\": [\"Antwort 1\", \"Antwort 2\"]}}
```
(options darf fehlen; für eine Freigabe \"needs\": \"approval\" und in \"text\", was du tun willst.) Die Antwort kommt als nächste Nachricht.
Schreibe zum Schluss immer eine kurze Zusammenfassung auf Deutsch: was du getan hast, was geändert wurde, was offen ist.{}",
            level_text(s, true), if rules.is_empty() { String::new() } else { format!("Regeln der Person:\n{rules}\n") }, todo)
    } else {
        format!("You work in Agent Studio for the person who set you up — without a window: nobody can answer you during the run.
Your rights in this run: {}. Anything beyond is refused automatically and shown to the person for approval — don't try to work around it then.
{}
When you need something from the person (a question, a decision, or approval for something you may not do), stop there and end your answer with exactly one block:
```agent-studio
{{\"needs\": \"question\", \"text\": \"your question\", \"options\": [\"answer 1\", \"answer 2\"]}}
```
(options may be left out; for an approval use \"needs\": \"approval\" and say in \"text\" what you want to do.) The answer comes as the next message.
Always end with a short summary: what you did, what changed, what is still open.{}",
            level_text(s, false), if rules.is_empty() { String::new() } else { format!("The person's rules:\n{rules}\n") }, todo)
    }
}

/// The arguments of `claude` (the prompt itself goes to its input).
pub fn args(s: &Spec) -> Vec<String> {
    let (allow, deny) = rules(s);
    let extra: Vec<&String> = s.dirs.iter().skip(1).collect();
    let settings = json!({"permissions": {"allow": allow, "deny": deny, "additionalDirectories": extra}});
    let mut a: Vec<String> = ["-p", "--output-format", "stream-json", "--verbose", "--permission-mode"].map(String::from).to_vec();
    a.push(if s.plan_only { "plan" } else { "dontAsk" }.into());
    a.extend(["--setting-sources".into(), String::new(), "--settings".into(), settings.to_string(), "--append-system-prompt".into(), system_prompt(s)]);
    if !s.model.trim().is_empty() { a.extend(["--model".into(), s.model.trim().into()]); }
    if let Some(r) = s.resume.as_ref().filter(|r| !r.is_empty()) { a.extend(["--resume".into(), r.clone()]); }
    a
}

/// One line of a run's log.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct LogLine {
    pub t: u64,
    /// "text" (Claude writes), "tool" (a step: file, command …), "denied", "error", "info".
    pub kind: String,
    pub text: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Denial {
    pub tool: String,
    /// The rule that would allow it (e.g. "Bash(git push origin main)").
    pub rule: String,
    pub what: String,
}

#[derive(Debug, PartialEq)]
pub enum Ev {
    Session(String),
    Log(&'static str, String),
    Result { text: String, error: bool, cost: f64, denials: Vec<Denial> },
}

fn short(s: &str, n: usize) -> String {
    let one = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if one.chars().count() <= n { one } else { one.chars().take(n).collect::<String>() + "…" }
}

/// What a tool call does, in a few words: "Bash: cargo test", "Edit: src/main.rs".
pub fn tool_summary(name: &str, input: &Value) -> String {
    let s = |k: &str| input.get(k).and_then(Value::as_str).unwrap_or("");
    let what = match name {
        "Bash" | "PowerShell" => s("command").to_string(),
        "Read" | "Edit" | "Write" | "MultiEdit" | "NotebookEdit" => s("file_path").to_string() + s("notebook_path"),
        "Grep" | "Glob" => s("pattern").to_string(),
        "WebFetch" => s("url").to_string(),
        "WebSearch" => s("query").to_string(),
        "Task" | "Agent" => s("description").to_string(),
        _ => String::new(),
    };
    if what.is_empty() { name.to_string() } else { format!("{name}: {}", short(&what, 160)) }
}

fn denial(d: &Value) -> Denial {
    let tool = d.get("tool_name").and_then(Value::as_str).unwrap_or("").to_string();
    let input = d.get("tool_input").cloned().unwrap_or(Value::Null);
    let rule = match (tool.as_str(), input.get("command").and_then(Value::as_str)) {
        ("Bash", Some(c)) => format!("Bash({})", c.trim()),
        _ => tool.clone(),
    };
    Denial { what: tool_summary(&tool, &input), tool, rule }
}

/// Reads one line of `claude`'s stream-json output.
pub fn parse_line(line: &str) -> Vec<Ev> {
    let Ok(j) = serde_json::from_str::<Value>(line) else {
        return if line.trim().is_empty() { vec![] } else { vec![Ev::Log("info", short(line, 400))] };
    };
    let mut out = Vec::new();
    match j.get("type").and_then(Value::as_str) {
        Some("system") if j.get("subtype").and_then(Value::as_str) == Some("init") => {
            if let Some(id) = j.get("session_id").and_then(Value::as_str) { out.push(Ev::Session(id.into())); }
        }
        Some("assistant") => for c in j.pointer("/message/content").and_then(Value::as_array).into_iter().flatten() {
            match c.get("type").and_then(Value::as_str) {
                Some("text") => {
                    let t = c.get("text").and_then(Value::as_str).unwrap_or("").trim();
                    if !t.is_empty() { out.push(Ev::Log("text", t.to_string())); }
                }
                Some("tool_use") => {
                    let name = c.get("name").and_then(Value::as_str).unwrap_or("?");
                    out.push(Ev::Log("tool", tool_summary(name, c.get("input").unwrap_or(&Value::Null))));
                }
                _ => {}
            }
        },
        Some("user") => for c in j.pointer("/message/content").and_then(Value::as_array).into_iter().flatten() {
            if c.get("type").and_then(Value::as_str) == Some("tool_result") && c.get("is_error").and_then(Value::as_bool) == Some(true) {
                let text = match c.get("content") { Some(Value::String(s)) => s.clone(), Some(Value::Array(a)) => a.iter().filter_map(|x| x.get("text").and_then(Value::as_str)).collect::<Vec<_>>().join(" "), _ => String::new() };
                let denied = text.contains("permission") || text.contains("denied") || text.contains("not allowed");
                out.push(Ev::Log(if denied { "denied" } else { "error" }, short(&text, 300)));
            }
        },
        Some("result") => {
            let mut denials: Vec<Denial> = Vec::new();
            for d in j.get("permission_denials").and_then(Value::as_array).into_iter().flatten().map(denial) {
                if !denials.iter().any(|x| x.rule == d.rule) { denials.push(d); }
            }
            let text = j.get("result").and_then(Value::as_str).unwrap_or("").to_string();
            let error = j.get("is_error").and_then(Value::as_bool).unwrap_or(false) || j.get("subtype").and_then(Value::as_str).is_some_and(|s| s.starts_with("error"));
            out.push(Ev::Result { text, error, cost: j.get("total_cost_usd").and_then(Value::as_f64).unwrap_or(0.0), denials });
        }
        _ => {}
    }
    out
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct Needs {
    /// "question" or "approval".
    pub kind: String,
    pub text: String,
    pub options: Vec<String>,
}

/// The ```agent-studio block at the end of Claude's answer (a question / an approval), and the answer without it.
pub fn needs(text: &str) -> (String, Option<Needs>) {
    let Some(start) = text.rfind("```agent-studio") else { return (text.trim().to_string(), None) };
    let body = &text[start + "```agent-studio".len()..];
    let end = body.find("```").unwrap_or(body.len());
    let rest = format!("{}{}", &text[..start], body.get(end + 3..).unwrap_or("")).trim().to_string();
    let Ok(v) = serde_json::from_str::<Value>(body[..end].trim()) else { return (rest, Some(Needs { kind: "question".into(), text: body[..end].trim().into(), options: vec![] })) };
    let kind = match v.get("needs").and_then(Value::as_str) { Some("approval") => "approval", _ => "question" };
    let options = v.get("options").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default();
    (rest, Some(Needs { kind: kind.into(), text: v.get("text").and_then(Value::as_str).unwrap_or("").to_string(), options }))
}

/// A command without a console window on Windows.
pub fn command(program: impl AsRef<std::ffi::OsStr>) -> Command {
    #[allow(unused_mut)]
    let mut c = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000);
    }
    c
}

/// Where `claude` is: the path set in the settings, else the usual places (installer, npm, Homebrew, the Claude
/// desktop app's own copy), else what the login shell finds.
pub fn find_claude(own: Option<&str>) -> Option<PathBuf> {
    if let Some(p) = own.map(str::trim).filter(|p| !p.is_empty()) { return Some(PathBuf::from(p)).filter(|p| p.is_file()); }
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from).unwrap_or_default();
    let mut c: Vec<PathBuf> = Vec::new();
    if cfg!(windows) {
        c.push(home.join(".local").join("bin").join("claude.exe"));
        if let Some(a) = std::env::var_os("APPDATA") { c.push(PathBuf::from(a).join("npm").join("claude.cmd")); }
    } else {
        c.extend([home.join(".local/bin/claude"), home.join(".claude/local/claude"), "/opt/homebrew/bin/claude".into(), "/usr/local/bin/claude".into(),
            home.join(".npm-global/bin/claude"), home.join(".bun/bin/claude")]);
    }
    if let Some(p) = c.into_iter().find(|p| p.is_file()) { return Some(p); }
    #[cfg(target_os = "macos")]
    if let Some(p) = desktop_copy(&home.join("Library/Application Support/Claude/claude-code")) { return Some(p); }
    let found = if cfg!(windows) {
        command("where").arg("claude").output().ok()
    } else {
        let sh = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
        command(sh).args(["-lc", "command -v claude"]).output().ok()
    }?;
    String::from_utf8_lossy(&found.stdout).lines().map(str::trim).map(PathBuf::from).find(|p| p.is_file())
}

/// The newest copy of Claude Code that the Claude desktop app keeps (<dir>/<version>/<id>/claude.app/…/claude).
#[cfg(target_os = "macos")]
fn desktop_copy(dir: &Path) -> Option<PathBuf> {
    let ver = |s: &str| s.split('.').map(|x| x.parse::<u32>().unwrap_or(0)).collect::<Vec<_>>();
    let mut vs: Vec<PathBuf> = std::fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    vs.sort_by_key(|p| ver(&p.file_name().unwrap_or_default().to_string_lossy()));
    vs.iter().rev().find_map(|v| std::fs::read_dir(v).ok()?.flatten().map(|e| e.path().join("claude.app/Contents/MacOS/claude")).find(|p| p.is_file()))
}

/// Removes what a surrounding Claude Code session put into the environment, so the run uses its own sign-in.
pub fn clean_env(c: &mut Command) {
    for (k, _) in std::env::vars_os() {
        let k = k.to_string_lossy();
        if k == "CLAUDECODE" || k.starts_with("CLAUDE_CODE_") || k.starts_with("CLAUDE_AGENT_SDK") || k == "CLAUDE_PID" || k == "CLAUDE_EFFORT" {
            c.env_remove(k.as_ref());
        }
    }
}

// ---- git: what a run changed ----

fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let o = command("git").arg("-C").arg(dir).args(args).output().ok()?;
    o.status.success().then(|| String::from_utf8_lossy(&o.stdout).into_owned())
}
fn lines(s: Option<String>) -> Vec<String> { s.unwrap_or_default().lines().map(str::trim).filter(|l| !l.is_empty()).map(str::to_string).collect() }

/// A repository before a run (to tell what the run changed, and to undo its commits).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Repo {
    pub dir: String,
    pub before: String,
    pub untracked_before: Vec<String>,
    /// After the run: its commits ("abc1234 message"), the files it changed, whether its commits are pushed.
    pub commits: Vec<String>,
    pub files: Vec<String>,
    pub pushed: bool,
}

pub fn snapshot(dirs: &[String]) -> Vec<Repo> {
    let mut out: Vec<Repo> = Vec::new();
    for d in dirs {
        let p = Path::new(d);
        let Some(top) = git(p, &["rev-parse", "--show-toplevel"]).map(|s| s.trim().to_string()) else { continue };
        if out.iter().any(|r| r.dir == top) { continue; }
        let Some(head) = git(p, &["rev-parse", "HEAD"]).map(|s| s.trim().to_string()) else { continue };
        out.push(Repo { untracked_before: lines(git(Path::new(&top), &["ls-files", "--others", "--exclude-standard"])), dir: top, before: head, ..Default::default() });
    }
    out
}

pub fn after(repos: &mut [Repo]) {
    for r in repos {
        let d = Path::new(&r.dir);
        let range = format!("{}..HEAD", r.before);
        r.commits = lines(git(d, &["log", "--format=%h %s", &range]));
        let mut files = lines(git(d, &["diff", "--name-only", &r.before]));
        for f in lines(git(d, &["ls-files", "--others", "--exclude-standard"])) { if !r.untracked_before.contains(&f) && !files.contains(&f) { files.push(f); } }
        r.files = files;
        r.pushed = !r.commits.is_empty() && !lines(git(d, &["branch", "-r", "--contains", "HEAD"])).is_empty();
    }
}

/// The changes of a run (its commits and what is not committed yet), for the diff view.
pub fn diff(repos: &[Repo]) -> String {
    let mut out = String::new();
    for r in repos {
        let d = Path::new(&r.dir);
        let text = git(d, &["diff", "--stat", "--patch", &r.before]).unwrap_or_default();
        let new: Vec<String> = lines(git(d, &["ls-files", "--others", "--exclude-standard"])).into_iter().filter(|f| !r.untracked_before.contains(f)).collect();
        if text.trim().is_empty() && new.is_empty() { continue; }
        out += &format!("### {}\n{}", r.dir, text);
        for f in new { out += &format!("\n+++ {f} (new file)\n"); }
        out += "\n";
        if out.len() > 600_000 { out.truncate(600_000); out += "\n…"; break; }
    }
    out
}

/// Takes back a run's commits (only those not pushed); the files go back to how they were before the run, changes
/// made since in the folder are kept (git reset --keep refuses otherwise).
pub fn undo(repos: &mut [Repo]) -> Result<usize, String> {
    let mut n = 0;
    for r in repos.iter_mut().filter(|r| !r.commits.is_empty()) {
        let d = Path::new(&r.dir);
        if !lines(git(d, &["branch", "-r", "--contains", "HEAD"])).is_empty() { return Err(format!("{}: pushed", r.dir)); }
        let o = command("git").arg("-C").arg(d).args(["reset", "--keep", &r.before]).output().map_err(|e| e.to_string())?;
        if !o.status.success() { return Err(String::from_utf8_lossy(&o.stderr).trim().to_string()); }
        n += r.commits.len();
        r.commits.clear();
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(level: &str) -> Spec { Spec { level: level.into(), lang: "de".into(), dirs: vec!["/a".into(), "/b".into()], ..Default::default() } }

    #[test]
    fn rights_grow_with_the_level() {
        let (r, _) = rules(&spec("read"));
        assert!(r.contains(&"Read".into()) && r.contains(&"Bash(git status *)".into()));
        assert!(!r.contains(&"Edit".into()) && !r.iter().any(|x| x.contains("git commit")));
        let mut s = spec("edit");
        s.commands = vec!["cargo test".into(), " ".into()];
        let (r, _) = rules(&s);
        assert!(r.contains(&"Edit".into()) && r.contains(&"Bash(cargo test)".into()) && r.contains(&"Bash(cargo test *)".into()));
        assert!(!r.iter().any(|x| x.contains("git commit")) && !r.contains(&"Bash()".into()));
        let (r, _) = rules(&spec("commit"));
        assert!(r.contains(&"Bash(git commit *)".into()) && !r.iter().any(|x| x.contains("git push")));
        let (r, d) = rules(&spec("publish"));
        assert!(r.contains(&"Bash(git push *)".into()) && d.contains(&"Bash(git push --force *)".into()));
        assert!(!r.contains(&"WebFetch".into()));
    }

    #[test]
    fn approval_beats_its_refusal() {
        let mut s = spec("edit");
        s.no_delete = true;
        assert!(rules(&s).1.contains(&"Bash(rm *)".into()));
        s.allow_once = vec!["Bash(rm build/old.txt)".into()];
        let (a, d) = rules(&s);
        assert!(a.contains(&"Bash(rm build/old.txt)".into()) && !d.iter().any(|x| x == "Bash(rm)" || x == "Bash(rm *)"));
        assert!(d.contains(&"Bash(git rm *)".into()), "other refusals stay");
    }

    #[test]
    fn arguments() {
        let mut s = spec("commit");
        s.model = "sonnet".into();
        s.resume = Some("abc".into());
        let a = args(&s);
        assert_eq!(&a[..6], &["-p", "--output-format", "stream-json", "--verbose", "--permission-mode", "dontAsk"]);
        let i = a.iter().position(|x| x == "--settings").unwrap();
        let v: Value = serde_json::from_str(&a[i + 1]).unwrap();
        assert_eq!(v["permissions"]["additionalDirectories"], json!(["/b"]));
        assert_eq!(a[a.iter().position(|x| x == "--setting-sources").unwrap() + 1], "");
        assert!(a.windows(2).any(|w| w == ["--model", "sonnet"]) && a.windows(2).any(|w| w == ["--resume", "abc"]));
        assert!(a[a.iter().position(|x| x == "--append-system-prompt").unwrap() + 1].contains("committen"));
        s.plan_only = true;
        assert_eq!(args(&s)[5], "plan");
    }

    #[test]
    fn reads_the_stream() {
        assert_eq!(parse_line(r#"{"type":"system","subtype":"init","session_id":"s1"}"#), vec![Ev::Session("s1".into())]);
        let a = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Ich schaue nach."},{"type":"tool_use","name":"Bash","input":{"command":"git status"}}]}}"#;
        assert_eq!(parse_line(a), vec![Ev::Log("text", "Ich schaue nach.".into()), Ev::Log("tool", "Bash: git status".into())]);
        let u = r#"{"type":"user","message":{"content":[{"type":"tool_result","is_error":true,"content":"Claude requested permissions to use Bash, but you haven't granted it yet."}]}}"#;
        assert!(matches!(&parse_line(u)[0], Ev::Log("denied", _)));
        let r = r#"{"type":"result","subtype":"success","is_error":false,"result":"Fertig.","total_cost_usd":0.12,"permission_denials":[{"tool_name":"Bash","tool_use_id":"x","tool_input":{"command":"git push origin main"}},{"tool_name":"Bash","tool_use_id":"y","tool_input":{"command":"git push origin main"}},{"tool_name":"Write","tool_input":{"file_path":"/etc/x"}}]}"#;
        let Ev::Result { text, error, cost, denials } = &parse_line(r)[0] else { panic!() };
        assert_eq!((text.as_str(), *error, *cost), ("Fertig.", false, 0.12));
        assert_eq!(denials.iter().map(|d| d.rule.as_str()).collect::<Vec<_>>(), vec!["Bash(git push origin main)", "Write"]);
        assert_eq!(denials[1].what, "Write: /etc/x");
        assert!(parse_line("").is_empty());
    }

    #[test]
    fn reads_the_request_block() {
        let t = "Ich habe alles vorbereitet.\n\n```agent-studio\n{\"needs\": \"approval\", \"text\": \"Release v1.2.0 pushen?\", \"options\": [\"Ja\", \"Nein\"]}\n```\n";
        let (rest, n) = needs(t);
        assert_eq!(rest, "Ich habe alles vorbereitet.");
        assert_eq!(n, Some(Needs { kind: "approval".into(), text: "Release v1.2.0 pushen?".into(), options: vec!["Ja".into(), "Nein".into()] }));
        assert_eq!(needs("Nur Text").1, None);
        let (_, n) = needs("x\n```agent-studio\nWelche Farbe?\n```");
        assert_eq!(n.unwrap().text, "Welche Farbe?");
    }

    #[test]
    fn finds_what_a_run_changed_and_takes_it_back() {
        let d = std::env::temp_dir().join(format!("agent-git-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let g = |a: &[&str]| assert!(command("git").arg("-C").arg(&d).args(["-c", "user.name=t", "-c", "user.email=t@t"]).args(a).output().unwrap().status.success(), "{a:?}");
        g(&["init", "-q"]);
        std::fs::write(d.join("a.txt"), "1").unwrap();
        std::fs::write(d.join("old.txt"), "untracked before").unwrap();
        g(&["add", "a.txt"]);
        g(&["commit", "-qm", "first"]);
        let mut repos = snapshot(&[d.to_string_lossy().into_owned(), d.join(".").to_string_lossy().into_owned()]);
        assert_eq!(repos.len(), 1, "the same repository once");
        // The run: a commit and a new file.
        std::fs::write(d.join("a.txt"), "2").unwrap();
        g(&["commit", "-qam", "change a"]);
        std::fs::write(d.join("new.txt"), "new").unwrap();
        after(&mut repos);
        let r = &repos[0];
        assert_eq!(r.commits.len(), 1);
        assert!(r.commits[0].ends_with("change a"));
        assert_eq!(r.files, vec!["a.txt".to_string(), "new.txt".to_string()]);
        assert!(!r.pushed);
        let df = diff(&repos);
        assert!(df.contains("+2") && df.contains("new.txt (new file)") && !df.contains("old.txt"));
        assert_eq!(undo(&mut repos).unwrap(), 1);
        assert_eq!(std::fs::read_to_string(d.join("a.txt")).unwrap(), "1");
        assert!(d.join("new.txt").exists(), "files that are not committed stay");
        let _ = std::fs::remove_dir_all(d);
    }
}
