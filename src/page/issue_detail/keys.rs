use crossterm::event::KeyCode;
use ratatui::prelude::*;

use crate::app::{App, ConfirmAction};
use crate::overlay::Overlay;
use crate::page::issue_list::keys::{build_ai_selector, build_status_selector};

pub async fn handle_key(
    key: KeyCode,
    app: &mut App,
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
) {
    match key {
        KeyCode::Esc => app.back(),
        KeyCode::Down => app.next(),
        KeyCode::Up => app.previous(),
        KeyCode::Char('y') => app.copy_id(),
        KeyCode::Char('e') => app.edit_description(terminal).await,
        KeyCode::Char('m') => {
            app.overlay = Overlay::Confirm(ConfirmAction::Merge);
            app.notification = Some(("Merge? (y/n)".into(), std::time::Instant::now()));
        }
        KeyCode::Char('d') => {
            app.overlay = Overlay::Confirm(ConfirmAction::Discard);
            app.notification = Some(("Discard? (y/n)".into(), std::time::Instant::now()));
        }
        KeyCode::Char('r') => {
            app.overlay = Overlay::Confirm(ConfirmAction::Retry);
            app.notification = Some(("Retry? (y/n)".into(), std::time::Instant::now()));
        }
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
        KeyCode::Char('p') => app.copy_worktree_path(),
        KeyCode::Char('c') => app.copy_resume_command(),
        KeyCode::Char('l') => app.copy_log_command(),
        KeyCode::Char('q') => app.quick_create_with_editor(terminal).await,
        KeyCode::Char('j') => app.navigate_issue(true).await,
        KeyCode::Char('k') => app.navigate_issue(false).await,
        _ => {}
    }
}
