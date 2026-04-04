use std::collections::HashSet;
use std::path::PathBuf;
use std::time::{Instant, SystemTime};

use anyhow::Result;
use ratatui::prelude::*;
use tokio::sync::mpsc;

use crate::ai_enrich::{self, EnrichManager, EnrichOutcome};
use crate::ai_implement::{self, ImplManager, ImplOutcome, ImplStatus};
use crate::ai_split::{self, SplitManager, SplitOutcome};
use crate::bd::{self, Issue};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmAction {
    Close,
    Merge,
    Discard,
    MergeEpic,
    Retry,
}

impl ConfirmAction {
    pub fn label(&self) -> &'static str {
        match self {
            ConfirmAction::Close => "confirm close",
            ConfirmAction::Merge => "confirm merge",
            ConfirmAction::Discard => "confirm discard",
            ConfirmAction::MergeEpic => "confirm merge epic to master",
            ConfirmAction::Retry => "confirm retry",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    AwaitingAI,
    AwaitingYank,
    AwaitingPriority,
    AwaitingConfirm(ConfirmAction),
}

#[derive(Debug)]
pub enum View {
    IssueList,
    IssueDetail {
        issue_id: String,
        scroll_offset: u16,
        diff: Option<Vec<u8>>,
    },
    EpicDetail {
        epic_id: String,
        children: Vec<Issue>,
        ready_ids: HashSet<String>,
        child_selected: usize,
        scroll_offset: u16,
    },
}

pub struct App {
    pub issues: Vec<Issue>,
    pub selected: usize,
    pub view: View,
    pub view_stack: Vec<View>,
    pub dir: Option<String>,
    pub enrich_manager: EnrichManager,
    pub enrich_rx: mpsc::Receiver<ai_enrich::EnrichEvent>,
    pub impl_manager: ImplManager,
    pub impl_rx: mpsc::Receiver<ai_implement::ImplEvent>,
    pub split_manager: SplitManager,
    pub split_rx: mpsc::Receiver<ai_split::SplitEvent>,
    pub notification: Option<(String, Instant)>,
    pub last_db_mtime: Option<SystemTime>,
    pub input_mode: InputMode,
}

impl App {
    pub fn new(dir: Option<String>) -> Self {
        let (enrich_tx, enrich_rx) = mpsc::channel(32);
        let (impl_tx, impl_rx) = mpsc::channel(32);
        let (split_tx, split_rx) = mpsc::channel(32);
        Self {
            issues: Vec::new(),
            selected: 0,
            view: View::IssueList,
            view_stack: Vec::new(),
            dir,
            enrich_manager: EnrichManager::new(enrich_tx),
            enrich_rx,
            impl_manager: ImplManager::new(impl_tx),
            impl_rx,
            split_manager: SplitManager::new(split_tx),
            split_rx,
            notification: None,
            last_db_mtime: None,
            input_mode: InputMode::Normal,
        }
    }

    pub fn notify(&mut self, msg: impl Into<String>) {
        self.notification = Some((msg.into(), Instant::now()));
    }

    pub async fn load_issues(&mut self) -> Result<()> {
        self.issues = bd::list_issues(self.dir.as_deref()).await?;
        self.last_db_mtime = self.db_mtime();
        Ok(())
    }

    pub async fn restore_impl_jobs(&mut self) {
        let repo_dir = self.repo_dir();
        let issue_ids: Vec<String> = self.issues.iter().map(|i| i.id.clone()).collect();
        self.impl_manager.restore_jobs(&repo_dir, &issue_ids).await;
    }

    fn beads_db_path(&self) -> PathBuf {
        self.repo_dir().join(".beads").join("beads.db")
    }

    fn db_mtime(&self) -> Option<SystemTime> {
        std::fs::metadata(self.beads_db_path())
            .and_then(|m| m.modified())
            .ok()
    }

    pub fn has_db_changed(&self) -> bool {
        let current = self.db_mtime();
        match (&self.last_db_mtime, &current) {
            (Some(last), Some(now)) => now > last,
            (None, Some(_)) => true,
            _ => false,
        }
    }

    pub fn next(&mut self) {
        match &mut self.view {
            View::IssueList => {
                if !self.issues.is_empty() {
                    self.selected = (self.selected + 1).min(self.issues.len() - 1);
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
        match &mut self.view {
            View::IssueList => {
                if !self.issues.is_empty() {
                    self.selected = self.selected.saturating_sub(1);
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
            if let Some(issue) = self.issues.get_mut(self.selected) {
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
        let issue_id = match &self.view {
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

        let old = std::mem::replace(&mut self.view, View::IssueList);
        self.view_stack.push(old);

        if children.is_empty() {
            self.view = View::IssueDetail {
                issue_id: issue_id.to_string(),
                scroll_offset: 0,
                diff: None,
            };
            self.load_issue_detail_diff(issue_id).await;
        } else {
            let ready_ids = bd::list_ready_ids(self.dir.as_deref(), issue_id)
                .await
                .unwrap_or_default();
            self.view = View::EpicDetail {
                epic_id: issue_id.to_string(),
                children,
                ready_ids,
                child_selected: 0,
                scroll_offset: 0,
            };
        }
    }

    pub fn back(&mut self) {
        if let Some(prev) = self.view_stack.pop() {
            self.view = prev;
        }
    }

    pub async fn reload_children(&mut self) {
        let epic_id = match &self.view {
            View::EpicDetail { epic_id, .. } => epic_id.clone(),
            _ => return,
        };
        let new_children = bd::list_children(self.dir.as_deref(), &epic_id)
            .await
            .unwrap_or_default();
        let new_ready = bd::list_ready_ids(self.dir.as_deref(), &epic_id)
            .await
            .unwrap_or_default();

        match &mut self.view {
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
        match &mut self.view {
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

        match ai_implement::worktree::rebase_impl_branch(&wt_path, &base).await {
            Ok(_) => {}
            Err(e) => {
                self.notify(format!("Rebase failed (retry recommended): {e}"));
            }
        }
    }

    /// issue_idに対するターゲットブランチ（master or epic branch）を返す
    fn target_branch_for(&self, _issue_id: &str) -> String {
        self.find_parent_epic_id()
            .map(|eid| ai_implement::epic_branch_name(&eid))
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
    fn find_parent_epic_id(&self) -> Option<String> {
        for view in self.view_stack.iter().rev() {
            if let View::EpicDetail { epic_id, .. } = view {
                return Some(epic_id.clone());
            }
        }
        None
    }

    pub fn selected_issue(&self) -> Option<&Issue> {
        self.issues.get(self.selected)
    }

    /// 現在のview contextで対象となるissue_idを返す
    pub fn current_issue_id(&self) -> Option<String> {
        match &self.view {
            View::IssueDetail { issue_id, .. } => Some(issue_id.clone()),
            View::EpicDetail { epic_id, .. } => Some(epic_id.clone()),
            _ => self.selected_issue().map(|i| i.id.clone()),
        }
    }

    /// 現在のview contextで対象となるissue_id + epic_idを返す
    fn current_issue_id_with_epic(&self) -> Option<(String, Option<String>)> {
        match &self.view {
            View::EpicDetail {
                children,
                child_selected,
                ..
            } => {
                let child = children.get(*child_selected)?;
                let epic_id = self.find_parent_epic_id();
                Some((child.id.clone(), epic_id))
            }
            View::IssueDetail { issue_id, .. } => {
                let epic_id = self.find_parent_epic_id();
                Some((issue_id.clone(), epic_id))
            }
            _ => self.selected_issue().map(|i| (i.id.clone(), None)),
        }
    }

    /// 現在のview contextで対象となるIssueを返す
    fn current_issue(&self) -> Option<Issue> {
        match &self.view {
            View::IssueDetail { issue_id, .. } => {
                // issuesまたはスタック内のEpicDetailのchildrenから探す
                self.issues
                    .iter()
                    .find(|i| i.id == *issue_id)
                    .cloned()
                    .or_else(|| self.find_issue_in_stack(issue_id))
            }
            View::EpicDetail {
                children,
                child_selected,
                ..
            } => children.get(*child_selected).cloned(),
            _ => self.selected_issue().cloned(),
        }
    }

    /// スタック内のEpicDetailのchildrenからissueを探す
    fn find_issue_in_stack(&self, issue_id: &str) -> Option<Issue> {
        for view in self.view_stack.iter().rev() {
            if let View::EpicDetail { children, .. } = view {
                if let Some(issue) = children.iter().find(|i| i.id == issue_id) {
                    return Some(issue.clone());
                }
            }
        }
        None
    }

    // --- Enrich ---

    pub fn start_enrich(&mut self) {
        let Some(issue) = self.current_issue() else {
            return;
        };
        self.enrich_manager.start(&issue, self.dir.clone());
    }

    pub fn auto_enrich(&mut self) {
        self.enrich_manager
            .auto_enrich(&self.issues, self.dir.clone());
    }

    pub async fn handle_enrich_event(&mut self, event: ai_enrich::EnrichEvent) {
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

    pub fn start_split(&mut self) {
        let Some(issue) = self.current_issue() else {
            return;
        };
        self.split_manager.start(&issue, self.dir.clone());
    }

    pub async fn handle_split_event(&mut self, event: ai_split::SplitEvent) {
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
                    &self.view,
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
                        self.view = View::EpicDetail {
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

    pub async fn start_implement(&mut self) {
        let Some(issue) = self.current_issue() else {
            return;
        };
        let epic_id = self.find_parent_epic_id().or_else(|| match &self.view {
            View::EpicDetail { epic_id, .. } => Some(epic_id.clone()),
            _ => None,
        });

        let repo_dir = self.repo_dir();
        if let Err(e) = self
            .impl_manager
            .start(&issue, epic_id.as_deref(), &repo_dir, self.dir.clone())
            .await
        {
            self.notify(format!("Failed to start impl: {e}"));
        }
    }

    pub fn handle_impl_event(&mut self, event: ai_implement::ImplEvent) {
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

    pub async fn merge_impl(&mut self) {
        let Some((issue_id, epic_id)) = self.current_issue_id_with_epic() else {
            return;
        };

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

    pub async fn discard_impl(&mut self) {
        let Some(issue_id) = self.current_issue_id() else {
            return;
        };

        let repo_dir = self.repo_dir();
        if let Err(e) = self.impl_manager.discard(&issue_id, &repo_dir).await {
            self.notify(format!("Discard failed: {e}"));
            return;
        }

        self.notify(format!("Discarded: {issue_id}"));
    }

    pub async fn retry_impl(&mut self) {
        let Some(issue_id) = self.current_issue_id() else {
            return;
        };

        let repo_dir = self.repo_dir();
        if let Err(e) = self.impl_manager.discard(&issue_id, &repo_dir).await {
            self.notify(format!("Retry failed (discard): {e}"));
            return;
        }

        self.start_implement().await;
    }

    /// 現在のissueのimpl jobがDoneかどうか
    pub fn impl_job_is_done(&self) -> bool {
        let Some(issue_id) = self.current_issue_id() else {
            return false;
        };
        matches!(
            self.impl_manager.get_job(&issue_id),
            Some(job) if matches!(job.status, ImplStatus::Done)
        )
    }

    /// 手動rebase + diff再計算
    pub async fn rebase_and_refresh_diff(&mut self) {
        let Some(issue_id) = self.current_issue_id() else {
            return;
        };
        self.rebase_impl(&issue_id).await;
        let computed = self.compute_diff(&issue_id).await;
        match &mut self.view {
            View::IssueDetail { diff, .. } => {
                *diff = computed;
            }
            _ => {}
        }
    }

    // --- Close Issue ---

    pub async fn close_issue(&mut self) {
        let Some(issue_id) = self.current_issue_id() else {
            return;
        };

        match bd::close_issue(self.dir.as_deref(), &issue_id).await {
            Ok(_) => {
                self.notify(format!("Closed: {issue_id}"));
                let _ = self.load_issues().await;
                self.reload_children().await;
                if self.selected >= self.issues.len() && self.selected > 0 {
                    self.selected -= 1;
                }
            }
            Err(e) => {
                self.notify(format!("Close failed: {e}"));
            }
        }
    }

    // --- Set Priority ---

    pub async fn set_priority(&mut self, priority: u8) {
        let Some(issue) = self.selected_issue() else {
            return;
        };
        let issue_id = issue.id.clone();

        match bd::update_priority(self.dir.as_deref(), &issue_id, priority).await {
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

    pub fn copy_id(&mut self) {
        let Some(id) = self.current_issue_id() else {
            return;
        };
        match crate::clipboard::copy(&id) {
            Ok(_) => self.notify(format!("Copied: {id}")),
            Err(e) => self.notify(format!("Copy failed: {e}")),
        }
    }

    pub fn copy_resume_command(&mut self) {
        let Some(issue_id) = self.current_issue_id() else {
            return;
        };
        let Some(job) = self.impl_manager.get_job(&issue_id) else {
            self.notify("No impl job found");
            return;
        };
        let Some(session_id) = &job.session_id else {
            self.notify("No session ID available");
            return;
        };
        let path = job.worktree_path.display();
        let cmd = format!("cd {} && claude --resume {}", path, session_id);
        match crate::clipboard::copy(&cmd) {
            Ok(_) => self.notify(format!("Copied: {cmd}")),
            Err(e) => self.notify(format!("Copy failed: {e}")),
        }
    }

    pub fn copy_worktree_path(&mut self) {
        let Some(issue_id) = self.current_issue_id() else {
            return;
        };
        let Some(job) = self.impl_manager.get_job(&issue_id) else {
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
            Ok(Some(create)) => {
                match bd::quick_create(self.dir.as_deref(), &create.title).await {
                    Ok(id) => {
                        self.notify(format!("Created: {id}"));
                        let _ = self.load_issues().await;
                        self.auto_enrich();
                    }
                    Err(e) => {
                        self.notify(format!("Create failed: {e}"));
                    }
                }
            }
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
    ) {
        // EpicDetailではepic自体を編集（子issueではなく）
        let issue = match &self.view {
            View::EpicDetail { epic_id, .. } => self
                .issues
                .iter()
                .find(|i| i.id == *epic_id)
                .cloned()
                .or_else(|| self.find_issue_in_stack(epic_id)),
            _ => self.current_issue(),
        };
        let Some(issue) = issue else { return };
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

    pub async fn merge_epic(&mut self) {
        let (epic_id, unclosed) = match &self.view {
            View::EpicDetail {
                epic_id, children, ..
            } => {
                let unclosed: Vec<String> = children
                    .iter()
                    .filter(|c| c.status != "closed")
                    .map(|c| c.id.clone())
                    .collect();
                (epic_id.clone(), unclosed)
            }
            _ => return,
        };

        if !unclosed.is_empty() {
            self.notify(format!("Unclosed children: {}", unclosed.join(", ")));
            return;
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
        if self.selected >= self.issues.len() && self.selected > 0 {
            self.selected -= 1;
        }
    }

    pub fn all_children_closed(&self) -> bool {
        match &self.view {
            View::EpicDetail { children, .. } => {
                !children.is_empty() && children.iter().all(|c| c.status == "closed")
            }
            _ => false,
        }
    }
}
