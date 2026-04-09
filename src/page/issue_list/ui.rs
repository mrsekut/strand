use ratatui::{
    prelude::*,
    widgets::{Cell, Paragraph, Row, Table, TableState},
};

use crate::app::{App, InputMode};
use crate::bd;
use crate::ui::{draw_notification, epic_icon, padded_keybar_line, priority_style};
use crate::widget::HorizontalSelector;

pub fn draw(frame: &mut Frame, app: &App) {
    let has_selector = matches!(
        app.input_mode,
        InputMode::AwaitingFilterStatus | InputMode::AwaitingFilterLabel
    );
    let has_indicator = app.filter.is_active() && !has_selector;

    let mut constraints = vec![Constraint::Min(1)]; // table
    if has_selector {
        constraints.push(Constraint::Length(1)); // selector bar
    }
    if has_indicator {
        constraints.push(Constraint::Length(1)); // filter indicator
    }
    constraints.push(Constraint::Length(1)); // keybar
    constraints.push(Constraint::Length(1)); // notification

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(frame.area());

    let mut idx = 0;
    let table_area = chunks[idx];
    idx += 1;

    let selector_area = if has_selector {
        let a = chunks[idx];
        idx += 1;
        Some(a)
    } else {
        None
    };

    let indicator_area = if has_indicator {
        let a = chunks[idx];
        idx += 1;
        Some(a)
    } else {
        None
    };

    let keybar_area = chunks[idx];
    let notif_area = chunks[idx + 1];

    // Table
    let displayed = app.displayed_issues();

    let rows: Vec<Row> = displayed
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
    frame.render_stateful_widget(table, table_area, &mut state);

    // Selector bar
    if let Some(area) = selector_area {
        match app.input_mode {
            InputMode::AwaitingFilterStatus => {
                let items = app.filter.status_items();
                let items_ref: Vec<(&str, bool)> = items.iter().map(|(s, b)| (*s, *b)).collect();
                let selector = HorizontalSelector::new(&items_ref, app.filter.status_cursor);
                frame.render_widget(selector, area);
            }
            InputMode::AwaitingFilterLabel => {
                let items = app.filter.label_items();
                let items_ref: Vec<(&str, bool)> = items.iter().map(|(s, b)| (*s, *b)).collect();
                let selector = HorizontalSelector::new(&items_ref, app.filter.label_cursor);
                frame.render_widget(selector, area);
            }
            _ => {}
        }
    }

    // Filter indicator
    if let Some(area) = indicator_area {
        let filter_text = app.filter.display_text();
        let indicator = Paragraph::new(Span::styled(
            format!(" {filter_text}"),
            Style::default().fg(Color::Yellow),
        ));
        frame.render_widget(indicator, area);
    }

    draw_keybar(frame, app, keybar_area);
    draw_notification(frame, app, notif_area);
}

fn draw_keybar(frame: &mut Frame, app: &App, area: Rect) {
    let keys: Vec<(&str, &str)> = match app.input_mode {
        InputMode::AwaitingAI => vec![
            ("e", "enrich"),
            ("i", "implement"),
            ("s", "split"),
            ("Esc", "cancel"),
        ],
        InputMode::AwaitingPriority => vec![("0-4", "priority"), ("Esc", "cancel")],
        InputMode::AwaitingStatus => vec![
            ("o", "open"),
            ("p", "in_progress"),
            ("d", "deferred"),
            ("c", "closed"),
            ("Esc", "cancel"),
        ],
        InputMode::AwaitingConfirm(action) => {
            vec![("y", action.label()), ("n", "cancel")]
        }
        InputMode::AwaitingFilter => vec![
            ("s", "status"),
            ("l", "label"),
            ("c", "clear"),
            ("Esc", "cancel"),
        ],
        InputMode::AwaitingFilterStatus | InputMode::AwaitingFilterLabel => {
            vec![("h/l", "move"), ("Space", "toggle"), ("Esc", "back")]
        }
        InputMode::Normal => {
            if app.filter.is_active() {
                vec![
                    ("Enter", "detail"),
                    ("q", "create"),
                    ("y", "copy id"),
                    ("p", "priority"),
                    ("a", "ai"),
                    ("s", "status"),
                    ("f", "filter*"),
                ]
            } else {
                vec![
                    ("Enter", "detail"),
                    ("q", "create"),
                    ("y", "copy id"),
                    ("p", "priority"),
                    ("a", "ai"),
                    ("s", "status"),
                    ("f", "filter"),
                ]
            }
        }
    };

    let line = padded_keybar_line(&keys);
    frame.render_widget(Paragraph::new(line), area);
}
