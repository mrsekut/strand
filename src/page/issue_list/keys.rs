use crossterm::event::KeyCode;
use ratatui::prelude::*;

use crate::app::{App, ConfirmAction, InputMode};
use crate::selector::{self, ExecuteResult, ExecuteSelector, ToggleResult, ToggleSelector};

pub async fn handle_key(
    key: KeyCode,
    app: &mut App,
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
) {
    match app.input_mode {
        InputMode::Selecting => {
            if app.execute_selector.is_some() {
                handle_execute_key(key, app, terminal).await;
            } else if app.toggle_selector.is_some() {
                handle_toggle_key(key, app);
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
            KeyCode::Char('a') => {
                app.execute_selector = Some(ExecuteSelector::new(selector::AI_ITEMS));
                app.input_mode = InputMode::Selecting;
            }
            KeyCode::Char('s') => {
                app.execute_selector = Some(ExecuteSelector::new(selector::STATUS_ITEMS));
                app.input_mode = InputMode::Selecting;
            }
            KeyCode::Char('p') => {
                app.execute_selector =
                    Some(ExecuteSelector::with_cursor(selector::PRIORITY_ITEMS, 2));
                app.input_mode = InputMode::Selecting;
            }
            KeyCode::Char('f') => {
                app.execute_selector = Some(ExecuteSelector::new(selector::FILTER_MENU_ITEMS));
                app.input_mode = InputMode::Selecting;
            }
            KeyCode::Char('q') => app.quick_create_with_editor(terminal).await,
            _ => {}
        },
    }
}

async fn handle_execute_key(
    key: KeyCode,
    app: &mut App,
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
) {
    let items = app.execute_selector.as_ref().unwrap().items;
    let result = app.execute_selector.as_mut().unwrap().handle_key(key);

    match result {
        ExecuteResult::Selected(idx) => {
            app.input_mode = InputMode::Normal;
            let sel = app.execute_selector.take().unwrap();
            execute_action(app, sel.items, idx, terminal).await;
        }
        ExecuteResult::Cancelled => {
            app.input_mode = InputMode::Normal;
            app.execute_selector = None;
            app.notification = None;
        }
        ExecuteResult::Continue => {}
    }
}

async fn execute_action(
    app: &mut App,
    items: &[(&str, &str)],
    idx: usize,
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
) {
    let Some((_, label)) = items.get(idx) else {
        return;
    };
    match *label {
        // AI actions
        "enrich" => app.start_enrich(),
        "implement" => app.start_implement().await,
        "split" => app.start_split(),
        // Status actions
        "open" | "in_progress" | "deferred" | "closed" => {
            app.set_status(label).await;
        }
        // Priority actions
        "P0" => app.set_priority(0).await,
        "P1" => app.set_priority(1).await,
        "P2" => app.set_priority(2).await,
        "P3" => app.set_priority(3).await,
        "P4" => app.set_priority(4).await,
        // Filter menu
        "status" => {
            let items: Vec<(String, bool)> = crate::filter::STATUSES
                .iter()
                .map(|s| (s.to_string(), app.filter.statuses.contains(*s)))
                .collect();
            app.toggle_selector = Some(ToggleSelector::new(items));
            app.input_mode = InputMode::Selecting;
        }
        "label" => {
            app.filter.refresh_labels(&app.issues);
            let items: Vec<(String, bool)> = app
                .filter
                .available_labels
                .iter()
                .map(|l| (l.clone(), app.filter.labels.contains(l)))
                .collect();
            app.toggle_selector = Some(ToggleSelector::new(items));
            app.input_mode = InputMode::Selecting;
        }
        "clear" => {
            app.filter.clear();
            app.selected = 0;
        }
        _ => {}
    }
}

fn handle_toggle_key(key: KeyCode, app: &mut App) {
    let result = app.toggle_selector.as_mut().unwrap().handle_key(key);

    match result {
        ToggleResult::Toggled => {
            // toggleの結果をfilterに反映
            sync_toggle_to_filter(app);
            app.selected = 0;
        }
        ToggleResult::Done => {
            app.input_mode = InputMode::Normal;
            app.toggle_selector = None;
            app.notification = None;
        }
        ToggleResult::Continue => {}
    }
}

/// ToggleSelectorの状態をFilter に反映
fn sync_toggle_to_filter(app: &mut App) {
    let Some(sel) = &app.toggle_selector else {
        return;
    };
    let selected: std::collections::HashSet<String> = sel
        .selected_labels()
        .into_iter()
        .map(|s| s.to_string())
        .collect();

    // status or labelかを最初の項目で判定
    let is_status = sel
        .items
        .first()
        .map(|(label, _)| crate::filter::STATUSES.contains(&label.as_str()))
        .unwrap_or(false);

    if is_status {
        app.filter.statuses = selected;
    } else {
        app.filter.labels = selected;
    }
}
