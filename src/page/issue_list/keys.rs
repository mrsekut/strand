use crossterm::event::KeyCode;
use ratatui::prelude::*;

use crate::app::{App, ConfirmAction, InputMode};

pub async fn handle_key(
    key: KeyCode,
    app: &mut App,
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
) {
    match app.input_mode {
        InputMode::Selecting => {
            app.input_mode = InputMode::Normal;
            app.notification = None;
            match key {
                KeyCode::Char('e') => app.start_enrich(),
                KeyCode::Char('i') => app.start_implement().await,
                KeyCode::Char('s') => app.start_split(),
                KeyCode::Char('o') => app.set_status("open").await,
                KeyCode::Char('p') => app.set_status("in_progress").await,
                KeyCode::Char('d') => app.set_status("deferred").await,
                KeyCode::Char('c') => app.set_status("closed").await,
                KeyCode::Char(c @ '0'..='4') => app.set_priority(c as u8 - b'0').await,
                _ => {}
            }
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
            KeyCode::Char('a') | KeyCode::Char('s') | KeyCode::Char('p') => {
                app.input_mode = InputMode::Selecting;
                app.notification = Some(("...".into(), std::time::Instant::now()));
            }
            KeyCode::Char('q') => app.quick_create_with_editor(terminal).await,
            _ => {}
        },
    }
}
