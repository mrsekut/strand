use std::collections::HashSet;
use std::sync::Arc;

use tokio::sync::mpsc;

use crate::ai::job;
use crate::bd::Issue;

use super::handler::SplitHandler;
use super::run::SplitEvent;

/// handle_eventの結果。app側が必要なアクションを判断するために使う。
pub enum SplitOutcome {
    Started { issue_id: String },
    Completed { issue_id: String, task_count: usize },
    Failed { issue_id: String, error: String },
}

pub struct SplitManager {
    handler: Arc<SplitHandler>,
    splitting_ids: HashSet<String>,
    tx: mpsc::Sender<SplitEvent>,
}

impl SplitManager {
    pub fn new(tx: mpsc::Sender<SplitEvent>) -> Self {
        Self {
            handler: Arc::new(SplitHandler),
            splitting_ids: HashSet::new(),
            tx,
        }
    }

    #[allow(dead_code)]
    pub fn is_splitting(&self, issue_id: &str) -> bool {
        self.splitting_ids.contains(issue_id)
    }

    pub fn start(&mut self, issue: &Issue, _dir: Option<String>) {
        if self.splitting_ids.contains(&issue.id) {
            return;
        }

        self.splitting_ids.insert(issue.id.clone());

        let handler = Arc::clone(&self.handler);
        let issue = issue.clone();
        let tx = self.tx.clone();

        tokio::spawn(async move {
            if let Err(e) = job::start_job(&handler, &issue, &(), &tx).await {
                let _ = tx
                    .send(SplitEvent::Failed {
                        issue_id: issue.id,
                        error: e.to_string(),
                    })
                    .await;
            }
        });
    }

    /// 再起動時にジョブを復元
    pub async fn restore_jobs(&mut self) {
        let active_jobs = job::restore_jobs(&self.handler, &self.tx).await;
        for aj in &active_jobs {
            self.splitting_ids.insert(aj.issue_id.clone());
        }
    }

    pub fn handle_event(&mut self, event: SplitEvent) -> SplitOutcome {
        match event {
            SplitEvent::Started { issue_id } => SplitOutcome::Started { issue_id },
            SplitEvent::Completed {
                issue_id,
                task_count,
            } => {
                self.splitting_ids.remove(&issue_id);
                // on_completed で既に子 issue 作成済み
                if let Ok(jobs_dir) = job::ensure_strand_dir() {
                    let short_id = crate::bd::short_id(&issue_id);
                    let job_dir = job::job_dir_path(&jobs_dir, "split", short_id);
                    job::cleanup_job(&job_dir);
                }
                SplitOutcome::Completed {
                    issue_id,
                    task_count,
                }
            }
            SplitEvent::Failed { issue_id, error } => {
                self.splitting_ids.remove(&issue_id);
                if let Ok(jobs_dir) = job::ensure_strand_dir() {
                    let short_id = crate::bd::short_id(&issue_id);
                    let job_dir = job::job_dir_path(&jobs_dir, "split", short_id);
                    job::cleanup_job(&job_dir);
                }
                SplitOutcome::Failed { issue_id, error }
            }
        }
    }
}
