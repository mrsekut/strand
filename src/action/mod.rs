pub mod ai;
pub mod clipboard;
pub mod editor;
pub mod impl_ops;
pub mod navigate;
pub mod state;

use std::collections::HashSet;

use crate::app::App;
use crate::core::{ConfirmAction, View};
use crate::widget::keybar::{KeyBar, ToggleSelector, ToggleTarget};

/// strand 上の全操作を表現するデータ型。
/// キーハンドラはこれを返すだけ。実行は process_action() が行う。
#[derive(Debug, Clone)]
#[allow(dead_code)] // variant は段階的に使用される
pub enum AppAction {
    // ── Navigation ──
    Next,
    Previous,
    OpenDetail(String),
    OpenChildDetail(String),
    Back,
    NavigateIssue {
        forward: bool,
    },

    // ── KeyBar（セレクタ・確認） ──
    OpenSelector(SelectorDef),
    OpenConfirm(ConfirmAction),
    CloseKeyBar,
    SyncFilter,
    Confirm(ConfirmAction),

    // ── AI workflows ──
    StartEnrich(String),
    StartImplement {
        issue_id: String,
        epic_id: Option<String>,
    },
    StartSplit(String),

    // ── Impl operations ──
    MergeImpl(String),
    DiscardImpl(String),
    RetryImpl(String),
    MergeEpic(String),

    // ── State changes ──
    SetStatus {
        issue_id: String,
        status: String,
    },
    SetPriority {
        issue_id: String,
        priority: u8,
    },

    // ── Editor ──
    QuickCreate,
    EditDescription(String),

    // ── Clipboard ──
    CopyId(String),
    CopyResumeCommand(String),
    CopyLogCommand(String),
    CopyWorktreePath(String),

    // ── Filter ──
    ClearFilter,
    OpenFilterStatusToggle,
    OpenFilterLabelToggle,

    // ── System ──
    Notify(String),
    ReloadIssues,
}

#[derive(Debug, Clone)]
pub struct SelectorDef {
    pub items: Vec<SelectorItem>,
    pub initial_cursor: usize,
}

#[derive(Debug, Clone)]
pub struct SelectorItem {
    pub shortcut: String,
    pub label: String,
    pub action: AppAction,
}

/// AppAction を処理する。全操作のディスパッチャ。
pub async fn process_action(
    app: &mut App,
    action: AppAction,
    terminal: &mut ratatui::Terminal<ratatui::prelude::CrosstermBackend<std::io::Stdout>>,
) {
    match action {
        // ── Navigation ──
        AppAction::Next => navigate::next(&mut app.core),
        AppAction::Previous => navigate::previous(&mut app.core),
        AppAction::OpenDetail(_) => navigate::open_detail(app).await,
        AppAction::OpenChildDetail(_) => navigate::open_child_detail(app).await,
        AppAction::Back => navigate::back(&mut app.core),
        AppAction::NavigateIssue { forward } => navigate::navigate_issue(app, forward).await,

        // ── KeyBar（セレクタ・確認） ──
        AppAction::OpenSelector(def) => {
            app.core.keybar = KeyBar::open_selector(def);
        }
        AppAction::OpenConfirm(confirm) => {
            app.core.notification =
                Some((confirm.confirm_message().into(), std::time::Instant::now()));
            app.core.keybar = KeyBar::Confirm(confirm);
        }
        AppAction::CloseKeyBar => {
            app.core.keybar = KeyBar::Default;
            app.core.notification = None;
        }
        AppAction::SyncFilter => {
            sync_keybar_to_filter(app);
        }
        AppAction::Confirm(confirm) => {
            let issue_id = app.current_issue_id().unwrap_or_default();
            match confirm {
                ConfirmAction::Merge => {
                    impl_ops::merge_impl(app, &issue_id).await;
                    if matches!(&app.core.view, View::IssueDetail { .. }) {
                        navigate::back(&mut app.core);
                    }
                }
                ConfirmAction::Discard => impl_ops::discard_impl(app, &issue_id).await,
                ConfirmAction::MergeEpic => impl_ops::merge_epic(app, &issue_id).await,
                ConfirmAction::Retry => impl_ops::retry_impl(app, &issue_id).await,
            }
        }

        // ── AI workflows ──
        AppAction::StartEnrich(id) => ai::start_enrich(app, &id),
        AppAction::StartImplement { issue_id, epic_id } => {
            ai::start_implement(app, &issue_id, epic_id.as_deref()).await
        }
        AppAction::StartSplit(id) => ai::start_split(app, &id),

        // ── Impl operations ──
        AppAction::MergeImpl(id) => impl_ops::merge_impl(app, &id).await,
        AppAction::DiscardImpl(id) => impl_ops::discard_impl(app, &id).await,
        AppAction::RetryImpl(id) => impl_ops::retry_impl(app, &id).await,
        AppAction::MergeEpic(id) => impl_ops::merge_epic(app, &id).await,

        // ── State changes ──
        AppAction::SetStatus { issue_id, status } => {
            state::set_status(app, &issue_id, &status).await;
            if status == "closed" && matches!(&app.core.view, View::IssueDetail { .. }) {
                navigate::back(&mut app.core);
            }
        }
        AppAction::SetPriority { issue_id, priority } => {
            state::set_priority(app, &issue_id, priority).await;
        }

        // ── Editor ──
        AppAction::QuickCreate => editor::quick_create_with_editor(app, terminal).await,
        AppAction::EditDescription(id) => editor::edit_description(app, terminal, &id).await,

        // ── Clipboard ──
        AppAction::CopyId(id) => match crate::clipboard::copy(&id) {
            Ok(_) => app.notify(format!("Copied: {id}")),
            Err(e) => app.notify(format!("Copy failed: {e}")),
        },
        AppAction::CopyResumeCommand(id) => clipboard::copy_resume_command(app, &id),
        AppAction::CopyLogCommand(id) => clipboard::copy_log_command(app, &id),
        AppAction::CopyWorktreePath(id) => clipboard::copy_worktree_path(app, &id),

        // ── Filter ──
        AppAction::ClearFilter => {
            app.core.filter.clear();
            app.core.issue_store.selected = 0;
        }
        AppAction::OpenFilterStatusToggle => open_filter_status_toggle(app),
        AppAction::OpenFilterLabelToggle => open_filter_label_toggle(app),

        // ── System ──
        AppAction::Notify(msg) => app.notify(msg),
        AppAction::ReloadIssues => {
            let _ = app.load_issues().await;
        }
    }
}

fn sync_keybar_to_filter(app: &mut App) {
    if let KeyBar::Toggle(sel) = &app.core.keybar {
        let selected: HashSet<String> = sel
            .selected_labels()
            .into_iter()
            .map(|s| s.to_string())
            .collect();
        match sel.target {
            ToggleTarget::FilterStatus => app.core.filter.statuses = selected,
            ToggleTarget::FilterLabel => app.core.filter.labels = selected,
        }
    }
    app.core.issue_store.selected = 0;
}

fn open_filter_status_toggle(app: &mut App) {
    let items: Vec<(String, bool)> = crate::core::STATUSES
        .iter()
        .map(|s| (s.to_string(), app.core.filter.statuses.contains(*s)))
        .collect();
    app.core.keybar = KeyBar::Toggle(ToggleSelector::new(ToggleTarget::FilterStatus, items));
}

fn open_filter_label_toggle(app: &mut App) {
    app.core.filter.refresh_labels(&app.core.issue_store.issues);
    let items: Vec<(String, bool)> = app
        .core
        .filter
        .available_labels
        .iter()
        .map(|l| (l.clone(), app.core.filter.labels.contains(l)))
        .collect();
    app.core.keybar = KeyBar::Toggle(ToggleSelector::new(ToggleTarget::FilterLabel, items));
}
