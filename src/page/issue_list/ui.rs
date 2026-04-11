use ratatui::{
    prelude::*,
    widgets::{Cell, Paragraph, Row, Table, TableState},
};

use crate::app::App;
use crate::bd;
use crate::core::Overlay;
use crate::ui::{
    draw_notification, epic_icon, execute_selector_line, padded_keybar_line, priority_style,
    toggle_selector_line,
};

pub fn draw(frame: &mut Frame, app: &App) {
    let has_indicator =
        app.core.filter.is_active() && !matches!(app.core.overlay, Overlay::ToggleSelector(_));

    let mut constraints = vec![Constraint::Min(1)]; // table
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

    let indicator_area = if has_indicator {
        let a = chunks[idx];
        idx += 1;
        Some(a)
    } else {
        None
    };

    let keybar_area = chunks[idx];
    let notif_area = chunks[idx + 1];

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
    if let Some(area) = indicator_area {
        let filter_text = app.core.filter.display_text();
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
    let line = match &app.core.overlay {
        Overlay::Selector(sel) => execute_selector_line(&sel.items, sel.cursor),
        Overlay::ToggleSelector(sel) => toggle_selector_line(&sel.items, sel.cursor),
        Overlay::Confirm(action) => padded_keybar_line(&[("y", action.label()), ("n", "cancel")]),
        Overlay::None => {
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
    };

    frame.render_widget(Paragraph::new(line), area);
}
