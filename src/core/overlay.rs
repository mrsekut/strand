use crate::action::SelectorDef;
use crate::core::ConfirmAction;
use crate::selector::{ExecuteSelector, ToggleSelector};

/// モーダルなUI状態。Overlay がアクティブな間、キーを優先的に消費する。
pub enum Overlay {
    None,
    Selector(ExecuteSelector),
    ToggleSelector(ToggleSelector),
    Confirm(ConfirmAction),
}

impl Overlay {
    pub fn open_selector(def: SelectorDef) -> Self {
        Overlay::Selector(ExecuteSelector::from_def(def))
    }
}
