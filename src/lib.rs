//! Relay: type `relay api` in the ⌘K palette to pack that agent's last turn (goal, tasks, files
//! touched, last plan) into a handoff brief for another agent.
// wit-bindgen's flattened `notify` import has 14 params.
#![allow(clippy::too_many_arguments)]

wit_bindgen::generate!({ path: "wit/haunt-plugin.wit", world: "plugin" });

mod brief;
mod turns;

use std::sync::Mutex;

use brief::Part;
use haunt::plugin::clipboard;
use haunt::plugin::log::log;
use haunt::plugin::palette;
use haunt::plugin::types::LogLevel;
use serde::Deserialize;
use turns::{Ev, Tabs};

const MODE: &str = "relay";

#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub unverified: bool,
}

#[derive(Default)]
struct Live {
    settings: Settings,
    tabs: Tabs,
}

static LIVE: Mutex<Option<Live>> = Mutex::new(None);

fn with<R>(f: impl FnOnce(&mut Live) -> R) -> R {
    f(LIVE.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(Live::default))
}

fn part_id(p: Part) -> &'static str {
    match p {
        Part::Brief => "brief",
        Part::Plan => "plan",
        Part::Files => "files",
    }
}

/// `brief:<surface>` back to its part and tab.
pub fn parse_id(id: &str) -> Option<(Part, &str)> {
    let (p, s) = id.split_once(':')?;
    let part = match p {
        "brief" => Part::Brief,
        "plan" => Part::Plan,
        "files" => Part::Files,
        _ => return None,
    };
    Some((part, s))
}

pub fn items(tabs: &Tabs, query: &str) -> Vec<PaletteItem> {
    tabs.matching(query)
        .into_iter()
        .flat_map(|(surface, turn)| {
            brief::rows(turn).into_iter().map(move |(part, title, right)| PaletteItem {
                id: format!("{}:{surface}", part_id(part)),
                title,
                subtitle: Some(right),
                at_ms: None,
                mark: None,
                surface: None,
            })
        })
        .collect()
}

struct Plugin;

impl Guest for Plugin {
    fn init(json: String) {
        with(|l| l.settings = serde_json::from_str(&json).unwrap_or_default());
        palette::mode(MODE, "Hand an agent's last turn to another");
    }

    fn on_settings_changed(json: String) {
        with(|l| l.settings = serde_json::from_str(&json).unwrap_or_default());
    }

    fn on_event(event: AgentEvent) {
        let Some(surface) = event.surface.as_deref() else { return };
        with(|l| {
            if !event.verified && !l.settings.unverified {
                return;
            }
            l.tabs.apply(&Ev {
                surface,
                kind: &event.kind,
                cwd: event.cwd.as_deref(),
                summary: event.summary.as_deref(),
                file_path: event.file_path.as_deref(),
                task_id: event.task_id.as_deref(),
            });
        });
    }

    fn palette_items(mode: String, query: String) -> Vec<PaletteItem> {
        if mode != MODE {
            return Vec::new();
        }
        with(|l| items(&l.tabs, &query))
    }

    fn on_action(action_id: String, _payload: String) {
        let Some((part, surface)) = parse_id(&action_id) else { return };
        let text = with(|l| l.tabs.tabs.get(surface).map(|t| t.turn.text(part)));
        if let Some(text) = text
            && let Err(e) = clipboard::write(&text)
        {
            log(LogLevel::Warn, &format!("relay: not copied: {e}"));
        }
    }

    fn row_menu(_surface: String) -> Vec<MenuItem> {
        Vec::new()
    }

    fn on_row_moved(_surface: String, _group: String, _before: Option<String>) {}

    fn on_theme_changed() {}

    fn on_tick(_now_ms: u64) {}

    fn on_exec(_job: u64, _outcome: Result<haunt::plugin::exec::Output, String>) {}

    fn on_away_changed(_away: bool, _away_ms: u64) {}

    fn on_hover_hold(_held: bool, _surface: String) {}

    fn on_fs_tree(_job: u64, _outcome: Result<haunt::plugin::fs_list::TreeResult, String>) {}
}

export!(Plugin);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_rows_read_like_the_mockup() {
        let mut t = Tabs::default();
        let ev = |kind, summary, file_path| Ev { surface: "s1", kind, cwd: Some("/w/api"), summary, file_path, task_id: None };
        t.apply(&ev("prompt_submit", Some("go"), None));
        t.apply(&ev("file_edited", None, Some("/w/api/a.rs")));
        t.apply(&ev("plan_exit", Some("1. a"), None));
        let it = items(&t, "api");
        let rows: Vec<_> = it.iter().map(|i| (i.id.as_str(), i.title.as_str(), i.subtitle.as_deref().unwrap())).collect();
        assert_eq!(
            rows,
            [
                ("brief:s1", "api: last turn → brief", "1 file · 0 tasks"),
                ("plan:s1", "api: plan only", "1 line"),
                ("files:s1", "api: files touched only", "1 path"),
            ]
        );
        assert!(items(&t, "web").is_empty());
        assert_eq!(parse_id("plan:s1").map(|(p, s)| (p, s.to_owned())), Some((Part::Plan, "s1".to_owned())));
        assert!(parse_id("x:s1").is_none());
    }

    #[test]
    fn manifest_asks_for_what_it_uses_and_defaults_match() {
        let m = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/haunt-plugin.toml")).unwrap();
        let t: toml::Table = toml::from_str(&m).unwrap();
        let caps = t["capabilities"].as_table().unwrap();
        assert_eq!(caps.keys().collect::<Vec<_>>(), ["clipboard", "event_details", "events"]);
        let defaults: serde_json::Map<String, serde_json::Value> =
            t["settings"].as_table().unwrap().iter().map(|(k, v)| (k.clone(), serde_json::to_value(&v["default"]).unwrap())).collect();
        assert_eq!(serde_json::from_value::<Settings>(serde_json::Value::Object(defaults)).unwrap(), Settings::default());
    }
}
