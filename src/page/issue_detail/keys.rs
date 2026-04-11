use crossterm::event::KeyCode;

use crate::action::AppAction;
use crate::app::App;
use crate::core::{ConfirmAction, View};
use crate::page::issue_list::keys::{build_ai_selector, build_status_selector};

pub fn handle_key(key: KeyCode, app: &App) -> Vec<AppAction> {
    match key {
        KeyCode::Esc => vec![AppAction::Back],
        KeyCode::Down => vec![AppAction::Next],
        KeyCode::Up => vec![AppAction::Previous],
        KeyCode::Char('y') => match app.current_issue_id() {
            Some(id) => vec![AppAction::CopyId(id)],
            None => vec![],
        },
        KeyCode::Char('e') => match current_issue_id_for_edit(app) {
            Some(id) => vec![AppAction::EditDescription(id)],
            None => vec![],
        },
        KeyCode::Char('m') => vec![AppAction::OpenConfirm(ConfirmAction::Merge)],
        KeyCode::Char('d') => vec![AppAction::OpenConfirm(ConfirmAction::Discard)],
        KeyCode::Char('r') => vec![AppAction::OpenConfirm(ConfirmAction::Retry)],
        KeyCode::Char('a') => match build_ai_selector(app) {
            Some(def) => vec![AppAction::OpenSelector(def)],
            None => vec![],
        },
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
        KeyCode::Char('l') => match app.current_issue_id() {
            Some(id) => vec![AppAction::CopyLogCommand(id)],
            None => vec![],
        },
        KeyCode::Char('q') => vec![AppAction::QuickCreate],
        KeyCode::Char('j') => vec![AppAction::NavigateIssue { forward: true }],
        KeyCode::Char('k') => vec![AppAction::NavigateIssue { forward: false }],
        _ => vec![],
    }
}

fn current_issue_id_for_edit(app: &App) -> Option<String> {
    // IssueDetail では issue_id を返す
    match &app.core.view {
        View::IssueDetail { issue_id, .. } => Some(issue_id.clone()),
        _ => app.current_issue_id(),
    }
}
