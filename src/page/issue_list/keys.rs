use crossterm::event::KeyCode;

use crate::action::{AppAction, SelectorDef, SelectorItem};
use crate::core::Core;

pub fn handle_key(key: KeyCode, core: &Core) -> Vec<AppAction> {
    match key {
        KeyCode::Down | KeyCode::Char('j') => vec![AppAction::Next],
        KeyCode::Up | KeyCode::Char('k') => vec![AppAction::Previous],
        KeyCode::Enter => match core.issue_store.selected_issue(&core.filter) {
            Some(issue) => vec![AppAction::OpenDetail(issue.id.clone())],
            None => vec![],
        },
        KeyCode::Char('y') => match core.current_issue_id() {
            Some(id) => vec![AppAction::CopyId(id)],
            None => vec![],
        },
        KeyCode::Char('a') => match build_ai_selector(core) {
            Some(def) => vec![AppAction::OpenSelector(def)],
            None => vec![],
        },
        KeyCode::Char('s') => match build_status_selector(core) {
            Some(def) => vec![AppAction::OpenSelector(def)],
            None => vec![],
        },
        KeyCode::Char('p') => match build_priority_selector(core) {
            Some(def) => vec![AppAction::OpenSelector(def)],
            None => vec![],
        },
        KeyCode::Char('E') => match core.issue_store.selected_issue(&core.filter) {
            Some(issue) => vec![AppAction::OpenEstimateInput {
                issue_id: issue.id.clone(),
                current: issue.estimate,
            }],
            None => vec![],
        },
        KeyCode::Char('f') => vec![AppAction::OpenSelector(build_filter_menu_selector())],
        KeyCode::Char('q') => vec![AppAction::QuickCreate],
        _ => vec![],
    }
}

pub fn build_ai_selector(core: &Core) -> Option<SelectorDef> {
    let issue_id = core.current_issue_id()?;
    let epic_id = core.find_parent_epic_id();
    Some(SelectorDef {
        items: vec![
            SelectorItem {
                shortcut: "e".into(),
                label: "enrich".into(),
                action: AppAction::StartEnrich(issue_id.clone()),
            },
            SelectorItem {
                shortcut: "i".into(),
                label: "implement".into(),
                action: AppAction::StartImplement {
                    issue_id: issue_id.clone(),
                    epic_id,
                },
            },
            SelectorItem {
                shortcut: "s".into(),
                label: "split".into(),
                action: AppAction::StartSplit(issue_id),
            },
        ],
        initial_cursor: 0,
    })
}

pub fn build_status_selector(core: &Core) -> Option<SelectorDef> {
    let issue_id = core.current_issue_id()?;
    Some(SelectorDef {
        items: vec![
            SelectorItem {
                shortcut: "o".into(),
                label: "open".into(),
                action: AppAction::SetStatus {
                    issue_id: issue_id.clone(),
                    status: "open".into(),
                },
            },
            SelectorItem {
                shortcut: "p".into(),
                label: "in_progress".into(),
                action: AppAction::SetStatus {
                    issue_id: issue_id.clone(),
                    status: "in_progress".into(),
                },
            },
            SelectorItem {
                shortcut: "d".into(),
                label: "deferred".into(),
                action: AppAction::SetStatus {
                    issue_id: issue_id.clone(),
                    status: "deferred".into(),
                },
            },
            SelectorItem {
                shortcut: "c".into(),
                label: "closed".into(),
                action: AppAction::SetStatus {
                    issue_id,
                    status: "closed".into(),
                },
            },
        ],
        initial_cursor: 0,
    })
}

pub fn build_priority_selector(core: &Core) -> Option<SelectorDef> {
    let issue_id = core.current_issue_id()?;
    Some(SelectorDef {
        items: (0..5)
            .map(|p| SelectorItem {
                shortcut: p.to_string(),
                label: format!("P{p}"),
                action: AppAction::SetPriority {
                    issue_id: issue_id.clone(),
                    priority: p,
                },
            })
            .collect(),
        initial_cursor: 2,
    })
}

pub fn build_filter_menu_selector() -> SelectorDef {
    SelectorDef {
        items: vec![
            SelectorItem {
                shortcut: "s".into(),
                label: "status".into(),
                action: AppAction::OpenFilterStatusToggle,
            },
            SelectorItem {
                shortcut: "l".into(),
                label: "label".into(),
                action: AppAction::OpenFilterLabelToggle,
            },
            SelectorItem {
                shortcut: "c".into(),
                label: "clear".into(),
                action: AppAction::ClearFilter,
            },
        ],
        initial_cursor: 0,
    }
}
