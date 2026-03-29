mod app;
mod bd;
mod enrich;
mod implement;
mod split;
mod ui;

use std::io::stdout;
use std::time::Duration;

use anyhow::Result;
use app::{App, ConfirmAction, InputMode, View};
use crossterm::{
    ExecutableCommand,
    event::{Event, EventStream, KeyCode},
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use futures::StreamExt;
use ratatui::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    let dir = args
        .iter()
        .position(|a| a == "--dir")
        .and_then(|i| args.get(i + 1))
        .cloned();

    // strand q "title" — quick capture (epic, P2)
    if args.get(1).map(|s| s.as_str()) == Some("q") {
        let title = args
            .get(2)
            .ok_or_else(|| anyhow::anyhow!("Usage: strand q <title>"))?;
        bd::check_init(dir.as_deref()).await?;
        let id = bd::quick_create(dir.as_deref(), title).await?;
        println!("{id}");
        return Ok(());
    }

    bd::check_init(dir.as_deref()).await?;

    let mut app = App::new(dir);
    app.load_issues().await?;
    app.restore_impl_jobs().await;
    app.auto_enrich();

    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    let result = run(&mut terminal, &mut app).await;

    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;

    result
}

async fn run(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    app: &mut App,
) -> Result<()> {
    let mut event_stream = EventStream::new();
    let mut poll_interval = tokio::time::interval(Duration::from_secs(2));
    poll_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        terminal.draw(|frame| ui::draw(frame, app))?;

        tokio::select! {
            maybe_event = event_stream.next() => {
                match maybe_event {
                    Some(Ok(Event::Key(key))) => {
                        if key.code == KeyCode::Char('q') && app.input_mode == InputMode::Normal {
                            break;
                        }
                        match &app.view {
                            View::IssueList => handle_issue_list_key(key.code, app).await,
                            View::IssueDetail { .. } => handle_issue_detail_key(key.code, app, terminal).await,
                            View::EpicDetail { .. } => handle_epic_detail_key(key.code, app, terminal).await,
                            View::ChildDetail { .. } => handle_child_detail_key(key.code, app, terminal).await,
                        }
                    }
                    Some(Ok(_)) => {}
                    Some(Err(_)) => break,
                    None => break,
                }
            }
            Some(event) = app.enrich_rx.recv() => {
                app.handle_enrich_event(event).await;
            }
            Some(event) = app.impl_rx.recv() => {
                app.handle_impl_event(event);
            }
            Some(event) = app.split_rx.recv() => {
                app.handle_split_event(event).await;
            }
            _ = poll_interval.tick() => {
                if app.has_db_changed() {
                    let _ = app.load_issues().await;
                    app.auto_enrich();
                    // Also reload children if we're in epic detail
                    app.reload_children().await;
                }
            }
        }
    }
    Ok(())
}

// --- Epic List ---

async fn handle_issue_list_key(key: KeyCode, app: &mut App) {
    match app.input_mode {
        InputMode::AwaitingAI => {
            app.input_mode = InputMode::Normal;
            app.notification = None;
            match key {
                KeyCode::Char('e') => app.start_enrich(),
                KeyCode::Char('i') => app.start_implement().await,
                KeyCode::Char('s') => app.start_split(),
                _ => {}
            }
        }
        InputMode::AwaitingPriority => {
            app.input_mode = InputMode::Normal;
            app.notification = None;
            if let KeyCode::Char(c @ '0'..='4') = key {
                app.set_priority(c as u8 - b'0').await;
            }
        }
        InputMode::AwaitingConfirm(action) => {
            app.input_mode = InputMode::Normal;
            app.notification = None;
            if let KeyCode::Char('y') = key {
                match action {
                    ConfirmAction::Close => app.close_issue().await,
                    ConfirmAction::Merge => app.merge_impl().await,
                    ConfirmAction::Discard => app.discard_impl().await,
                    ConfirmAction::MergeEpic => app.merge_epic().await,
                }
            }
        }
        InputMode::Normal => match key {
            KeyCode::Down | KeyCode::Char('j') => app.next(),
            KeyCode::Up | KeyCode::Char('k') => app.previous(),
            KeyCode::Enter => app.open_detail().await,
            KeyCode::Char('c') => app.copy_id(),
            KeyCode::Char('a') => {
                app.input_mode = InputMode::AwaitingAI;
                app.notification = Some(("a-...".into(), std::time::Instant::now()));
            }
            KeyCode::Char('x') => {
                app.input_mode = InputMode::AwaitingConfirm(ConfirmAction::Close);
                app.notification = Some(("Close? (y/n)".into(), std::time::Instant::now()));
            }
            KeyCode::Char('p') => {
                app.input_mode = InputMode::AwaitingPriority;
                app.notification = Some(("p-...".into(), std::time::Instant::now()));
            }
            _ => {}
        },
    }
}

// --- Epic Detail ---

async fn handle_epic_detail_key(
    key: KeyCode,
    app: &mut App,
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
) {
    match app.input_mode {
        InputMode::AwaitingAI => {
            app.input_mode = InputMode::Normal;
            app.notification = None;
            if let KeyCode::Char('e') = key {
                app.start_enrich();
            }
            return;
        }
        InputMode::AwaitingConfirm(action) => {
            app.input_mode = InputMode::Normal;
            app.notification = None;
            if let KeyCode::Char('y') = key {
                match action {
                    ConfirmAction::Close => app.close_issue().await,
                    ConfirmAction::Merge => app.merge_impl().await,
                    ConfirmAction::Discard => app.discard_impl().await,
                    ConfirmAction::MergeEpic => app.merge_epic().await,
                }
            }
            return;
        }
        _ => {}
    }

    match key {
        KeyCode::Esc => app.back(),
        KeyCode::Down | KeyCode::Char('j') => app.next(),
        KeyCode::Up | KeyCode::Char('k') => app.previous(),
        KeyCode::Enter => app.open_child_detail().await,
        KeyCode::Char('c') => app.copy_id(),
        KeyCode::Char('e') => app.edit_description(terminal).await,
        KeyCode::Char('a') => {
            app.input_mode = InputMode::AwaitingAI;
            app.notification = Some(("a-...".into(), std::time::Instant::now()));
        }
        KeyCode::Char('m') if app.all_children_closed() => {
            app.input_mode = InputMode::AwaitingConfirm(ConfirmAction::MergeEpic);
            app.notification = Some(("Merge epic to master? (y/n)".into(), std::time::Instant::now()));
        }
        KeyCode::Char('x') => {
            app.input_mode = InputMode::AwaitingConfirm(ConfirmAction::Close);
            app.notification = Some(("Close? (y/n)".into(), std::time::Instant::now()));
        }
        _ => {}
    }
}

// --- Standalone Issue Detail (子なしissue) ---

async fn handle_issue_detail_key(
    key: KeyCode,
    app: &mut App,
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
) {
    match app.input_mode {
        InputMode::AwaitingAI => {
            app.input_mode = InputMode::Normal;
            app.notification = None;
            match key {
                KeyCode::Char('e') => app.start_enrich(),
                KeyCode::Char('i') => app.start_implement().await,
                KeyCode::Char('s') => app.start_split(),
                _ => {}
            }
            return;
        }
        InputMode::AwaitingConfirm(action) => {
            app.input_mode = InputMode::Normal;
            app.notification = None;
            if let KeyCode::Char('y') = key {
                match action {
                    ConfirmAction::Close => {
                        app.close_issue().await;
                        app.back();
                    }
                    ConfirmAction::Merge => {
                        app.merge_impl().await;
                        app.back();
                    }
                    ConfirmAction::Discard => app.discard_impl().await,
                    _ => {}
                }
            }
            return;
        }
        _ => {}
    }

    match key {
        KeyCode::Esc => app.back(),
        KeyCode::Down => app.next(),
        KeyCode::Up => app.previous(),
        KeyCode::Char('c') => app.copy_id(),
        KeyCode::Char('p') => app.copy_worktree_path(),
        KeyCode::Char('e') => app.edit_description(terminal).await,
        KeyCode::Char('m') => {
            app.input_mode = InputMode::AwaitingConfirm(ConfirmAction::Merge);
            app.notification = Some(("Merge? (y/n)".into(), std::time::Instant::now()));
        }
        KeyCode::Char('d') => {
            app.input_mode = InputMode::AwaitingConfirm(ConfirmAction::Discard);
            app.notification = Some(("Discard? (y/n)".into(), std::time::Instant::now()));
        }
        KeyCode::Char('a') => {
            app.input_mode = InputMode::AwaitingAI;
            app.notification = Some(("a-...".into(), std::time::Instant::now()));
        }
        KeyCode::Char('x') => {
            app.input_mode = InputMode::AwaitingConfirm(ConfirmAction::Close);
            app.notification = Some(("Close? (y/n)".into(), std::time::Instant::now()));
        }
        _ => {}
    }
}

// --- Child Detail (epic配下のissue) ---

async fn handle_child_detail_key(
    key: KeyCode,
    app: &mut App,
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
) {
    match app.input_mode {
        InputMode::AwaitingAI => {
            app.input_mode = InputMode::Normal;
            app.notification = None;
            match key {
                KeyCode::Char('e') => app.start_enrich(),
                KeyCode::Char('i') => app.start_implement().await,
                _ => {}
            }
            return;
        }
        InputMode::AwaitingConfirm(action) => {
            app.input_mode = InputMode::Normal;
            app.notification = None;
            if let KeyCode::Char('y') = key {
                match action {
                    ConfirmAction::Close => {
                        app.close_issue().await;
                        app.back();
                    }
                    ConfirmAction::Merge => {
                        app.merge_impl().await;
                        app.back();
                    }
                    ConfirmAction::Discard => app.discard_impl().await,
                    ConfirmAction::MergeEpic => app.merge_epic().await,
                }
            }
            return;
        }
        _ => {}
    }

    match key {
        KeyCode::Esc => app.back(),
        KeyCode::Down => app.next(),
        KeyCode::Up => app.previous(),
        KeyCode::Char('c') => app.copy_id(),
        KeyCode::Char('p') => app.copy_worktree_path(),
        KeyCode::Char('e') => app.edit_description(terminal).await,
        KeyCode::Char('m') => {
            app.input_mode = InputMode::AwaitingConfirm(ConfirmAction::Merge);
            app.notification = Some(("Merge? (y/n)".into(), std::time::Instant::now()));
        }
        KeyCode::Char('d') => {
            app.input_mode = InputMode::AwaitingConfirm(ConfirmAction::Discard);
            app.notification = Some(("Discard? (y/n)".into(), std::time::Instant::now()));
        }
        KeyCode::Char('a') => {
            app.input_mode = InputMode::AwaitingAI;
            app.notification = Some(("a-...".into(), std::time::Instant::now()));
        }
        KeyCode::Char('x') => {
            app.input_mode = InputMode::AwaitingConfirm(ConfirmAction::Close);
            app.notification = Some(("Close? (y/n)".into(), std::time::Instant::now()));
        }
        _ => {}
    }
}
