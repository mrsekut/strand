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
        InputMode::AwaitingYank => {
            app.input_mode = InputMode::Normal;
            app.notification = None;
            match key {
                KeyCode::Char('i') => app.copy_id(),
                KeyCode::Char('p') => app.copy_worktree_path(),
                KeyCode::Char('r') => app.copy_resume_command(),
                _ => {}
            }
            return;
        }
        InputMode::AwaitingConfirm(action) => {
            app.input_mode = InputMode::Normal;
            app.notification = None;
            if let KeyCode::Char('y') = key {
                match action {
                    ConfirmAction::Close => {
                        app.close_issue().await;
                        app.back();
                    }
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
        KeyCode::Char('y') => {
            app.input_mode = InputMode::AwaitingYank;
            app.notification = Some(("y-...".into(), std::time::Instant::now()));
        }
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
        KeyCode::Char('x') => {
            app.input_mode = InputMode::AwaitingConfirm(ConfirmAction::Close);
            app.notification = Some(("Close? (y/n)".into(), std::time::Instant::now()));
        }
        KeyCode::Char('q') => app.quick_create_with_editor(terminal).await,
        _ => {}
    }
}
