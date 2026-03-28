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
use crate::enrich::{self, EnrichEvent};
use crate::implement::{self, ImplEvent, ImplJob, ImplStatus};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmAction {
    Close,
    Merge,
    Discard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    AwaitingAI,
    AwaitingPriority,
    AwaitingConfirm(ConfirmAction),
}

pub struct App {
    pub issues: Vec<Issue>,
    pub selected: usize,
    pub show_detail: bool,
    pub dir: Option<String>,
    pub enrich_tx: mpsc::Sender<EnrichEvent>,
    pub enrich_rx: mpsc::Receiver<EnrichEvent>,
    pub enriching_ids: HashSet<String>,
    pub impl_tx: mpsc::Sender<ImplEvent>,
    pub impl_rx: mpsc::Receiver<ImplEvent>,
    pub impl_jobs: HashMap<String, ImplJob>,
    pub notification: Option<(String, Instant)>,
    pub last_db_mtime: Option<SystemTime>,
    pub input_mode: InputMode,
    pub detail_diff: Option<Vec<u8>>,
    pub scroll_offset: u16,
}

impl App {
    pub fn new(dir: Option<String>) -> Self {
        let (enrich_tx, enrich_rx) = mpsc::channel(32);
        let (impl_tx, impl_rx) = mpsc::channel(32);
        Self {
            issues: Vec::new(),
            selected: 0,
            show_detail: false,
            dir,
            enrich_tx,
            enrich_rx,
            enriching_ids: HashSet::new(),
            impl_tx,
            impl_rx,
            impl_jobs: HashMap::new(),
            notification: None,
            last_db_mtime: None,
            input_mode: InputMode::Normal,
            detail_diff: None,
            scroll_offset: 0,
        }
    }

    fn notify(&mut self, msg: impl Into<String>) {
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
        if !self.issues.is_empty() {
            self.selected = (self.selected + 1).min(self.issues.len() - 1);
        }
    }

    pub fn previous(&mut self) {
        if !self.issues.is_empty() {
            self.selected = self.selected.saturating_sub(1);
        }
    }

    pub async fn open_detail(&mut self) {
        self.show_detail = true;
        self.scroll_offset = 0;
        self.load_detail_diff().await;

        if let Some(issue) = self.issues.get_mut(self.selected) {
            if issue.labels.contains(&"strand-unread".to_string()) {
                issue.labels.retain(|l| l != "strand-unread");
                let id = issue.id.clone();
                let dir = self.dir.clone();
                tokio::spawn(async move {
                    let _ = bd::remove_label(dir.as_deref(), &id, "strand-unread").await;
                });
            }
        }
    }

    async fn load_detail_diff(&mut self) {
        self.detail_diff = None;

        let Some(issue) = self.selected_issue() else {
            return;
        };
        let Some(job) = self.impl_jobs.get(&issue.id) else {
            return;
        };
        if !matches!(job.status, ImplStatus::Done) {
            return;
        }

        let branch = job.branch.clone();
        let repo_dir = self.repo_dir();

        let range = format!("master..{branch}");

        let output = tokio::process::Command::new("sh")
            .args(["-c", &format!(
                "git diff --stat --color=always {range} && echo && git diff --color=always {range} | $(git config core.pager || echo cat)"
            )])
            .current_dir(&repo_dir)
            .output()
            .await;

        if let Ok(out) = output {
            if out.status.success() && !out.stdout.iter().all(|&b| b.is_ascii_whitespace()) {
                self.detail_diff = Some(out.stdout);
            }
        }
    }

    pub fn back_to_list(&mut self) {
        self.show_detail = false;
        self.detail_diff = None;
        self.scroll_offset = 0;
    }

    pub fn selected_issue(&self) -> Option<&Issue> {
        self.issues.get(self.selected)
    }

    // --- Enrich ---

    pub fn start_enrich(&mut self) {
        let Some(issue) = self.selected_issue() else {
            return;
        };
        self.enrich_issue(issue.clone());
    }

    fn enrich_issue(&mut self, issue: Issue) {
        let issue_id = issue.id.clone();

        if self.enriching_ids.contains(&issue_id) {
            return;
        }

        self.enriching_ids.insert(issue_id.clone());

        let request = enrich::EnrichRequest {
            issue_id,
            title: issue.title.clone(),
            description: issue.description.clone(),
        };
        let dir = self.dir.clone();
        let tx = self.enrich_tx.clone();

        tokio::spawn(async move {
            let _ = enrich::run(request, dir, tx).await;
        });
    }

    /// epicタイプかつenrichedラベルがない未enrich issueを自動的にenrichする
    pub fn auto_enrich(&mut self) {
        let unenriched: Vec<Issue> = self
            .issues
            .iter()
            .filter(|issue| {
                issue.issue_type.as_deref() == Some("epic")
                    && !issue.labels.contains(&"strand-enriched".to_string())
                    && !self.enriching_ids.contains(&issue.id)
            })
            .cloned()
            .collect();

        for issue in unenriched {
            self.enrich_issue(issue);
        }
    }

    pub async fn handle_enrich_event(&mut self, event: EnrichEvent) {
        match event {
            EnrichEvent::Started { issue_id } => {
                self.notify(format!("Enriching: {issue_id}..."));
            }
            EnrichEvent::Completed { issue_id } => {
                self.enriching_ids.remove(&issue_id);
                self.notify(format!("Enriched: {issue_id}"));
                let _ = self.load_issues().await;
                self.auto_implement_if_eligible(&issue_id);
            }
            EnrichEvent::Failed { issue_id, error } => {
                self.enriching_ids.remove(&issue_id);
                self.notify(format!("Enrich failed: {issue_id}: {error}"));
            }
        }
    }

    /// p0/p1かつenrich済みのissueに対して自動でimplementを開始する
    fn auto_implement_if_eligible(&mut self, issue_id: &str) {
        if self.impl_jobs.contains_key(issue_id) {
            return;
        }
        if let Some(issue) = self.issues.iter().find(|i| i.id == issue_id).cloned() {
            let is_high_priority = issue.priority.map_or(false, |p| p <= 1);
            let is_enriched = issue.labels.contains(&"strand-enriched".to_string());
            if is_high_priority && is_enriched {
                self.start_implement_issue(&issue);
            }
        }
    }

    // --- Implement ---

    fn repo_dir(&self) -> PathBuf {
        match &self.dir {
            Some(d) => PathBuf::from(d),
            None => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        }
    }

    pub fn start_implement(&mut self) {
        let Some(issue) = self.selected_issue().cloned() else {
            return;
        };
        self.start_implement_issue(&issue);
    }

    fn start_implement_issue(&mut self, issue: &Issue) {
        let issue_id = issue.id.clone();
        let title = issue.title.clone();
        let description = issue.description.clone();

        if self.impl_jobs.contains_key(&issue_id) {
            return;
        }

        let repo_dir = self.repo_dir();
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
        let Some(issue) = self.selected_issue() else {
            return;
        };
        let issue_id = issue.id.clone();

        let job = match self.impl_jobs.get(&issue_id) {
            Some(job) if matches!(job.status, ImplStatus::Done) => job.clone(),
            _ => return,
        };

        let repo_dir = self.repo_dir();

        if let Err(e) = implement::merge_branch(&repo_dir, &job.branch).await {
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
    }

    pub async fn discard_impl(&mut self) {
        let Some(issue) = self.selected_issue() else {
            return;
        };
        let issue_id = issue.id.clone();

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
        let Some(issue) = self.selected_issue() else {
            return;
        };
        let issue_id = issue.id.clone();

        match bd::close_issue(self.dir.as_deref(), &issue_id).await {
            Ok(_) => {
                self.notify(format!("Closed: {issue_id}"));
                let _ = self.load_issues().await;
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

                // p0/p1に設定された場合、未enrichならenrich開始（完了時にauto-implが発火）
                if priority <= 1 {
                    if let Some(issue) = self.issues.iter().find(|i| i.id == issue_id).cloned() {
                        if !issue.labels.contains(&"strand-enriched".to_string())
                            && !self.enriching_ids.contains(&issue_id)
                        {
                            self.enrich_issue(issue);
                        } else {
                            self.auto_implement_if_eligible(&issue_id);
                        }
                    }
                }
            }
            Err(e) => {
                self.notify(format!("Priority update failed: {e}"));
            }
        }
    }

    // --- Copy ID ---

    pub fn copy_id(&mut self) {
        let Some(issue) = self.selected_issue() else {
            return;
        };
        let id = issue.id.clone();

        let result = std::process::Command::new("pbcopy")
            .stdin(std::process::Stdio::piped())
            .spawn()
            .and_then(|mut child| {
                use std::io::Write;
                child.stdin.as_mut().unwrap().write_all(id.as_bytes())?;
                child.wait()
            });

        match result {
            Ok(_) => {
                self.notify(format!("Copied: {id}"));
            }
            Err(e) => {
                self.notify(format!("Copy failed: {e}"));
            }
        }
    }

    pub fn copy_worktree_path(&mut self) {
        let Some(issue) = self.selected_issue() else {
            return;
        };
        let job = self.impl_jobs.get(&issue.id);
        let Some(job) = job else {
            self.notify("No impl job found");
            return;
        };
        let path = job.worktree_path.display().to_string();

        let result = std::process::Command::new("pbcopy")
            .stdin(std::process::Stdio::piped())
            .spawn()
            .and_then(|mut child| {
                use std::io::Write;
                child.stdin.as_mut().unwrap().write_all(path.as_bytes())?;
                child.wait()
            });

        match result {
            Ok(_) => {
                self.notify(format!("Copied: {path}"));
            }
            Err(e) => {
                self.notify(format!("Copy failed: {e}"));
            }
        }
    }

    // --- Edit Description ---

    pub async fn edit_description(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    ) {
        let Some(issue) = self.selected_issue() else {
            return;
        };
        let issue_id = issue.id.clone();
        let current_title = issue.title.clone();
        let current_desc = issue.description.clone().unwrap_or_default();

        // 一時ファイルに書き出し（1行目: title, 2行目: 空行, 3行目以降: description）
        let content = format!("{}\n\n{}", current_title, current_desc);
        let tmp = std::env::temp_dir().join(format!("strand-{issue_id}.md"));
        if std::fs::write(&tmp, &content).is_err() {
            self.notify("Failed to create temp file");
            return;
        }

        // TUIを一時離脱してエディタ起動
        let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".into());
        disable_raw_mode().ok();
        stdout().execute(LeaveAlternateScreen).ok();
        terminal.show_cursor().ok();

        let status = std::process::Command::new(&editor).arg(&tmp).status();

        // TUI復帰
        stdout().execute(EnterAlternateScreen).ok();
        enable_raw_mode().ok();
        terminal.clear().ok();

        match status {
            Ok(s) if s.success() => {
                if let Ok(new_content) = std::fs::read_to_string(&tmp) {
                    // 1行目: title, 2行目: 空行, 3行目以降: description
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
}
