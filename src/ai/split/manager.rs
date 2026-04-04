use std::collections::HashSet;

use tokio::sync::mpsc;

use crate::bd::Issue;

use super::SplitRequest;
use super::run::{self, SplitEvent};

/// handle_eventの結果。app側が必要なアクションを判断するために使う。
pub enum SplitOutcome {
    Started { issue_id: String },
    Completed { issue_id: String, task_count: usize },
    Failed { issue_id: String, error: String },
}

pub struct SplitManager {
    splitting_ids: HashSet<String>,
    tx: mpsc::Sender<SplitEvent>,
}

impl SplitManager {
    pub fn new(tx: mpsc::Sender<SplitEvent>) -> Self {
        Self {
            splitting_ids: HashSet::new(),
            tx,
        }
    }

    #[allow(dead_code)]
    pub fn is_splitting(&self, issue_id: &str) -> bool {
        self.splitting_ids.contains(issue_id)
    }

    pub fn start(&mut self, issue: &Issue, dir: Option<String>) {
        if self.splitting_ids.contains(&issue.id) {
            return;
        }

        self.splitting_ids.insert(issue.id.clone());

        let request = SplitRequest {
            issue_id: issue.id.clone(),
            title: issue.title.clone(),
            description: issue.description.clone(),
        };
        let tx = self.tx.clone();

        tokio::spawn(async move {
            let _ = run::run(request, dir, tx).await;
        });
    }

    pub fn handle_event(&mut self, event: SplitEvent) -> SplitOutcome {
        match event {
            SplitEvent::Started { issue_id } => SplitOutcome::Started { issue_id },
            SplitEvent::Completed {
                issue_id,
                task_count,
            } => {
                self.splitting_ids.remove(&issue_id);
                SplitOutcome::Completed {
                    issue_id,
                    task_count,
                }
            }
            SplitEvent::Failed { issue_id, error } => {
                self.splitting_ids.remove(&issue_id);
                SplitOutcome::Failed { issue_id, error }
            }
        }
    }
}
