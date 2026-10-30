//! ChatGPT as the other AI: runs OpenAI's Codex CLI without a window (`codex exec --json`). The person's sign-in is
//! Codex's own (their ChatGPT account) — Agent Studio stores no key or password. Rights come from Codex's sandbox, which
//! knows no rules per command: read-only or writing in the run's folders, never more. So commits and pushes (Codex keeps
//! .git read-only) stay with the person; "never delete" and the allowed commands are only told to the model.

use crate::runner::{self, Ev, Spec};
use serde_json::Value;
use std::path::PathBuf;

/// A model of ChatGPT ("chatgpt" = the one Codex recommends) — the model decides which AI does a run.
pub fn is_codex(model: &str) -> bool {
    let m = model.trim();
    m == "chatgpt" || m.starts_with("gpt-")
}

/// A value for `-c key=value` (TOML; a JSON string is a valid TOML string).
fn toml_str(s: &str) -> String { serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into()) }

/// The arguments of `codex` (the prompt itself goes to its input). Only Agent Studio's rights count: no config or rules
/// of the person's own, which could allow more than the sandbox.
pub fn args(s: &Spec) -> Vec<String> {
    let write = runner::level_no(&s.level) >= 1 && !s.plan_only;
    let mut a: Vec<String> = ["exec", "--json", "--skip-git-repo-check", "--ignore-user-config", "--ignore-rules", "--sandbox"].map(String::from).to_vec();
    a.push(if write { "workspace-write" } else { "read-only" }.into());
    if write { for d in s.dirs.iter().skip(1) { a.extend(["--add-dir".into(), d.clone()]); } }
    let mut set = |k: &str, v: String| a.extend(["-c".into(), format!("{k}={v}")]);
    set("approval_policy", toml_str("never"));
    set("sandbox_workspace_write.network_access", s.web.to_string());
    set("web_search", toml_str(if s.web { "live" } else { "disabled" }));
    set("developer_instructions", toml_str(&runner::system_prompt(s)));
    let m = s.model.trim();
    if !m.is_empty() && m != "chatgpt" { a.extend(["--model".into(), m.into()]); }
    if let Some(r) = s.resume.as_ref().filter(|r| !r.is_empty()) { a.extend(["resume".into(), r.clone()]); }
    a.push("-".into());
    a
}

/// Reads `codex exec --json`; the last message of the agent is the run's answer.
#[derive(Default)]
pub struct Reader {
    last: String,
    error: String,
    done: bool,
}

impl Reader {
    pub fn line(&mut self, line: &str) -> Vec<Ev> {
        let Ok(j) = serde_json::from_str::<Value>(line) else {
            return if line.trim().is_empty() { vec![] } else { vec![Ev::Log("info", runner::short(line, 400))] };
        };
        let msg = |v: Option<&Value>| v.and_then(Value::as_str).unwrap_or("").trim().to_string();
        match j.get("type").and_then(Value::as_str) {
            Some("thread.started") => j.get("thread_id").and_then(Value::as_str).map(|id| vec![Ev::Session(id.into())]).unwrap_or_default(),
            Some("item.started") => match j.pointer("/item/type").and_then(Value::as_str) {
                Some("command_execution") => vec![Ev::Log("tool", format!("Bash: {}", runner::short(&msg(j.pointer("/item/command")), 160)))],
                _ => vec![],
            },
            Some("item.completed") => self.item(j.get("item").unwrap_or(&Value::Null)),
            Some("turn.completed") => {
                self.done = true;
                vec![Ev::Result { text: self.last.clone(), error: false, cost: 0.0, denials: vec![] }]
            }
            Some("turn.failed") => {
                self.done = true;
                vec![Ev::Result { text: msg(j.pointer("/error/message")), error: true, cost: 0.0, denials: vec![] }]
            }
            // Also comes for what Codex retries by itself, so it only counts when no result follows.
            Some("error") => {
                self.error = msg(j.get("message"));
                vec![Ev::Log("error", runner::short(&self.error, 300))]
            }
            _ => vec![],
        }
    }

    fn item(&mut self, it: &Value) -> Vec<Ev> {
        let s = |k: &str| it.get(k).and_then(Value::as_str).unwrap_or("").trim();
        match s("type") {
            "agent_message" if !s("text").is_empty() => {
                self.last = s("text").to_string();
                vec![Ev::Log("text", self.last.clone())]
            }
            "command_execution" if s("status") == "declined" => vec![Ev::Log("denied", runner::short(s("command"), 300))],
            "command_execution" if s("status") == "failed" => {
                let out = s("aggregated_output");
                let denied = out.contains("Operation not permitted") || out.contains("Read-only file system") || out.contains("Permission denied");
                vec![Ev::Log(if denied { "denied" } else { "error" }, runner::short(if out.is_empty() { s("command") } else { out }, 300))]
            }
            "file_change" => it.get("changes").and_then(Value::as_array).into_iter().flatten().map(|c| {
                let what = if c.get("kind").and_then(Value::as_str) == Some("delete") { "Delete" } else { "Edit" };
                Ev::Log("tool", format!("{what}: {}", c.get("path").and_then(Value::as_str).unwrap_or("")))
            }).collect(),
            "web_search" if !s("query").is_empty() => vec![Ev::Log("tool", format!("WebSearch: {}", runner::short(s("query"), 160)))],
            "error" => vec![Ev::Log("error", runner::short(s("message"), 300))],
            _ => vec![],
        }
    }

    /// The output has ended: an error without a result is the run's result.
    pub fn end(&self) -> Option<Ev> {
        (!self.done && !self.error.is_empty()).then(|| Ev::Result { text: self.error.clone(), error: true, cost: 0.0, denials: vec![] })
    }
}

/// Where `codex` is: the path set in the settings, else the usual places (npm, Homebrew, the Codex app), else what the
/// login shell finds.
pub fn find(own: Option<&str>) -> Option<PathBuf> {
    if let Some(p) = own.map(str::trim).filter(|p| !p.is_empty()) { return Some(PathBuf::from(p)).filter(|p| p.is_file()); }
    let home = runner::home();
    let mut c: Vec<PathBuf> = Vec::new();
    if cfg!(windows) {
        if let Some(a) = std::env::var_os("APPDATA") { c.push(PathBuf::from(a).join("npm").join("codex.cmd")); }
    } else {
        c.extend([home.join(".local/bin/codex"), "/opt/homebrew/bin/codex".into(), "/usr/local/bin/codex".into(), home.join(".npm-global/bin/codex"), home.join(".bun/bin/codex"),
            "/Applications/Codex.app/Contents/Resources/codex".into()]);
    }
    c.into_iter().find(|p| p.is_file()).or_else(|| runner::on_path("codex"))
}

/// Whether someone is signed in: `codex login status` (says so on stderr, fails when not).
pub fn status_of(ok: bool, out: &str) -> (bool, String) {
    let line = out.lines().map(str::trim).find(|l| l.starts_with("Logged in") || l.starts_with("API key")).unwrap_or("");
    (ok && !line.is_empty(), line.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(level: &str) -> Spec { Spec { level: level.into(), lang: "de".into(), model: "chatgpt".into(), dirs: vec!["/a".into(), "/b".into()], ..Default::default() } }
    fn has(a: &[String], k: &str, v: &str) -> bool { a.windows(2).any(|w| w[0] == k && w[1] == v) }

    #[test]
    fn the_model_decides_the_ai() {
        assert!(is_codex("chatgpt") && is_codex("gpt-6.1-sol"));
        assert!(!is_codex("") && !is_codex("opus") && !is_codex("sonnet"));
    }

    #[test]
    fn rights_become_the_sandbox() {
        let a = args(&spec("read"));
        assert_eq!(&a[..2], &["exec", "--json"]);
        assert!(has(&a, "--sandbox", "read-only") && !a.contains(&"--add-dir".to_string()) && !a.contains(&"--model".to_string()));
        assert!(a.contains(&"--ignore-user-config".to_string()) && a.contains(&"--ignore-rules".to_string()));
        assert_eq!(a.last().unwrap(), "-");
        for l in ["edit", "commit", "publish"] {
            let a = args(&spec(l));
            assert!(has(&a, "--sandbox", "workspace-write") && has(&a, "--add-dir", "/b"), "{l}");
            assert!(!a.iter().any(|x| x.contains("danger")), "never without the sandbox");
        }
        let mut s = spec("publish");
        s.plan_only = true;
        assert!(has(&args(&s), "--sandbox", "read-only"), "plan only changes nothing");
        assert!(has(&args(&spec("edit")), "-c", "sandbox_workspace_write.network_access=false"));
        assert!(has(&args(&spec("edit")), "-c", "web_search=\"disabled\""));
        s.web = true;
        assert!(has(&args(&s), "-c", "sandbox_workspace_write.network_access=true") && has(&args(&s), "-c", "web_search=\"live\""));
    }

    #[test]
    fn goes_on_and_takes_a_model() {
        let mut s = spec("edit");
        s.model = "gpt-6.1-sol".into();
        s.resume = Some("t1".into());
        let a = args(&s);
        assert!(has(&a, "--model", "gpt-6.1-sol"));
        assert_eq!(&a[a.len() - 3..], &["resume", "t1", "-"], "the options come before the subcommand");
    }

    #[test]
    fn instructions_stay_one_toml_string() {
        let a = args(&spec("edit"));
        let v = a.iter().find_map(|x| x.strip_prefix("developer_instructions=")).unwrap();
        let text: String = serde_json::from_str(v).unwrap();
        assert!(text.contains("```agent-studio") && text.contains("Commits") && !v.contains('\n'));
    }

    #[test]
    fn reads_the_stream() {
        let mut r = Reader::default();
        assert_eq!(r.line(r#"{"type":"thread.started","thread_id":"t1"}"#), vec![Ev::Session("t1".into())]);
        assert!(r.line(r#"{"type":"turn.started"}"#).is_empty());
        assert_eq!(r.line(r#"{"type":"item.started","item":{"id":"i","type":"command_execution","command":"bash -lc ls","status":"in_progress"}}"#), vec![Ev::Log("tool", "Bash: bash -lc ls".into())]);
        assert!(r.line(r#"{"type":"item.completed","item":{"id":"i","type":"command_execution","command":"ls","aggregated_output":"x","exit_code":0,"status":"completed"}}"#).is_empty());
        let e = r.line(r#"{"type":"item.completed","item":{"id":"i","type":"command_execution","command":"touch /x","aggregated_output":"touch: /x: Operation not permitted","exit_code":1,"status":"failed"}}"#);
        assert!(matches!(&e[0], Ev::Log("denied", _)));
        assert_eq!(r.line(r#"{"type":"item.completed","item":{"id":"i","type":"file_change","changes":[{"path":"a.md","kind":"update"},{"path":"b.md","kind":"delete"}],"status":"completed"}}"#),
            vec![Ev::Log("tool", "Edit: a.md".into()), Ev::Log("tool", "Delete: b.md".into())]);
        assert_eq!(r.line(r#"{"type":"item.completed","item":{"id":"i","type":"agent_message","text":"Fertig."}}"#), vec![Ev::Log("text", "Fertig.".into())]);
        assert_eq!(r.line(r#"{"type":"turn.completed","usage":{"input_tokens":1,"output_tokens":1}}"#), vec![Ev::Result { text: "Fertig.".into(), error: false, cost: 0.0, denials: vec![] }]);
        assert_eq!(r.end(), None);
    }

    #[test]
    fn errors_count_only_without_a_result() {
        let mut r = Reader::default();
        r.line(r#"{"type":"error","message":"Reconnecting... 1/5"}"#);
        assert!(matches!(r.end(), Some(Ev::Result { error: true, .. })));
        r.line(r#"{"type":"turn.completed","usage":{}}"#);
        assert_eq!(r.end(), None, "it went on after the retry");
        let mut r = Reader::default();
        assert_eq!(r.line(r#"{"type":"turn.failed","error":{"message":"Kaputt"}}"#), vec![Ev::Result { text: "Kaputt".into(), error: true, cost: 0.0, denials: vec![] }]);
    }

    #[test]
    fn reads_the_sign_in() {
        assert_eq!(status_of(true, "Logged in using ChatGPT\n"), (true, "Logged in using ChatGPT".to_string()));
        assert!(!status_of(false, "Not logged in\n").0);
    }
}
