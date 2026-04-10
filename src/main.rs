mod action;
mod ai;
mod app;
mod bd;
mod clipboard;
mod editor;
mod filter;
mod overlay;
mod page;
mod selector;
mod ui;
mod widget;

use std::io::stdout;
use std::time::Duration;

use anyhow::Result;
use app::{App, View};
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
    --dir <path>    Set working directory
    -h, --help      Show this help

ENVIRONMENT VARIABLES:
    STRAND_ENRICH_SKILL    Agent skill name to use for enrich (default: built-in problem/solution analysis)
                           Example: STRAND_ENRICH_SKILL=my-analysis-skill strand"
    );
}

/// Overlay から返された AppAction を実行する。
/// Step 3 で App::process_action() に統合予定。
async fn execute_action(
    app: &mut App,
    action: action::AppAction,
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
) {
    use action::AppAction;
    match action {
        AppAction::Confirm(confirm) => match confirm {
            app::ConfirmAction::Merge => {
                app.merge_impl().await;
                // IssueDetail にいる場合は back
                if matches!(&app.view, View::IssueDetail { .. }) {
                    app.back();
                }
            }
            app::ConfirmAction::Discard => app.discard_impl().await,
            app::ConfirmAction::MergeEpic => app.merge_epic().await,
            app::ConfirmAction::Retry => app.retry_impl().await,
        },
        AppAction::StartEnrich(id) => {
            // current_issue を探して start_enrich を呼ぶ
            app.start_enrich();
        }
        AppAction::StartImplement { .. } => {
            app.start_implement().await;
        }
        AppAction::StartSplit(_) => {
            app.start_split();
        }
        AppAction::SetStatus { issue_id, status } => {
            app.set_status(&status).await;
            // IssueDetail で closed にした場合は back
            if status == "closed" && matches!(&app.view, View::IssueDetail { .. }) {
                app.back();
            }
        }
        AppAction::SetPriority { issue_id, priority } => {
            app.set_priority(priority).await;
        }
        AppAction::ClearFilter => {
            app.filter.clear();
            app.selected = 0;
        }
        AppAction::OpenFilterStatusToggle => {
            overlay::open_filter_status_toggle(app);
        }
        AppAction::OpenFilterLabelToggle => {
            overlay::open_filter_label_toggle(app);
        }
        AppAction::QuickCreate => {
            app.quick_create_with_editor(terminal).await;
        }
        _ => {}
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
        terminal.draw(|frame| ui::draw(frame, app))?;

        tokio::select! {
            maybe_event = event_stream.next() => {
                match maybe_event {
                    Some(Ok(Event::Key(key))) => {
                        if key.code == KeyCode::Char('c') && key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) {
                            break;
                        }
                        // Overlay が先にキーを消費
                        let outcome = overlay::handle_overlay_key(key.code, app);
                        match outcome {
                            overlay::OverlayOutcome::NotConsumed => {
                                // ページにキーを渡す
                                match &app.view {
                                    View::IssueList => page::issue_list::keys::handle_key(key.code, app, terminal).await,
                                    View::IssueDetail { .. } => page::issue_detail::keys::handle_key(key.code, app, terminal).await,
                                    View::EpicDetail { .. } => page::epic_detail::keys::handle_key(key.code, app, terminal).await,
                                }
                            }
                            overlay::OverlayOutcome::Consumed => {}
                            overlay::OverlayOutcome::Action(action) => {
                                execute_action(app, action, terminal).await;
                            }
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
