//! Linked to-do lists: any Markdown file with checkboxes (`- [ ] …`, `* [x] …`, `1. [ ] …`). The file stays the
//! person's own — it is only read; a box is ticked only when they allowed it for that file, and a new item is only
//! written when they add one in the app.

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

/// The headings of the file (where a new item can go), in order, without repeats.
pub fn sections(md: &str) -> Vec<String> {
    let (mut out, mut fence) = (Vec::<String>::new(), false);
    for line in md.lines() {
        let tr = line.trim_start();
        if tr.starts_with("```") || tr.starts_with("~~~") { fence = !fence; continue; }
        if fence || !tr.starts_with('#') { continue; }
        let h = tr.trim_start_matches('#').trim().to_string();
        if !h.is_empty() && !out.contains(&h) { out.push(h); }
    }
    out
}

/// The file with a new open item `text` at the end of `section` (its first heading of that name; "" = before the first
/// heading), in the list style of that section. A missing section gets the item at the end of the file.
/// Returns the new file and the item's line.
pub fn add(md: &str, section: &str, text: &str) -> Option<(String, usize)> {
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.is_empty() { return None; }
    let nl = if md.contains("\r\n") { "\r\n" } else { "\n" };
    let lines: Vec<&str> = md.lines().collect();
    let (mut cur, mut fence) = (String::new(), false);
    let (mut head, mut last, mut marker) = (None::<usize>, None::<usize>, None::<String>);
    for (i, line) in lines.iter().enumerate() {
        let tr = line.trim_start();
        if tr.starts_with("```") || tr.starts_with("~~~") { fence = !fence; continue; }
        if fence { continue; }
        if tr.starts_with('#') {
            if cur == section { break; } // the end of the section
            cur = tr.trim_start_matches('#').trim().to_string();
            if cur == section { head = Some(i); }
            continue;
        }
        if cur != section { continue; }
        if checkbox(line).is_some() {
            last = Some(i);
            if marker.is_none() && tr.len() == line.len() {
                // "-", "*" or "+" like the items above; numbered lists get "-" (no renumbering).
                let m = &line[..1];
                marker = Some(if m == "*" || m == "+" { m.to_string() } else { "-".into() });
            }
        } else if last.is_some() && !line.trim().is_empty() && line.len() != tr.len() {
            last = Some(i); // indented text that belongs to the item above
        }
    }
    let item = format!("{} [ ] {text}", marker.as_deref().unwrap_or("-"));
    let at = match (last, head) {
        (Some(l), _) => l + 1,
        (None, Some(h)) => h + 1,
        (None, None) if section.is_empty() && lines.iter().any(|l| l.trim_start().starts_with('#')) => 0,
        _ => {
            let mut out = md.to_string();
            if !out.is_empty() && !out.ends_with('\n') { out.push_str(nl); }
            out.push_str(&item);
            out.push_str(nl);
            return Some((out, lines.len()));
        }
    };
    let mut out: Vec<&str> = lines.clone();
    out.insert(at, &item);
    let mut s = out.join(nl);
    if md.ends_with('\n') || at == lines.len() { s.push_str(nl); }
    Some((s, at))
}

/// The file with the box of `line` ticked — only when that line still holds `text`; when the file changed meanwhile,
/// the one open item with exactly that text (if there is just one).
pub fn tick(md: &str, line: usize, text: &str) -> Option<String> {
    let mut lines: Vec<String> = md.split('\n').map(str::to_string).collect();
    let holds = |l: &str| checkbox(l.trim_end_matches('\r')).is_some_and(|(n, done)| !done && l[n..].trim() == text);
    let line = if lines.get(line).is_some_and(|l| holds(l)) {
        line
    } else {
        let same: Vec<_> = parse(md).into_iter().filter(|i| !i.done && i.text == text).collect();
        if same.len() != 1 { return None; }
        same[0].line
    };
    let l = &lines[line];
    let (n, _) = checkbox(l.trim_end_matches('\r'))?;
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
        let moved = tick(MD, 1, "Nummeriert").unwrap();
        assert!(moved.contains("1. [x] Nummeriert"), "found again after lines moved");
        assert!(tick("- [ ] a\n- [ ] a\n- [ ] b\n", 2, "a").is_none(), "two items with that text");
    }

    #[test]
    fn lists_sections() {
        assert_eq!(sections(MD), vec!["To-do", "Hohe Priorität", "Niedrige Priorität"]);
    }

    #[test]
    fn adds_at_the_end_of_a_section() {
        let (out, line) = add(MD, "Hohe Priorität", "  Neues\n Ding ").unwrap();
        assert_eq!(line, 5);
        assert!(out.contains("  - [x] Ursache finden\n- [ ] Neues Ding\n\n#### Niedrige"));
        assert_eq!(parse(&out).iter().find(|i| i.line == line).unwrap().section, "Hohe Priorität");

        let (out, line) = add(MD, "Niedrige Priorität", "Noch was").unwrap();
        assert!(out.contains("1. [ ] Nummeriert\n- [] kein Kästchen\n- [ ]\n* [ ] Noch was\n```"), "{out}");
        assert_eq!(out.lines().nth(line), Some("* [ ] Noch was"));

        let md = "# Liste\n\n## Leer\n\n## Voll\n* [ ] a\n  Notiz zu a\n";
        assert_eq!(add(md, "Leer", "x").unwrap().0, "# Liste\n\n## Leer\n- [ ] x\n\n## Voll\n* [ ] a\n  Notiz zu a\n");
        assert_eq!(add(md, "Voll", "y").unwrap().0, "# Liste\n\n## Leer\n\n## Voll\n* [ ] a\n  Notiz zu a\n* [ ] y\n");
        assert_eq!(add(md, "Weg", "z").unwrap(), ("# Liste\n\n## Leer\n\n## Voll\n* [ ] a\n  Notiz zu a\n- [ ] z\n".into(), 7));
        assert_eq!(add("- [ ] a", "", "b").unwrap(), ("- [ ] a\n- [ ] b\n".into(), 1));
        assert_eq!(add("", "", "b").unwrap(), ("- [ ] b\n".into(), 0));
        assert_eq!(add("a\r\n## S\r\n- [ ] b\r\n", "S", "c").unwrap().0, "a\r\n## S\r\n- [ ] b\r\n- [ ] c\r\n");
        assert!(add(MD, "Hohe Priorität", "  ").is_none());
    }
}
