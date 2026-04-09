//! セレクタモードの共通キーハンドリング。
//! 各画面のkeys.rsから呼び出す。

use crossterm::event::KeyCode;

use crate::app::{App, InputMode};
use crate::selector::{ExecuteResult, SelectTarget, ToggleResult, ToggleSelector, ToggleTarget};

/// Selectingモードのキー入力を処理。
/// 戻り値: (SelectTarget, 選択されたindex) — 画面固有のpost-action用
pub async fn handle_selecting_key(key: KeyCode, app: &mut App) -> Option<(SelectTarget, usize)> {
    if app.execute_selector.is_some() {
        handle_execute_key(key, app).await
    } else if app.toggle_selector.is_some() {
        handle_toggle_key(key, app);
        None
    } else {
        None
    }
}

async fn handle_execute_key(key: KeyCode, app: &mut App) -> Option<(SelectTarget, usize)> {
    let result = app.execute_selector.as_mut().unwrap().handle_key(key);

    match result {
        ExecuteResult::Selected(idx) => {
            let sel = app.execute_selector.take().unwrap();
            app.input_mode = InputMode::Normal;
            execute_action(app, sel.target, idx).await;
            Some((sel.target, idx))
        }
        ExecuteResult::Cancelled => {
            app.input_mode = InputMode::Normal;
            app.execute_selector = None;
            app.notification = None;
            None
        }
        ExecuteResult::Continue => None,
    }
}

/// target + index に基づいてアクションを実行
async fn execute_action(app: &mut App, target: SelectTarget, idx: usize) {
    match target {
        SelectTarget::AI => match idx {
            0 => app.start_enrich(),
            1 => app.start_implement().await,
            2 => app.start_split(),
            _ => {}
        },
        SelectTarget::Status => {
            let statuses = ["open", "in_progress", "deferred", "closed"];
            if let Some(status) = statuses.get(idx) {
                app.set_status(status).await;
            }
        }
        SelectTarget::Priority => {
            if idx <= 4 {
                app.set_priority(idx as u8).await;
            }
        }
        SelectTarget::FilterMenu => match idx {
            0 => {
                // status toggle
                let items: Vec<(String, bool)> = crate::filter::STATUSES
                    .iter()
                    .map(|s| (s.to_string(), app.filter.statuses.contains(*s)))
                    .collect();
                app.toggle_selector = Some(ToggleSelector::new(ToggleTarget::FilterStatus, items));
                app.input_mode = InputMode::Selecting;
            }
            1 => {
                // label toggle
                app.filter.refresh_labels(&app.issues);
                let items: Vec<(String, bool)> = app
                    .filter
                    .available_labels
                    .iter()
                    .map(|l| (l.clone(), app.filter.labels.contains(l)))
                    .collect();
                app.toggle_selector = Some(ToggleSelector::new(ToggleTarget::FilterLabel, items));
                app.input_mode = InputMode::Selecting;
            }
            2 => {
                // clear
                app.filter.clear();
                app.selected = 0;
            }
            _ => {}
        },
    }
}

fn handle_toggle_key(key: KeyCode, app: &mut App) {
    let result = app.toggle_selector.as_mut().unwrap().handle_key(key);

    match result {
        ToggleResult::Toggled => {
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

/// ToggleSelectorの状態をFilterに反映
fn sync_toggle_to_filter(app: &mut App) {
    let Some(sel) = &app.toggle_selector else {
        return;
    };
    let selected: std::collections::HashSet<String> = sel
        .selected_labels()
        .into_iter()
        .map(|s| s.to_string())
        .collect();

    match sel.target {
        ToggleTarget::FilterStatus => app.filter.statuses = selected,
        ToggleTarget::FilterLabel => app.filter.labels = selected,
    }
}
