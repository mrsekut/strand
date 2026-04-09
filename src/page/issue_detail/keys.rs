use crossterm::event::KeyCode;
use ratatui::prelude::*;

use crate::app::{App, ConfirmAction, InputMode};
use crate::page::selector_keys;
use crate::selector::{self, ExecuteSelector, SelectTarget};

pub async fn handle_key(
    key: KeyCode,
    app: &mut App,
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
) {
    match app.input_mode {
        InputMode::Selecting => {
            let result = selector_keys::handle_selecting_key(key, app).await;
            // issue_detail固有: closed時にback
            if result == Some((SelectTarget::Status, 3)) {
                app.back();
            }
            return;
        }
        InputMode::AwaitingConfirm(action) => {
            app.input_mode = InputMode::Normal;
            app.notification = None;
            if let KeyCode::Char('y') = key {
                match action {
                    ConfirmAction::Merge => {
                        app.merge_impl().await;
                        app.back();
                    }
                    ConfirmAction::Discard => app.discard_impl().await,
                    ConfirmAction::Retry => app.retry_impl().await,
                    _ => {}
                }
            }
            return;
        }
        _ => {}
    }

    match key {
        KeyCode::Esc => app.back(),
        KeyCode::Down => app.next(),
        KeyCode::Up => app.previous(),
        KeyCode::Char('y') => app.copy_id(),
        KeyCode::Char('e') => app.edit_description(terminal).await,
        KeyCode::Char('m') => {
            app.input_mode = InputMode::AwaitingConfirm(ConfirmAction::Merge);
            app.notification = Some(("Merge? (y/n)".into(), std::time::Instant::now()));
        }
        KeyCode::Char('d') => {
            app.input_mode = InputMode::AwaitingConfirm(ConfirmAction::Discard);
            app.notification = Some(("Discard? (y/n)".into(), std::time::Instant::now()));
        }
        KeyCode::Char('r') => {
            app.input_mode = InputMode::AwaitingConfirm(ConfirmAction::Retry);
            app.notification = Some(("Retry? (y/n)".into(), std::time::Instant::now()));
        }
        KeyCode::Char('a') => {
            app.execute_selector = Some(ExecuteSelector::new(SelectTarget::AI, selector::AI_ITEMS));
            app.input_mode = InputMode::Selecting;
        }
        KeyCode::Char('s') => {
            app.execute_selector = Some(ExecuteSelector::new(
                SelectTarget::Status,
                selector::STATUS_ITEMS,
            ));
            app.input_mode = InputMode::Selecting;
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
