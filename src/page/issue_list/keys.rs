use crossterm::event::KeyCode;
use ratatui::prelude::*;

use crate::action::{AppAction, SelectorDef, SelectorItem};
use crate::app::App;
use crate::overlay::Overlay;

pub async fn handle_key(
    key: KeyCode,
    app: &mut App,
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
) {
    match key {
        KeyCode::Down | KeyCode::Char('j') => app.next(),
        KeyCode::Up | KeyCode::Char('k') => app.previous(),
        KeyCode::Enter => app.open_detail().await,
        KeyCode::Char('y') => app.copy_id(),
        KeyCode::Char('a') => {
            if let Some(def) = build_ai_selector(app) {
                app.overlay = Overlay::open_selector(def);
            }
        }
        KeyCode::Char('s') => {
            if let Some(def) = build_status_selector(app) {
                app.overlay = Overlay::open_selector(def);
            }
        }
        KeyCode::Char('p') => {
            if let Some(def) = build_priority_selector(app) {
                app.overlay = Overlay::open_selector(def);
            }
        }
        KeyCode::Char('f') => {
            app.overlay = Overlay::open_selector(build_filter_menu_selector());
        }
        KeyCode::Char('q') => app.quick_create_with_editor(terminal).await,
        _ => {}
    }
}

pub fn build_ai_selector(app: &App) -> Option<SelectorDef> {
    let issue_id = app.current_issue_id()?;
    let epic_id = app.find_parent_epic_id();
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

pub fn build_status_selector(app: &App) -> Option<SelectorDef> {
    let issue_id = app.current_issue_id()?;
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

pub fn build_priority_selector(app: &App) -> Option<SelectorDef> {
    let issue_id = app.current_issue_id()?;
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
