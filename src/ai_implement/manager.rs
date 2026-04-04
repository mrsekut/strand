use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use tokio::sync::mpsc;

use crate::bd::{self, Issue};

use super::run::{self, ImplEvent};
use super::worktree;
use super::{ImplJob, ImplRequest, ImplStatus, branch_name};

/// handle_eventの結果。app側が必要なアクションを判断するために使う。
pub enum ImplOutcome {
    Started { issue_id: String },
    Completed { issue_id: String, summary: String },
    Failed { issue_id: String, error: String },
}

pub struct ImplManager {
    jobs: HashMap<String, ImplJob>,
    tx: mpsc::Sender<ImplEvent>,
}

impl ImplManager {
    pub fn new(tx: mpsc::Sender<ImplEvent>) -> Self {
        Self {
            jobs: HashMap::new(),
            tx,
        }
    }

    pub fn get_job(&self, issue_id: &str) -> Option<&ImplJob> {
        self.jobs.get(issue_id)
    }

    /// 既存worktreeからジョブを復元
    pub async fn restore_jobs(&mut self, repo_dir: &Path, issue_ids: &[String]) {
        let discovered = worktree::discover_worktrees(repo_dir, issue_ids).await;
        for job in discovered {
            self.jobs.entry(job.issue_id.clone()).or_insert(job);
        }
    }

    /// impl開始。epicブランチ確保 + worktree作成 + Claude spawn
    pub async fn start(
        &mut self,
        issue: &Issue,
        epic_id: Option<&str>,
        repo_dir: &Path,
        _dir: Option<String>,
    ) -> Result<()> {
        if self.jobs.contains_key(&issue.id) {
            return Ok(());
        }

        let base_branch = if let Some(eid) = epic_id {
            worktree::ensure_epic_branch(repo_dir, eid).await?
        } else {
            "master".to_string()
        };

        let wt_path = worktree::worktree_path(repo_dir, &issue.id);
        let branch = branch_name(&issue.id);

        self.jobs.insert(
            issue.id.clone(),
            ImplJob {
                issue_id: issue.id.clone(),
                branch: branch.clone(),
                worktree_path: wt_path,
                status: ImplStatus::Running,
                completed_at: None,
                session_id: None,
            },
        );

        let request = ImplRequest {
            issue_id: issue.id.clone(),
            title: issue.title.clone(),
            description: issue.description.clone(),
            design: None,
            repo_dir: repo_dir.to_path_buf(),
            base_branch,
        };
        let tx = self.tx.clone();

        tokio::spawn(async move {
            let _ = run::run(request, tx).await;
        });

        Ok(())
    }

    /// merge: impl branchをtarget branchに統合、worktree/branch削除、issue close
    pub async fn merge(
        &mut self,
        issue_id: &str,
        epic_id: Option<&str>,
        repo_dir: &Path,
        dir: Option<&str>,
    ) -> Result<()> {
        let job = match self.jobs.get(issue_id) {
            Some(job) if matches!(job.status, ImplStatus::Done) => job.clone(),
            _ => return Ok(()),
        };

        let target = match epic_id {
            Some(eid) => super::epic_branch_name(eid),
            None => "master".to_string(),
        };
        super::merge::merge_into_branch(repo_dir, &job.branch, &target).await?;

        worktree::remove_worktree(repo_dir, &job.worktree_path).await?;
        let _ = worktree::delete_branch(repo_dir, &job.branch).await;
        let _ = bd::close_issue(dir, issue_id).await;

        self.jobs.remove(issue_id);
        Ok(())
    }

    /// discard: worktree/branch削除のみ（issueはcloseしない）
    pub async fn discard(&mut self, issue_id: &str, repo_dir: &Path) -> Result<()> {
        let job = match self.jobs.get(issue_id) {
            Some(job) => job.clone(),
            None => return Ok(()),
        };

        worktree::remove_worktree(repo_dir, &job.worktree_path).await?;
        let _ = worktree::delete_branch(repo_dir, &job.branch).await;

        self.jobs.remove(issue_id);
        Ok(())
    }

    /// イベント処理
    pub fn handle_event(&mut self, event: ImplEvent, dir: &str) -> ImplOutcome {
        match event {
            ImplEvent::Started { issue_id } => ImplOutcome::Started { issue_id },
            ImplEvent::Completed { issue_id, summary, session_id } => {
                if let Some(job) = self.jobs.get_mut(&issue_id) {
                    job.status = ImplStatus::Done;
                    job.completed_at = Some(chrono::Local::now().to_rfc3339());
                    if session_id.is_some() {
                        job.session_id = session_id;
                    }
                }
                // descriptionへのログ追記をspawn
                let id = issue_id.clone();
                let dir_owned = dir.to_string();
                let summary_clone = summary.clone();
                tokio::spawn(async move {
                    let content = format!("## Implementation Log\n{summary_clone}");
                    if let Err(e) = bd::append_to_description(Some(&dir_owned), &id, &content).await
                    {
                        eprintln!("Failed to append impl log to {id}: {e}");
                    }
                });
                ImplOutcome::Completed { issue_id, summary }
            }
            ImplEvent::Failed { issue_id, error, session_id } => {
                if let Some(job) = self.jobs.get_mut(&issue_id) {
                    job.status = ImplStatus::Failed(error.clone());
                    if session_id.is_some() {
                        job.session_id = session_id;
                    }
                }
                ImplOutcome::Failed { issue_id, error }
            }
        }
    }

    /// epicブランチをmasterにmerge
    pub async fn merge_epic(
        &mut self,
        epic_id: &str,
        repo_dir: &Path,
        dir: Option<&str>,
    ) -> Result<()> {
        if !worktree::epic_branch_exists(repo_dir, epic_id).await {
            let _ = bd::close_issue(dir, epic_id).await;
            anyhow::bail!("no_epic_branch");
        }

        super::merge::merge_epic_to_master(repo_dir, epic_id).await?;
        let _ = bd::close_issue(dir, epic_id).await;
        Ok(())
    }
}
