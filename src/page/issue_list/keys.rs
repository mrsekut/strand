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
            selector_keys::handle_selecting_key(key, app).await;
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
        }
        InputMode::Normal => match key {
            KeyCode::Down | KeyCode::Char('j') => app.next(),
            KeyCode::Up | KeyCode::Char('k') => app.previous(),
            KeyCode::Enter => app.open_detail().await,
            KeyCode::Char('y') => app.copy_id(),
            KeyCode::Char('a') => {
                app.execute_selector =
                    Some(ExecuteSelector::new(SelectTarget::AI, selector::AI_ITEMS));
                app.input_mode = InputMode::Selecting;
            }
            KeyCode::Char('s') => {
                app.execute_selector = Some(ExecuteSelector::new(
                    SelectTarget::Status,
                    selector::STATUS_ITEMS,
                ));
                app.input_mode = InputMode::Selecting;
            }
            KeyCode::Char('p') => {
                app.execute_selector = Some(ExecuteSelector::with_cursor(
                    SelectTarget::Priority,
                    selector::PRIORITY_ITEMS,
                    2,
                ));
                app.input_mode = InputMode::Selecting;
            }
            KeyCode::Char('f') => {
                app.execute_selector = Some(ExecuteSelector::new(
                    SelectTarget::FilterMenu,
                    selector::FILTER_MENU_ITEMS,
                ));
                app.input_mode = InputMode::Selecting;
            }
            KeyCode::Char('q') => app.quick_create_with_editor(terminal).await,
            _ => {}
        },
    }
}
