use crossterm::event::KeyCode;
use ratatui::prelude::*;

use crate::app::{App, ConfirmAction, InputMode};
use crate::page::selector_keys;
use crate::selector::{self, ExecuteSelector};

pub async fn handle_key(
    key: KeyCode,
    app: &mut App,
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
) {
    match app.input_mode {
        InputMode::Selecting => {
            selector_keys::handle_selecting_key(key, app).await;
            return;
        }
        InputMode::AwaitingConfirm(action) => {
            app.input_mode = InputMode::Normal;
            app.notification = None;
            if let KeyCode::Char('y') = key {
                match action {
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
        KeyCode::Char('y') => app.copy_id(),
        KeyCode::Char('e') => app.edit_description(terminal).await,
        KeyCode::Char('a') => {
            app.execute_selector = Some(ExecuteSelector::new(selector::AI_ITEMS));
            app.input_mode = InputMode::Selecting;
        }
        KeyCode::Char('m') if app.all_children_closed() => {
            app.input_mode = InputMode::AwaitingConfirm(ConfirmAction::MergeEpic);
            app.notification = Some((
                "Merge epic to master? (y/n)".into(),
                std::time::Instant::now(),
            ));
        }
        KeyCode::Char('s') => {
            app.execute_selector = Some(ExecuteSelector::new(selector::STATUS_ITEMS));
            app.input_mode = InputMode::Selecting;
        }
        KeyCode::Char('p') => app.copy_worktree_path(),
        KeyCode::Char('c') => app.copy_resume_command(),
        KeyCode::Char('q') => app.quick_create_with_editor(terminal).await,
        _ => {}
    }
}
