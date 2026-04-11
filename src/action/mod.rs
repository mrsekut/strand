pub mod ai;
pub mod clipboard;
pub mod editor;
pub mod impl_ops;
pub mod navigate;
pub mod state;

use std::collections::HashSet;

use crate::app::App;
use crate::core::{ConfirmAction, Core, View};
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
    let core = &mut app.core;
    let ai = &mut app.ai;

    match action {
        // ── Navigation ──
        AppAction::Next => navigate::next(core),
        AppAction::Previous => navigate::previous(core),
        AppAction::OpenDetail(_) => navigate::open_detail(core, ai).await,
        AppAction::OpenChildDetail(_) => navigate::open_child_detail(core, ai).await,
        AppAction::Back => navigate::back(core),
        AppAction::NavigateIssue { forward } => navigate::navigate_issue(core, ai, forward).await,

        // ── KeyBar（セレクタ・確認） ──
        AppAction::OpenSelector(def) => {
            core.keybar = KeyBar::open_selector(def);
        }
        AppAction::OpenConfirm(confirm) => {
            core.notification = Some((confirm.confirm_message().into(), std::time::Instant::now()));
            core.keybar = KeyBar::Confirm(confirm);
        }
        AppAction::CloseKeyBar => {
            core.keybar = KeyBar::Default;
            core.notification = None;
        }
        AppAction::SyncFilter => {
            sync_keybar_to_filter(core);
        }
        AppAction::Confirm(confirm) => {
            let issue_id = core.current_issue_id().unwrap_or_default();
            match confirm {
                ConfirmAction::Merge => {
                    impl_ops::merge_impl(core, ai, &issue_id).await;
                    if matches!(&core.view, View::IssueDetail { .. }) {
                        navigate::back(core);
                    }
                }
                ConfirmAction::Discard => impl_ops::discard_impl(core, ai, &issue_id).await,
                ConfirmAction::MergeEpic => impl_ops::merge_epic(core, ai, &issue_id).await,
                ConfirmAction::Retry => impl_ops::retry_impl(core, ai, &issue_id).await,
            }
        }

        // ── AI workflows ──
        AppAction::StartEnrich(id) => ai::start_enrich(core, ai, &id),
        AppAction::StartImplement { issue_id, epic_id } => {
            ai::start_implement(core, ai, &issue_id, epic_id.as_deref()).await
        }
        AppAction::StartSplit(id) => ai::start_split(core, ai, &id),

        // ── Impl operations ──
        AppAction::MergeImpl(id) => impl_ops::merge_impl(core, ai, &id).await,
        AppAction::DiscardImpl(id) => impl_ops::discard_impl(core, ai, &id).await,
        AppAction::RetryImpl(id) => impl_ops::retry_impl(core, ai, &id).await,
        AppAction::MergeEpic(id) => impl_ops::merge_epic(core, ai, &id).await,

        // ── State changes ──
        AppAction::SetStatus { issue_id, status } => {
            state::set_status(core, &issue_id, &status).await;
            if status == "closed" && matches!(&core.view, View::IssueDetail { .. }) {
                navigate::back(core);
            }
        }
        AppAction::SetPriority { issue_id, priority } => {
            state::set_priority(core, &issue_id, priority).await;
        }

        // ── Editor ──
        AppAction::QuickCreate => editor::quick_create_with_editor(core, ai, terminal).await,
        AppAction::EditDescription(id) => editor::edit_description(core, terminal, &id).await,

        // ── Clipboard ──
        AppAction::CopyId(id) => match crate::clipboard::copy(&id) {
            Ok(_) => core.notify(format!("Copied: {id}")),
            Err(e) => core.notify(format!("Copy failed: {e}")),
        },
        AppAction::CopyResumeCommand(id) => clipboard::copy_resume_command(core, ai, &id),
        AppAction::CopyLogCommand(id) => clipboard::copy_log_command(core, ai, &id),
        AppAction::CopyWorktreePath(id) => clipboard::copy_worktree_path(core, ai, &id),

        // ── Filter ──
        AppAction::ClearFilter => {
            core.filter.clear();
            core.issue_store.selected = 0;
        }
        AppAction::OpenFilterStatusToggle => open_filter_status_toggle(core),
        AppAction::OpenFilterLabelToggle => open_filter_label_toggle(core),

        // ── System ──
        AppAction::Notify(msg) => core.notify(msg),
        AppAction::ReloadIssues => {
            let _ = core.load_issues().await;
        }
    }
}

fn sync_keybar_to_filter(core: &mut Core) {
    if let KeyBar::Toggle(sel) = &core.keybar {
        let selected: HashSet<String> = sel
            .selected_labels()
            .into_iter()
            .map(|s| s.to_string())
            .collect();
        match sel.target {
            ToggleTarget::FilterStatus => core.filter.statuses = selected,
            ToggleTarget::FilterLabel => core.filter.labels = selected,
        }
    }
    core.issue_store.selected = 0;
}

fn open_filter_status_toggle(core: &mut Core) {
    let items: Vec<(String, bool)> = crate::core::STATUSES
        .iter()
        .map(|s| (s.to_string(), core.filter.statuses.contains(*s)))
        .collect();
    core.keybar = KeyBar::Toggle(ToggleSelector::new(ToggleTarget::FilterStatus, items));
}

fn open_filter_label_toggle(core: &mut Core) {
    core.filter.refresh_labels(&core.issue_store.issues);
    let items: Vec<(String, bool)> = core
        .filter
        .available_labels
        .iter()
        .map(|l| (l.clone(), core.filter.labels.contains(l)))
        .collect();
    core.keybar = KeyBar::Toggle(ToggleSelector::new(ToggleTarget::FilterLabel, items));
}
