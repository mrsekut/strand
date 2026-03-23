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
pub enum InputMode {
    Normal,
    AwaitingAI,
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
        }
    }

    pub async fn load_issues(&mut self) -> Result<()> {
        self.issues = bd::list_issues(self.dir.as_deref()).await?;
        self.last_db_mtime = self.db_mtime();
        Ok(())
    }

    fn beads_db_path(&self) -> PathBuf {
        let base = match &self.dir {
            Some(d) => PathBuf::from(d),
            None => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        };
        base.join(".beads").join("beads.db")
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
            self.selected = (self.selected + 1) % self.issues.len();
        }
    }

    pub fn previous(&mut self) {
        if !self.issues.is_empty() {
            self.selected = (self.selected + self.issues.len() - 1) % self.issues.len();
        }
    }

    pub fn open_detail(&mut self) {
        self.show_detail = true;
    }

    pub fn back_to_list(&mut self) {
        self.show_detail = false;
    }

    pub fn selected_issue(&self) -> Option<&Issue> {
        self.issues.get(self.selected)
    }

    // --- Enrich ---

    pub fn start_enrich(&mut self) {
        let Some(issue) = self.selected_issue() else {
            return;
        };
        let issue_id = issue.id.clone();
        let title = issue.title.clone();
        let description = issue.description.clone();

        if self.enriching_ids.contains(&issue_id) {
            return;
        }

        self.enriching_ids.insert(issue_id.clone());

        let request = enrich::EnrichRequest {
            issue_id,
            title,
            description,
        };
        let dir = self.dir.clone();
        let tx = self.enrich_tx.clone();

        tokio::spawn(async move {
            let _ = enrich::run(request, dir, tx).await;
        });
    }

    pub async fn handle_enrich_event(&mut self, event: EnrichEvent) {
        match event {
            EnrichEvent::Started { issue_id } => {
                self.notification = Some((format!("Enriching: {issue_id}..."), Instant::now()));
            }
            EnrichEvent::Completed { issue_id } => {
                self.enriching_ids.remove(&issue_id);
                self.notification = Some((format!("Enriched: {issue_id}"), Instant::now()));
                let _ = self.load_issues().await;
            }
            EnrichEvent::Failed { issue_id, error } => {
                self.enriching_ids.remove(&issue_id);
                self.notification = Some((
                    format!("Enrich failed: {issue_id}: {error}"),
                    Instant::now(),
                ));
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
        let Some(issue) = self.selected_issue() else {
            return;
        };
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
            },
        );

        let request = implement::ImplRequest {
            issue_id: issue_id.clone(),
            title,
            description,
            design: None, // TODO: designフィールドがIssueに追加されたら対応
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
                self.notification = Some((format!("Implementing: {issue_id}..."), Instant::now()));
            }
            ImplEvent::Completed { issue_id } => {
                if let Some(job) = self.impl_jobs.get_mut(&issue_id) {
                    job.status = ImplStatus::Done;
                }
                self.notification =
                    Some((format!("Implementation done: {issue_id}"), Instant::now()));
            }
            ImplEvent::Failed { issue_id, error } => {
                if let Some(job) = self.impl_jobs.get_mut(&issue_id) {
                    job.status = ImplStatus::Failed(error.clone());
                }
                self.notification = Some((
                    format!("Implement failed: {issue_id}: {error}"),
                    Instant::now(),
                ));
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
            self.notification = Some((format!("Merge failed: {e}"), Instant::now()));
            return;
        }

        if let Err(e) = implement::remove_worktree(&repo_dir, &job.worktree_path).await {
            self.notification = Some((format!("Worktree remove failed: {e}"), Instant::now()));
            return;
        }

        let _ = implement::delete_branch(&repo_dir, &job.branch).await;
        let _ = bd::close_issue(self.dir.as_deref(), &issue_id).await;

        self.impl_jobs.remove(&issue_id);
        self.notification = Some((format!("Merged & closed: {issue_id}"), Instant::now()));
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
            self.notification = Some((format!("Worktree remove failed: {e}"), Instant::now()));
            return;
        }

        let _ = implement::delete_branch(&repo_dir, &job.branch).await;

        self.impl_jobs.remove(&issue_id);
        self.notification = Some((format!("Discarded: {issue_id}"), Instant::now()));
    }

    // --- Copy ID ---

    pub fn copy_id(&mut self) {
        let Some(issue) = self.selected_issue() else {
            return;
        };
        let id = issue.id.clone();

        let result = if cfg!(target_os = "macos") {
            std::process::Command::new("pbcopy")
                .stdin(std::process::Stdio::piped())
                .spawn()
                .and_then(|mut child| {
                    use std::io::Write;
                    child.stdin.as_mut().unwrap().write_all(id.as_bytes())?;
                    child.wait()
                })
        } else {
            std::process::Command::new("xclip")
                .args(["-selection", "clipboard"])
                .stdin(std::process::Stdio::piped())
                .spawn()
                .and_then(|mut child| {
                    use std::io::Write;
                    child.stdin.as_mut().unwrap().write_all(id.as_bytes())?;
                    child.wait()
                })
        };

        match result {
            Ok(_) => {
                self.notification = Some((format!("Copied: {id}"), Instant::now()));
            }
            Err(e) => {
                self.notification = Some((format!("Copy failed: {e}"), Instant::now()));
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
            self.notification = Some(("Failed to create temp file".into(), Instant::now()));
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
                                self.notification =
                                    Some((format!("Title update failed: {e}"), Instant::now()));
                                ok = false;
                            }
                        }
                        if desc_changed {
                            if let Err(e) =
                                bd::update_description(self.dir.as_deref(), &issue_id, &new_desc)
                                    .await
                            {
                                self.notification = Some((
                                    format!("Description update failed: {e}"),
                                    Instant::now(),
                                ));
                                ok = false;
                            }
                        }
                        if ok {
                            self.notification =
                                Some((format!("Updated: {issue_id}"), Instant::now()));
                            let _ = self.load_issues().await;
                        }
                    }
                }
            }
            _ => {
                self.notification = Some(("Editor exited with error".into(), Instant::now()));
            }
        }

        let _ = std::fs::remove_file(&tmp);
    }
}
