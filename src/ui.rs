use ratatui::{prelude::*, widgets::Paragraph};

use chrono::{DateTime, FixedOffset};

use crate::ai::implement::ImplStatus;
use crate::app::{App, View};
use crate::bd::Issue;
use crate::page;

pub fn draw(frame: &mut Frame, app: &App) {
    match &app.view {
        View::IssueList => page::issue_list::ui::draw(frame, app),
        View::IssueDetail { .. } => page::issue_detail::ui::draw(frame, app),
        View::EpicDetail { .. } => page::epic_detail::ui::draw(frame, app),
    }
}

// --- Shared helpers ---

pub fn format_timestamp(iso: &str) -> String {
    match iso.parse::<DateTime<FixedOffset>>() {
        Ok(dt) => dt.format("%Y/%-m/%-d %H:%M").to_string(),
        Err(_) => iso.to_string(),
    }
}

pub fn priority_style(priority: Option<u8>) -> Style {
    match priority {
        Some(0) => Style::default().fg(Color::Magenta),
        Some(1) => Style::default().fg(Color::Red),
        Some(2) => Style::default().fg(Color::Yellow),
        Some(_) => Style::default().fg(Color::DarkGray),
        None => Style::default().fg(Color::DarkGray),
    }
}

pub fn status_style(status: &str) -> Style {
    match status {
        "open" => Style::default().fg(Color::Green),
        "in_progress" => Style::default().fg(Color::Cyan),
        "deferred" => Style::default().fg(Color::Blue),
        "closed" => Style::default().fg(Color::DarkGray),
        _ => Style::default().fg(Color::DarkGray),
    }
}

pub fn epic_icon(app: &App, issue: &Issue) -> (&'static str, Style) {
    if let Some(job) = app.impl_manager.get_job(&issue.id) {
        return match &job.status {
            ImplStatus::Running => ("⚡", Style::default().fg(Color::Magenta)),
            ImplStatus::Done => ("✓", Style::default().fg(Color::Green)),
            ImplStatus::Failed(_) => ("✗", Style::default().fg(Color::Red)),
            ImplStatus::Interrupted => ("⚠", Style::default().fg(Color::Yellow)),
        };
    }
    if app.enrich_manager.is_enriching(&issue.id) {
        return ("⟳", Style::default().fg(Color::Yellow));
    }
    if issue.labels.contains(&"strand-unread".to_string()) {
        return ("●", Style::default().fg(Color::Cyan));
    }
    (" ", Style::default())
}

pub fn padded_keybar_line(keys: &[(&str, &str)]) -> Line<'static> {
    let mut line = keybar_line(keys);
    line.spans.insert(0, Span::raw(" "));
    line
}

pub fn keybar_line(keys: &[(&str, &str)]) -> Line<'static> {
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

/// ExecuteSelector用: キーバー風セレクタ。カーソル位置をハイライト。
pub fn execute_selector_line(items: &[(&str, &str)], cursor: usize) -> Line<'static> {
    let mut spans = vec![Span::raw(" ")];
    for (i, (key, desc)) in items.iter().enumerate() {
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
        spans.push(Span::styled(key.to_string(), key_style));
        spans.push(Span::styled(format!(" {desc}"), desc_style));
    }
    Line::from(spans)
}

/// ToggleSelector用: 選択状態を色で表現。カーソル位置をハイライト。
pub fn toggle_selector_line(items: &[(String, bool)], cursor: usize) -> Line<'static> {
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

pub fn draw_notification(frame: &mut Frame, app: &App, area: Rect) {
    if let Some((msg, time)) = &app.notification {
        if time.elapsed().as_secs() < 5 {
            let status =
                Paragraph::new(format!(" {msg}")).style(Style::default().fg(Color::Yellow));
            frame.render_widget(status, area);
        }
    }
}
