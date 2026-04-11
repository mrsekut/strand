use std::collections::HashSet;

use crossterm::event::KeyCode;

use crate::action::AppAction;
use crate::app::App;
use crate::core::{Filter, Overlay};
use crate::selector::{SelectorResult, ToggleResult, ToggleTarget};

/// Overlay のキー処理結果。
pub enum OverlayOutcome {
    /// Overlay がアクティブでない — ページに処理を任せる
    NotConsumed,
    /// Overlay がキーを消費した（カーソル移動等、追加 action なし）
    Consumed,
    /// Overlay がキーを消費し、AppAction が発生した
    Action(AppAction),
}

/// Overlay のキーハンドリング。全ページ共通。
pub fn handle_overlay_key(key: KeyCode, app: &mut App) -> OverlayOutcome {
    // overlay を一時的に取り出して borrow 問題を回避
    let mut overlay = std::mem::replace(&mut app.core.overlay, Overlay::None);

    let outcome = match &mut overlay {
        Overlay::None => {
            app.core.overlay = overlay;
            return OverlayOutcome::NotConsumed;
        }
        Overlay::Selector(sel) => {
            let result = sel.handle_key(key);
            match result {
                SelectorResult::Selected(action) => {
                    // overlay は閉じる（None のまま）
                    OverlayOutcome::Action(action)
                }
                SelectorResult::Cancelled => {
                    app.core.notification = None;
                    OverlayOutcome::Consumed
                }
                SelectorResult::Continue => {
                    // overlay を戻す
                    app.core.overlay = overlay;
                    return OverlayOutcome::Consumed;
                }
            }
        }
        Overlay::ToggleSelector(sel) => {
            let result = sel.handle_key(key);
            match result {
                ToggleResult::Toggled => {
                    sync_toggle_to_filter(sel, &mut app.core.filter);
                    app.core.issue_store.selected = 0;
                    app.core.overlay = overlay;
                    return OverlayOutcome::Consumed;
                }
                ToggleResult::Done => {
                    app.core.notification = None;
                    OverlayOutcome::Consumed
                }
                ToggleResult::Continue => {
                    app.core.overlay = overlay;
                    return OverlayOutcome::Consumed;
                }
            }
        }
        Overlay::Confirm(action) => {
            let action = *action;
            if let KeyCode::Char('y') = key {
                app.core.notification = None;
                OverlayOutcome::Action(AppAction::Confirm(action))
            } else {
                app.core.notification = None;
                OverlayOutcome::Consumed
            }
        }
    };

    // overlay を閉じた状態で返す（None のまま）
    outcome
}

/// ToggleSelector の状態を Filter に反映
fn sync_toggle_to_filter(sel: &crate::selector::ToggleSelector, filter: &mut Filter) {
    let selected: HashSet<String> = sel
        .selected_labels()
        .into_iter()
        .map(|s| s.to_string())
        .collect();

    match sel.target {
        ToggleTarget::FilterStatus => filter.statuses = selected,
        ToggleTarget::FilterLabel => filter.labels = selected,
    }
}

/// FilterMenu の "status" 選択時: ToggleSelector を開く
pub fn open_filter_status_toggle(app: &mut App) {
    let items: Vec<(String, bool)> = crate::core::STATUSES
        .iter()
        .map(|s| (s.to_string(), app.core.filter.statuses.contains(*s)))
        .collect();
    app.core.overlay = Overlay::ToggleSelector(crate::selector::ToggleSelector::new(
        ToggleTarget::FilterStatus,
        items,
    ));
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
    app.core.overlay = Overlay::ToggleSelector(crate::selector::ToggleSelector::new(
        ToggleTarget::FilterLabel,
        items,
    ));
}
