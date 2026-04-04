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
            // list画面ではAwaitingYankに入らない想定だが、念のためNormalに戻す
            app.input_mode = InputMode::Normal;
            app.notification = None;
        }
        InputMode::AwaitingPriority => {
            app.input_mode = InputMode::Normal;
            app.notification = None;
            if let KeyCode::Char(c @ '0'..='4') = key {
                app.set_priority(c as u8 - b'0').await;
            }
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
        }
        InputMode::Normal => match key {
            KeyCode::Down | KeyCode::Char('j') => app.next(),
            KeyCode::Up | KeyCode::Char('k') => app.previous(),
            KeyCode::Enter => app.open_detail().await,
            KeyCode::Char('y') => app.copy_id(),
            KeyCode::Char('a') => {
                app.input_mode = InputMode::AwaitingAI;
                app.notification = Some(("a-...".into(), std::time::Instant::now()));
            }
            KeyCode::Char('x') => {
                app.input_mode = InputMode::AwaitingConfirm(ConfirmAction::Close);
                app.notification = Some(("Close? (y/n)".into(), std::time::Instant::now()));
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
