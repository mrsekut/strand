use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState, Wrap},
};

use crate::app::App;
use crate::bd::Issue;
use crate::implement::ImplStatus;

pub fn draw(frame: &mut Frame, app: &App) {
    if app.show_detail {
        draw_detail(frame, app);
    } else {
        draw_list(frame, app);
    }
}

fn priority_style(priority: Option<u8>) -> Style {
    match priority {
        Some(0 | 1) => Style::default().fg(Color::Red),
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
        _ => Style::default().fg(Color::DarkGray),
    }
}

fn short_id(id: &str) -> &str {
    id.rsplit_once('-').map(|(_, s)| s).unwrap_or(id)
}

fn short_status(status: &str) -> &str {
    match status {
        "in_progress" => "prog",
        "deferred" => "defer",
        "closed" => "close",
        s => s,
    }
}

fn issue_icon(app: &App, issue: &Issue) -> (&'static str, Style) {
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
    (" ", Style::default())
}

fn issue_row<'a>(issue: &'a Issue, icon: &'a str, icon_style: Style) -> Row<'a> {
    let priority_text = issue.priority.map(|p| format!("P{p}")).unwrap_or_default();

    Row::new(vec![
        Cell::from(icon).style(icon_style),
        Cell::from(short_id(&issue.id).to_string()).style(Style::default().fg(Color::DarkGray)),
        Cell::from(short_status(&issue.status).to_string()).style(status_style(&issue.status)),
        Cell::from(priority_text).style(priority_style(issue.priority)),
        Cell::from(issue.title.clone()),
    ])
}

fn draw_list(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(frame.area());

    let header = Row::new(vec![
        Cell::from(""),
        Cell::from("ID"),
        Cell::from("Status"),
        Cell::from("Pri"),
        Cell::from("Title"),
    ])
    .style(
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    );

    let rows: Vec<Row> = app
        .issues
        .iter()
        .map(|issue| {
            let (icon, icon_style) = issue_icon(app, issue);
            issue_row(issue, icon, icon_style)
        })
        .collect();

    let widths = [
        Constraint::Length(2),
        Constraint::Length(4),
        Constraint::Length(6),
        Constraint::Length(3),
        Constraint::Min(10),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .title(" strand - Issues (q:quit j/k:move Enter:detail e:enrich i:implement)")
                .borders(Borders::ALL),
        )
        .row_highlight_style(Style::default().bg(Color::Rgb(70, 70, 90)))
        .highlight_symbol("▶ ");

    let mut state = TableState::default();
    state.select(Some(app.selected));

    frame.render_stateful_widget(table, chunks[0], &mut state);

    draw_notification(frame, app, chunks[1]);
}

fn draw_detail(frame: &mut Frame, app: &App) {
    let Some(issue) = app.selected_issue() else {
        return;
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(frame.area());

    let priority = issue
        .priority
        .map(|p| format!("P{p}"))
        .unwrap_or_else(|| "N/A".into());

    let mut lines = vec![
        Line::from(vec![
            Span::styled("ID: ", Style::default().fg(Color::Cyan)),
            Span::raw(&issue.id),
        ]),
        Line::from(vec![
            Span::styled("Title: ", Style::default().fg(Color::Cyan)),
            Span::raw(&issue.title),
        ]),
        Line::from(vec![
            Span::styled("Status: ", Style::default().fg(Color::Cyan)),
            Span::styled(&issue.status, status_style(&issue.status)),
        ]),
        Line::from(vec![
            Span::styled("Priority: ", Style::default().fg(Color::Cyan)),
            Span::styled(priority, priority_style(issue.priority)),
        ]),
        Line::from(""),
    ];

    // Impl job info
    if let Some(job) = app.impl_jobs.get(&issue.id) {
        let (status_text, style) = match &job.status {
            ImplStatus::Running => ("⚡ Implementing...", Style::default().fg(Color::Magenta)),
            ImplStatus::Done => ("✓ Implementation done", Style::default().fg(Color::Green)),
            ImplStatus::Failed(e) => {
                lines.push(Line::from(vec![
                    Span::styled("Impl: ", Style::default().fg(Color::Cyan)),
                    Span::styled(format!("✗ Failed: {e}"), Style::default().fg(Color::Red)),
                ]));
                // skip the default push below
                ("", Style::default())
            }
        };
        if !status_text.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("Impl: ", Style::default().fg(Color::Cyan)),
                Span::styled(status_text, style),
            ]));
        }
        lines.push(Line::from(vec![
            Span::styled("Branch: ", Style::default().fg(Color::Cyan)),
            Span::raw(&job.branch),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Worktree: ", Style::default().fg(Color::Cyan)),
            Span::raw(job.worktree_path.to_string_lossy().to_string()),
        ]));
        if matches!(job.status, ImplStatus::Done) {
            lines.push(Line::from(Span::styled(
                "[m: merge] [d: discard]",
                Style::default().fg(Color::Yellow),
            )));
        }
        lines.push(Line::from(""));
    }

    let desc = issue.description.as_deref().unwrap_or("(no description)");
    for l in desc.lines() {
        lines.push(Line::from(l.to_string()));
    }

    let detail_title = if app
        .impl_jobs
        .get(&issue.id)
        .is_some_and(|j| matches!(j.status, ImplStatus::Done))
    {
        " Issue Detail (Enter:back q:quit e:edit m:merge d:discard) "
    } else {
        " Issue Detail (Enter:back q:quit e:edit) "
    };

    let paragraph = Paragraph::new(lines)
        .block(Block::default().title(detail_title).borders(Borders::ALL))
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, chunks[0]);

    draw_notification(frame, app, chunks[1]);
}

fn draw_notification(frame: &mut Frame, app: &App, area: Rect) {
    if let Some((msg, time)) = &app.notification {
        if time.elapsed().as_secs() < 5 {
            let status = Paragraph::new(msg.as_str()).style(Style::default().fg(Color::Yellow));
            frame.render_widget(status, area);
        }
    }
}
