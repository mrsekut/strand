mod app;
mod bd;
mod ui;

use anyhow::Result;
use app::App;
use crossterm::{
    event::{self, Event, KeyCode},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::prelude::*;
use std::io::stdout;

#[tokio::main]
async fn main() -> Result<()> {
    let mut app = App::new();
    app.load_issues().await?;

    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    let result = run(&mut terminal, &mut app).await;

    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;

    result
}

async fn run(terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>, app: &mut App) -> Result<()> {
    loop {
        terminal.draw(|frame| ui::draw(frame, app))?;

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Char('j') | KeyCode::Down => app.next(),
                    KeyCode::Char('k') | KeyCode::Up => app.previous(),
                    KeyCode::Enter => app.toggle_detail(),
                    _ => {}
                }
            }
        }
    }
    Ok(())
}
