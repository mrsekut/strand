use crossterm::event::KeyCode;

use crate::action::{AppAction, SelectorDef, SelectorItem};

/// 「選択して実行」型セレクタ。Enterで選択、モードを抜ける。
/// SelectorDef から生成され、各選択肢に対応する AppAction を持つ。
/// Selector 自身は App を知らない — 選ばれた Action を返すだけ。
pub struct ExecuteSelector {
    pub items: Vec<SelectorItem>,
    pub cursor: usize,
}

impl ExecuteSelector {
    pub fn from_def(def: SelectorDef) -> Self {
        let cursor = def.initial_cursor.min(def.items.len().saturating_sub(1));
        Self {
            items: def.items,
            cursor,
        }
    }

    pub fn move_left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn move_right(&mut self) {
        if !self.items.is_empty() {
            self.cursor = (self.cursor + 1).min(self.items.len() - 1);
        }
    }

    /// キー入力を処理。App を触らない。結果を返すだけ。
    pub fn handle_key(&mut self, key: KeyCode) -> SelectorResult {
        match key {
            KeyCode::Left | KeyCode::Char('h') => {
                self.move_left();
                SelectorResult::Continue
            }
            KeyCode::Right | KeyCode::Char('l') => {
                self.move_right();
                SelectorResult::Continue
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                SelectorResult::Selected(self.items[self.cursor].action.clone())
            }
            KeyCode::Esc => SelectorResult::Cancelled,
            KeyCode::Char(c) => {
                let c_str = c.to_string();
                if let Some(item) = self.items.iter().find(|item| item.shortcut == c_str) {
                    SelectorResult::Selected(item.action.clone())
                } else {
                    SelectorResult::Continue
                }
            }
            _ => SelectorResult::Continue,
        }
    }
}

/// Selector がキー入力を処理した結果。App を知らない。
pub enum SelectorResult {
    /// カーソル移動等、まだ確定していない
    Continue,
    /// 確定。この Action を emit する。
    Selected(AppAction),
    /// Escでキャンセル
    Cancelled,
}

/// トグルセレクタの種別
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToggleTarget {
    FilterStatus,
    FilterLabel,
}

/// 「複数トグル」型セレクタ。Spaceで ON/OFF、Escで抜ける。
pub struct ToggleSelector {
    pub target: ToggleTarget,
    pub items: Vec<(String, bool)>, // (label, selected)
    pub cursor: usize,
}

impl ToggleSelector {
    pub fn new(target: ToggleTarget, items: Vec<(String, bool)>) -> Self {
        Self {
            target,
            items,
            cursor: 0,
        }
    }

    pub fn move_left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn move_right(&mut self) {
        if !self.items.is_empty() {
            self.cursor = (self.cursor + 1).min(self.items.len() - 1);
        }
    }

    pub fn toggle_at_cursor(&mut self) {
        if let Some(item) = self.items.get_mut(self.cursor) {
            item.1 = !item.1;
        }
    }

    /// 選択されたlabel一覧を返す
    pub fn selected_labels(&self) -> Vec<&str> {
        self.items
            .iter()
            .filter(|(_, sel)| *sel)
            .map(|(label, _)| label.as_str())
            .collect()
    }

    /// キー入力を処理。
    pub fn handle_key(&mut self, key: KeyCode) -> ToggleResult {
        match key {
            KeyCode::Left | KeyCode::Char('h') => {
                self.move_left();
                ToggleResult::Continue
            }
            KeyCode::Right | KeyCode::Char('l') => {
                self.move_right();
                ToggleResult::Continue
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                self.toggle_at_cursor();
                ToggleResult::Toggled
            }
            KeyCode::Esc => ToggleResult::Done,
            _ => ToggleResult::Continue,
        }
    }
}

pub enum ToggleResult {
    /// カーソル移動等
    Continue,
    /// トグルされた
    Toggled,
    /// Escで完了
    Done,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_selector(items: &[(&str, &str)]) -> ExecuteSelector {
        ExecuteSelector::from_def(SelectorDef {
            items: items
                .iter()
                .map(|(s, l)| SelectorItem {
                    shortcut: s.to_string(),
                    label: l.to_string(),
                    action: AppAction::Notify(l.to_string()),
                })
                .collect(),
            initial_cursor: 0,
        })
    }

    #[test]
    fn execute_selector_shortcut() {
        let mut sel = test_selector(&[("e", "enrich"), ("i", "implement"), ("s", "split")]);
        assert!(matches!(
            sel.handle_key(KeyCode::Char('i')),
            SelectorResult::Selected(AppAction::Notify(ref s)) if s == "implement"
        ));
    }

    #[test]
    fn execute_selector_cursor_move_and_enter() {
        let mut sel = test_selector(&[("e", "enrich"), ("i", "implement"), ("s", "split")]);
        sel.handle_key(KeyCode::Right);
        sel.handle_key(KeyCode::Right);
        assert_eq!(sel.cursor, 2);
        assert!(matches!(
            sel.handle_key(KeyCode::Enter),
            SelectorResult::Selected(AppAction::Notify(ref s)) if s == "split"
        ));
    }

    #[test]
    fn execute_selector_cursor_clamp() {
        let mut sel = test_selector(&[("e", "enrich"), ("i", "implement"), ("s", "split")]);
        sel.handle_key(KeyCode::Left);
        assert_eq!(sel.cursor, 0);
        for _ in 0..10 {
            sel.handle_key(KeyCode::Right);
        }
        assert_eq!(sel.cursor, 2);
    }

    #[test]
    fn toggle_selector_basic() {
        let mut sel = ToggleSelector::new(
            ToggleTarget::FilterStatus,
            vec![("open".into(), false), ("closed".into(), false)],
        );
        sel.handle_key(KeyCode::Char(' '));
        assert!(sel.items[0].1);
        assert!(!sel.items[1].1);
        assert_eq!(sel.selected_labels(), vec!["open"]);
    }

    #[test]
    fn toggle_selector_move_and_toggle() {
        let mut sel = ToggleSelector::new(
            ToggleTarget::FilterStatus,
            vec![
                ("a".into(), false),
                ("b".into(), false),
                ("c".into(), false),
            ],
        );
        sel.handle_key(KeyCode::Right);
        sel.handle_key(KeyCode::Char(' '));
        sel.handle_key(KeyCode::Right);
        sel.handle_key(KeyCode::Char(' '));
        assert_eq!(sel.selected_labels(), vec!["b", "c"]);
    }
}
