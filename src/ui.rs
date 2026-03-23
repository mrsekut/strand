use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};

use crate::app::App;

pub fn draw(frame: &mut Frame, app: &App) {
    if app.show_detail {
        draw_detail(frame, app);
    } else {
        draw_list(frame, app);
    }
}

fn draw_list(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(frame.area());

    let items: Vec<ListItem> = app
        .issues
        .iter()
        .map(|issue| {
            let enriching = if app.enriching_ids.contains(&issue.id) {
                "[*] "
            } else {
                ""
            };
            let priority = issue.priority.map(|p| format!("P{p}")).unwrap_or_default();
            let line = format!(
                "{enriching}{} [{}] {} {}",
                issue.id, issue.status, priority, issue.title
            );
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title(" bdtui - Issues (q:quit j/k:move Enter:detail e:enrich) ")
                .borders(Borders::ALL),
        )
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .highlight_symbol("▶ ");

    let mut state = ListState::default();
    state.select(Some(app.selected));

    frame.render_stateful_widget(list, chunks[0], &mut state);

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

    let text = format!(
        "ID: {}\nTitle: {}\nStatus: {}\nPriority: {}\n\n{}",
        issue.id,
        issue.title,
        issue.status,
        priority,
        issue.description.as_deref().unwrap_or("(no description)"),
    );

    let paragraph = Paragraph::new(text)
        .block(
            Block::default()
                .title(" Issue Detail (Enter:back q:quit) ")
                .borders(Borders::ALL),
        )
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
