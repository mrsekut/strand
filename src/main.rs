mod action;
mod ai;
mod app;
mod bd;
mod clipboard;
mod config;
mod core;
mod editor;
mod page;
mod ui;
mod widget;

use std::io::stdout;
use std::time::Duration;

use anyhow::Result;
use app::App;
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

    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return Ok(());
    }

    check_prerequisites()?;

    // strand q "title" — quick capture (epic, P2)
    if args.get(1).map(|s| s.as_str()) == Some("q") {
        let title = args
            .get(2)
            .ok_or_else(|| anyhow::anyhow!("Usage: strand q <title>"))?;
        bd::check_init(None).await?;
        let id = bd::quick_create(None, title, "").await?;
        println!("{id}");
        return Ok(());
    }

    bd::check_init(None).await?;

    let mut app = App::new();
    app.core.load_issues().await?;
    app.ai.restore_jobs().await;
    action::ai::auto_enrich(&app.core, &mut app.ai);

    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    let result = run(&mut terminal, &mut app).await;

    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;

    result
}

fn check_prerequisites() -> Result<()> {
    let mut missing = Vec::new();

    if which("bd").is_none() {
        missing.push("  ✗ bd (beads CLI) — https://github.com/steveyegge/beads");
    }
    if which("claude").is_none() {
        missing.push("  ✗ claude (Claude Code CLI) — https://claude.ai/claude-code");
    }

    if !missing.is_empty() {
        anyhow::bail!(
            "Required dependencies not found:\n{}\n\nInstall them and try again.",
            missing.join("\n")
        );
    }
    Ok(())
}

fn which(cmd: &str) -> Option<std::path::PathBuf> {
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths).find_map(|dir| {
            let path = dir.join(cmd);
            path.is_file().then_some(path)
        })
    })
}

fn print_help() {
    println!(
        "\
strand — AI-powered issue management TUI

USAGE:
    strand [OPTIONS]
    strand q <title>

COMMANDS:
    q <title>    Quick-capture an issue

OPTIONS:
    -h, --help      Show this help

CONFIG:
    .strand/config.toml

    [enrich]
    skill = \"my-analysis-skill\"  # Agent skill name for enrich (default: built-in)"
    );
}

/// Layer に応じてキーハンドラを呼ぶ
fn dispatch_key(key: KeyCode, app: &mut App) -> Vec<action::AppAction> {
    use core::Layer;
    match app.core.layer() {
        Layer::KeyBar => app.core.keybar.handle_key(key),
        Layer::IssueList => page::issue_list::keys::handle_key(key, &app.core),
        Layer::IssueDetail => page::issue_detail::keys::handle_key(key, &app.core),
        Layer::EpicDetail => page::epic_detail::keys::handle_key(key, &app.core),
    }
}

async fn run(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    app: &mut App,
) -> Result<()> {
    let mut event_stream = EventStream::new();
    let mut poll_interval = tokio::time::interval(Duration::from_secs(2));
    poll_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        terminal.draw(|frame| ui::draw(frame, &app.core, &app.ai))?;

        tokio::select! {
            maybe_event = event_stream.next() => {
                match maybe_event {
                    Some(Ok(Event::Key(key))) => {
                        if key.code == KeyCode::Char('c') && key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) {
                            break;
                        }
                        let actions = dispatch_key(key.code, app);
                        for action in actions {
                            action::process_action(app, action, terminal).await;
                        }
                    }
                    Some(Ok(_)) => {}
                    Some(Err(_)) => break,
                    None => break,
                }
            }
            Some(event) = app.enrich_rx.recv() => {
                action::ai::handle_enrich_event(&mut app.core, &mut app.ai, event).await;
            }
            Some(event) = app.impl_rx.recv() => {
                action::ai::handle_impl_event(&mut app.core, &mut app.ai, event);
            }
            Some(event) = app.split_rx.recv() => {
                action::ai::handle_split_event(&mut app.core, &mut app.ai, event).await;
            }
            _ = poll_interval.tick() => {
                if app.core.has_db_changed() {
                    let _ = app.core.load_issues().await;
                    action::ai::auto_enrich(&app.core, &mut app.ai);
                    action::navigate::reload_children(&mut app.core).await;
                }
            }
        }
    }
    Ok(())
}
