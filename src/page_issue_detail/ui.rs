use ratatui::{
    prelude::*,
    widgets::{Paragraph, Wrap},
};

use chrono::{DateTime, FixedOffset};

use crate::ai_implement::ImplStatus;
use crate::app::{App, InputMode, View};
use crate::ui::{
    draw_notification, format_timestamp, keybar_line, padded_keybar_line, priority_style,
    status_style,
};

pub fn draw(frame: &mut Frame, app: &App) {
    let (issue_id, scroll_offset, detail_diff) = match &app.view {
        View::IssueDetail {
            issue_id,
            scroll_offset,
            diff,
        } => (issue_id, *scroll_offset, diff),
        _ => return,
    };
    // TopLevelのissuesまたはスタック内EpicDetailのchildrenから探す
    let issue = match app.issues.iter().find(|i| i.id == *issue_id).or_else(|| {
        app.view_stack.iter().rev().find_map(|v| {
            if let View::EpicDetail { children, .. } = v {
                children.iter().find(|i| i.id == *issue_id)
            } else {
                None
            }
        })
    }) {
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
    if let Some(job) = app.impl_manager.get_job(&issue.id) {
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

    if let Some(diff_bytes) = detail_diff {
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
        .scroll((scroll_offset, 0));

    frame.render_widget(paragraph, content_area);

    draw_keybar(frame, app, chunks[1]);
    draw_notification(frame, app, chunks[2]);
}

fn draw_keybar(frame: &mut Frame, app: &App, area: Rect) {
    let keys: Vec<(&str, &str)> = match app.input_mode {
        InputMode::AwaitingAI => vec![
            ("e", "enrich"),
            ("i", "implement"),
            ("s", "split"),
            ("Esc", "cancel"),
        ],
        InputMode::AwaitingConfirm(action) => {
            vec![("y", action.label()), ("n", "cancel")]
        }
        _ => vec![
            ("Esc", "back"),
            ("c", "copy id"),
            ("p", "copy path"),
            ("e", "edit"),
            ("a", "ai"),
            ("x", "close"),
        ],
    };

    let line = padded_keybar_line(&keys);
    frame.render_widget(Paragraph::new(line), area);
}
