use std::collections::HashSet;

use crossterm::event::KeyCode;

use crate::action::{AppAction, SelectorDef};
use crate::app::{App, ConfirmAction};
use crate::filter::Filter;
use crate::selector::{
    ExecuteSelector, SelectorResult, ToggleResult, ToggleSelector, ToggleTarget,
};

/// モーダルなUI状態。Overlay がアクティブな間、キーを優先的に消費する。
pub enum Overlay {
    None,
    Selector(ExecuteSelector),
    ToggleSelector(ToggleSelector),
    Confirm(ConfirmAction),
}

impl Overlay {
    pub fn is_active(&self) -> bool {
        !matches!(self, Overlay::None)
    }

    pub fn open_selector(def: SelectorDef) -> Self {
        Overlay::Selector(ExecuteSelector::from_def(def))
    }
}

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
    let mut overlay = std::mem::replace(&mut app.overlay, Overlay::None);

    let outcome = match &mut overlay {
        Overlay::None => {
            app.overlay = overlay;
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
                    app.notification = None;
                    OverlayOutcome::Consumed
                }
                SelectorResult::Continue => {
                    // overlay を戻す
                    app.overlay = overlay;
                    return OverlayOutcome::Consumed;
                }
            }
        }
        Overlay::ToggleSelector(sel) => {
            let result = sel.handle_key(key);
            match result {
                ToggleResult::Toggled => {
                    sync_toggle_to_filter(sel, &mut app.filter);
                    app.selected = 0;
                    app.overlay = overlay;
                    return OverlayOutcome::Consumed;
                }
                ToggleResult::Done => {
                    app.notification = None;
                    OverlayOutcome::Consumed
                }
                ToggleResult::Continue => {
                    app.overlay = overlay;
                    return OverlayOutcome::Consumed;
                }
            }
        }
        Overlay::Confirm(action) => {
            let action = *action;
            if let KeyCode::Char('y') = key {
                app.notification = None;
                OverlayOutcome::Action(AppAction::Confirm(action))
            } else {
                app.notification = None;
                OverlayOutcome::Consumed
            }
        }
    };

    // overlay を閉じた状態で返す（None のまま）
    outcome
}

/// ToggleSelector の状態を Filter に反映
fn sync_toggle_to_filter(sel: &ToggleSelector, filter: &mut Filter) {
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
    let items: Vec<(String, bool)> = crate::filter::STATUSES
        .iter()
        .map(|s| (s.to_string(), app.filter.statuses.contains(*s)))
        .collect();
    app.overlay = Overlay::ToggleSelector(ToggleSelector::new(ToggleTarget::FilterStatus, items));
}

/// FilterMenu の "label" 選択時: ToggleSelector を開く
pub fn open_filter_label_toggle(app: &mut App) {
    app.filter.refresh_labels(&app.issues);
    let items: Vec<(String, bool)> = app
        .filter
        .available_labels
        .iter()
        .map(|l| (l.clone(), app.filter.labels.contains(l)))
        .collect();
    app.overlay = Overlay::ToggleSelector(ToggleSelector::new(ToggleTarget::FilterLabel, items));
}
