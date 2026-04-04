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
        }
        InputMode::AwaitingYank => {
            app.input_mode = InputMode::Normal;
            app.notification = None;
            match key {
                KeyCode::Char('i') => app.copy_id(),
                _ => {}
            }
        }
        InputMode::AwaitingPriority => {
            app.input_mode = InputMode::Normal;
            app.notification = None;
            if let KeyCode::Char(c @ '0'..='4') = key {
                app.set_priority(c as u8 - b'0').await;
            }
        }
        InputMode::AwaitingStatus => {
            app.input_mode = InputMode::Normal;
            app.notification = None;
            match key {
                KeyCode::Char('o') => app.set_status("open").await,
                KeyCode::Char('p') => app.set_status("in_progress").await,
                KeyCode::Char('d') => app.set_status("deferred").await,
                KeyCode::Char('c') => app.set_status("closed").await,
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
            KeyCode::Char('y') => {
                app.input_mode = InputMode::AwaitingYank;
                app.notification = Some(("y-...".into(), std::time::Instant::now()));
            }
            KeyCode::Char('a') => {
                app.input_mode = InputMode::AwaitingAI;
                app.notification = Some(("a-...".into(), std::time::Instant::now()));
            }
            KeyCode::Char('s') => {
                app.input_mode = InputMode::AwaitingStatus;
                app.notification = Some(("s-...".into(), std::time::Instant::now()));
            }
            KeyCode::Char('p') => {
                app.input_mode = InputMode::AwaitingPriority;
                app.notification = Some(("p-...".into(), std::time::Instant::now()));
            }
            KeyCode::Char('q') => app.quick_create_with_editor(terminal).await,
            _ => {}
        },
    }
}
