mod manager;
mod merge;
mod run;
mod worktree;

pub use manager::{ImplManager, ImplOutcome};
pub use run::ImplEvent;

use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum ImplStatus {
    Running,
    Done,
    Failed(String),
}

#[derive(Debug, Clone)]
pub struct ImplJob {
    pub issue_id: String,
    pub branch: String,
    pub worktree_path: PathBuf,
    pub status: ImplStatus,
    pub completed_at: Option<String>,
}

pub struct ImplRequest {
    pub issue_id: String,
    pub title: String,
    pub description: Option<String>,
    pub design: Option<String>,
    pub repo_dir: PathBuf,
    pub base_branch: String,
}

pub fn branch_name(issue_id: &str) -> String {
    format!("impl/{issue_id}")
}

pub fn epic_branch_name(epic_id: &str) -> String {
    format!("epic/{epic_id}")
}
