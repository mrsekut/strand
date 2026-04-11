use crossterm::event::KeyCode;

use crate::action::AppAction;
use crate::app::App;
use crate::core::{ConfirmAction, View};
use crate::page::issue_list::keys::{build_ai_selector, build_status_selector};

pub fn handle_key(key: KeyCode, app: &App) -> Vec<AppAction> {
    match key {
        KeyCode::Esc => vec![AppAction::Back],
        KeyCode::Down | KeyCode::Char('j') => vec![AppAction::Next],
        KeyCode::Up | KeyCode::Char('k') => vec![AppAction::Previous],
        KeyCode::Enter => match selected_child_id(app) {
            Some(id) => vec![AppAction::OpenChildDetail(id)],
            None => vec![],
        },
        KeyCode::Char('y') => match app.current_issue_id() {
            Some(id) => vec![AppAction::CopyId(id)],
            None => vec![],
        },
        KeyCode::Char('e') => match epic_id_for_edit(app) {
            Some(id) => vec![AppAction::EditDescription(id)],
            None => vec![],
        },
        KeyCode::Char('a') => match build_ai_selector(app) {
            Some(def) => vec![AppAction::OpenSelector(def)],
            None => vec![],
        },
        KeyCode::Char('m') if app.all_children_closed() => {
            vec![AppAction::OpenConfirm(ConfirmAction::MergeEpic)]
        }
        KeyCode::Char('s') => match build_status_selector(app) {
            Some(def) => vec![AppAction::OpenSelector(def)],
            None => vec![],
        },
        KeyCode::Char('p') => match app.current_issue_id() {
            Some(id) => vec![AppAction::CopyWorktreePath(id)],
            None => vec![],
        },
        KeyCode::Char('c') => match app.current_issue_id() {
            Some(id) => vec![AppAction::CopyResumeCommand(id)],
            None => vec![],
        },
        KeyCode::Char('q') => vec![AppAction::QuickCreate],
        _ => vec![],
    }
}

fn selected_child_id(app: &App) -> Option<String> {
    match &app.core.view {
        View::EpicDetail {
            children,
            child_selected,
            ..
        } => children.get(*child_selected).map(|c| c.id.clone()),
        _ => None,
    }
}

fn epic_id_for_edit(app: &App) -> Option<String> {
    match &app.core.view {
        View::EpicDetail { epic_id, .. } => Some(epic_id.clone()),
        _ => app.current_issue_id(),
    }
}
