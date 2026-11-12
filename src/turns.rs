//! Each tab's latest turn, built from its agent events. Free of host calls.

use std::collections::BTreeMap;

use crate::brief::Turn;

/// Files and tasks kept per tab.
pub const LIST_MAX: usize = 200;

/// The parts of an agent event Relay reads.
#[derive(Debug, Clone, Copy, Default)]
pub struct Ev<'a> {
    pub surface: &'a str,
    pub kind: &'a str,
    pub cwd: Option<&'a str>,
    pub summary: Option<&'a str>,
    pub file_path: Option<&'a str>,
    pub task_id: Option<&'a str>,
}

#[derive(Debug, Default)]
pub struct Tab {
    pub turn: Turn,
    /// Task id → its place in `turn.tasks`.
    task_at: BTreeMap<String, usize>,
}

#[derive(Debug, Default)]
pub struct Tabs {
    pub tabs: BTreeMap<String, Tab>,
}

fn name_of(cwd: &str) -> String {
    cwd.trim_end_matches('/').rsplit('/').next().filter(|n| !n.is_empty()).unwrap_or(cwd).to_owned()
}

impl Tabs {
    pub fn apply(&mut self, e: &Ev) {
        let tab = self.tabs.entry(e.surface.to_owned()).or_default();
        if let Some(c) = e.cwd {
            tab.turn.cwd = Some(c.to_owned());
            tab.turn.name = name_of(c);
        }
        let t = &mut tab.turn;
        match e.kind {
            "session_start" => {
                *tab = Tab { turn: Turn { name: t.name.clone(), cwd: t.cwd.clone(), ..Turn::default() }, ..Tab::default() };
            }
            "prompt_submit" => {
                t.prompt = e.summary.map(str::to_owned);
                t.files.clear();
            }
            "file_edited" => {
                if let Some(p) = e.file_path
                    && !t.files.iter().any(|f| f == p)
                    && t.files.len() < LIST_MAX
                {
                    t.files.push(p.to_owned());
                }
            }
            "plan_exit" => {
                if let Some(p) = e.summary {
                    t.plan = Some(p.to_owned());
                }
            }
            "task_created" => {
                if let (Some(id), Some(subject)) = (e.task_id, e.summary)
                    && !tab.task_at.contains_key(id)
                    && t.tasks.len() < LIST_MAX
                {
                    tab.task_at.insert(id.to_owned(), t.tasks.len());
                    t.tasks.push((subject.to_owned(), "pending".into()));
                }
            }
            "task_updated" => {
                if let (Some(i), Some(status)) = (e.task_id.and_then(|id| tab.task_at.get(id)), e.summary) {
                    t.tasks[*i].1 = status.to_owned();
                }
            }
            _ => {}
        }
    }

    /// Tabs whose name contains `query`.
    pub fn matching(&self, query: &str) -> Vec<(&str, &Turn)> {
        let q = query.trim().to_lowercase();
        self.tabs.iter().filter(|(_, t)| !t.turn.name.is_empty() && t.turn.name.to_lowercase().contains(&q)).map(|(s, t)| (s.as_str(), &t.turn)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev<'a>(kind: &'a str, summary: Option<&'a str>) -> Ev<'a> {
        Ev { surface: "s1", kind, cwd: Some("/w/api"), summary, ..Ev::default() }
    }

    #[test]
    fn a_turn_collects_goal_tasks_files_and_plan() {
        let mut t = Tabs::default();
        t.apply(&ev("prompt_submit", Some("Add session refresh")));
        t.apply(&Ev { task_id: Some("1"), ..ev("task_created", Some("Write tests")) });
        t.apply(&Ev { task_id: Some("2"), ..ev("task_created", Some("Ship")) });
        t.apply(&Ev { task_id: Some("1"), ..ev("task_updated", Some("completed")) });
        t.apply(&Ev { task_id: Some("9"), ..ev("task_updated", Some("completed")) });
        for f in ["/w/api/a.rs", "/w/api/b.rs", "/w/api/a.rs"] {
            t.apply(&Ev { file_path: Some(f), ..ev("file_edited", None) });
        }
        t.apply(&ev("plan_exit", Some("1. a 2. b")));
        let turn = &t.tabs["s1"].turn;
        assert_eq!(turn.name, "api");
        assert_eq!(turn.prompt.as_deref(), Some("Add session refresh"));
        assert_eq!(turn.tasks, [("Write tests".to_owned(), "completed".to_owned()), ("Ship".to_owned(), "pending".to_owned())]);
        assert_eq!(turn.files, ["/w/api/a.rs", "/w/api/b.rs"]);
        assert_eq!(turn.plan.as_deref(), Some("1. a 2. b"));
    }

    #[test]
    fn a_new_prompt_starts_the_files_over_and_a_new_session_everything() {
        let mut t = Tabs::default();
        t.apply(&Ev { file_path: Some("/w/api/a.rs"), ..ev("file_edited", None) });
        t.apply(&ev("plan_exit", Some("p")));
        t.apply(&ev("prompt_submit", Some("next")));
        assert!(t.tabs["s1"].turn.files.is_empty());
        assert_eq!(t.tabs["s1"].turn.plan.as_deref(), Some("p"), "the last plan carries over");
        t.apply(&ev("session_start", None));
        let turn = &t.tabs["s1"].turn;
        assert_eq!((turn.prompt.as_ref(), turn.plan.as_ref(), turn.name.as_str()), (None, None, "api"));
    }

    #[test]
    fn the_query_matches_tab_names() {
        let mut t = Tabs::default();
        t.apply(&ev("prompt_submit", None));
        t.apply(&Ev { surface: "s2", cwd: Some("/w/web"), ..ev("prompt_submit", None) });
        assert_eq!(t.matching("AP").len(), 1);
        assert_eq!(t.matching("").len(), 2);
    }
}
