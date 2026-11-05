//! The person's own Claude Code setup for a run with "Connectors, skills & plugins" on: the plugins they installed and
//! turned on (~/.claude/plugins/installed_plugins.json + enabledPlugins), their MCP servers (~/.claude.json, the
//! project's .mcp.json, the plugins' own) and the claude.ai connectors seen in earlier runs. Runs still start with
//! --setting-sources "" (only Agent Studio's rights count), so the plugins go in with --plugin-dir, the servers with
//! --mcp-config, and their tools are allowed by name (mcp__<server>__*).

use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Default, serde::Serialize)]
pub struct Plugin {
    /// The plugin's name (its commands and skills are "/name:command").
    pub name: String,
    /// "name@marketplace" as installed.
    pub id: String,
    pub path: String,
}

fn json_file(p: &Path) -> Value { std::fs::read_to_string(p).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(Value::Null) }

fn same_dir(a: &str, b: &str) -> bool { a.trim_end_matches(['/', '\\']) == b.trim_end_matches(['/', '\\']) }

/// Which plugins are turned on: the person's settings, then each folder's (a later "false" turns one off again).
fn enabled(home: &Path, dirs: &[String]) -> Vec<(String, bool)> {
    let mut files = vec![home.join(".claude/settings.json")];
    for d in dirs { files.extend([Path::new(d).join(".claude/settings.json"), Path::new(d).join(".claude/settings.local.json")]); }
    let mut out: Vec<(String, bool)> = Vec::new();
    for f in files {
        let Some(m) = json_file(&f).get("enabledPlugins").and_then(Value::as_object).cloned() else { continue };
        for (k, v) in m {
            let on = v.as_bool().unwrap_or(!v.is_null());
            out.retain(|(x, _)| *x != k);
            out.push((k, on));
        }
    }
    out
}

/// The installed plugins that are turned on, for a run in these folders.
pub fn plugins(home: &Path, dirs: &[String]) -> Vec<Plugin> {
    let on = enabled(home, dirs);
    let installed = json_file(&home.join(".claude/plugins/installed_plugins.json"));
    let mut out: Vec<Plugin> = Vec::new();
    for (id, v) in installed.get("plugins").and_then(Value::as_object).into_iter().flatten() {
        if !on.iter().any(|(k, b)| k == id && *b) { continue; }
        // Version 2: a list of installs (per scope); version 1: one object.
        let entries: Vec<&Value> = match v { Value::Array(a) => a.iter().collect(), o => vec![o] };
        let entry = entries.into_iter().find(|e| match e.get("projectPath").and_then(Value::as_str) {
            Some(pp) if e.get("scope").and_then(Value::as_str) != Some("user") => dirs.iter().any(|d| same_dir(d, pp)),
            _ => true,
        });
        let Some(path) = entry.and_then(|e| e.get("installPath")).and_then(Value::as_str).filter(|p| Path::new(p).is_dir()) else { continue };
        let manifest = json_file(&Path::new(path).join(".claude-plugin/plugin.json"));
        let name = manifest.get("name").and_then(Value::as_str).map(str::to_string)
            .unwrap_or_else(|| id.split('@').next().unwrap_or(id).to_string());
        if !out.iter().any(|p| p.name == name) { out.push(Plugin { name, id: id.clone(), path: path.to_string() }); }
    }
    out
}

/// The servers of an "mcpServers" object (or of a file that is that object itself).
fn server_map(v: &Value) -> Map<String, Value> {
    match v.get("mcpServers").and_then(Value::as_object) {
        Some(m) => m.clone(),
        None => v.as_object().filter(|m| m.values().all(|x| x.get("command").is_some() || x.get("url").is_some() || x.get("type").is_some())).cloned().unwrap_or_default(),
    }
}

/// The person's MCP servers for a run in these folders: user and local ones from ~/.claude.json and the folders'
/// .mcp.json (a folder's wins over the person's of the same name).
pub fn servers(home: &Path, dirs: &[String]) -> Map<String, Value> {
    let cfg = json_file(&home.join(".claude.json"));
    let mut out = server_map(&cfg);
    for d in dirs {
        if let Some(p) = cfg.get("projects").and_then(Value::as_object).and_then(|m| m.iter().find(|(k, _)| same_dir(k, d))).map(|(_, v)| v) {
            out.extend(server_map(p));
        }
        out.extend(server_map(&json_file(&Path::new(d).join(".mcp.json"))));
    }
    out
}

/// The names a plugin's servers have in tool names ("plugin_<plugin>_<server>").
fn plugin_servers(p: &Plugin) -> Vec<String> {
    let dir = Path::new(&p.path);
    let mut m = server_map(&json_file(&dir.join(".mcp.json")));
    if let Some(x) = json_file(&dir.join(".claude-plugin/plugin.json")).get("mcpServers").and_then(Value::as_object) { m.extend(x.clone()); }
    m.keys().map(|k| format!("plugin_{}_{k}", p.name)).collect()
}

/// How a server's name appears in its tools' names: "claude.ai Canva" → "claude_ai_Canva".
pub fn tool_name(server: &str) -> String {
    server.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '_' }).collect()
}

/// The rule that allows every tool of a server.
pub fn allow_rule(server: &str) -> String { format!("mcp__{}__*", tool_name(server)) }

/// What a run with connectors gets: the plugin folders, the MCP config (JSON, empty when there is none) and the
/// servers whose tools it may use (`learned`: names seen in earlier runs, e.g. the claude.ai connectors).
pub fn setup(home: &Path, dirs: &[String], learned: &[String]) -> (Vec<String>, String, Vec<String>) {
    let ps = plugins(home, dirs);
    let srv = servers(home, dirs);
    let mut names: Vec<String> = srv.keys().cloned().collect();
    for p in &ps { names.extend(plugin_servers(p)); }
    names.extend(learned.iter().cloned());
    let mut seen: Vec<String> = Vec::new();
    names.retain(|n| { let k = tool_name(n); if seen.contains(&k) { false } else { seen.push(k); true } });
    let config = if srv.is_empty() { String::new() } else { json!({"mcpServers": srv}).to_string() };
    (ps.into_iter().map(|p| p.path).collect(), config, names)
}

/// The file of the server names seen in runs (the claude.ai connectors can only be learned that way).
pub fn learned_file(data: &Path) -> PathBuf { data.join("mcp-servers.json") }

pub fn learned(data: &Path) -> Vec<String> {
    serde_json::from_value(json_file(&learned_file(data))).unwrap_or_default()
}

/// Adds server names seen in a run to the file; true when one was new.
pub fn learn(data: &Path, names: &[String]) -> bool {
    let mut l = learned(data);
    let before = l.len();
    for n in names { if !n.trim().is_empty() && !l.contains(n) { l.push(n.clone()); } }
    if l.len() == before { return false; }
    std::fs::write(learned_file(data), serde_json::to_string(&l).unwrap_or_default()).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_plugins_servers_and_rules() {
        let d = std::env::temp_dir().join(format!("agent-connect-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let (home, proj) = (d.join("home"), d.join("proj"));
        let w = |p: PathBuf, s: &str| { std::fs::create_dir_all(p.parent().unwrap()).unwrap(); std::fs::write(p, s).unwrap(); };
        let (pa, pb, pc) = (d.join("cache/design"), d.join("cache/off"), d.join("cache/other-proj"));
        w(pa.join(".claude-plugin/plugin.json"), r#"{"name":"design","mcpServers":{"figma":{"url":"https://x"}}}"#);
        w(pa.join("skills/critique/SKILL.md"), "---\ndescription: Critique a design\n---\nLook closely.");
        w(pb.join("commands/x.md"), "x");
        w(pc.join("commands/y.md"), "y");
        let proj_s = proj.to_string_lossy().into_owned();
        w(home.join(".claude/plugins/installed_plugins.json"), &json!({"version": 2, "plugins": {
            "design@official": [{"scope": "user", "installPath": pa}],
            "off@official": [{"scope": "user", "installPath": pb}],
            "elsewhere@official": [{"scope": "project", "projectPath": "/somewhere/else", "installPath": pc}],
            "gone@official": [{"scope": "user", "installPath": d.join("cache/missing")}]}}).to_string());
        w(home.join(".claude/settings.json"), r#"{"enabledPlugins":{"design@official":true,"off@official":true,"elsewhere@official":true,"gone@official":true}}"#);
        w(proj.join(".claude/settings.json"), r#"{"enabledPlugins":{"off@official":false}}"#);
        w(home.join(".claude.json"), &json!({"mcpServers": {"github": {"command": "gh-mcp"}},
            "projects": {proj_s.clone() + "/": {"mcpServers": {"local-db": {"command": "db"}}}}}).to_string());
        w(proj.join(".mcp.json"), r#"{"mcpServers":{"github":{"command":"gh-mcp","args":["--project"]},"docs":{"url":"https://d"}}}"#);

        let ps = plugins(&home, std::slice::from_ref(&proj_s));
        assert_eq!(ps.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), vec!["design"], "off in the project, other project, missing folder");
        let (dirs, config, names) = setup(&home, &[proj_s.clone()], &["claude.ai Canva".into(), "github".into()]);
        assert_eq!(dirs, vec![pa.to_string_lossy().into_owned()]);
        let c: Value = serde_json::from_str(&config).unwrap();
        assert_eq!(c.pointer("/mcpServers/github/args/0").and_then(Value::as_str), Some("--project"), "the project's server wins");
        assert!(c.pointer("/mcpServers/local-db").is_some() && c.pointer("/mcpServers/docs").is_some());
        let mut n = names.clone();
        n.sort();
        assert_eq!(n, vec!["claude.ai Canva", "docs", "github", "local-db", "plugin_design_figma"]);
        assert_eq!(allow_rule("claude.ai Canva"), "mcp__claude_ai_Canva__*");
        assert!(setup(&home, &[], &[]).1.contains("github"));

        let data = d.join("data");
        std::fs::create_dir_all(&data).unwrap();
        assert!(learn(&data, &["claude.ai Gmail".into()]) && !learn(&data, &["claude.ai Gmail".into()]));
        assert_eq!(learned(&data), vec!["claude.ai Gmail"]);
        let _ = std::fs::remove_dir_all(d);
    }
}
