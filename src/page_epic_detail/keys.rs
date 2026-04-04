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
            if let KeyCode::Char('e') = key {
                app.start_enrich();
            }
            return;
        }
        InputMode::AwaitingYank => {
            app.input_mode = InputMode::Normal;
            app.notification = None;
            match key {
                KeyCode::Char('i') => app.copy_id(),
                KeyCode::Char('p') => app.copy_worktree_path(),
                _ => {}
            }
            return;
        }
        InputMode::AwaitingConfirm(action) => {
            app.input_mode = InputMode::Normal;
            app.notification = None;
            if let KeyCode::Char('y') = key {
                match action {
                    ConfirmAction::Close => app.close_issue().await,
                    ConfirmAction::Merge => app.merge_impl().await,
                    ConfirmAction::Discard => app.discard_impl().await,
                    ConfirmAction::MergeEpic => app.merge_epic().await,
                    ConfirmAction::Retry => app.retry_impl().await,
                }
            }
            return;
        }
        _ => {}
    }

    match key {
        KeyCode::Esc => app.back(),
        KeyCode::Down | KeyCode::Char('j') => app.next(),
        KeyCode::Up | KeyCode::Char('k') => app.previous(),
        KeyCode::Enter => app.open_child_detail().await,
        KeyCode::Char('y') => {
            app.input_mode = InputMode::AwaitingYank;
            app.notification = Some(("y-...".into(), std::time::Instant::now()));
        }
        KeyCode::Char('e') => app.edit_description(terminal).await,
        KeyCode::Char('a') => {
            app.input_mode = InputMode::AwaitingAI;
            app.notification = Some(("a-...".into(), std::time::Instant::now()));
        }
        KeyCode::Char('m') if app.all_children_closed() => {
            app.input_mode = InputMode::AwaitingConfirm(ConfirmAction::MergeEpic);
            app.notification = Some((
                "Merge epic to master? (y/n)".into(),
                std::time::Instant::now(),
            ));
        }
        KeyCode::Char('x') => {
            app.input_mode = InputMode::AwaitingConfirm(ConfirmAction::Close);
            app.notification = Some(("Close? (y/n)".into(), std::time::Instant::now()));
        }
        _ => {}
    }
}
