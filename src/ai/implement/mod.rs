pub mod handler;
mod manager;
mod merge;
pub mod run;
pub mod worktree;

pub use manager::{ImplManager, ImplOutcome};
pub use run::ImplEvent;

use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum ImplStatus {
    Running,
    Done,
    Failed(String),
    #[allow(dead_code)]
    Interrupted,
}

#[derive(Debug, Clone)]
pub struct ImplJob {
    #[allow(dead_code)]
    pub issue_id: String,
    pub branch: String,
    pub worktree_path: PathBuf,
    pub status: ImplStatus,
    pub completed_at: Option<String>,
    pub session_id: Option<String>,
}

pub fn branch_name(issue_id: &str) -> String {
    format!("impl/{issue_id}")
}

pub fn epic_branch_name(epic_id: &str) -> String {
    format!("epic/{epic_id}")
}
