use std::path::PathBuf;

use anyhow::Result;
use tokio::sync::mpsc;

use crate::ai::enrich::{self, EnrichManager};
use crate::ai::implement::{self, ImplManager};
use crate::ai::split::{self, SplitManager};
use crate::bd::{self, Issue};
use crate::core::{ConfirmAction, Core, View};
use crate::widget::keybar::KeyBar;

pub struct App {
    pub core: Core,
    pub enrich_manager: EnrichManager,
    pub enrich_rx: mpsc::Receiver<enrich::EnrichEvent>,
    pub impl_manager: ImplManager,
    pub impl_rx: mpsc::Receiver<implement::ImplEvent>,
    pub split_manager: SplitManager,
    pub split_rx: mpsc::Receiver<split::SplitEvent>,
    pub dir: Option<String>,
}

impl App {
    pub fn new(dir: Option<String>) -> Self {
        let (enrich_tx, enrich_rx) = mpsc::channel(32);
        let (impl_tx, impl_rx) = mpsc::channel(32);
        let (split_tx, split_rx) = mpsc::channel(32);
        Self {
            core: Core::new(),
            dir,
            enrich_manager: EnrichManager::new(enrich_tx),
            enrich_rx,
            impl_manager: ImplManager::new(impl_tx),
            impl_rx,
            split_manager: SplitManager::new(split_tx),
            split_rx,
        }
    }

    pub fn notify(&mut self, msg: impl Into<String>) {
        self.core.notification = Some((msg.into(), std::time::Instant::now()));
    }

    pub async fn load_issues(&mut self) -> Result<()> {
        self.core.issue_store.issues = bd::list_issues(self.dir.as_deref()).await?;
        self.core.issue_store.last_db_mtime =
            crate::core::IssueStore::db_mtime(&self.beads_db_path());
        Ok(())
    }

    pub async fn restore_impl_jobs(&mut self) {
        let repo_dir = self.repo_dir();
        let issue_ids: Vec<String> = self
            .core
            .issue_store
            .issues
            .iter()
            .map(|i| i.id.clone())
            .collect();
        self.impl_manager.restore_jobs(&repo_dir, &issue_ids).await;
    }

    pub fn repo_dir(&self) -> PathBuf {
        match &self.dir {
            Some(d) => PathBuf::from(d),
            None => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        }
    }

    fn beads_db_path(&self) -> PathBuf {
        self.repo_dir().join(".beads").join("beads.db")
    }

    pub fn has_db_changed(&self) -> bool {
        self.core.issue_store.has_db_changed(&self.beads_db_path())
    }

    pub fn displayed_issues(&self) -> Vec<&Issue> {
        self.core.issue_store.displayed_issues(&self.core.filter)
    }

    pub fn find_parent_epic_id(&self) -> Option<String> {
        self.core.find_parent_epic_id()
    }

    pub fn selected_issue(&self) -> Option<&Issue> {
        self.core.issue_store.selected_issue(&self.core.filter)
    }

    pub fn current_issue_id(&self) -> Option<String> {
        self.core.current_issue_id()
    }

    pub fn find_issue(&self, issue_id: &str) -> Option<Issue> {
        self.core.find_issue(issue_id)
    }

    pub fn all_children_closed(&self) -> bool {
        self.core.all_children_closed()
    }

    // --- Filter ---

    fn sync_keybar_to_filter(&mut self) {
        use crate::widget::keybar::ToggleTarget;
        if let KeyBar::Toggle(sel) = &self.core.keybar {
            let selected: std::collections::HashSet<String> = sel
                .selected_labels()
                .into_iter()
                .map(|s| s.to_string())
                .collect();
            match sel.target {
                ToggleTarget::FilterStatus => self.core.filter.statuses = selected,
                ToggleTarget::FilterLabel => self.core.filter.labels = selected,
            }
        }
        self.core.issue_store.selected = 0;
    }

    fn open_filter_status_toggle(&mut self) {
        use crate::widget::keybar::{ToggleSelector, ToggleTarget};
        let items: Vec<(String, bool)> = crate::core::STATUSES
            .iter()
            .map(|s| (s.to_string(), self.core.filter.statuses.contains(*s)))
            .collect();
        self.core.keybar = KeyBar::Toggle(ToggleSelector::new(ToggleTarget::FilterStatus, items));
    }

    fn open_filter_label_toggle(&mut self) {
        use crate::widget::keybar::{ToggleSelector, ToggleTarget};
        self.core
            .filter
            .refresh_labels(&self.core.issue_store.issues);
        let items: Vec<(String, bool)> = self
            .core
            .filter
            .available_labels
            .iter()
            .map(|l| (l.clone(), self.core.filter.labels.contains(l)))
            .collect();
        self.core.keybar = KeyBar::Toggle(ToggleSelector::new(ToggleTarget::FilterLabel, items));
    }

    /// AppAction を処理する。全操作のディスパッチ���。
    pub async fn process_action(
        &mut self,
        action: crate::action::AppAction,
        terminal: &mut ratatui::Terminal<ratatui::prelude::CrosstermBackend<std::io::Stdout>>,
    ) {
        use crate::action::AppAction;

        match action {
            // ── Navigation ──
            AppAction::Next => crate::action::navigate::next(&mut self.core),
            AppAction::Previous => crate::action::navigate::previous(&mut self.core),
            AppAction::OpenDetail(_) => crate::action::navigate::open_detail(self).await,
            AppAction::OpenChildDetail(_) => crate::action::navigate::open_child_detail(self).await,
            AppAction::Back => crate::action::navigate::back(&mut self.core),
            AppAction::NavigateIssue { forward } => {
                crate::action::navigate::navigate_issue(self, forward).await
            }

            // ── KeyBar（セレクタ・確認） ──
            AppAction::OpenSelector(def) => {
                self.core.keybar = KeyBar::open_selector(def);
            }
            AppAction::OpenConfirm(confirm) => {
                self.core.notification =
                    Some((confirm.confirm_message().into(), std::time::Instant::now()));
                self.core.keybar = KeyBar::Confirm(confirm);
            }
            AppAction::CloseKeyBar => {
                self.core.keybar = KeyBar::Default;
                self.core.notification = None;
            }
            AppAction::SyncFilter => {
                self.sync_keybar_to_filter();
            }
            AppAction::Confirm(confirm) => {
                let issue_id = self.current_issue_id().unwrap_or_default();
                match confirm {
                    ConfirmAction::Merge => {
                        crate::action::impl_ops::merge_impl(self, &issue_id).await;
                        if matches!(&self.core.view, View::IssueDetail { .. }) {
                            crate::action::navigate::back(&mut self.core);
                        }
                    }
                    ConfirmAction::Discard => {
                        crate::action::impl_ops::discard_impl(self, &issue_id).await
                    }
                    ConfirmAction::MergeEpic => {
                        crate::action::impl_ops::merge_epic(self, &issue_id).await
                    }
                    ConfirmAction::Retry => {
                        crate::action::impl_ops::retry_impl(self, &issue_id).await
                    }
                }
            }

            // ── AI workflows ──
            AppAction::StartEnrich(id) => crate::action::ai::start_enrich(self, &id),
            AppAction::StartImplement { issue_id, epic_id } => {
                crate::action::ai::start_implement(self, &issue_id, epic_id.as_deref()).await
            }
            AppAction::StartSplit(id) => crate::action::ai::start_split(self, &id),

            // ── Impl operations ──
            AppAction::MergeImpl(id) => crate::action::impl_ops::merge_impl(self, &id).await,
            AppAction::DiscardImpl(id) => crate::action::impl_ops::discard_impl(self, &id).await,
            AppAction::RetryImpl(id) => crate::action::impl_ops::retry_impl(self, &id).await,
            AppAction::MergeEpic(id) => crate::action::impl_ops::merge_epic(self, &id).await,

            // ── State changes ──
            AppAction::SetStatus { issue_id, status } => {
                crate::action::state::set_status(self, &issue_id, &status).await;
                if status == "closed" && matches!(&self.core.view, View::IssueDetail { .. }) {
                    crate::action::navigate::back(&mut self.core);
                }
            }
            AppAction::SetPriority { issue_id, priority } => {
                crate::action::state::set_priority(self, &issue_id, priority).await;
            }

            // ── Editor ──
            AppAction::QuickCreate => {
                crate::action::editor::quick_create_with_editor(self, terminal).await
            }
            AppAction::EditDescription(id) => {
                crate::action::editor::edit_description(self, terminal, &id).await
            }

            // ── Clipboard ──
            AppAction::CopyId(id) => match crate::clipboard::copy(&id) {
                Ok(_) => self.notify(format!("Copied: {id}")),
                Err(e) => self.notify(format!("Copy failed: {e}")),
            },
            AppAction::CopyResumeCommand(id) => {
                crate::action::clipboard::copy_resume_command(self, &id)
            }
            AppAction::CopyLogCommand(id) => crate::action::clipboard::copy_log_command(self, &id),
            AppAction::CopyWorktreePath(id) => {
                crate::action::clipboard::copy_worktree_path(self, &id)
            }

            // ── Filter ──
            AppAction::ClearFilter => {
                self.core.filter.clear();
                self.core.issue_store.selected = 0;
            }
            AppAction::OpenFilterStatusToggle => self.open_filter_status_toggle(),
            AppAction::OpenFilterLabelToggle => self.open_filter_label_toggle(),

            // ── System ──
            AppAction::Notify(msg) => self.notify(msg),
            AppAction::ReloadIssues => {
                let _ = self.load_issues().await;
            }
        }
    }
}
