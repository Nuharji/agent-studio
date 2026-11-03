//! What Agent Studio offers over MCP (mcp.rs): the Markdown to-do lists, its status and read-only tasks. Ticking stays
//! with the app (only for lists the person allowed), and a task from a client never gets more than read rights or
//! internet, and goes through the same switches as tasks from the other apps (Settings → Control).

use crate::{agentlink, mcp, todo};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

pub fn run() { mcp::serve("agent-studio", env!("CARGO_PKG_VERSION"), &tools(), &call) }

fn tools() -> Vec<Value> {
    let path = json!({"type": "string", "description": "Full path of the Markdown file"});
    vec![
        mcp::tool("todo_read", "Reads a Markdown to-do list: its items (line, text, done, section, depth) and sections.", json!({"path": path}), &["path"]),
        mcp::tool("todo_add", "Adds a new item at the end of a section of a Markdown to-do list.", json!({"path": path,
            "section": {"type": "string", "description": "Heading to add it under; empty for the end of the file"},
            "text": {"type": "string"}}), &["path", "text"]),
        mcp::tool("agent_status", "Whether Agent Studio is open, and what needs the person, runs and was done today.", json!({}), &[]),
        mcp::tool("send_task", "Sends a task to Agent Studio. It runs with read rights and without internet as soon as Agent Studio is open and takes tasks. Returns the task's id.", json!({
            "task": {"type": "string"}, "title": {"type": "string"},
            "files": {"type": "array", "items": {"type": "string"}, "description": "Full paths of files it is about"},
            "dir": {"type": "string", "description": "Full path of a folder it may read"}}), &["task"]),
        mcp::tool("task_result", "How a task from send_task goes and what came out of it.", json!({"id": {"type": "string"}}), &["id"]),
    ]
}

fn agent_dir() -> Result<PathBuf, String> { mcp::studio_dir().map(|d| d.join("agent")).ok_or_else(|| "no config folder".to_string()) }

fn read_json(p: &Path) -> Option<Value> { std::fs::read_to_string(p).ok().and_then(|s| serde_json::from_str(&s).ok()) }

fn now() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0) }

fn status(dir: &Path) -> Value {
    let st = read_json(&dir.join("status.json"));
    let alive = st.as_ref().and_then(|s| s.get("beat")).and_then(Value::as_u64).is_some_and(|b| now().saturating_sub(b) < 75_000);
    json!({"alive": alive, "status": st})
}

fn send_task(dir: &Path, args: &Value) -> Result<Value, String> {
    let id = format!("mcp-{:x}-{:x}", now(), std::process::id());
    let task = agentlink::clean_task(&json!({"app": "MCP", "title": mcp::opt_text(args, "title"), "task": mcp::text(args, "task")?,
        "files": args.get("files").cloned().unwrap_or_else(|| json!([])), "dir": mcp::opt_text(args, "dir"), "level": "read", "web": false}), &id)?;
    let inbox = dir.join("inbox");
    std::fs::create_dir_all(&inbox).map_err(|e| e.to_string())?;
    // Written under another name first: Agent Studio must never take half a task.
    let tmp = inbox.join(format!("{id}.tmp"));
    mcp::put(&tmp, task.to_string().as_bytes(), false)?;
    std::fs::rename(&tmp, inbox.join(format!("{id}.json"))).map_err(|e| e.to_string())?;
    Ok(json!({"id": id, "agentStudioOpen": status(dir)["alive"]}))
}

fn task_result(dir: &Path, id: &str) -> Result<Value, String> {
    if !agentlink::valid_id(id) { return Err("bad id".into()); }
    let file = format!("{id}.json");
    if let Some(r) = read_json(&dir.join("outbox").join(&file)) { return Ok(r); }
    if dir.join("inbox").join(&file).is_file() { return Ok(json!({"id": id, "status": "waiting"})); }
    Err(format!("no task {id}"))
}

fn call(name: &str, args: &Value) -> Result<Value, String> {
    match name {
        "todo_read" => {
            let s = std::fs::read_to_string(mcp::path(args, "path")?).map_err(|e| e.to_string())?;
            Ok(json!({"items": todo::parse(&s), "sections": todo::sections(&s)}))
        }
        "todo_add" => {
            let p = mcp::path(args, "path")?;
            let s = std::fs::read_to_string(&p).map_err(|e| e.to_string())?;
            let (out, line) = todo::add(&s, &mcp::opt_text(args, "section"), &mcp::text(args, "text")?).ok_or("empty")?;
            mcp::put(&p, out.as_bytes(), true)?;
            Ok(json!({"line": line}))
        }
        "agent_status" => Ok(status(&agent_dir()?)),
        "send_task" => send_task(&agent_dir()?, args),
        "task_result" => task_result(&agent_dir()?, &mcp::text(args, "id")?),
        _ => Err(format!("unknown tool: {name}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("agent-mcp-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn to_do_lists() {
        let d = dir("todo");
        let f = d.join("TODO.md");
        std::fs::write(&f, "## Wichtig\n- [ ] Eins\n\n## Später\n- [x] Zwei\n").unwrap();
        let p = f.to_string_lossy();
        let r = call("todo_read", &json!({"path": p})).unwrap();
        assert_eq!(r["items"][0]["text"], "Eins");
        assert_eq!(r["items"][1]["done"], true);
        call("todo_add", &json!({"path": p, "section": "Wichtig", "text": "Drei"})).unwrap();
        let r = call("todo_read", &json!({"path": p})).unwrap();
        assert_eq!(r["items"][1]["text"], "Drei");
        assert_eq!(r["items"][1]["section"], "Wichtig");
        assert!(call("todo_tick", &json!({"path": p})).is_err(), "ticking stays with the app");
        assert!(call("todo_read", &json!({"path": "TODO.md"})).is_err(), "only full paths");
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn tasks_only_read() {
        let d = dir("task");
        // Whatever a client asks for: read rights, no internet.
        let sent = send_task(&d, &json!({"task": "Fasse zusammen", "level": "edit", "web": true, "files": ["/no/such/file"]})).unwrap();
        let id = sent["id"].as_str().unwrap();
        assert_eq!(sent["agentStudioOpen"], false);
        let t = read_json(&d.join("inbox").join(format!("{id}.json"))).unwrap();
        assert_eq!((t["level"].as_str(), t["web"].as_bool(), t["app"].as_str()), (Some("read"), Some(false), Some("MCP")));
        assert_eq!(t["files"], json!([]));
        assert!(agentlink::clean_task(&t, id).is_ok(), "Agent Studio takes it");
        assert_eq!(task_result(&d, id).unwrap()["status"], "waiting");
        std::fs::create_dir_all(d.join("outbox")).unwrap();
        std::fs::write(d.join("outbox").join(format!("{id}.json")), r#"{"status":"done","summary":"Fertig"}"#).unwrap();
        assert_eq!(task_result(&d, id).unwrap()["summary"], "Fertig");
        assert!(task_result(&d, "../x").is_err());
        assert!(send_task(&d, &json!({"task": " "})).is_err());
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn heartbeat() {
        let d = dir("status");
        assert_eq!(status(&d)["alive"], false);
        std::fs::write(d.join("status.json"), json!({"beat": now(), "needs": 2}).to_string()).unwrap();
        let s = status(&d);
        assert_eq!((s["alive"].as_bool(), s["status"]["needs"].as_u64()), (Some(true), Some(2)));
        let _ = std::fs::remove_dir_all(d);
    }
}
