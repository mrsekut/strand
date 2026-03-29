use std::collections::{HashMap, HashSet};
use std::io::stdout;
use std::path::PathBuf;
use std::time::{Instant, SystemTime};

use anyhow::Result;
use crossterm::ExecutableCommand;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::prelude::*;
use tokio::sync::mpsc;

use crate::bd::{self, Issue};
use crate::enrich::{self, EnrichManager, EnrichOutcome};
use crate::implement::{self, ImplEvent, ImplJob, ImplStatus};
use crate::split::{self, SplitEvent};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmAction {
    Close,
    Merge,
    Discard,
    MergeEpic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    AwaitingAI,
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
    ChildDetail {
        epic_id: String,
        issue_id: String,
        children: Vec<Issue>,
        ready_ids: HashSet<String>,
        child_selected: usize,
        scroll_offset: u16,
        diff: Option<Vec<u8>>,
    },
}

pub struct App {
    pub issues: Vec<Issue>,
    pub selected: usize,
    pub view: View,
    pub dir: Option<String>,
    pub enrich_manager: EnrichManager,
    pub enrich_rx: mpsc::Receiver<enrich::EnrichEvent>,
    pub impl_tx: mpsc::Sender<ImplEvent>,
    pub impl_rx: mpsc::Receiver<ImplEvent>,
    pub impl_jobs: HashMap<String, ImplJob>,
    pub split_tx: mpsc::Sender<SplitEvent>,
    pub split_rx: mpsc::Receiver<SplitEvent>,
    pub splitting_ids: HashSet<String>,
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
            dir,
            enrich_manager: EnrichManager::new(enrich_tx),
            enrich_rx,
            impl_tx,
            impl_rx,
            impl_jobs: HashMap::new(),
            split_tx,
            split_rx,
            splitting_ids: HashSet::new(),
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
        let discovered = implement::discover_worktrees(&repo_dir, &issue_ids).await;
        for job in discovered {
            self.impl_jobs.entry(job.issue_id.clone()).or_insert(job);
        }
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
            View::IssueDetail { scroll_offset, .. } | View::ChildDetail { scroll_offset, .. } => {
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
            View::IssueDetail { scroll_offset, .. } | View::ChildDetail { scroll_offset, .. } => {
                *scroll_offset = scroll_offset.saturating_sub(1);
            }
        }
    }

    // --- Navigation ---

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

        // 子issueの有無で分岐
        let children = bd::list_children(self.dir.as_deref(), &issue_id)
            .await
            .unwrap_or_default();

        if children.is_empty() {
            // 子なし → IssueDetail
            self.view = View::IssueDetail {
                issue_id: issue_id.clone(),
                scroll_offset: 0,
                diff: None,
            };
            self.load_issue_detail_diff(&issue_id).await;
        } else {
            // 子あり → EpicDetail
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

        // EpicDetailからChildDetailへ状態を移動
        let old = std::mem::replace(&mut self.view, View::IssueList);
        let View::EpicDetail {
            epic_id,
            children,
            ready_ids,
            child_selected,
            ..
        } = old
        else {
            unreachable!()
        };

        self.view = View::ChildDetail {
            epic_id,
            issue_id: issue_id.clone(),
            children,
            ready_ids,
            child_selected,
            scroll_offset: 0,
            diff: None,
        };
        self.load_issue_detail_diff(&issue_id).await;
    }

    pub fn back(&mut self) {
        let old = std::mem::replace(&mut self.view, View::IssueList);
        self.view = match old {
            v @ View::IssueList => v,
            View::IssueDetail { .. } => View::IssueList,
            View::EpicDetail { .. } => View::IssueList,
            View::ChildDetail {
                epic_id,
                children,
                ready_ids,
                child_selected,
                ..
            } => View::EpicDetail {
                epic_id,
                children,
                ready_ids,
                child_selected,
                scroll_offset: 0,
            },
        };
    }

    pub async fn reload_children(&mut self) {
        let epic_id = match &self.view {
            View::EpicDetail { epic_id, .. } | View::ChildDetail { epic_id, .. } => epic_id.clone(),
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
            }
            | View::ChildDetail {
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
        let computed = self.compute_diff(issue_id).await;
        match &mut self.view {
            View::IssueDetail { diff, .. } | View::ChildDetail { diff, .. } => {
                *diff = computed;
            }
            _ => {}
        }
    }

    async fn compute_diff(&self, issue_id: &str) -> Option<Vec<u8>> {
        let Some(job) = self.impl_jobs.get(issue_id) else {
            return None;
        };
        if !matches!(job.status, ImplStatus::Done) {
            return None;
        }

        let branch = job.branch.clone();
        let repo_dir = self.repo_dir();

        // epicコンテキストな���epicブランチとの差分を表示
        let base = match &self.view {
            View::ChildDetail { epic_id, .. } => implement::epic_branch_name(epic_id),
            _ => "master".to_string(),
        };
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

    pub fn selected_issue(&self) -> Option<&Issue> {
        self.issues.get(self.selected)
    }

    // --- Enrich ---

    pub fn start_enrich(&mut self) {
        let issue = match &self.view {
            View::ChildDetail {
                issue_id, children, ..
            } => children.iter().find(|i| i.id == *issue_id).cloned(),
            View::IssueDetail { issue_id, .. } => {
                self.issues.iter().find(|i| i.id == *issue_id).cloned()
            }
            _ => self.selected_issue().cloned(),
        };
        let Some(issue) = issue else { return };
        self.enrich_manager.start(&issue, self.dir.clone());
    }

    pub fn auto_enrich(&mut self) {
        self.enrich_manager
            .auto_enrich(&self.issues, self.dir.clone());
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

    pub fn start_split(&mut self) {
        let issue = match &self.view {
            View::IssueDetail { issue_id, .. } => {
                self.issues.iter().find(|i| i.id == *issue_id).cloned()
            }
            _ => self.selected_issue().cloned(),
        };
        let Some(issue) = issue else { return };

        if self.splitting_ids.contains(&issue.id) {
            return;
        }

        self.splitting_ids.insert(issue.id.clone());

        let request = split::SplitRequest {
            issue_id: issue.id.clone(),
            title: issue.title.clone(),
            description: issue.description.clone(),
        };
        let dir = self.dir.clone();
        let tx = self.split_tx.clone();

        tokio::spawn(async move {
            let _ = split::run(request, dir, tx).await;
        });
    }

    pub async fn handle_split_event(&mut self, event: SplitEvent) {
        match event {
            SplitEvent::Started { issue_id } => {
                self.notify(format!("Splitting: {issue_id}..."));
            }
            SplitEvent::Completed {
                issue_id,
                task_count,
            } => {
                self.splitting_ids.remove(&issue_id);
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
            SplitEvent::Failed { issue_id, error } => {
                self.splitting_ids.remove(&issue_id);
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
        let (issue, epic_id) = match &self.view {
            View::ChildDetail {
                issue_id,
                epic_id,
                children,
                ..
            } => {
                let issue = children.iter().find(|i| i.id == *issue_id).cloned();
                (issue, Some(epic_id.clone()))
            }
            View::EpicDetail {
                epic_id,
                children,
                child_selected,
                ..
            } => {
                let issue = children.get(*child_selected).cloned();
                (issue, Some(epic_id.clone()))
            }
            View::IssueDetail { issue_id, .. } => {
                let issue = self.issues.iter().find(|i| i.id == *issue_id).cloned();
                (issue, None)
            }
            _ => (self.selected_issue().cloned(), None),
        };
        let Some(issue) = issue else { return };
        self.start_implement_issue(&issue, epic_id.as_deref()).await;
    }

    async fn start_implement_issue(&mut self, issue: &Issue, epic_id: Option<&str>) {
        let issue_id = issue.id.clone();
        let title = issue.title.clone();
        let description = issue.description.clone();

        if self.impl_jobs.contains_key(&issue_id) {
            return;
        }

        let repo_dir = self.repo_dir();

        let base_branch = if let Some(eid) = epic_id {
            match implement::ensure_epic_branch(&repo_dir, eid).await {
                Ok(branch) => branch,
                Err(e) => {
                    self.notify(format!("Failed to create epic branch: {e}"));
                    return;
                }
            }
        } else {
            "master".to_string()
        };

        let wt_path = implement::worktree_path(&repo_dir, &issue_id);
        let branch = implement::branch_name(&issue_id);

        self.impl_jobs.insert(
            issue_id.clone(),
            ImplJob {
                issue_id: issue_id.clone(),
                branch: branch.clone(),
                worktree_path: wt_path,
                status: ImplStatus::Running,
                completed_at: None,
            },
        );

        let request = implement::ImplRequest {
            issue_id: issue_id.clone(),
            title,
            description,
            design: None,
            repo_dir,
            base_branch,
        };
        let tx = self.impl_tx.clone();

        tokio::spawn(async move {
            let _ = implement::run(request, tx).await;
        });
    }

    pub fn handle_impl_event(&mut self, event: ImplEvent) {
        match event {
            ImplEvent::Started { issue_id } => {
                self.notify(format!("Implementing: {issue_id}..."));
            }
            ImplEvent::Completed { issue_id, summary } => {
                if let Some(job) = self.impl_jobs.get_mut(&issue_id) {
                    job.status = ImplStatus::Done;
                    job.completed_at = Some(chrono::Local::now().to_rfc3339());
                }
                let summary_len = summary.len();
                let dir = self.repo_dir().to_string_lossy().to_string();
                let id = issue_id.clone();
                tokio::spawn(async move {
                    let content = format!("## Implementation Log\n{summary}");
                    if let Err(e) = bd::append_to_description(Some(&dir), &id, &content).await {
                        eprintln!("Failed to append impl log to {id}: {e}");
                    }
                });
                self.notify(format!(
                    "Implementation done: {issue_id} (log: {summary_len} bytes)"
                ));
            }
            ImplEvent::Failed { issue_id, error } => {
                if let Some(job) = self.impl_jobs.get_mut(&issue_id) {
                    job.status = ImplStatus::Failed(error.clone());
                }
                self.notify(format!("Implement failed: {issue_id}: {error}"));
            }
        }
    }

    pub async fn merge_impl(&mut self) {
        let (issue_id, epic_id) = match &self.view {
            View::ChildDetail {
                issue_id, epic_id, ..
            } => (issue_id.clone(), Some(epic_id.clone())),
            View::EpicDetail {
                epic_id,
                children,
                child_selected,
                ..
            } => {
                let Some(child) = children.get(*child_selected) else {
                    return;
                };
                (child.id.clone(), Some(epic_id.clone()))
            }
            View::IssueDetail { issue_id, .. } => (issue_id.clone(), None),
            _ => {
                let Some(issue) = self.selected_issue() else {
                    return;
                };
                (issue.id.clone(), None)
            }
        };

        let job = match self.impl_jobs.get(&issue_id) {
            Some(job) if matches!(job.status, ImplStatus::Done) => job.clone(),
            _ => return,
        };

        let repo_dir = self.repo_dir();

        let target = match &epic_id {
            Some(eid) => implement::epic_branch_name(eid),
            None => "master".to_string(),
        };
        let merge_result = implement::merge_into_branch(&repo_dir, &job.branch, &target).await;

        if let Err(e) = merge_result {
            self.notify(format!("Merge failed: {e}"));
            return;
        }

        if let Err(e) = implement::remove_worktree(&repo_dir, &job.worktree_path).await {
            self.notify(format!("Worktree remove failed: {e}"));
            return;
        }

        let _ = implement::delete_branch(&repo_dir, &job.branch).await;
        let _ = bd::close_issue(self.dir.as_deref(), &issue_id).await;

        self.impl_jobs.remove(&issue_id);
        self.notify(format!("Merged & closed: {issue_id}"));
        let _ = self.load_issues().await;
        self.reload_children().await;
    }

    pub async fn discard_impl(&mut self) {
        let issue_id = match &self.view {
            View::IssueDetail { issue_id, .. } | View::ChildDetail { issue_id, .. } => {
                issue_id.clone()
            }
            _ => {
                let Some(issue) = self.selected_issue() else {
                    return;
                };
                issue.id.clone()
            }
        };

        let job = match self.impl_jobs.get(&issue_id) {
            Some(job) => job.clone(),
            None => return,
        };

        let repo_dir = self.repo_dir();

        if let Err(e) = implement::remove_worktree(&repo_dir, &job.worktree_path).await {
            self.notify(format!("Worktree remove failed: {e}"));
            return;
        }

        let _ = implement::delete_branch(&repo_dir, &job.branch).await;

        self.impl_jobs.remove(&issue_id);
        self.notify(format!("Discarded: {issue_id}"));
    }

    // --- Close Issue ---

    pub async fn close_issue(&mut self) {
        let issue_id = match &self.view {
            View::IssueDetail { issue_id, .. } | View::ChildDetail { issue_id, .. } => {
                issue_id.clone()
            }
            _ => {
                let Some(issue) = self.selected_issue() else {
                    return;
                };
                issue.id.clone()
            }
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
        let id = match &self.view {
            View::IssueDetail { issue_id, .. } | View::ChildDetail { issue_id, .. } => {
                issue_id.clone()
            }
            _ => {
                let Some(issue) = self.selected_issue() else {
                    return;
                };
                issue.id.clone()
            }
        };
        self.copy_to_clipboard(&id);
    }

    pub fn copy_worktree_path(&mut self) {
        let issue_id = match &self.view {
            View::IssueDetail { issue_id, .. } | View::ChildDetail { issue_id, .. } => {
                issue_id.clone()
            }
            _ => {
                let Some(issue) = self.selected_issue() else {
                    return;
                };
                issue.id.clone()
            }
        };
        let Some(job) = self.impl_jobs.get(&issue_id) else {
            self.notify("No impl job found");
            return;
        };
        let path = job.worktree_path.display().to_string();
        self.copy_to_clipboard(&path);
    }

    fn copy_to_clipboard(&mut self, text: &str) {
        let result = std::process::Command::new("pbcopy")
            .stdin(std::process::Stdio::piped())
            .spawn()
            .and_then(|mut child| {
                use std::io::Write;
                child.stdin.as_mut().unwrap().write_all(text.as_bytes())?;
                child.wait()
            });

        match result {
            Ok(_) => self.notify(format!("Copied: {text}")),
            Err(e) => self.notify(format!("Copy failed: {e}")),
        }
    }

    // --- Edit Description ---

    pub async fn edit_description(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    ) {
        let (issue_id, current_title, current_desc) = match &self.view {
            View::ChildDetail {
                issue_id, children, ..
            } => {
                let issue = children.iter().find(|i| i.id == *issue_id);
                let Some(issue) = issue else { return };
                (
                    issue.id.clone(),
                    issue.title.clone(),
                    issue.description.clone().unwrap_or_default(),
                )
            }
            View::IssueDetail { issue_id, .. } => {
                let issue = self.issues.iter().find(|i| i.id == *issue_id);
                let Some(issue) = issue else { return };
                (
                    issue.id.clone(),
                    issue.title.clone(),
                    issue.description.clone().unwrap_or_default(),
                )
            }
            View::EpicDetail { epic_id, .. } => {
                let issue = self.issues.iter().find(|i| i.id == *epic_id);
                let Some(issue) = issue else { return };
                (
                    issue.id.clone(),
                    issue.title.clone(),
                    issue.description.clone().unwrap_or_default(),
                )
            }
            _ => {
                let Some(issue) = self.selected_issue() else {
                    return;
                };
                (
                    issue.id.clone(),
                    issue.title.clone(),
                    issue.description.clone().unwrap_or_default(),
                )
            }
        };

        let content = format!("{}\n\n{}", current_title, current_desc);
        let tmp = std::env::temp_dir().join(format!("strand-{issue_id}.md"));
        if std::fs::write(&tmp, &content).is_err() {
            self.notify("Failed to create temp file");
            return;
        }

        let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".into());
        disable_raw_mode().ok();
        stdout().execute(LeaveAlternateScreen).ok();
        terminal.show_cursor().ok();

        let status = std::process::Command::new(&editor).arg(&tmp).status();

        stdout().execute(EnterAlternateScreen).ok();
        enable_raw_mode().ok();
        terminal.clear().ok();

        match status {
            Ok(s) if s.success() => {
                if let Ok(new_content) = std::fs::read_to_string(&tmp) {
                    let new_title = new_content.lines().next().unwrap_or("").trim().to_string();
                    let new_desc = new_content
                        .lines()
                        .skip(1)
                        .collect::<Vec<_>>()
                        .join("\n")
                        .trim()
                        .to_string();

                    let title_changed = new_title != current_title.trim();
                    let desc_changed = new_desc != current_desc.trim();

                    if title_changed || desc_changed {
                        let mut ok = true;
                        if title_changed {
                            if let Err(e) =
                                bd::update_title(self.dir.as_deref(), &issue_id, &new_title).await
                            {
                                self.notify(format!("Title update failed: {e}"));
                                ok = false;
                            }
                        }
                        if desc_changed {
                            if let Err(e) =
                                bd::update_description(self.dir.as_deref(), &issue_id, &new_desc)
                                    .await
                            {
                                self.notify(format!("Description update failed: {e}"));
                                ok = false;
                            }
                        }
                        if ok {
                            self.notify(format!("Updated: {issue_id}"));
                            let _ = self.load_issues().await;
                            self.reload_children().await;
                        }
                    }
                }
            }
            _ => {
                self.notify("Editor exited with error");
            }
        }

        let _ = std::fs::remove_file(&tmp);
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

        if !implement::epic_branch_exists(&repo_dir, &epic_id).await {
            let _ = bd::close_issue(self.dir.as_deref(), &epic_id).await;
            self.notify(format!("No epic branch — closed: {epic_id}"));
            self.view = View::IssueList;
            let _ = self.load_issues().await;
            if self.selected >= self.issues.len() && self.selected > 0 {
                self.selected -= 1;
            }
            return;
        }

        if let Err(e) = implement::merge_epic_to_master(&repo_dir, &epic_id).await {
            self.notify(format!("Epic merge failed: {e}"));
            return;
        }

        let _ = bd::close_issue(self.dir.as_deref(), &epic_id).await;
        self.notify(format!("Merged & closed epic: {epic_id}"));
        self.view = View::IssueList;
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
