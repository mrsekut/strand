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
    let items: Vec<ListItem> = app
        .issues
        .iter()
        .map(|issue| {
            let priority = issue.priority.map(|p| format!("P{p}")).unwrap_or_default();
            let line = format!(
                "{} [{}] {} {}",
                issue.id, issue.status, priority, issue.title
            );
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title(" bdtui - Issues (q:quit j/k:move Enter:detail) ")
                .borders(Borders::ALL),
        )
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .highlight_symbol("▶ ");

    let mut state = ListState::default();
    state.select(Some(app.selected));

    frame.render_stateful_widget(list, frame.area(), &mut state);
}

fn draw_detail(frame: &mut Frame, app: &App) {
    let Some(issue) = app.selected_issue() else {
        return;
    };

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

    frame.render_widget(paragraph, frame.area());
}
