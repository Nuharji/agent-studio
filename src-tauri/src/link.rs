//! Agent Studio's side of the link to the other Studio apps (the files in <config dir>/Studio/agent/, see
//! agentlink.rs): takes the tasks the apps send (inbox), reports how they go and what came out (outbox) and keeps
//! status.json up to date for Studio Hub (badge) and Plan Studio (morning overview).

use crate::agentlink;
use crate::runs::{now, Data, Run};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use tauri::AppHandle;

/// Where a run came from when another Studio app sent it.
#[derive(Deserialize, Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Origin {
    pub app: String,
    /// The id of the task (its file in the inbox / outbox).
    pub task: String,
    pub title: String,
    pub created: u64,
}

fn write(p: &Path, v: &Value) {
    if let Some(d) = p.parent() { let _ = std::fs::create_dir_all(d); }
    let tmp = p.with_extension("tmp");
    if std::fs::write(&tmp, serde_json::to_string_pretty(v).unwrap_or_default()).is_ok() { let _ = std::fs::rename(&tmp, p); }
}

/// The tasks waiting in the inbox (oldest first); they leave it.
pub fn take(app: &AppHandle) -> Vec<Value> {
    let Some(d) = agentlink::dir(app) else { return vec![] };
    let mut out: Vec<Value> = Vec::new();
    for e in std::fs::read_dir(d.join("inbox")).into_iter().flatten().flatten() {
        let p = e.path();
        if p.extension().is_none_or(|x| x != "json") { continue; }
        let id = p.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
        let v = std::fs::read_to_string(&p).ok().and_then(|s| serde_json::from_str::<Value>(&s).ok());
        let _ = std::fs::remove_file(&p);
        if !agentlink::valid_id(&id) { continue; }
        if let Some(t) = v.and_then(|v| agentlink::clean_task(&v, &id).ok().map(|mut t| { t["created"] = v["created"].clone(); t })) { out.push(t); }
    }
    out.sort_by_key(|t| t.get("created").and_then(Value::as_u64).unwrap_or(0));
    out
}

/// Tells the app that sent a task what happened to it without a run (Agent Studio takes no tasks, the limit …).
pub fn decline(app: &AppHandle, task: &Value, why: &str) {
    let Some(d) = agentlink::dir(app) else { return };
    let id = task.get("id").and_then(Value::as_str).unwrap_or("");
    if !agentlink::valid_id(id) { return; }
    write(&d.join("outbox").join(format!("{id}.json")), &json!({"id": id, "app": task["app"], "title": task["title"], "status": "declined",
        "summary": why, "files": [], "created": task["created"], "updated": now()}));
}

/// The ```studio-result block at the end of an answer (data for the app, e.g. Plan Studio's entries) and the answer
/// without it.
pub fn result_block(text: &str) -> (String, Option<Value>) {
    let tag = "```studio-result";
    let Some(start) = text.rfind(tag) else { return (text.to_string(), None) };
    let body = &text[start + tag.len()..];
    let end = body.find("```").unwrap_or(body.len());
    let rest = format!("{}{}", &text[..start], body.get(end + 3..).unwrap_or("")).trim().to_string();
    (rest, serde_json::from_str(body[..end].trim()).ok())
}

/// Files in the run's first folder (and one level below) changed since the run started — what it made.
pub fn new_files(dir: &Path, since: u64) -> Vec<String> {
    fn walk(d: &Path, depth: u32, since: u64, out: &mut Vec<String>) {
        for e in std::fs::read_dir(d).into_iter().flatten().flatten() {
            let p = e.path();
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') || ["node_modules", "target", "dist", "build"].contains(&name.as_str()) || out.len() >= 30 { continue; }
            let Ok(m) = e.metadata() else { continue };
            if m.is_dir() {
                if depth > 0 { walk(&p, depth - 1, since, out); }
            } else if m.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).is_some_and(|t| t.as_millis() as u64 >= since) {
                out.push(p.to_string_lossy().into_owned());
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, 1, since, &mut out);
    out.sort();
    out
}

/// Writes how a task of another app goes (at the start of each of its runs and at the end).
pub fn report(app: &AppHandle, r: &Run) {
    let Some(o) = r.spec.origin.as_ref() else { return };
    let Some(d) = agentlink::dir(app) else { return };
    if !agentlink::valid_id(&o.task) { return; }
    let (summary, data) = result_block(&r.summary);
    let mut files: Vec<String> = Vec::new();
    if r.status != "running" {
        for dir in r.spec.dirs.iter().take(1).map(PathBuf::from) {
            for f in new_files(&dir, r.started) { if !files.contains(&f) { files.push(f); } }
        }
    }
    let status = match r.status.as_str() { "done" => "done", "running" => "running", "needsYou" => "needsYou", "stopped" => "stopped", _ => "failed" };
    let mut v = json!({"id": o.task, "app": o.app, "title": o.title, "status": status, "summary": if r.error.is_empty() { summary } else { r.error.clone() },
        "files": files, "runId": r.id, "created": o.created, "updated": now()});
    if let Some(n) = r.needs.as_ref() { v["question"] = json!(n.text); }
    if let Some(x) = data { v["data"] = x; }
    write(&d.join("outbox").join(format!("{}.json", o.task)), &v);
}

/// status.json for the other apps.
pub fn status_of(d: &Data) -> Value {
    let short = |s: &str, n: usize| -> String { let one = s.split_whitespace().collect::<Vec<_>>().join(" "); if one.chars().count() > n { one.chars().take(n).collect::<String>() + "…" } else { one } };
    let app_of = |r: &Run| r.spec.origin.as_ref().map(|o| o.app.clone()).unwrap_or_default();
    let needs = d.inbox.iter().filter(|i| i.kind == "question" || i.kind == "approval").count();
    let problems = d.inbox.iter().filter(|i| i.kind == "problem").count();
    let items: Vec<Value> = d.inbox.iter().take(8).map(|i| json!({"kind": i.kind, "title": i.title, "text": short(&i.text, 200), "created": i.created})).collect();
    let runs: Vec<Value> = d.runs.iter().filter(|r| r.status == "running")
        .map(|r| json!({"title": r.spec.title, "last": short(&r.last, 120), "started": r.started, "app": app_of(r)})).collect();
    let day = now().saturating_sub(36 * 3600 * 1000);
    // What was done for the person: to-dos, routines, tasks and the apps' tasks — not their own chats; a conversation once.
    let mut titles: Vec<&str> = Vec::new();
    let done: Vec<Value> = d.runs.iter().filter(|r| r.status == "done" && r.ended >= day && (!r.spec.chat || r.spec.origin.is_some()))
        .filter(|r| { let new = !titles.contains(&r.spec.title.as_str()); titles.push(&r.spec.title); new }).take(20)
        .map(|r| json!({"title": r.spec.title, "summary": short(&result_block(&r.summary).0, 300), "ended": r.ended, "app": app_of(r)}))
        .collect();
    let minutes: u64 = d.runs.iter().filter(|r| r.started >= day).map(|r| (if r.ended > 0 { r.ended } else { now() }).saturating_sub(r.started)).sum::<u64>() / 60_000;
    json!({"v": 1, "beat": now(), "exe": std::env::current_exe().ok().map(|p| p.to_string_lossy().into_owned()),
        "needsYou": needs, "problems": problems, "items": items, "running": runs, "done": done, "minutes": minutes})
}

pub fn write_status(app: &AppHandle, d: &Data) {
    if let Some(dir) = agentlink::dir(app) { write(&dir.join("status.json"), &status_of(d)); }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::Spec;

    #[test]
    fn result_blocks_and_status() {
        let (rest, data) = result_block("Eingetragen.\n```studio-result\n{\"tasks\": [{\"title\": \"S. 34\"}]}\n```");
        assert_eq!(rest, "Eingetragen.");
        assert_eq!(data.unwrap()["tasks"][0]["title"], "S. 34");
        assert_eq!(result_block("nur Text").1, None);
        let origin = Origin { app: "Plan Studio".into(), task: "t1".into(), ..Default::default() };
        let d = Data {
            runs: vec![
                Run { id: "a".into(), status: "done".into(), started: now() - 60_000, ended: now(), summary: "Fertig".into(), spec: Spec { title: "Gegenlesen".into(), origin: Some(origin), ..Default::default() }, ..Default::default() },
                Run { id: "b".into(), status: "running".into(), started: now(), spec: Spec { title: "Läuft".into(), ..Default::default() }, ..Default::default() },
                Run { id: "c".into(), status: "done".into(), ended: now(), spec: Spec { title: "Eigener Chat".into(), chat: true, ..Default::default() }, ..Default::default() },
                Run { id: "d".into(), status: "done".into(), ended: now() - 5000, summary: "Älter".into(), spec: Spec { title: "Gegenlesen".into(), ..Default::default() }, ..Default::default() },
            ],
            inbox: vec![crate::runs::Item { kind: "question".into(), title: "Frage".into(), ..Default::default() }, crate::runs::Item { kind: "done".into(), ..Default::default() }],
        };
        let s = status_of(&d);
        assert_eq!((s["needsYou"].as_u64(), s["running"].as_array().unwrap().len(), s["done"].as_array().unwrap().len()), (Some(1), 1, 1));
        assert_eq!(s["done"][0]["app"], "Plan Studio", "own chats are not listed, a title once");
        assert!(s["beat"].as_u64().unwrap() > 0);
    }

    #[test]
    fn new_files_since_the_start() {
        let d = std::env::temp_dir().join(format!("agent-link-files-{}", std::process::id()));
        std::fs::create_dir_all(d.join("sub").join("deep")).unwrap();
        std::fs::create_dir_all(d.join(".git")).unwrap();
        let t0 = now() - 1000;
        for f in ["a.md", "sub/b.md", "sub/deep/c.md", ".git/x"] { std::fs::write(d.join(f), "x").unwrap(); }
        let f = new_files(&d, t0);
        assert_eq!(f.len(), 2, "{f:?}");
        assert!(new_files(&d, now() + 60_000).is_empty());
        let _ = std::fs::remove_dir_all(d);
    }
}
