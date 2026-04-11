use std::collections::HashSet;

use crossterm::event::KeyCode;

use crate::action::AppAction;
use crate::app::App;
use crate::widget::keybar::{KeyBar, ToggleSelector, ToggleTarget};

/// Overlay のキー処理結果。（3-3 で廃止予定）
pub enum OverlayOutcome {
    NotConsumed,
    Consumed,
    Action(AppAction),
}

/// KeyBar 経由のキーハンドリング。（3-3 で KeyBar::handle_key に直接置き換え）
pub fn handle_overlay_key(key: KeyCode, app: &mut App) -> OverlayOutcome {
    if app.core.keybar.is_default() {
        return OverlayOutcome::NotConsumed;
    }

    let actions = app.core.keybar.handle_key(key);

    if actions.is_empty() {
        return OverlayOutcome::Consumed;
    }

    let mut result_action = None;
    for action in actions {
        match action {
            AppAction::CloseKeyBar => {
                app.core.keybar = KeyBar::Default;
                app.core.notification = None;
            }
            AppAction::SyncFilter => {
                sync_keybar_to_filter(app);
            }
            other => {
                result_action = Some(other);
            }
        }
    }

    match result_action {
        Some(action) => OverlayOutcome::Action(action),
        None => OverlayOutcome::Consumed,
    }
}

/// ToggleSelector の状態を Filter に反映
fn sync_keybar_to_filter(app: &mut App) {
    if let KeyBar::Toggle(sel) = &app.core.keybar {
        let selected: HashSet<String> = sel
            .selected_labels()
            .into_iter()
            .map(|s| s.to_string())
            .collect();

        match sel.target {
            ToggleTarget::FilterStatus => app.core.filter.statuses = selected,
            ToggleTarget::FilterLabel => app.core.filter.labels = selected,
        }
    }
    app.core.issue_store.selected = 0;
}

/// FilterMenu の "status" 選択時: ToggleSelector を開く
pub fn open_filter_status_toggle(app: &mut App) {
    let items: Vec<(String, bool)> = crate::core::STATUSES
        .iter()
        .map(|s| (s.to_string(), app.core.filter.statuses.contains(*s)))
        .collect();
    app.core.keybar = KeyBar::Toggle(ToggleSelector::new(ToggleTarget::FilterStatus, items));
}

/// FilterMenu の "label" 選択時: ToggleSelector を開く
pub fn open_filter_label_toggle(app: &mut App) {
    app.core.filter.refresh_labels(&app.core.issue_store.issues);
    let items: Vec<(String, bool)> = app
        .core
        .filter
        .available_labels
        .iter()
        .map(|l| (l.clone(), app.core.filter.labels.contains(l)))
        .collect();
    app.core.keybar = KeyBar::Toggle(ToggleSelector::new(ToggleTarget::FilterLabel, items));
}
