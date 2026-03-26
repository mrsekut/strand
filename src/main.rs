mod app;
mod bd;
mod enrich;
mod implement;
mod ui;

use std::io::stdout;
use std::time::Duration;

use anyhow::Result;
use app::{App, InputMode};
use crossterm::{
    ExecutableCommand,
    event::{Event, EventStream, KeyCode},
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use futures::StreamExt;
use ratatui::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    let dir = std::env::args()
        .position(|a| a == "--dir")
        .and_then(|i| std::env::args().nth(i + 1));

    bd::check_init(dir.as_deref())?;

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
                        if key.code == KeyCode::Char('q') {
                            break;
                        }
                        if app.show_detail {
                            handle_detail_key(key.code, app, terminal).await;
                        } else {
                            handle_list_key(key.code, app).await;
                        }
                    }
                    Some(Ok(_)) => {} // リサイズ等のイベントは無視
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
            _ = poll_interval.tick() => {
                if app.has_db_changed() {
                    let _ = app.load_issues().await;
                    app.auto_enrich();
                }
            }
        }
    }
    Ok(())
}

async fn handle_list_key(key: KeyCode, app: &mut App) {
    match app.input_mode {
        InputMode::AwaitingAI => {
            app.input_mode = InputMode::Normal;
            app.notification = None;
            match key {
                KeyCode::Char('e') => app.start_enrich(),
                KeyCode::Char('i') => app.start_implement(),
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
        InputMode::AwaitingCloseConfirm => {
            app.input_mode = InputMode::Normal;
            app.notification = None;
            if let KeyCode::Char('y') = key {
                app.close_issue().await;
            }
        }
        InputMode::Normal => match key {
            KeyCode::Down => app.next(),
            KeyCode::Up => app.previous(),
            KeyCode::Enter => app.open_detail().await,
            KeyCode::Char('c') => app.copy_id(),
            KeyCode::Char('a') => {
                app.input_mode = InputMode::AwaitingAI;
                app.notification = Some(("a-...".into(), std::time::Instant::now()));
            }
            KeyCode::Char('x') => {
                app.input_mode = InputMode::AwaitingCloseConfirm;
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

async fn handle_detail_key(
    key: KeyCode,
    app: &mut App,
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
) {
    if app.input_mode == InputMode::AwaitingCloseConfirm {
        app.input_mode = InputMode::Normal;
        app.notification = None;
        if let KeyCode::Char('y') = key {
            app.close_issue().await;
        }
        return;
    }

    match key {
        KeyCode::Esc => app.back_to_list(),
        KeyCode::Down => app.next(),
        KeyCode::Up => app.previous(),
        KeyCode::Char('c') => app.copy_id(),
        KeyCode::Char('p') => app.copy_worktree_path(),
        KeyCode::Char('e') => app.edit_description(terminal).await,
        KeyCode::Char('m') => app.merge_impl().await,
        KeyCode::Char('d') => app.discard_impl().await,
        KeyCode::Char('x') => {
            app.input_mode = InputMode::AwaitingCloseConfirm;
            app.notification = Some(("Close? (y/n)".into(), std::time::Instant::now()));
        }
        _ => {}
    }
}
