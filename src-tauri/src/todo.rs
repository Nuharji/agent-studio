//! Linked to-do lists: any Markdown file with checkboxes (`- [ ] …`, `* [x] …`, `1. [ ] …`). The file stays the
//! person's own — it is only read, and a box is ticked only when they allowed it for that file.

use serde::Serialize;

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Item {
    /// Line number (0-based) in the file.
    pub line: usize,
    pub text: String,
    pub done: bool,
    /// The heading above it ("Hohe Priorität", …), empty before the first heading.
    pub section: String,
    /// Indentation level (sub-items).
    pub depth: usize,
}

/// The checkbox of a line: (prefix length up to and including "] ", done).
fn checkbox(line: &str) -> Option<(usize, bool)> {
    let start = line.len() - line.trim_start().len();
    let rest = &line[start..];
    let after_marker = if rest.starts_with("- ") || rest.starts_with("* ") || rest.starts_with("+ ") {
        2
    } else {
        let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
        if digits == 0 || !(rest[digits..].starts_with(". ") || rest[digits..].starts_with(") ")) { return None; }
        digits + 2
    };
    let b = rest[after_marker..].as_bytes();
    if b.len() < 3 || b[0] != b'[' || b[2] != b']' || !(b.len() == 3 || b[3] == b' ') { return None; }
    let done = match b[1] { b' ' => false, b'x' | b'X' => true, _ => return None };
    Some((start + after_marker + 3 + usize::from(b.len() > 3), done))
}

pub fn parse(md: &str) -> Vec<Item> {
    let (mut out, mut section, mut fence) = (Vec::new(), String::new(), false);
    for (i, line) in md.lines().enumerate() {
        let tr = line.trim_start();
        if tr.starts_with("```") || tr.starts_with("~~~") { fence = !fence; continue; }
        if fence { continue; }
        if tr.starts_with('#') {
            section = tr.trim_start_matches('#').trim().to_string();
            continue;
        }
        if let Some((n, done)) = checkbox(line) {
            let text = line[n..].trim().to_string();
            if text.is_empty() { continue; }
            let indent = line.len() - tr.len();
            out.push(Item { line: i, text, done, section: section.clone(), depth: indent / 2 });
        }
    }
    out
}

/// The file with the box of `line` ticked — only when that line still holds `text` (the file may have changed).
pub fn tick(md: &str, line: usize, text: &str) -> Option<String> {
    let mut lines: Vec<String> = md.split('\n').map(str::to_string).collect();
    let l = lines.get(line)?;
    let (n, done) = checkbox(l.trim_end_matches('\r'))?;
    if done || l[n..].trim() != text { return None; }
    let open = l[..n].rfind("[ ]")?;
    lines[line].replace_range(open..open + 3, "[x]");
    Some(lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const MD: &str = "# To-do\n\n#### Hohe Priorität\n- [ ] Viewer-Icon fixen\n  - [x] Ursache finden\n\n#### Niedrige Priorität\n* [X] Erledigt\n1. [ ] Nummeriert\n- [] kein Kästchen\n- [ ]\n```\n- [ ] im Code\n```\n- Liste ohne Kästchen\n";

    #[test]
    fn parses_boxes_with_sections() {
        let items = parse(MD);
        let short: Vec<_> = items.iter().map(|i| (i.text.as_str(), i.done, i.section.as_str(), i.depth)).collect();
        assert_eq!(short, vec![
            ("Viewer-Icon fixen", false, "Hohe Priorität", 0),
            ("Ursache finden", true, "Hohe Priorität", 1),
            ("Erledigt", true, "Niedrige Priorität", 0),
            ("Nummeriert", false, "Niedrige Priorität", 0),
        ]);
        assert_eq!(items[0].line, 3);
    }

    #[test]
    fn ticks_only_the_same_item() {
        let out = tick(MD, 3, "Viewer-Icon fixen").unwrap();
        assert!(out.contains("- [x] Viewer-Icon fixen") && out.contains("1. [ ] Nummeriert"));
        assert_eq!(out.len(), MD.len());
        assert!(tick(MD, 3, "Etwas anderes").is_none(), "the line changed meanwhile");
        assert!(tick(MD, 4, "Ursache finden").is_none(), "already done");
        assert!(tick("a\r\n- [ ] b\r\n", 1, "b").unwrap().contains("- [x] b\r"));
    }
}
