use crossterm::event::KeyCode;
use ratatui::prelude::*;

use crate::app::{App, ConfirmAction, InputMode};

pub async fn handle_key(
    key: KeyCode,
    app: &mut App,
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
) {
    match app.input_mode {
        InputMode::AwaitingAI => {
            app.input_mode = InputMode::Normal;
            app.notification = None;
            match key {
                KeyCode::Char('e') => app.start_enrich(),
                KeyCode::Char('i') => app.start_implement().await,
                KeyCode::Char('s') => app.start_split(),
                _ => {}
            }
            return;
        }
        InputMode::AwaitingStatus => {
            app.input_mode = InputMode::Normal;
            app.notification = None;
            match key {
                KeyCode::Char('o') => app.set_status("open").await,
                KeyCode::Char('p') => app.set_status("in_progress").await,
                KeyCode::Char('d') => app.set_status("deferred").await,
                KeyCode::Char('c') => {
                    app.set_status("closed").await;
                    app.back();
                }
                _ => {}
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
            app.input_mode = InputMode::AwaitingAI;
            app.notification = Some(("a-...".into(), std::time::Instant::now()));
        }
        KeyCode::Char('s') => {
            app.input_mode = InputMode::AwaitingStatus;
            app.notification = Some(("s-...".into(), std::time::Instant::now()));
        }
        KeyCode::Char('p') => app.copy_worktree_path(),
        KeyCode::Char('c') => app.copy_resume_command(),
        KeyCode::Char('l') => app.copy_log_command(),
        KeyCode::Char('q') => app.quick_create_with_editor(terminal).await,
        _ => {}
    }
}
