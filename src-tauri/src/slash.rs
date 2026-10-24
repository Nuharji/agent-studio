//! The person's own slash commands and skills, as in the Claude desktop app: Markdown files in ~/.claude/commands,
//! ~/.claude/skills/<name>/SKILL.md and the same in a run's folders (<dir>/.claude/…). Runs start with
//! --setting-sources "" (Claude Code loads none of them itself), so Agent Studio expands a command to its text.

use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Serialize, Clone, Debug, PartialEq, Default)]
pub struct Cmd {
    /// What comes after the slash ("review", "frontend:lint" for commands/frontend/lint.md).
    pub name: String,
    pub description: String,
    /// The hint for the arguments ("argument-hint" in the front matter).
    pub hint: String,
    /// "command" or "skill".
    pub kind: String,
    /// "user" (~/.claude) or "project" (a run's folder).
    pub source: String,
    pub path: String,
}

/// The front matter (`---` … `---`) as key/value pairs, and the text after it.
fn front(text: &str) -> (Vec<(String, String)>, &str) {
    let t = text.strip_prefix('\u{feff}').unwrap_or(text);
    let Some(rest) = t.strip_prefix("---").filter(|r| r.starts_with('\n') || r.starts_with("\r\n")) else { return (vec![], t) };
    let Some(end) = rest.find("\n---") else { return (vec![], t) };
    let body = rest[end + 4..].trim_start_matches(['-']).trim_start_matches(['\r', '\n']);
    let kv = rest[..end].lines().filter_map(|l| {
        let (k, v) = l.split_once(':')?;
        if k.starts_with(' ') || k.starts_with('\t') { return None; }
        Some((k.trim().to_string(), v.trim().trim_matches(['"', '\'']).to_string()))
    }).collect();
    (kv, body)
}

fn get(kv: &[(String, String)], k: &str) -> String { kv.iter().find(|(x, _)| x == k).map(|(_, v)| v.clone()).unwrap_or_default() }

/// The first line of real text (when there is no description).
fn first_line(body: &str) -> String {
    let l = body.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    let l = l.trim_start_matches('#').trim();
    if l.chars().count() > 100 { l.chars().take(100).collect::<String>() + "…" } else { l.to_string() }
}

fn read(path: &Path, name: String, kind: &str, source: &str) -> Option<Cmd> {
    let text = std::fs::read_to_string(path).ok()?;
    let (kv, body) = front(&text);
    let name = if kind == "skill" { Some(get(&kv, "name")).filter(|n| !n.is_empty()).unwrap_or(name) } else { name };
    let d = get(&kv, "description");
    Some(Cmd { name, description: if d.is_empty() { first_line(body) } else { d }, hint: get(&kv, "argument-hint"), kind: kind.into(), source: source.into(), path: path.to_string_lossy().into_owned() })
}

/// commands/*.md, subfolders as "folder:name".
fn commands(dir: &Path, prefix: &str, source: &str, out: &mut Vec<Cmd>, depth: usize) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut es: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
    es.sort();
    for p in es {
        let stem = p.file_stem().unwrap_or_default().to_string_lossy().into_owned();
        if stem.starts_with('.') { continue; }
        if p.is_dir() && depth < 3 { commands(&p, &format!("{prefix}{stem}:"), source, out, depth + 1); }
        else if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("md")) {
            if let Some(c) = read(&p, format!("{prefix}{stem}"), "command", source) { out.push(c); }
        }
    }
}

fn skills(dir: &Path, source: &str, out: &mut Vec<Cmd>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut es: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    es.sort();
    for p in es {
        let f = p.join("SKILL.md");
        let name = p.file_name().unwrap_or_default().to_string_lossy().into_owned();
        if f.is_file() && !name.starts_with('.') { if let Some(c) = read(&f, name, "skill", source) { out.push(c); } }
    }
}

/// All commands and skills for a run in these folders (`home` = ~). A project's one wins over the person's own
/// of the same name, as in Claude Code.
pub fn list(home: &Path, dirs: &[String]) -> Vec<Cmd> {
    let mut out: Vec<Cmd> = Vec::new();
    for d in dirs {
        let c = Path::new(d).join(".claude");
        if c == home.join(".claude") { continue; }
        commands(&c.join("commands"), "", "project", &mut out, 0);
        skills(&c.join("skills"), "project", &mut out);
    }
    let c = home.join(".claude");
    commands(&c.join("commands"), "", "user", &mut out, 0);
    skills(&c.join("skills"), "user", &mut out);
    let mut seen: Vec<String> = Vec::new();
    out.retain(|c| { let k = c.name.to_lowercase(); if seen.contains(&k) { false } else { seen.push(k); true } });
    out
}

/// The words of the arguments; "…" keeps spaces together.
fn words(args: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let (mut q, mut any) = (false, false);
    for ch in args.chars() {
        match ch {
            '"' => { q = !q; any = true; }
            c if c.is_whitespace() && !q => { if any { out.push(std::mem::take(&mut cur)); any = false; } }
            c => { cur.push(c); any = true; }
        }
    }
    if any { out.push(cur); }
    out
}

/// What is sent to Claude for "/name args": a command's text with $ARGUMENTS / $1 … $9 filled in (the arguments
/// below it when it has no place for them), a skill as instructions with its folder (for the files it mentions).
pub fn expand(path: &str, kind: &str, name: &str, args: &str, de: bool) -> Result<String, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let (_, body) = front(&text);
    let args = args.trim();
    if kind == "skill" {
        let dir = Path::new(path).parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
        let task = if args.is_empty() { String::new() } else if de { format!("\n\nAufgabe: {args}") } else { format!("\n\nTask: {args}") };
        return Ok(if de {
            format!("Nutze dafür den Skill „{name}“ (Ordner: {dir}). Seine Anleitung:\n\n<skill name=\"{name}\">\n{}\n</skill>{task}", body.trim())
        } else {
            format!("Use the skill \"{name}\" for this (folder: {dir}). Its instructions:\n\n<skill name=\"{name}\">\n{}\n</skill>{task}", body.trim())
        });
    }
    let w = words(args);
    let mut out = body.trim().replace("$ARGUMENTS", args);
    let mut placed = out != body.trim();
    for i in (1..=9).rev() {
        let k = format!("${i}");
        if out.contains(&k) { out = out.replace(&k, w.get(i - 1).map(String::as_str).unwrap_or("")); placed = true; }
    }
    if !placed && !args.is_empty() { out += &format!("\n\nARGUMENTS: {args}"); }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_commands_and_skills() {
        let d = std::env::temp_dir().join(format!("agent-slash-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let (home, proj) = (d.join("home"), d.join("proj"));
        let w = |p: PathBuf, s: &str| { std::fs::create_dir_all(p.parent().unwrap()).unwrap(); std::fs::write(p, s).unwrap(); };
        w(home.join(".claude/commands/review.md"), "---\ndescription: Look over the diff\nargument-hint: [file]\n---\nReview $ARGUMENTS carefully.");
        w(home.join(".claude/commands/git/sync.md"), "# Pull and push\nPull, then push.");
        w(home.join(".claude/commands/notes.txt"), "not a command");
        w(home.join(".claude/skills/pdf/SKILL.md"), "---\nname: pdf-tools\ndescription: \"Work with PDFs\"\n---\nUse pdftotext.");
        w(home.join(".claude/skills/empty/README.md"), "no skill file");
        w(proj.join(".claude/commands/review.md"), "Project review of $1 and $2.");
        let l = list(&home, &[proj.to_string_lossy().into_owned()]);
        let names: Vec<&str> = l.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["review", "git:sync", "pdf-tools"]);
        assert_eq!((l[0].source.as_str(), l[0].description.as_str()), ("project", "Project review of $1 and $2."), "the project's command wins");
        assert_eq!((l[1].description.as_str(), l[1].kind.as_str()), ("Pull and push", "command"));
        assert_eq!((l[2].description.as_str(), l[2].kind.as_str()), ("Work with PDFs", "skill"));
        let user = list(&home, &[]);
        assert_eq!(user.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), vec!["git:sync", "review", "pdf-tools"]);
        let user = &user[1..];
        assert_eq!((user[0].description.as_str(), user[0].hint.as_str()), ("Look over the diff", "[file]"));

        assert_eq!(expand(&user[0].path, "command", "review", "src/a.rs", false).unwrap(), "Review src/a.rs carefully.");
        assert_eq!(expand(&l[0].path, "command", "review", "\"a b\" c", false).unwrap(), "Project review of a b and c.");
        assert_eq!(expand(&l[1].path, "command", "git:sync", "main", false).unwrap(), "# Pull and push\nPull, then push.\n\nARGUMENTS: main");
        let s = expand(&l[2].path, "skill", "pdf-tools", "read x.pdf", true).unwrap();
        assert!(s.contains("„pdf-tools“") && s.contains("Use pdftotext.") && s.ends_with("Aufgabe: read x.pdf") && !s.contains("description"));
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn front_matter() {
        let (kv, body) = front("---\r\ndescription: x\r\nallowed-tools:\r\n  - Bash\r\n---\r\nBody");
        assert_eq!(get(&kv, "description"), "x");
        assert_eq!(body, "Body");
        assert_eq!(front("No front matter").1, "No front matter");
        assert_eq!(words("a \"b c\" d"), vec!["a", "b c", "d"]);
    }
}
