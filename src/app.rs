use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::Instant;

use anyhow::Result;
use tokio::sync::mpsc;

use crate::bd::{self, Issue};
use crate::enrich::{self, EnrichEvent};
use crate::implement::{self, ImplEvent, ImplJob, ImplStatus};

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
        }
    }

    pub async fn load_issues(&mut self) -> Result<()> {
        self.issues = bd::list_issues(self.dir.as_deref()).await?;
        Ok(())
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

    pub fn toggle_detail(&mut self) {
        self.show_detail = !self.show_detail;
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
}
