use ratatui::{
    prelude::*,
    widgets::{Cell, Paragraph, Row, Table, TableState, Wrap},
};

use chrono::{DateTime, FixedOffset};

use crate::app::{App, View};
use crate::bd::{self, Issue};
use crate::implement::ImplStatus;

pub fn draw(frame: &mut Frame, app: &App) {
    match &app.view {
        View::EpicList => draw_epic_list(frame, app),
        View::EpicDetail { .. } => draw_epic_detail(frame, app),
        View::IssueDetail { .. } => draw_issue_detail(frame, app),
    }
}

fn format_timestamp(iso: &str) -> String {
    match iso.parse::<DateTime<FixedOffset>>() {
        Ok(dt) => dt.format("%Y/%-m/%-d %H:%M").to_string(),
        Err(_) => iso.to_string(),
    }
}

fn priority_style(priority: Option<u8>) -> Style {
    match priority {
        Some(0) => Style::default().fg(Color::Magenta),
        Some(1) => Style::default().fg(Color::Red),
        Some(2) => Style::default().fg(Color::Yellow),
        Some(_) => Style::default().fg(Color::DarkGray),
        None => Style::default().fg(Color::DarkGray),
    }
}

fn status_style(status: &str) -> Style {
    match status {
        "open" => Style::default().fg(Color::Green),
        "in_progress" => Style::default().fg(Color::Cyan),
        "deferred" => Style::default().fg(Color::Blue),
        "closed" => Style::default().fg(Color::DarkGray),
        _ => Style::default().fg(Color::DarkGray),
    }
}

fn epic_icon(app: &App, issue: &Issue) -> (&'static str, Style) {
    if let Some(job) = app.impl_jobs.get(&issue.id) {
        return match &job.status {
            ImplStatus::Running => ("⚡", Style::default().fg(Color::Magenta)),
            ImplStatus::Done => ("✓", Style::default().fg(Color::Green)),
            ImplStatus::Failed(_) => ("✗", Style::default().fg(Color::Red)),
        };
    }
    if app.enriching_ids.contains(&issue.id) {
        return ("⟳", Style::default().fg(Color::Yellow));
    }
    if issue.labels.contains(&"strand-unread".to_string()) {
        return ("●", Style::default().fg(Color::Cyan));
    }
    (" ", Style::default())
}

fn child_icon(app: &App, issue: &Issue) -> (&'static str, Style) {
    if let Some(job) = app.impl_jobs.get(&issue.id) {
        return match &job.status {
            ImplStatus::Running => ("⚡", Style::default().fg(Color::Magenta)),
            ImplStatus::Done => ("✓", Style::default().fg(Color::Green)),
            ImplStatus::Failed(_) => ("✗", Style::default().fg(Color::Red)),
        };
    }
    if issue.status == "closed" {
        return ("✓", Style::default().fg(Color::DarkGray));
    }
    if app.ready_ids.contains(&issue.id) {
        return ("○", Style::default().fg(Color::Green));
    }
    ("·", Style::default().fg(Color::DarkGray))
}

// --- Epic List ---

fn draw_epic_list(frame: &mut Frame, app: &App) {
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

// --- Epic Detail ---

fn draw_epic_detail(frame: &mut Frame, app: &App) {
    let epic_id = match &app.view {
        View::EpicDetail { epic_id } => epic_id,
        _ => return,
    };
    let epic = match app.issues.iter().find(|i| i.id == *epic_id) {
        Some(e) => e,
        None => return,
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(frame.area());

    // Split content area: description (top) + child issue table (bottom)
    let content_area = chunks[0].inner(Margin {
        horizontal: 2,
        vertical: 1,
    });

    // Calculate layout: description gets scroll, children get fixed rows
    let children_height = (app.children.len() as u16 + 2).min(content_area.height / 2); // +2 for header spacing
    let content_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),                  // description
            Constraint::Length(children_height), // children table
        ])
        .split(content_area);

    // Description section
    let priority = epic
        .priority
        .map(|p| format!("P{p}"))
        .unwrap_or_else(|| "N/A".into());

    let mut lines = vec![
        Line::from(vec![Span::styled(
            &epic.title,
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![
            Span::styled(&epic.id, Style::default().fg(Color::DarkGray)),
            Span::raw("  "),
            Span::styled(&epic.status, status_style(&epic.status)),
            Span::raw("  "),
            Span::styled(priority, priority_style(epic.priority)),
        ]),
    ];

    if let Some(dt) = epic.updated_at.as_deref() {
        lines.push(Line::from(vec![
            Span::styled("updated ", Style::default().fg(Color::DarkGray)),
            Span::styled(format_timestamp(dt), Style::default().fg(Color::DarkGray)),
        ]));
    }

    lines.push(Line::from(""));

    let desc = epic.description.as_deref().unwrap_or("(no description)");
    let md_text = tui_markdown::from_str(desc);
    lines.extend(md_text.lines);

    let paragraph = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((app.scroll_offset, 0));
    frame.render_widget(paragraph, content_chunks[0]);

    // Children section
    if !app.children.is_empty() {
        let rows: Vec<Row> = app
            .children
            .iter()
            .map(|issue| {
                let (icon, icon_style) = child_icon(app, issue);
                let priority_text = issue.priority.map(|p| format!("P{p}")).unwrap_or_default();
                Row::new(vec![
                    Cell::from(icon).style(icon_style),
                    Cell::from(bd::short_id(&issue.id).to_string())
                        .style(Style::default().fg(Color::DarkGray)),
                    Cell::from(priority_text).style(priority_style(issue.priority)),
                    Cell::from(Span::styled(&issue.status, status_style(&issue.status))),
                    Cell::from(issue.title.clone()),
                ])
            })
            .collect();

        let widths = [
            Constraint::Length(2),
            Constraint::Length(8),
            Constraint::Length(3),
            Constraint::Length(12),
            Constraint::Min(10),
        ];

        let table = Table::new(rows, widths)
            .row_highlight_style(Style::default().bg(Color::Rgb(70, 70, 90)))
            .highlight_symbol("▶ ");

        let mut state = TableState::default();
        state.select(Some(app.child_selected));

        frame.render_stateful_widget(table, content_chunks[1], &mut state);
    }

    draw_epic_detail_keybar(frame, app, chunks[1]);
    draw_notification(frame, app, chunks[2]);
}

// --- Issue Detail ---

fn draw_issue_detail(frame: &mut Frame, app: &App) {
    let issue_id = match &app.view {
        View::IssueDetail { issue_id, .. } => issue_id,
        _ => return,
    };
    let issue = match app.children.iter().find(|i| i.id == *issue_id) {
        Some(i) => i,
        None => return,
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(frame.area());

    let content_area = chunks[0].inner(Margin {
        horizontal: 2,
        vertical: 1,
    });

    let priority = issue
        .priority
        .map(|p| format!("P{p}"))
        .unwrap_or_else(|| "N/A".into());

    let mut lines = vec![
        Line::from(vec![Span::styled(
            &issue.title,
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![
            Span::styled(&issue.id, Style::default().fg(Color::DarkGray)),
            Span::raw("  "),
            Span::styled(&issue.status, status_style(&issue.status)),
            Span::raw("  "),
            Span::styled(priority, priority_style(issue.priority)),
        ]),
    ];

    if let Some(dt) = issue.updated_at.as_deref() {
        lines.push(Line::from(vec![
            Span::styled("updated ", Style::default().fg(Color::DarkGray)),
            Span::styled(format_timestamp(dt), Style::default().fg(Color::DarkGray)),
        ]));
    }

    lines.push(Line::from(""));

    // Impl job info
    if let Some(job) = app.impl_jobs.get(&issue.id) {
        let is_stale = {
            let impl_completed = job.completed_at.as_deref();
            let desc_updated = issue.updated_at.as_deref();
            match (desc_updated, impl_completed) {
                (Some(d), Some(i)) => {
                    let d_parsed = d.parse::<DateTime<FixedOffset>>().ok();
                    let i_parsed = i.parse::<DateTime<FixedOffset>>().ok();
                    matches!((d_parsed, i_parsed), (Some(dp), Some(ip)) if dp > ip)
                }
                _ => false,
            }
        };

        let mut impl_spans: Vec<Span> = match &job.status {
            ImplStatus::Running => vec![
                Span::styled("⚡ ", Style::default().fg(Color::Magenta)),
                Span::styled(&job.branch, Style::default().fg(Color::Magenta)),
            ],
            ImplStatus::Done => vec![
                Span::styled("✓ ", Style::default().fg(Color::Green)),
                Span::raw(&job.branch),
            ],
            ImplStatus::Failed(e) => vec![
                Span::styled("✗ ", Style::default().fg(Color::Red)),
                Span::styled(&job.branch, Style::default().fg(Color::Red)),
                Span::styled(format!("  {e}"), Style::default().fg(Color::Red)),
            ],
        };

        if is_stale {
            impl_spans.push(Span::styled(
                "  ⚠ stale",
                Style::default().fg(Color::Yellow),
            ));
        }

        lines.push(Line::from(impl_spans));

        // Impl-related keys
        let mut impl_keys: Vec<(&str, &str)> = vec![("p", "copy path")];
        if matches!(job.status, ImplStatus::Done) {
            impl_keys.push(("m", "merge"));
            impl_keys.push(("d", "discard"));
        }
        lines.push(keybar_line(&impl_keys));
        lines.push(Line::from(""));
    }

    let desc = issue.description.as_deref().unwrap_or("(no description)");
    let md_text = tui_markdown::from_str(desc);
    lines.extend(md_text.lines);

    if let Some(diff_bytes) = &app.detail_diff {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "── Changes ──",
            Style::default().fg(Color::DarkGray),
        )));
        use ansi_to_tui::IntoText;
        if let Ok(diff_text) = diff_bytes.into_text() {
            lines.extend(diff_text.lines);
        }
    }

    let paragraph = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((app.scroll_offset, 0));

    frame.render_widget(paragraph, content_area);

    draw_issue_detail_keybar(frame, app, chunks[1]);
    draw_notification(frame, app, chunks[2]);
}

// --- Key bars ---

fn padded_keybar_line(keys: &[(&str, &str)]) -> Line<'static> {
    let mut line = keybar_line(keys);
    line.spans.insert(0, Span::raw(" "));
    line
}

fn keybar_line(keys: &[(&str, &str)]) -> Line<'static> {
    let sep_style = Style::default().fg(Color::DarkGray);
    let key_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);
    let desc_style = Style::default().fg(Color::DarkGray);

    let mut spans = Vec::new();
    for (i, (key, desc)) in keys.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(" │ ", sep_style));
        }
        spans.push(Span::styled(key.to_string(), key_style));
        spans.push(Span::styled(format!(" {desc}"), desc_style));
    }
    Line::from(spans)
}

fn draw_keybar(frame: &mut Frame, app: &App, area: Rect) {
    use crate::app::{ConfirmAction, InputMode};

    let keys: Vec<(&str, &str)> = match app.input_mode {
        InputMode::AwaitingAI => vec![("e", "enrich"), ("i", "implement"), ("Esc", "cancel")],
        InputMode::AwaitingPriority => vec![("0-4", "priority"), ("Esc", "cancel")],
        InputMode::AwaitingConfirm(action) => {
            let label = match action {
                ConfirmAction::Close => "confirm close",
                ConfirmAction::Merge => "confirm merge",
                ConfirmAction::Discard => "confirm discard",
                ConfirmAction::MergeEpic => "confirm merge epic to master",
            };
            vec![("y", label), ("n", "cancel")]
        }
        InputMode::Normal => vec![
            ("Enter", "detail"),
            ("c", "copy id"),
            ("p", "priority"),
            ("a", "ai"),
            ("x", "close"),
            ("q", "quit"),
        ],
    };

    let line = padded_keybar_line(&keys);
    frame.render_widget(Paragraph::new(line), area);
}

fn draw_epic_detail_keybar(frame: &mut Frame, app: &App, area: Rect) {
    use crate::app::{ConfirmAction, InputMode};

    let keys: Vec<(&str, &str)> = match app.input_mode {
        InputMode::AwaitingAI => vec![("e", "enrich"), ("Esc", "cancel")],
        InputMode::AwaitingConfirm(action) => {
            let label = match action {
                ConfirmAction::Close => "confirm close",
                ConfirmAction::Merge => "confirm merge",
                ConfirmAction::Discard => "confirm discard",
                ConfirmAction::MergeEpic => "confirm merge epic to master",
            };
            vec![("y", label), ("n", "cancel")]
        }
        _ => {
            let mut keys = vec![
                ("Enter", "open issue"),
                ("Esc", "back"),
                ("c", "copy id"),
                ("e", "edit"),
                ("a", "ai"),
                ("x", "close"),
            ];
            if app.all_children_closed() {
                keys.push(("m", "merge to master"));
            }
            keys.push(("q", "quit"));
            keys
        }
    };

    let line = padded_keybar_line(&keys);
    frame.render_widget(Paragraph::new(line), area);
}

fn draw_issue_detail_keybar(frame: &mut Frame, app: &App, area: Rect) {
    use crate::app::{ConfirmAction, InputMode};

    let keys: Vec<(&str, &str)> = match app.input_mode {
        InputMode::AwaitingAI => vec![("e", "enrich"), ("i", "implement"), ("Esc", "cancel")],
        InputMode::AwaitingConfirm(action) => {
            let label = match action {
                ConfirmAction::Close => "confirm close",
                ConfirmAction::Merge => "confirm merge",
                ConfirmAction::Discard => "confirm discard",
                ConfirmAction::MergeEpic => "confirm merge epic to master",
            };
            vec![("y", label), ("n", "cancel")]
        }
        _ => vec![
            ("Esc", "back"),
            ("c", "copy id"),
            ("p", "copy path"),
            ("e", "edit"),
            ("a", "ai"),
            ("x", "close"),
            ("q", "quit"),
        ],
    };

    let line = padded_keybar_line(&keys);
    frame.render_widget(Paragraph::new(line), area);
}

fn draw_notification(frame: &mut Frame, app: &App, area: Rect) {
    if let Some((msg, time)) = &app.notification {
        if time.elapsed().as_secs() < 5 {
            let status =
                Paragraph::new(format!(" {msg}")).style(Style::default().fg(Color::Yellow));
            frame.render_widget(status, area);
        }
    }
}
