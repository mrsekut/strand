use std::collections::HashSet;

use ratatui::{
    prelude::*,
    widgets::{Cell, Paragraph, Row, Table, TableState, Wrap},
};

use crate::ai::implement::{ImplManager, ImplStatus};
use crate::bd;
use crate::core::{Core, View};
use crate::ui::{format_timestamp, padded_keybar_line, priority_style, status_style};

fn child_icon(
    impl_manager: &ImplManager,
    issue: &bd::Issue,
    ready_ids: &HashSet<String>,
) -> (&'static str, Style) {
    if let Some(job) = impl_manager.get_job(&issue.id) {
        return match &job.status {
            ImplStatus::Running => ("⚡", Style::default().fg(Color::Magenta)),
            ImplStatus::Done => ("✓", Style::default().fg(Color::Green)),
            ImplStatus::Failed(_) => ("✗", Style::default().fg(Color::Red)),
            ImplStatus::Interrupted => ("⚠", Style::default().fg(Color::Yellow)),
        };
    }
    if issue.status == "closed" {
        return ("✓", Style::default().fg(Color::DarkGray));
    }
    if ready_ids.contains(&issue.id) {
        return ("○", Style::default().fg(Color::Green));
    }
    ("·", Style::default().fg(Color::DarkGray))
}

pub fn draw(frame: &mut Frame, core: &Core, impl_manager: &ImplManager, area: Rect) {
    let (epic_id, children, ready_ids, child_selected, scroll_offset) = match &core.view {
        View::EpicDetail {
            epic_id,
            children,
            ready_ids,
            child_selected,
            scroll_offset,
        } => (
            epic_id,
            children,
            ready_ids,
            *child_selected,
            *scroll_offset,
        ),
        _ => return,
    };
    // TopLevelのissuesまたはスタック内EpicDetailのchildrenから探す
    let epic = match core
        .issue_store
        .issues
        .iter()
        .find(|i| i.id == *epic_id)
        .or_else(|| {
            core.view_stack.iter().rev().find_map(|v| {
                if let View::EpicDetail { children, .. } = v {
                    children.iter().find(|i| i.id == *epic_id)
                } else {
                    None
                }
            })
        }) {
        Some(e) => e,
        None => return,
    };

    // Split content area: description (top) + child issue table (bottom)
    let content_area = area.inner(Margin {
        horizontal: 2,
        vertical: 1,
    });

    // Calculate layout: description gets scroll, children get fixed rows
    let children_height = (children.len() as u16 + 2).min(content_area.height / 2);
    let content_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(children_height)])
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
        .scroll((scroll_offset, 0));
    frame.render_widget(paragraph, content_chunks[0]);

    // Children section
    if !children.is_empty() {
        let rows: Vec<Row> = children
            .iter()
            .map(|issue| {
                let (icon, icon_style) = child_icon(impl_manager, issue, ready_ids);
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
        state.select(Some(child_selected));

        frame.render_stateful_widget(table, content_chunks[1], &mut state);
    }
}

pub fn key_hints(core: &Core, impl_manager: &ImplManager) -> Line<'static> {
    let mut keys = vec![
        ("Enter", "open issue"),
        ("Esc", "back"),
        ("q", "create"),
        ("y", "copy id"),
        ("e", "edit"),
        ("E", "estimate"),
        ("a", "ai"),
        ("s", "status"),
        ("w", "session"),
    ];
    if let Some(issue_id) = core.current_issue_id() {
        if let Some(job) = impl_manager.get_job(&issue_id) {
            if job.session_id.is_some() {
                keys.push(("c", "continue"));
            }
        }
    }
    if core.all_children_closed() {
        keys.push(("m", "merge to master"));
    }
    padded_keybar_line(&keys)
}
