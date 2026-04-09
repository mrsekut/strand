use ratatui::{
    prelude::*,
    widgets::{Cell, Paragraph, Row, Table, TableState},
};

use crate::app::{App, InputMode};
use crate::bd;
use crate::ui::{draw_notification, epic_icon, padded_keybar_line, priority_style};

pub fn draw(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(frame.area());

    let rows: Vec<Row> = app
        .issues
        .iter()
        .map(|issue| {
            let (icon, icon_style) = epic_icon(app, issue);
            let priority_text = issue.priority.map(|p| format!("P{p}")).unwrap_or_default();
            Row::new(vec![
                Cell::from(icon).style(icon_style),
                Cell::from(bd::short_id(&issue.id).to_string())
                    .style(Style::default().fg(Color::DarkGray)),
                Cell::from(priority_text).style(priority_style(issue.priority)),
                Cell::from(issue.title.clone()),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(2),
        Constraint::Length(4),
        Constraint::Length(3),
        Constraint::Min(10),
    ];

    let table = Table::new(rows, widths)
        .row_highlight_style(Style::default().bg(Color::Rgb(70, 70, 90)))
        .highlight_symbol("▶ ");

    let mut state = TableState::default();
    state.select(Some(app.selected));

    frame.render_stateful_widget(table, chunks[0], &mut state);

    draw_keybar(frame, app, chunks[1]);
    draw_notification(frame, app, chunks[2]);
}

fn draw_keybar(frame: &mut Frame, app: &App, area: Rect) {
    let keys: Vec<(&str, &str)> = match app.input_mode {
        InputMode::Selecting => vec![],  // TODO: Step 2でセレクタ表示に置き換え
        InputMode::AwaitingConfirm(action) => {
            vec![("y", action.label()), ("n", "cancel")]
        }
        InputMode::Normal => vec![
            ("Enter", "detail"),
            ("q", "create"),
            ("y", "copy id"),
            ("p", "priority"),
            ("a", "ai"),
            ("s", "status"),
        ],
    };

    let line = padded_keybar_line(&keys);
    frame.render_widget(Paragraph::new(line), area);
}
