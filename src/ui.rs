use ratatui::{prelude::*, widgets::Paragraph};

use chrono::{DateTime, FixedOffset};

use crate::ai::enrich::EnrichManager;
use crate::ai::implement::{ImplManager, ImplStatus};
use crate::bd::Issue;
use crate::core::{Core, View};
use crate::page;
use crate::widget::keybar::KeyBar;

pub fn draw(
    frame: &mut Frame,
    core: &Core,
    impl_manager: &ImplManager,
    enrich_manager: &EnrichManager,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),    // ページコンテンツ
            Constraint::Length(1), // KeyBar
            Constraint::Length(1), // notification
        ])
        .split(frame.area());

    // ページコンテンツ
    match &core.view {
        View::IssueList => {
            page::issue_list::ui::draw(frame, core, impl_manager, enrich_manager, chunks[0])
        }
        View::IssueDetail { .. } => {
            page::issue_detail::ui::draw(frame, core, impl_manager, chunks[0])
        }
        View::EpicDetail { .. } => {
            page::epic_detail::ui::draw(frame, core, impl_manager, chunks[0])
        }
    }

    // KeyBar
    match &core.keybar {
        KeyBar::Default => {
            let line = match &core.view {
                View::IssueList => page::issue_list::ui::key_hints(core),
                View::IssueDetail { .. } => page::issue_detail::ui::key_hints(),
                View::EpicDetail { .. } => page::epic_detail::ui::key_hints(core, impl_manager),
            };
            frame.render_widget(Paragraph::new(line), chunks[1]);
        }
        keybar => keybar.render(chunks[1], frame),
    }

    // notification
    draw_notification(frame, core, chunks[2]);
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

pub fn epic_icon(
    impl_manager: &ImplManager,
    enrich_manager: &EnrichManager,
    issue: &Issue,
) -> (&'static str, Style) {
    if let Some(job) = impl_manager.get_job(&issue.id) {
        return match &job.status {
            ImplStatus::Running => ("⚡", Style::default().fg(Color::Magenta)),
            ImplStatus::Done => ("✓", Style::default().fg(Color::Green)),
            ImplStatus::Failed(_) => ("✗", Style::default().fg(Color::Red)),
            ImplStatus::Interrupted => ("⚠", Style::default().fg(Color::Yellow)),
        };
    }
    if enrich_manager.is_enriching(&issue.id) {
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

fn draw_notification(frame: &mut Frame, core: &Core, area: Rect) {
    if let Some((msg, time)) = &core.notification {
        if time.elapsed().as_secs() < 5 {
            let status =
                Paragraph::new(format!(" {msg}")).style(Style::default().fg(Color::Yellow));
            frame.render_widget(status, area);
        }
    }
}
