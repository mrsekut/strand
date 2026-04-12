use ratatui::{
    prelude::*,
    widgets::{Paragraph, Wrap},
};

use chrono::{DateTime, FixedOffset};

use crate::ai::implement::{ImplManager, ImplStatus};
use crate::core::{Core, View};
use crate::ui::{format_timestamp, keybar_line, padded_keybar_line, priority_style, status_style};

pub fn draw(frame: &mut Frame, core: &Core, impl_manager: &ImplManager, area: Rect) {
    let (issue_id, scroll_offset, detail_diff) = match &core.view {
        View::IssueDetail {
            issue_id,
            scroll_offset,
            diff,
        } => (issue_id, *scroll_offset, diff),
        _ => return,
    };
    // TopLevelのissuesまたはスタック内EpicDetailのchildrenから探す
    let issue = match core
        .issue_store
        .issues
        .iter()
        .find(|i| i.id == *issue_id)
        .or_else(|| {
            core.view_stack.iter().rev().find_map(|v| {
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

    let content_area = area.inner(Margin {
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
    if let Some(job) = impl_manager.get_job(&issue.id) {
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
            ImplStatus::Interrupted => vec![
                Span::styled("⚠ ", Style::default().fg(Color::Yellow)),
                Span::styled(&job.branch, Style::default().fg(Color::Yellow)),
                Span::styled("  interrupted", Style::default().fg(Color::Yellow)),
            ],
        };

        if is_stale {
            impl_spans.push(Span::styled(
                "  ⚠ stale",
                Style::default().fg(Color::Yellow),
            ));
        }

        lines.push(Line::from(impl_spans));

        let mut impl_keys: Vec<(&str, &str)> = vec![];
        match &job.status {
            ImplStatus::Done => {
                impl_keys.push(("m", "merge"));
                impl_keys.push(("d", "discard"));
                impl_keys.push(("r", "retry"));
            }
            ImplStatus::Interrupted | ImplStatus::Failed(_) => {
                impl_keys.push(("d", "discard"));
                impl_keys.push(("r", "retry"));
            }
            _ => {}
        }
        impl_keys.push(("p", "path"));
        if job.session_id.is_some() {
            impl_keys.push(("c", "continue"));
        }
        if !impl_keys.is_empty() {
            lines.push(keybar_line(&impl_keys));
        }
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
}

pub fn key_hints() -> Line<'static> {
    padded_keybar_line(&[
        ("Esc", "back"),
        ("q", "create"),
        ("y", "copy id"),
        ("e", "edit"),
        ("a", "ai"),
        ("s", "status"),
    ])
}
