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
        KeyCode::Down | KeyCode::Char('j') => app.next(),
        KeyCode::Up | KeyCode::Char('k') => app.previous(),
        KeyCode::Enter => app.open_child_detail().await,
        KeyCode::Char('y') => app.copy_id(),
        KeyCode::Char('e') => app.edit_description(terminal).await,
        KeyCode::Char('a') => {
            if let Some(def) = build_ai_selector(app) {
                app.overlay = Overlay::open_selector(def);
            }
        }
        KeyCode::Char('m') if app.all_children_closed() => {
            app.overlay = Overlay::Confirm(ConfirmAction::MergeEpic);
            app.notification = Some((
                "Merge epic to master? (y/n)".into(),
                std::time::Instant::now(),
            ));
        }
        KeyCode::Char('s') => {
            if let Some(def) = build_status_selector(app) {
                app.overlay = Overlay::open_selector(def);
            }
        }
        KeyCode::Char('p') => app.copy_worktree_path(),
        KeyCode::Char('c') => app.copy_resume_command(),
        KeyCode::Char('q') => app.quick_create_with_editor(terminal).await,
        _ => {}
    }
}
