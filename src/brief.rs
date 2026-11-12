//! A finished turn packed into a handoff brief, and the `relay` palette rows. Free of host calls.

/// What Relay knows about one tab's last turn.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Turn {
    pub name: String,
    pub prompt: Option<String>,
    /// Subject and status (`completed`, `in_progress`, `pending`).
    pub tasks: Vec<(String, String)>,
    pub files: Vec<String>,
    pub plan: Option<String>,
    pub cwd: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Part {
    Brief,
    Plan,
    Files,
}

/// Agent text is untrusted: no control or format characters (tabs and newlines kept where asked).
pub fn clean(s: &str, keep_newlines: bool) -> String {
    s.chars()
        .filter_map(|c| match c {
            '\n' if keep_newlines => Some('\n'),
            '\t' | '\n' | '\r' => Some(' '),
            c if c.is_control() => None,
            '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2060}'..='\u{206f}' | '\u{feff}' | '\u{e0000}'..='\u{e0fff}' => None,
            c => Some(c),
        })
        .collect()
}

fn line(s: &str) -> String {
    clean(s, false).split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Paths relative to the turn's folder where they are inside it.
fn rel<'a>(cwd: Option<&str>, p: &'a str) -> &'a str {
    cwd.map(|c| c.trim_end_matches('/'))
        .and_then(|c| p.strip_prefix(c)?.strip_prefix('/'))
        .filter(|r| !r.is_empty())
        .unwrap_or(p)
}

impl Turn {
    pub fn plan_lines(&self) -> usize {
        self.plan.as_deref().map_or(0, |p| p.lines().count())
    }

    pub fn text(&self, part: Part) -> String {
        let files = || self.files.iter().map(|f| format!("- {}", line(rel(self.cwd.as_deref(), f)))).collect::<Vec<_>>().join("\n");
        match part {
            Part::Plan => clean(self.plan.as_deref().unwrap_or_default(), true).trim().to_owned(),
            Part::Files => files(),
            Part::Brief => {
                let mut out = format!("Handoff from {}", line(&self.name));
                if let Some(c) = &self.cwd {
                    out.push_str(&format!(" ({})", line(c)));
                }
                out.push_str("\n\n");
                if let Some(p) = self.prompt.as_deref().map(line).filter(|p| !p.is_empty()) {
                    out.push_str(&format!("## Goal\n{p}\n\n"));
                }
                if !self.tasks.is_empty() {
                    out.push_str("## Tasks\n");
                    for (t, s) in &self.tasks {
                        let mark = if s == "completed" { "x" } else { " " };
                        out.push_str(&format!("- [{mark}] {}\n", line(t)));
                    }
                    out.push('\n');
                }
                if !self.files.is_empty() {
                    out.push_str(&format!("## Files touched\n{}\n\n", files()));
                }
                if self.plan.is_some() {
                    out.push_str(&format!("## Last plan\n{}\n", self.text(Part::Plan)));
                }
                out.trim_end().to_owned()
            }
        }
    }
}

fn count(n: usize, what: &str) -> String {
    format!("{n} {what}{}", if n == 1 { "" } else { "s" })
}

/// A palette row: id, title and the count on the right.
pub fn rows(t: &Turn) -> Vec<(Part, String, String)> {
    let n = line(&t.name);
    let mut out = vec![(Part::Brief, format!("{n}: last turn → brief"), format!("{} · {}", count(t.files.len(), "file"), count(t.tasks.len(), "task")))];
    if t.plan.is_some() {
        out.push((Part::Plan, format!("{n}: plan only"), count(t.plan_lines(), "line")));
    }
    if !t.files.is_empty() {
        out.push((Part::Files, format!("{n}: files touched only"), count(t.files.len(), "path")));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn turn() -> Turn {
        Turn {
            name: "api".into(),
            prompt: Some("Add session\nrefresh".into()),
            tasks: vec![("Write tests".into(), "completed".into()), ("Ship".into(), "pending".into())],
            files: vec!["/w/api/src/session.rs".into(), "/etc/hosts".into()],
            plan: Some("1. a\n2. b\n3. c".into()),
            cwd: Some("/w/api".into()),
        }
    }

    #[test]
    fn rows_read_like_the_mockup() {
        let r = rows(&turn());
        assert_eq!(r[0].1, "api: last turn → brief");
        assert_eq!(r[0].2, "2 files · 2 tasks");
        assert_eq!((r[1].1.as_str(), r[1].2.as_str()), ("api: plan only", "3 lines"));
        assert_eq!((r[2].1.as_str(), r[2].2.as_str()), ("api: files touched only", "2 paths"));
        assert_eq!(rows(&Turn { name: "x".into(), ..Turn::default() }).len(), 1, "only rows with something in them");
    }

    #[test]
    fn the_brief_has_every_section() {
        let b = turn().text(Part::Brief);
        assert!(b.starts_with("Handoff from api (/w/api)\n\n## Goal\nAdd session refresh\n"), "{b}");
        assert!(b.contains("- [x] Write tests\n- [ ] Ship"));
        assert!(b.contains("## Files touched\n- src/session.rs\n- /etc/hosts"));
        assert!(b.ends_with("## Last plan\n1. a\n2. b\n3. c"));
        assert_eq!(turn().text(Part::Files), "- src/session.rs\n- /etc/hosts");
    }

    #[test]
    fn untrusted_text_is_cleaned() {
        let t = Turn { name: "a\u{202e}pi".into(), plan: Some("x\u{1b}[2J\ny".into()), ..Turn::default() };
        assert_eq!(t.text(Part::Plan), "x[2J\ny");
        assert!(t.text(Part::Brief).starts_with("Handoff from api"));
    }
}
