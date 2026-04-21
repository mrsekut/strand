use crossterm::event::KeyCode;
use ratatui::{prelude::*, widgets::Paragraph};

use crate::action::{AppAction, InputTarget, SelectorDef, SelectorItem};
use crate::core::ConfirmAction;

/// トグルセレクタの種別
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToggleTarget {
    FilterStatus,
    FilterLabel,
}

/// 「選択して実行」型セレクタ。
pub struct Selector {
    pub items: Vec<SelectorItem>,
    pub cursor: usize,
}

impl Selector {
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
}

/// 「複数トグル」型セレクタ。
pub struct ToggleSelector {
    pub target: ToggleTarget,
    pub items: Vec<(String, bool)>,
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

    pub fn selected_labels(&self) -> Vec<&str> {
        self.items
            .iter()
            .filter(|(_, sel)| *sel)
            .map(|(label, _)| label.as_str())
            .collect()
    }
}

/// 数値入力モード。空文字でEnter時はclear扱い。
pub struct NumericInput {
    pub label: String,
    pub buffer: String,
    pub target: InputTarget,
}

impl NumericInput {
    pub fn new(label: String, initial: String, target: InputTarget) -> Self {
        Self {
            label,
            buffer: initial,
            target,
        }
    }
}

/// 画面最下部の 1 行 Widget。状態・キー処理・描画を自己完結で持つ。
pub enum KeyBar {
    /// ページデフォルト（ヒントは描画時に page から取得）
    Default,
    /// 選択モード: 1 つ選んで確定
    Selector(Selector),
    /// トグルモード: 複数 ON/OFF
    Toggle(ToggleSelector),
    /// 確認モード: y/n
    Confirm(ConfirmAction),
    /// 数値入力モード
    NumericInput(NumericInput),
}

impl KeyBar {
    pub fn open_selector(def: SelectorDef) -> Self {
        KeyBar::Selector(Selector::from_def(def))
    }

    pub fn open_numeric_input(label: String, initial: String, target: InputTarget) -> Self {
        KeyBar::NumericInput(NumericInput::new(label, initial, target))
    }

    pub fn is_default(&self) -> bool {
        matches!(self, KeyBar::Default)
    }

    /// キー入力を処理。カーソル移動は内部で行い、確定/キャンセル時に AppAction を返す。
    pub fn handle_key(&mut self, key: KeyCode) -> Vec<AppAction> {
        match self {
            KeyBar::Default => vec![],

            KeyBar::Selector(sel) => match key {
                KeyCode::Left | KeyCode::Char('h') => {
                    sel.move_left();
                    vec![]
                }
                KeyCode::Right | KeyCode::Char('l') => {
                    sel.move_right();
                    vec![]
                }
                KeyCode::Enter | KeyCode::Char(' ') => {
                    let action = sel.items[sel.cursor].action.clone();
                    vec![AppAction::CloseKeyBar, action]
                }
                KeyCode::Esc => vec![AppAction::CloseKeyBar],
                KeyCode::Char(c) => {
                    let c_str = c.to_string();
                    if let Some(item) = sel.items.iter().find(|i| i.shortcut == c_str) {
                        vec![AppAction::CloseKeyBar, item.action.clone()]
                    } else {
                        vec![]
                    }
                }
                _ => vec![],
            },

            KeyBar::Toggle(sel) => match key {
                KeyCode::Left | KeyCode::Char('h') => {
                    sel.move_left();
                    vec![]
                }
                KeyCode::Right | KeyCode::Char('l') => {
                    sel.move_right();
                    vec![]
                }
                KeyCode::Enter | KeyCode::Char(' ') => {
                    sel.toggle_at_cursor();
                    vec![AppAction::SyncFilter]
                }
                KeyCode::Esc => vec![AppAction::CloseKeyBar],
                _ => vec![],
            },

            KeyBar::Confirm(action) => {
                let action = *action;
                match key {
                    KeyCode::Char('y') => {
                        vec![AppAction::CloseKeyBar, AppAction::Confirm(action)]
                    }
                    _ => vec![AppAction::CloseKeyBar],
                }
            }

            KeyBar::NumericInput(input) => match key {
                KeyCode::Esc => vec![AppAction::CloseKeyBar],
                KeyCode::Backspace => {
                    input.buffer.pop();
                    vec![]
                }
                KeyCode::Enter => {
                    let minutes: u32 = if input.buffer.is_empty() {
                        0
                    } else {
                        match input.buffer.parse() {
                            Ok(n) => n,
                            Err(_) => return vec![],
                        }
                    };
                    let action = match input.target.clone() {
                        InputTarget::Estimate { issue_id } => {
                            AppAction::SetEstimate { issue_id, minutes }
                        }
                    };
                    vec![AppAction::CloseKeyBar, action]
                }
                KeyCode::Char(c) if c.is_ascii_digit() => {
                    input.buffer.push(c);
                    vec![]
                }
                _ => vec![],
            },
        }
    }

    /// KeyBar を描画する。Default の場合は何もしない（page が描画する）。
    pub fn render(&self, area: Rect, frame: &mut Frame) {
        let line = match self {
            KeyBar::Default => return,
            KeyBar::Selector(sel) => selector_line(&sel.items, sel.cursor),
            KeyBar::Toggle(sel) => toggle_line(&sel.items, sel.cursor),
            KeyBar::Confirm(action) => {
                crate::ui::padded_keybar_line(&[("y", action.label()), ("n", "cancel")])
            }
            KeyBar::NumericInput(input) => numeric_input_line(input),
        };
        frame.render_widget(Paragraph::new(line), area);
    }
}

// --- 描画ヘルパー ---

fn selector_line(items: &[SelectorItem], cursor: usize) -> Line<'static> {
    let mut spans = vec![Span::raw(" ")];
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("  "));
        }
        let is_cursor = i == cursor;
        let (key_style, desc_style) = if is_cursor {
            (
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::UNDERLINED),
            )
        } else {
            (
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
                Style::default().fg(Color::DarkGray),
            )
        };
        spans.push(Span::styled(item.shortcut.clone(), key_style));
        spans.push(Span::styled(format!(" {}", item.label), desc_style));
    }
    Line::from(spans)
}

fn numeric_input_line(input: &NumericInput) -> Line<'static> {
    Line::from(vec![
        Span::raw(" "),
        Span::styled(
            input.label.clone(),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" > "),
        Span::styled(
            input.buffer.clone(),
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        ),
        Span::styled("_", Style::default().fg(Color::DarkGray)),
    ])
}

fn toggle_line(items: &[(String, bool)], cursor: usize) -> Line<'static> {
    let mut spans = vec![Span::raw(" ")];
    for (i, (label, selected)) in items.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("  "));
        }
        let is_cursor = i == cursor;
        let style = if is_cursor && *selected {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
        } else if is_cursor {
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
        } else if *selected {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        spans.push(Span::styled(label.clone(), style));
    }
    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_selector(items: &[(&str, &str)]) -> Selector {
        Selector::from_def(SelectorDef {
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

    fn test_keybar_selector(items: &[(&str, &str)]) -> KeyBar {
        KeyBar::Selector(test_selector(items))
    }

    #[test]
    fn selector_shortcut() {
        let mut kb = test_keybar_selector(&[("e", "enrich"), ("i", "implement"), ("s", "split")]);
        let actions = kb.handle_key(KeyCode::Char('i'));
        assert_eq!(actions.len(), 2);
        assert!(matches!(actions[0], AppAction::CloseKeyBar));
        assert!(matches!(actions[1], AppAction::Notify(ref s) if s == "implement"));
    }

    #[test]
    fn selector_cursor_move_and_enter() {
        let mut kb = test_keybar_selector(&[("e", "enrich"), ("i", "implement"), ("s", "split")]);
        kb.handle_key(KeyCode::Right);
        kb.handle_key(KeyCode::Right);
        if let KeyBar::Selector(sel) = &kb {
            assert_eq!(sel.cursor, 2);
        }
        let actions = kb.handle_key(KeyCode::Enter);
        assert_eq!(actions.len(), 2);
        assert!(matches!(actions[0], AppAction::CloseKeyBar));
        assert!(matches!(actions[1], AppAction::Notify(ref s) if s == "split"));
    }

    #[test]
    fn selector_cursor_clamp() {
        let mut kb = test_keybar_selector(&[("e", "enrich"), ("i", "implement"), ("s", "split")]);
        kb.handle_key(KeyCode::Left);
        if let KeyBar::Selector(sel) = &kb {
            assert_eq!(sel.cursor, 0);
        }
        for _ in 0..10 {
            kb.handle_key(KeyCode::Right);
        }
        if let KeyBar::Selector(sel) = &kb {
            assert_eq!(sel.cursor, 2);
        }
    }

    #[test]
    fn selector_esc_closes() {
        let mut kb = test_keybar_selector(&[("e", "enrich")]);
        let actions = kb.handle_key(KeyCode::Esc);
        assert_eq!(actions.len(), 1);
        assert!(matches!(actions[0], AppAction::CloseKeyBar));
    }

    #[test]
    fn toggle_basic() {
        let mut kb = KeyBar::Toggle(ToggleSelector::new(
            ToggleTarget::FilterStatus,
            vec![("open".into(), false), ("closed".into(), false)],
        ));
        let actions = kb.handle_key(KeyCode::Char(' '));
        assert!(matches!(actions[0], AppAction::SyncFilter));
        if let KeyBar::Toggle(sel) = &kb {
            assert!(sel.items[0].1);
            assert!(!sel.items[1].1);
            assert_eq!(sel.selected_labels(), vec!["open"]);
        }
    }

    #[test]
    fn toggle_move_and_toggle() {
        let mut kb = KeyBar::Toggle(ToggleSelector::new(
            ToggleTarget::FilterStatus,
            vec![
                ("a".into(), false),
                ("b".into(), false),
                ("c".into(), false),
            ],
        ));
        kb.handle_key(KeyCode::Right);
        kb.handle_key(KeyCode::Char(' '));
        kb.handle_key(KeyCode::Right);
        kb.handle_key(KeyCode::Char(' '));
        if let KeyBar::Toggle(sel) = &kb {
            assert_eq!(sel.selected_labels(), vec!["b", "c"]);
        }
    }

    #[test]
    fn confirm_y() {
        let mut kb = KeyBar::Confirm(ConfirmAction::Merge);
        let actions = kb.handle_key(KeyCode::Char('y'));
        assert_eq!(actions.len(), 2);
        assert!(matches!(actions[0], AppAction::CloseKeyBar));
        assert!(matches!(
            actions[1],
            AppAction::Confirm(ConfirmAction::Merge)
        ));
    }

    #[test]
    fn numeric_input_digits_and_enter() {
        let mut kb = KeyBar::open_numeric_input(
            "estimate".into(),
            String::new(),
            InputTarget::Estimate {
                issue_id: "strand-1".into(),
            },
        );
        kb.handle_key(KeyCode::Char('3'));
        kb.handle_key(KeyCode::Char('0'));
        let actions = kb.handle_key(KeyCode::Enter);
        assert_eq!(actions.len(), 2);
        assert!(matches!(actions[0], AppAction::CloseKeyBar));
        assert!(matches!(
            actions[1],
            AppAction::SetEstimate { ref issue_id, minutes: 30 } if issue_id == "strand-1"
        ));
    }

    #[test]
    fn numeric_input_empty_enter_clears() {
        let mut kb = KeyBar::open_numeric_input(
            "estimate".into(),
            String::new(),
            InputTarget::Estimate {
                issue_id: "strand-1".into(),
            },
        );
        let actions = kb.handle_key(KeyCode::Enter);
        assert_eq!(actions.len(), 2);
        assert!(matches!(
            actions[1],
            AppAction::SetEstimate { minutes: 0, .. }
        ));
    }

    #[test]
    fn numeric_input_rejects_non_digit() {
        let mut kb = KeyBar::open_numeric_input(
            "estimate".into(),
            String::new(),
            InputTarget::Estimate {
                issue_id: "strand-1".into(),
            },
        );
        kb.handle_key(KeyCode::Char('a'));
        kb.handle_key(KeyCode::Char('5'));
        if let KeyBar::NumericInput(input) = &kb {
            assert_eq!(input.buffer, "5");
        } else {
            panic!("expected NumericInput");
        }
    }

    #[test]
    fn numeric_input_backspace_and_esc() {
        let mut kb = KeyBar::open_numeric_input(
            "estimate".into(),
            "123".into(),
            InputTarget::Estimate {
                issue_id: "strand-1".into(),
            },
        );
        kb.handle_key(KeyCode::Backspace);
        if let KeyBar::NumericInput(input) = &kb {
            assert_eq!(input.buffer, "12");
        }
        let actions = kb.handle_key(KeyCode::Esc);
        assert_eq!(actions.len(), 1);
        assert!(matches!(actions[0], AppAction::CloseKeyBar));
    }

    #[test]
    fn confirm_other_closes() {
        let mut kb = KeyBar::Confirm(ConfirmAction::Merge);
        let actions = kb.handle_key(KeyCode::Char('n'));
        assert_eq!(actions.len(), 1);
        assert!(matches!(actions[0], AppAction::CloseKeyBar));
    }
}
