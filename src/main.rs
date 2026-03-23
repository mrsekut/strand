mod app;
mod bd;
mod enrich;
mod ui;

use anyhow::Result;
use app::App;
use crossterm::{
    ExecutableCommand,
    event::{Event, EventStream, KeyCode},
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use futures::StreamExt;
use ratatui::prelude::*;
use std::io::stdout;

#[tokio::main]
async fn main() -> Result<()> {
    let dir = std::env::args()
        .position(|a| a == "--dir")
        .and_then(|i| std::env::args().nth(i + 1));

    let mut app = App::new(dir);
    app.load_issues().await?;

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

    loop {
        terminal.draw(|frame| ui::draw(frame, app))?;

        tokio::select! {
            maybe_event = event_stream.next() => {
                match maybe_event {
                    Some(Ok(Event::Key(key))) => {
                        match key.code {
                            KeyCode::Char('q') => break,
                            KeyCode::Char('j') | KeyCode::Down => app.next(),
                            KeyCode::Char('k') | KeyCode::Up => app.previous(),
                            KeyCode::Enter => app.toggle_detail(),
                            KeyCode::Char('e') => app.start_enrich(),
                            _ => {}
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
        }
    }
    Ok(())
}
