//! セレクタモードの共通キーハンドリング。
//! 各画面のkeys.rsから呼び出す。

use crossterm::event::KeyCode;

use crate::app::{App, InputMode};
use crate::selector::{ExecuteResult, ToggleResult, ToggleSelector};

/// Selectingモードのキー入力を処理。
/// 戻り値: execute_actionで選ばれたlabel（画面側でpost-action処理に使う）
pub async fn handle_selecting_key(key: KeyCode, app: &mut App) -> Option<&'static str> {
    if app.execute_selector.is_some() {
        handle_execute_key(key, app).await
    } else if app.toggle_selector.is_some() {
        handle_toggle_key(key, app);
        None
    } else {
        None
    }
}

async fn handle_execute_key(key: KeyCode, app: &mut App) -> Option<&'static str> {
    let result = app.execute_selector.as_mut().unwrap().handle_key(key);

    match result {
        ExecuteResult::Selected(idx) => {
            let sel = app.execute_selector.take().unwrap();
            app.input_mode = InputMode::Normal;
            let label = sel.items.get(idx).map(|(_, l)| *l).unwrap_or("");
            execute_action(app, label).await;
            Some(label)
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

/// 選ばれたlabelに基づいてアクションを実行
async fn execute_action(app: &mut App, label: &str) {
    match label {
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
