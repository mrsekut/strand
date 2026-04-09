use crossterm::event::KeyCode;
use ratatui::prelude::*;

use crate::app::{App, ConfirmAction, InputMode};
use crate::filter;

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
        InputMode::AwaitingFilter => match key {
            KeyCode::Char('s') => {
                app.input_mode = InputMode::AwaitingFilterStatus;
            }
            KeyCode::Char('l') => {
                app.filter.refresh_labels(&app.issues);
                app.input_mode = InputMode::AwaitingFilterLabel;
            }
            KeyCode::Char('c') => {
                app.filter.clear();
                app.selected = 0;
                app.input_mode = InputMode::Normal;
                app.notification = None;
            }
            _ => {
                app.input_mode = InputMode::Normal;
                app.notification = None;
            }
        },
        InputMode::AwaitingFilterStatus => match key {
            KeyCode::Left | KeyCode::Char('h') => {
                app.filter.status_cursor = app.filter.status_cursor.saturating_sub(1);
            }
            KeyCode::Right | KeyCode::Char('l') => {
                app.filter.status_cursor =
                    (app.filter.status_cursor + 1).min(filter::STATUSES.len() - 1);
            }
            KeyCode::Char(' ') | KeyCode::Enter => {
                app.filter.toggle_status_at_cursor();
                app.selected = 0;
            }
            KeyCode::Esc => {
                app.input_mode = InputMode::AwaitingFilter;
            }
            _ => {}
        },
        InputMode::AwaitingFilterLabel => match key {
            KeyCode::Left | KeyCode::Char('h') => {
                app.filter.label_cursor = app.filter.label_cursor.saturating_sub(1);
            }
            KeyCode::Right | KeyCode::Char('l') => {
                let len = app.filter.available_labels.len();
                if len > 0 {
                    app.filter.label_cursor = (app.filter.label_cursor + 1).min(len - 1);
                }
            }
            KeyCode::Char(' ') | KeyCode::Enter => {
                app.filter.toggle_label_at_cursor();
                app.selected = 0;
            }
            KeyCode::Esc => {
                app.input_mode = InputMode::AwaitingFilter;
            }
            _ => {}
        },
        InputMode::Normal => match key {
            KeyCode::Down | KeyCode::Char('j') => app.next(),
            KeyCode::Up | KeyCode::Char('k') => app.previous(),
            KeyCode::Enter => app.open_detail().await,
            KeyCode::Char('y') => app.copy_id(),
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
            KeyCode::Char('f') => {
                app.input_mode = InputMode::AwaitingFilter;
            }
            KeyCode::Char('q') => app.quick_create_with_editor(terminal).await,
            _ => {}
        },
    }
}
