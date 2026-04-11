use std::path::PathBuf;

use anyhow::Result;
use ratatui::prelude::*;
use tokio::sync::mpsc;

use crate::ai::enrich::{self, EnrichManager, EnrichOutcome};
use crate::ai::implement::{self, ImplManager, ImplOutcome, ImplStatus};
use crate::ai::split::{self, SplitManager, SplitOutcome};
use crate::bd::{self, Issue};
use crate::core::{ConfirmAction, Core, Overlay, View};

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

    fn beads_db_path(&self) -> PathBuf {
        self.repo_dir().join(".beads").join("beads.db")
    }

    pub fn has_db_changed(&self) -> bool {
        self.core.issue_store.has_db_changed(&self.beads_db_path())
    }

    pub fn displayed_issues(&self) -> Vec<&Issue> {
        self.core.issue_store.displayed_issues(&self.core.filter)
    }

    pub fn next(&mut self) {
        match &mut self.core.view {
            View::IssueList => {
                let len = self.displayed_issues().len();
                if len > 0 {
                    self.core.issue_store.selected =
                        (self.core.issue_store.selected + 1).min(len - 1);
                }
            }
            View::EpicDetail {
                children,
                child_selected,
                ..
            } => {
                if !children.is_empty() {
                    *child_selected = (*child_selected + 1).min(children.len() - 1);
                }
            }
            View::IssueDetail { scroll_offset, .. } => {
                *scroll_offset = scroll_offset.saturating_add(1);
            }
        }
    }

    pub fn previous(&mut self) {
        match &mut self.core.view {
            View::IssueList => {
                let len = self.displayed_issues().len();
                if len > 0 {
                    self.core.issue_store.selected =
                        self.core.issue_store.selected.saturating_sub(1);
                }
            }
            View::EpicDetail { child_selected, .. } => {
                *child_selected = child_selected.saturating_sub(1);
            }
            View::IssueDetail { scroll_offset, .. } => {
                *scroll_offset = scroll_offset.saturating_sub(1);
            }
        }
    }

    // --- Navigation ---

    /// IssueListからissueを開く
    pub async fn open_detail(&mut self) {
        let Some(issue) = self.selected_issue() else {
            return;
        };
        let issue_id = issue.id.clone();

        // Clear unread label
        if issue.labels.contains(&"strand-unread".to_string()) {
            if let Some(issue) = self
                .core
                .issue_store
                .issues
                .get_mut(self.core.issue_store.selected)
            {
                issue.labels.retain(|l| l != "strand-unread");
            }
            let id = issue_id.clone();
            let dir = self.dir.clone();
            tokio::spawn(async move {
                let _ = bd::remove_label(dir.as_deref(), &id, "strand-unread").await;
            });
        }

        self.push_view_for_issue(&issue_id).await;
    }

    /// EpicDetailから子issueを開く
    pub async fn open_child_detail(&mut self) {
        let issue_id = match &self.core.view {
            View::EpicDetail {
                children,
                child_selected,
                ..
            } => children.get(*child_selected).map(|i| i.id.clone()),
            _ => None,
        };
        let Some(issue_id) = issue_id else { return };

        self.push_view_for_issue(&issue_id).await;
    }

    /// issue_idに応じてIssueDetail or EpicDetailをスタックにpush
    async fn push_view_for_issue(&mut self, issue_id: &str) {
        let children = bd::list_children(self.dir.as_deref(), issue_id)
            .await
            .unwrap_or_default();

        let old = std::mem::replace(&mut self.core.view, View::IssueList);
        self.core.view_stack.push(old);

        if children.is_empty() {
            self.core.view = View::IssueDetail {
                issue_id: issue_id.to_string(),
                scroll_offset: 0,
                diff: None,
            };
            self.load_issue_detail_diff(issue_id).await;
        } else {
            let ready_ids = bd::list_ready_ids(self.dir.as_deref(), issue_id)
                .await
                .unwrap_or_default();
            self.core.view = View::EpicDetail {
                epic_id: issue_id.to_string(),
                children,
                ready_ids,
                child_selected: 0,
                scroll_offset: 0,
            };
        }
    }

    pub fn back(&mut self) {
        if let Some(prev) = self.core.view_stack.pop() {
            self.core.view = prev;
        }
    }

    /// IssueDetail内で次/前のissueに移動する
    pub async fn navigate_issue(&mut self, forward: bool) {
        let View::IssueDetail { issue_id, .. } = &self.core.view else {
            return;
        };
        let current_id = issue_id.clone();

        // view_stackの直上がEpicDetailなら子issue間を移動、IssueListならtop-level間を移動
        let parent = self.core.view_stack.last();
        let (issues, selected_idx) = match parent {
            Some(View::EpicDetail {
                children,
                child_selected,
                ..
            }) => {
                let idx = children
                    .iter()
                    .position(|i| i.id == current_id)
                    .unwrap_or(*child_selected);
                (children.clone(), idx)
            }
            _ => {
                let idx = self
                    .core
                    .issue_store
                    .issues
                    .iter()
                    .position(|i| i.id == current_id)
                    .unwrap_or(self.core.issue_store.selected);
                (self.core.issue_store.issues.clone(), idx)
            }
        };

        if issues.is_empty() {
            return;
        }

        let new_idx = if forward {
            (selected_idx + 1).min(issues.len() - 1)
        } else {
            selected_idx.saturating_sub(1)
        };

        if new_idx == selected_idx {
            return;
        }

        let new_issue_id = issues[new_idx].id.clone();

        // 親viewのselectedも更新
        match self.core.view_stack.last_mut() {
            Some(View::EpicDetail { child_selected, .. }) => {
                *child_selected = new_idx;
            }
            _ => {
                self.core.issue_store.selected = new_idx;
            }
        }

        // 新しいissueに切り替え（子を持つかで分岐）
        let children = bd::list_children(self.dir.as_deref(), &new_issue_id)
            .await
            .unwrap_or_default();

        if children.is_empty() {
            self.core.view = View::IssueDetail {
                issue_id: new_issue_id.clone(),
                scroll_offset: 0,
                diff: None,
            };
            self.load_issue_detail_diff(&new_issue_id).await;
        } else {
            let ready_ids = bd::list_ready_ids(self.dir.as_deref(), &new_issue_id)
                .await
                .unwrap_or_default();
            self.core.view = View::EpicDetail {
                epic_id: new_issue_id,
                children,
                ready_ids,
                child_selected: 0,
                scroll_offset: 0,
            };
        }
    }

    pub async fn reload_children(&mut self) {
        let epic_id = match &self.core.view {
            View::EpicDetail { epic_id, .. } => epic_id.clone(),
            _ => return,
        };
        let new_children = bd::list_children(self.dir.as_deref(), &epic_id)
            .await
            .unwrap_or_default();
        let new_ready = bd::list_ready_ids(self.dir.as_deref(), &epic_id)
            .await
            .unwrap_or_default();

        match &mut self.core.view {
            View::EpicDetail {
                children,
                ready_ids,
                ..
            } => {
                *children = new_children;
                *ready_ids = new_ready;
            }
            _ => {}
        }
    }

    async fn load_issue_detail_diff(&mut self, issue_id: &str) {
        self.rebase_impl(issue_id).await;
        let computed = self.compute_diff(issue_id).await;
        match &mut self.core.view {
            View::IssueDetail { diff, .. } => {
                *diff = computed;
            }
            _ => {}
        }
    }

    /// impl branchをターゲットブランチにrebaseする
    pub async fn rebase_impl(&mut self, issue_id: &str) {
        let Some(job) = self.impl_manager.get_job(issue_id) else {
            return;
        };
        if !matches!(job.status, ImplStatus::Done) {
            return;
        }
        let wt_path = job.worktree_path.clone();
        let base = self.target_branch_for(issue_id);

        match implement::worktree::rebase_impl_branch(&wt_path, &base).await {
            Ok(_) => {}
            Err(e) => {
                self.notify(format!("Rebase failed (retry recommended): {e}"));
            }
        }
    }

    /// issue_idに対するターゲットブランチ（master or epic branch）を返す
    fn target_branch_for(&self, _issue_id: &str) -> String {
        self.find_parent_epic_id()
            .map(|eid| implement::epic_branch_name(&eid))
            .unwrap_or_else(|| "master".to_string())
    }

    async fn compute_diff(&self, issue_id: &str) -> Option<Vec<u8>> {
        let Some(job) = self.impl_manager.get_job(issue_id) else {
            return None;
        };
        if !matches!(job.status, ImplStatus::Done) {
            return None;
        }

        let branch = job.branch.clone();
        let repo_dir = self.repo_dir();

        let base = self.target_branch_for(issue_id);
        let range = format!("{base}..{branch}");

        let output = tokio::process::Command::new("sh")
            .args([
                "-c",
                &format!(
                    "git diff --stat --color=always {range} && echo && git diff --color=always {range} | $(git config core.pager || echo cat)"
                ),
            ])
            .current_dir(&repo_dir)
            .output()
            .await;

        match output {
            Ok(out)
                if out.status.success() && !out.stdout.iter().all(|&b| b.is_ascii_whitespace()) =>
            {
                Some(out.stdout)
            }
            _ => None,
        }
    }

    /// スタックを遡って直近のEpicDetailのepic_idを探す
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

    // --- Enrich ---

    pub fn start_enrich(&mut self, issue_id: &str) {
        let Some(issue) = self.find_issue(issue_id) else {
            return;
        };
        self.enrich_manager.start(&issue, self.dir.clone());
    }

    pub fn auto_enrich(&mut self) {
        self.enrich_manager
            .auto_enrich(&self.core.issue_store.issues, self.dir.clone());
    }

    pub async fn handle_enrich_event(&mut self, event: enrich::EnrichEvent) {
        let outcome = self.enrich_manager.handle_event(event);
        match outcome {
            EnrichOutcome::Started { issue_id } => {
                self.notify(format!("Enriching: {issue_id}..."));
            }
            EnrichOutcome::Completed { issue_id } => {
                self.notify(format!("Enriched: {issue_id}"));
                let _ = self.load_issues().await;
            }
            EnrichOutcome::Failed { issue_id, error } => {
                self.notify(format!("Enrich failed: {issue_id}: {error}"));
            }
        }
    }

    // --- Split ---

    pub fn start_split(&mut self, issue_id: &str) {
        let Some(issue) = self.find_issue(issue_id) else {
            return;
        };
        self.split_manager.start(&issue, self.dir.clone());
    }

    pub async fn handle_split_event(&mut self, event: split::SplitEvent) {
        let outcome = self.split_manager.handle_event(event);
        match outcome {
            SplitOutcome::Started { issue_id } => {
                self.notify(format!("Splitting: {issue_id}..."));
            }
            SplitOutcome::Completed {
                issue_id,
                task_count,
            } => {
                self.notify(format!("Split: {issue_id} → {task_count} tasks"));
                let _ = self.load_issues().await;
                // IssueDetailにいた場合、子ができたのでEpicDetailに遷移
                let should_transition = matches!(
                    &self.core.view,
                    View::IssueDetail { issue_id: vid, .. } if *vid == issue_id
                );
                if should_transition {
                    let children = bd::list_children(self.dir.as_deref(), &issue_id)
                        .await
                        .unwrap_or_default();
                    if !children.is_empty() {
                        let ready_ids = bd::list_ready_ids(self.dir.as_deref(), &issue_id)
                            .await
                            .unwrap_or_default();
                        self.core.view = View::EpicDetail {
                            epic_id: issue_id,
                            children,
                            ready_ids,
                            child_selected: 0,
                            scroll_offset: 0,
                        };
                    }
                }
            }
            SplitOutcome::Failed { issue_id, error } => {
                self.notify(format!("Split failed: {issue_id}: {error}"));
            }
        }
    }

    // --- Implement ---

    pub fn repo_dir(&self) -> PathBuf {
        match &self.dir {
            Some(d) => PathBuf::from(d),
            None => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        }
    }

    pub async fn start_implement(&mut self, issue_id: &str, epic_id: Option<&str>) {
        let Some(issue) = self.find_issue(issue_id) else {
            return;
        };

        let repo_dir = self.repo_dir();
        if let Err(e) = self
            .impl_manager
            .start(&issue, epic_id, &repo_dir, self.dir.clone())
            .await
        {
            self.notify(format!("Failed to start impl: {e}"));
        }
    }

    pub fn handle_impl_event(&mut self, event: implement::ImplEvent) {
        let dir = self.repo_dir().to_string_lossy().to_string();
        let outcome = self.impl_manager.handle_event(event, &dir);
        match outcome {
            ImplOutcome::Started { issue_id } => {
                self.notify(format!("Implementing: {issue_id}..."));
            }
            ImplOutcome::Completed { issue_id, summary } => {
                self.notify(format!(
                    "Implementation done: {issue_id} (log: {} bytes)",
                    summary.len()
                ));
            }
            ImplOutcome::Failed { issue_id, error } => {
                self.notify(format!("Implement failed: {issue_id}: {error}"));
            }
        }
    }

    pub async fn merge_impl(&mut self, issue_id: &str) {
        let epic_id = self.find_parent_epic_id();
        let repo_dir = self.repo_dir();
        if let Err(e) = self
            .impl_manager
            .merge(
                &issue_id,
                epic_id.as_deref(),
                &repo_dir,
                self.dir.as_deref(),
            )
            .await
        {
            self.notify(format!("Merge failed: {e}"));
            return;
        }

        self.notify(format!("Merged & closed: {issue_id}"));
        let _ = self.load_issues().await;
        self.reload_children().await;
    }

    pub async fn discard_impl(&mut self, issue_id: &str) {
        let repo_dir = self.repo_dir();
        if let Err(e) = self.impl_manager.discard(issue_id, &repo_dir).await {
            self.notify(format!("Discard failed: {e}"));
            return;
        }

        self.notify(format!("Discarded: {issue_id}"));
    }

    pub async fn retry_impl(&mut self, issue_id: &str) {
        let repo_dir = self.repo_dir();
        if let Err(e) = self.impl_manager.discard(issue_id, &repo_dir).await {
            self.notify(format!("Retry failed (discard): {e}"));
            return;
        }

        let epic_id = self.find_parent_epic_id();
        self.start_implement(issue_id, epic_id.as_deref()).await;
    }

    // --- Set Status ---

    pub async fn set_status(&mut self, issue_id: &str, status: &str) {
        if status == "closed" {
            match bd::close_issue(self.dir.as_deref(), issue_id).await {
                Ok(_) => {
                    self.notify(format!("Closed: {issue_id}"));
                    let _ = self.load_issues().await;
                    self.reload_children().await;
                    if self.core.issue_store.selected >= self.core.issue_store.issues.len()
                        && self.core.issue_store.selected > 0
                    {
                        self.core.issue_store.selected -= 1;
                    }
                }
                Err(e) => {
                    self.notify(format!("Status change failed: {e}"));
                }
            }
        } else {
            match bd::update_status(self.dir.as_deref(), issue_id, status).await {
                Ok(_) => {
                    self.notify(format!("Status: {issue_id} → {status}"));
                    let _ = self.load_issues().await;
                    self.reload_children().await;
                }
                Err(e) => {
                    self.notify(format!("Status change failed: {e}"));
                }
            }
        }
    }

    // --- Set Priority ---

    pub async fn set_priority(&mut self, issue_id: &str, priority: u8) {
        match bd::update_priority(self.dir.as_deref(), issue_id, priority).await {
            Ok(_) => {
                self.notify(format!("Priority set: {issue_id} → P{priority}"));
                let _ = self.load_issues().await;
            }
            Err(e) => {
                self.notify(format!("Priority update failed: {e}"));
            }
        }
    }

    // --- Copy ---

    pub fn copy_resume_command(&mut self, issue_id: &str) {
        let Some(job) = self.impl_manager.get_job(issue_id) else {
            self.notify("No impl job found");
            return;
        };
        let Some(session_id) = &job.session_id else {
            self.notify("No session ID available");
            return;
        };
        let path = job.worktree_path.display();
        let cmd = format!(
            "cd {} && claude --dangerously-skip-permissions --resume {}",
            path, session_id
        );
        match crate::clipboard::copy(&cmd) {
            Ok(_) => self.notify(format!("Copied: {cmd}")),
            Err(e) => self.notify(format!("Copy failed: {e}")),
        }
    }

    pub fn copy_log_command(&mut self, issue_id: &str) {
        let Some(job) = self.impl_manager.get_job(issue_id) else {
            self.notify("No impl job found");
            return;
        };
        let log_path = crate::ai::implement::run::log_file_path(&job.worktree_path);
        let cmd = format!("tail -f {} | jq .", log_path.display());
        match crate::clipboard::copy(&cmd) {
            Ok(_) => self.notify(format!("Copied: {cmd}")),
            Err(e) => self.notify(format!("Copy failed: {e}")),
        }
    }

    pub fn copy_worktree_path(&mut self, issue_id: &str) {
        let Some(job) = self.impl_manager.get_job(issue_id) else {
            self.notify("No impl job found");
            return;
        };
        let path = job.worktree_path.display().to_string();
        match crate::clipboard::copy(&path) {
            Ok(_) => self.notify(format!("Copied: {path}")),
            Err(e) => self.notify(format!("Copy failed: {e}")),
        }
    }

    // --- Quick Create ---

    pub async fn quick_create_with_editor(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    ) {
        let result = crate::editor::open_editor_for_create(terminal);

        match result {
            Ok(Some(create)) => match bd::quick_create(self.dir.as_deref(), &create.title).await {
                Ok(id) => {
                    self.notify(format!("Created: {id}"));
                    let _ = self.load_issues().await;
                    self.auto_enrich();
                }
                Err(e) => {
                    self.notify(format!("Create failed: {e}"));
                }
            },
            Ok(None) => {} // empty title or no changes
            Err(e) => {
                self.notify(format!("{e}"));
            }
        }
    }

    // --- Edit Description ---

    pub async fn edit_description(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
        issue_id: &str,
    ) {
        let Some(issue) = self.find_issue(issue_id) else {
            return;
        };
        let current_desc = issue.description.as_deref().unwrap_or_default();

        let result = crate::editor::open_editor(terminal, &issue.id, &issue.title, current_desc);

        match result {
            Ok(Some(edit)) => {
                let mut ok = true;
                if edit.title_changed {
                    if let Err(e) =
                        bd::update_title(self.dir.as_deref(), &edit.issue_id, &edit.new_title).await
                    {
                        self.notify(format!("Title update failed: {e}"));
                        ok = false;
                    }
                }
                if edit.desc_changed {
                    if let Err(e) =
                        bd::update_description(self.dir.as_deref(), &edit.issue_id, &edit.new_desc)
                            .await
                    {
                        self.notify(format!("Description update failed: {e}"));
                        ok = false;
                    }
                }
                if ok {
                    self.notify(format!("Updated: {}", edit.issue_id));
                    let _ = self.load_issues().await;
                    self.reload_children().await;
                }
            }
            Ok(None) => {} // no changes
            Err(e) => {
                self.notify(format!("{e}"));
            }
        }
    }

    // --- Merge Epic ---

    pub async fn merge_epic(&mut self, epic_id: &str) {
        // 子が全て closed か再確認
        if let View::EpicDetail { children, .. } = &self.core.view {
            let unclosed: Vec<String> = children
                .iter()
                .filter(|c| c.status != "closed")
                .map(|c| c.id.clone())
                .collect();
            if !unclosed.is_empty() {
                self.notify(format!("Unclosed children: {}", unclosed.join(", ")));
                return;
            }
        }

        let repo_dir = self.repo_dir();

        match self
            .impl_manager
            .merge_epic(&epic_id, &repo_dir, self.dir.as_deref())
            .await
        {
            Ok(_) => {
                self.notify(format!("Merged & closed epic: {epic_id}"));
            }
            Err(e) => {
                let msg = e.to_string();
                if msg == "no_epic_branch" {
                    self.notify(format!("No epic branch — closed: {epic_id}"));
                } else {
                    self.notify(format!("Epic merge failed: {e}"));
                    return;
                }
            }
        }

        self.back();
        let _ = self.load_issues().await;
        if self.core.issue_store.selected >= self.core.issue_store.issues.len()
            && self.core.issue_store.selected > 0
        {
            self.core.issue_store.selected -= 1;
        }
    }

    pub fn all_children_closed(&self) -> bool {
        self.core.all_children_closed()
    }

    /// AppAction を処理する。全操作のディスパッチャ。
    pub async fn process_action(
        &mut self,
        action: crate::action::AppAction,
        terminal: &mut ratatui::Terminal<ratatui::prelude::CrosstermBackend<std::io::Stdout>>,
    ) {
        use crate::action::AppAction;

        match action {
            // ── Navigation ──
            AppAction::Next => self.next(),
            AppAction::Previous => self.previous(),
            AppAction::OpenDetail(_) => self.open_detail().await,
            AppAction::OpenChildDetail(_) => self.open_child_detail().await,
            AppAction::Back => self.back(),
            AppAction::NavigateIssue { forward } => self.navigate_issue(forward).await,

            // ── Overlay ──
            AppAction::OpenSelector(def) => {
                self.core.overlay = Overlay::open_selector(def);
            }
            AppAction::OpenConfirm(confirm) => {
                self.core.notification =
                    Some((confirm.confirm_message().into(), std::time::Instant::now()));
                self.core.overlay = Overlay::Confirm(confirm);
            }
            AppAction::CloseOverlay => {
                self.core.overlay = Overlay::None;
                self.core.notification = None;
            }
            AppAction::Confirm(confirm) => {
                let issue_id = self.current_issue_id().unwrap_or_default();
                match confirm {
                    ConfirmAction::Merge => {
                        self.merge_impl(&issue_id).await;
                        if matches!(&self.core.view, View::IssueDetail { .. }) {
                            self.back();
                        }
                    }
                    ConfirmAction::Discard => self.discard_impl(&issue_id).await,
                    ConfirmAction::MergeEpic => self.merge_epic(&issue_id).await,
                    ConfirmAction::Retry => self.retry_impl(&issue_id).await,
                }
            }

            // ── AI workflows ──
            AppAction::StartEnrich(id) => self.start_enrich(&id),
            AppAction::StartImplement { issue_id, epic_id } => {
                self.start_implement(&issue_id, epic_id.as_deref()).await;
            }
            AppAction::StartSplit(id) => self.start_split(&id),

            // ── Impl operations ──
            AppAction::MergeImpl(id) => self.merge_impl(&id).await,
            AppAction::DiscardImpl(id) => self.discard_impl(&id).await,
            AppAction::RetryImpl(id) => self.retry_impl(&id).await,
            AppAction::MergeEpic(id) => self.merge_epic(&id).await,

            // ── State changes ──
            AppAction::SetStatus { issue_id, status } => {
                self.set_status(&issue_id, &status).await;
                if status == "closed" && matches!(&self.core.view, View::IssueDetail { .. }) {
                    self.back();
                }
            }
            AppAction::SetPriority { issue_id, priority } => {
                self.set_priority(&issue_id, priority).await;
            }

            // ── Editor ──
            AppAction::QuickCreate => self.quick_create_with_editor(terminal).await,
            AppAction::EditDescription(id) => self.edit_description(terminal, &id).await,

            // ── Clipboard ──
            AppAction::CopyId(id) => match crate::clipboard::copy(&id) {
                Ok(_) => self.notify(format!("Copied: {id}")),
                Err(e) => self.notify(format!("Copy failed: {e}")),
            },
            AppAction::CopyResumeCommand(id) => self.copy_resume_command(&id),
            AppAction::CopyLogCommand(id) => self.copy_log_command(&id),
            AppAction::CopyWorktreePath(id) => self.copy_worktree_path(&id),

            // ── Filter ──
            AppAction::ClearFilter => {
                self.core.filter.clear();
                self.core.issue_store.selected = 0;
            }
            AppAction::OpenFilterStatusToggle => crate::overlay::open_filter_status_toggle(self),
            AppAction::OpenFilterLabelToggle => crate::overlay::open_filter_label_toggle(self),

            // ── KeyBar（3-2 で実装） ──
            AppAction::CloseKeyBar => {}
            AppAction::SyncFilter => {}

            // ── System ──
            AppAction::Notify(msg) => self.notify(msg),
            AppAction::ReloadIssues => {
                let _ = self.load_issues().await;
            }
        }
    }
}
