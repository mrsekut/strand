use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use tokio::sync::mpsc;

use crate::ai::job;
use crate::bd::{self, Issue};

use super::handler::{ImplConfig, ImplHandler};
use super::run::ImplEvent;
use super::worktree;
use super::{ImplJob, ImplStatus, branch_name};

/// handle_eventの結果。app側が必要なアクションを判断するために使う。
pub enum ImplOutcome {
    Started { issue_id: String },
    Completed { issue_id: String, summary: String },
    Failed { issue_id: String, error: String },
}

pub struct ImplManager {
    handler: Arc<ImplHandler>,
    jobs: HashMap<String, ImplJob>,
    tx: mpsc::Sender<ImplEvent>,
}

impl ImplManager {
    pub fn new(tx: mpsc::Sender<ImplEvent>) -> Self {
        Self {
            handler: Arc::new(ImplHandler),
            jobs: HashMap::new(),
            tx,
        }
    }

    pub fn get_job(&self, issue_id: &str) -> Option<&ImplJob> {
        self.jobs.get(issue_id)
    }

    /// 既存の .strand/jobs/ + worktree からジョブを復元
    pub async fn restore_jobs(&mut self, repo_dir: &Path, issue_ids: &[String]) {
        // 新方式: .strand/jobs/ からの復元
        let active_jobs = job::restore_jobs(&self.handler, &self.tx).await;
        for aj in &active_jobs {
            let branch = branch_name(&aj.issue_id);
            let wt_path = aj
                .worktree_path
                .as_ref()
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| worktree::worktree_path(repo_dir, &aj.issue_id));

            self.jobs.entry(aj.issue_id.clone()).or_insert(ImplJob {
                issue_id: aj.issue_id.clone(),
                branch,
                worktree_path: wt_path,
                status: ImplStatus::Running,
                completed_at: None,
                session_id: None,
            });
        }

        // 旧方式: worktree からの復元（互換性）
        let discovered = worktree::discover_worktrees(repo_dir, issue_ids).await;
        for job in discovered {
            self.jobs.entry(job.issue_id.clone()).or_insert(job);
        }
    }

    /// impl開始（デタッチ方式）
    pub async fn start(
        &mut self,
        issue: &Issue,
        epic_id: Option<&str>,
        _repo_dir: &Path,
        _dir: Option<String>,
    ) -> Result<()> {
        if self.jobs.contains_key(&issue.id) {
            return Ok(());
        }

        let config = ImplConfig {
            epic_id: epic_id.map(|s| s.to_string()),
        };

        let job_meta = job::start_job(&self.handler, issue, &config, &self.tx).await?;

        let branch = branch_name(&issue.id);
        let wt_path = job_meta
            .worktree_path
            .as_ref()
            .map(std::path::PathBuf::from)
            .unwrap_or_default();

        self.jobs.insert(
            issue.id.clone(),
            ImplJob {
                issue_id: issue.id.clone(),
                branch,
                worktree_path: wt_path,
                status: ImplStatus::Running,
                completed_at: None,
                session_id: None,
            },
        );

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

        // job ディレクトリも削除
        if let Ok(jobs_dir) = job::ensure_strand_dir() {
            let short_id = crate::bd::short_id(issue_id);
            let job_dir = job::job_dir_path(&jobs_dir, "impl", short_id);
            job::cleanup_job(&job_dir);
        }

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

        // job ディレクトリも削除
        if let Ok(jobs_dir) = job::ensure_strand_dir() {
            let short_id = crate::bd::short_id(issue_id);
            let job_dir = job::job_dir_path(&jobs_dir, "impl", short_id);
            job::cleanup_job(&job_dir);
        }

        self.jobs.remove(issue_id);
        Ok(())
    }

    /// イベント処理
    pub fn handle_event(&mut self, event: ImplEvent, _dir: &str) -> ImplOutcome {
        match event {
            ImplEvent::Started { issue_id } => ImplOutcome::Started { issue_id },
            ImplEvent::Completed {
                issue_id,
                summary,
                session_id,
            } => {
                if let Some(job) = self.jobs.get_mut(&issue_id) {
                    job.status = ImplStatus::Done;
                    job.completed_at = Some(chrono::Local::now().to_rfc3339());
                    if session_id.is_some() {
                        job.session_id = session_id;
                    }
                }
                // on_completed で既に description 追記済み
                ImplOutcome::Completed { issue_id, summary }
            }
            ImplEvent::Failed {
                issue_id,
                error,
                session_id,
            } => {
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
