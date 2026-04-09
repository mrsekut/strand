use crossterm::event::KeyCode;

/// 「選択して実行」型セレクタ。Enterで選択、モードを抜ける。
pub struct ExecuteSelector {
    /// (shortcut_key, label)
    pub items: &'static [(&'static str, &'static str)],
    pub cursor: usize,
}

impl ExecuteSelector {
    pub fn new(items: &'static [(&'static str, &'static str)]) -> Self {
        Self { items, cursor: 0 }
    }

    pub fn with_cursor(items: &'static [(&'static str, &'static str)], cursor: usize) -> Self {
        Self {
            items,
            cursor: cursor.min(items.len().saturating_sub(1)),
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

    /// キー入力を処理。選択されたindex を返す（モードを抜けるべき）。
    /// None なら未確定（カーソル移動等）。
    pub fn handle_key(&mut self, key: KeyCode) -> ExecuteResult {
        match key {
            KeyCode::Left | KeyCode::Char('h') => {
                self.move_left();
                ExecuteResult::Continue
            }
            KeyCode::Right | KeyCode::Char('l') => {
                self.move_right();
                ExecuteResult::Continue
            }
            KeyCode::Enter | KeyCode::Char(' ') => ExecuteResult::Selected(self.cursor),
            KeyCode::Esc => ExecuteResult::Cancelled,
            // ショートカットキーのチェック
            KeyCode::Char(c) => {
                let c_str = &c.to_string();
                for (i, (key, _)) in self.items.iter().enumerate() {
                    if *key == c_str {
                        return ExecuteResult::Selected(i);
                    }
                }
                ExecuteResult::Continue
            }
            _ => ExecuteResult::Continue,
        }
    }
}

pub enum ExecuteResult {
    /// カーソル移動等、まだ確定していない
    Continue,
    /// index番目が選択された
    Selected(usize),
    /// Escでキャンセル
    Cancelled,
}

/// 「複数トグル」型セレクタ。Spaceで ON/OFF、Escで抜ける。
pub struct ToggleSelector {
    pub items: Vec<(String, bool)>, // (label, selected)
    pub cursor: usize,
}

impl ToggleSelector {
    pub fn new(items: Vec<(String, bool)>) -> Self {
        Self { items, cursor: 0 }
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

// --- プリセット定義 ---

pub const AI_ITEMS: &[(&str, &str)] = &[
    ("e", "enrich"),
    ("i", "implement"),
    ("s", "split"),
];

pub const STATUS_ITEMS: &[(&str, &str)] = &[
    ("o", "open"),
    ("p", "in_progress"),
    ("d", "deferred"),
    ("c", "closed"),
];

pub const PRIORITY_ITEMS: &[(&str, &str)] = &[
    ("0", "P0"),
    ("1", "P1"),
    ("2", "P2"),
    ("3", "P3"),
    ("4", "P4"),
];

pub const FILTER_MENU_ITEMS: &[(&str, &str)] = &[
    ("s", "status"),
    ("l", "label"),
    ("c", "clear"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn execute_selector_shortcut() {
        let mut sel = ExecuteSelector::new(AI_ITEMS);
        assert!(matches!(
            sel.handle_key(KeyCode::Char('i')),
            ExecuteResult::Selected(1)
        ));
    }

    #[test]
    fn execute_selector_cursor_move_and_enter() {
        let mut sel = ExecuteSelector::new(AI_ITEMS);
        sel.handle_key(KeyCode::Right);
        sel.handle_key(KeyCode::Right);
        assert_eq!(sel.cursor, 2);
        assert!(matches!(
            sel.handle_key(KeyCode::Enter),
            ExecuteResult::Selected(2)
        ));
    }

    #[test]
    fn execute_selector_cursor_clamp() {
        let mut sel = ExecuteSelector::new(AI_ITEMS);
        sel.handle_key(KeyCode::Left);
        assert_eq!(sel.cursor, 0);
        for _ in 0..10 {
            sel.handle_key(KeyCode::Right);
        }
        assert_eq!(sel.cursor, 2);
    }

    #[test]
    fn toggle_selector_basic() {
        let mut sel = ToggleSelector::new(vec![
            ("open".into(), false),
            ("closed".into(), false),
        ]);
        sel.handle_key(KeyCode::Char(' '));
        assert!(sel.items[0].1);
        assert!(!sel.items[1].1);
        assert_eq!(sel.selected_labels(), vec!["open"]);
    }

    #[test]
    fn toggle_selector_move_and_toggle() {
        let mut sel = ToggleSelector::new(vec![
            ("a".into(), false),
            ("b".into(), false),
            ("c".into(), false),
        ]);
        sel.handle_key(KeyCode::Right);
        sel.handle_key(KeyCode::Char(' '));
        sel.handle_key(KeyCode::Right);
        sel.handle_key(KeyCode::Char(' '));
        assert_eq!(sel.selected_labels(), vec!["b", "c"]);
    }
}
