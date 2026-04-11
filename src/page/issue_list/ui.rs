use ratatui::{
    prelude::*,
    widgets::{Cell, Paragraph, Row, Table, TableState},
};

use crate::app::App;
use crate::bd;
use crate::ui::{epic_icon, padded_keybar_line, priority_style};
use crate::widget::keybar::KeyBar;

pub fn draw(frame: &mut Frame, app: &App, area: Rect) {
    let has_indicator =
        app.core.filter.is_active() && !matches!(app.core.keybar, KeyBar::Toggle(_));

    let constraints: Vec<Constraint> = if has_indicator {
        vec![Constraint::Min(1), Constraint::Length(1)]
    } else {
        vec![Constraint::Min(1)]
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    let table_area = chunks[0];

    // Table (with filter applied)
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
    state.select(Some(app.core.issue_store.selected));
    frame.render_stateful_widget(table, table_area, &mut state);

    // Filter indicator
    if has_indicator {
        let filter_text = app.core.filter.display_text();
        let indicator = Paragraph::new(Span::styled(
            format!(" {filter_text}"),
            Style::default().fg(Color::Yellow),
        ));
        frame.render_widget(indicator, chunks[1]);
    }
}

pub fn key_hints(app: &App) -> Line<'static> {
    if app.core.filter.is_active() {
        padded_keybar_line(&[
            ("Enter", "detail"),
            ("q", "create"),
            ("y", "copy id"),
            ("p", "priority"),
            ("a", "ai"),
            ("s", "status"),
            ("f", "filter*"),
        ])
    } else {
        padded_keybar_line(&[
            ("Enter", "detail"),
            ("q", "create"),
            ("y", "copy id"),
            ("p", "priority"),
            ("a", "ai"),
            ("s", "status"),
            ("f", "filter"),
        ])
    }
}
