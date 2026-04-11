use std::path::PathBuf;

use anyhow::Result;
use tokio::sync::mpsc;

use crate::ai::enrich::{self, EnrichManager};
use crate::ai::implement::{self, ImplManager};
use crate::ai::split::{self, SplitManager};
use crate::bd::{self, Issue};
use crate::core::Core;

pub struct App {
    pub core: Core,
    pub enrich_manager: EnrichManager,
    pub enrich_rx: mpsc::Receiver<enrich::EnrichEvent>,
    pub impl_manager: ImplManager,
    pub impl_rx: mpsc::Receiver<implement::ImplEvent>,
    pub split_manager: SplitManager,
    pub split_rx: mpsc::Receiver<split::SplitEvent>,
    pub dir: Option<String>,
}

impl App {
    pub fn new(dir: Option<String>) -> Self {
        let (enrich_tx, enrich_rx) = mpsc::channel(32);
        let (impl_tx, impl_rx) = mpsc::channel(32);
        let (split_tx, split_rx) = mpsc::channel(32);
        Self {
            core: Core::new(),
            dir,
            enrich_manager: EnrichManager::new(enrich_tx),
            enrich_rx,
            impl_manager: ImplManager::new(impl_tx),
            impl_rx,
            split_manager: SplitManager::new(split_tx),
            split_rx,
        }
    }

    pub fn notify(&mut self, msg: impl Into<String>) {
        self.core.notification = Some((msg.into(), std::time::Instant::now()));
    }

    pub async fn load_issues(&mut self) -> Result<()> {
        self.core.issue_store.issues = bd::list_issues(self.dir.as_deref()).await?;
        self.core.issue_store.last_db_mtime =
            crate::core::IssueStore::db_mtime(&self.beads_db_path());
        Ok(())
    }

    pub async fn restore_impl_jobs(&mut self) {
        let repo_dir = self.repo_dir();
        let issue_ids: Vec<String> = self
            .core
            .issue_store
            .issues
            .iter()
            .map(|i| i.id.clone())
            .collect();
        self.impl_manager.restore_jobs(&repo_dir, &issue_ids).await;
    }

    pub fn repo_dir(&self) -> PathBuf {
        match &self.dir {
            Some(d) => PathBuf::from(d),
            None => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        }
    }

    fn beads_db_path(&self) -> PathBuf {
        self.repo_dir().join(".beads").join("beads.db")
    }

    pub fn has_db_changed(&self) -> bool {
        self.core.issue_store.has_db_changed(&self.beads_db_path())
    }

    pub fn find_parent_epic_id(&self) -> Option<String> {
        self.core.find_parent_epic_id()
    }

    pub fn selected_issue(&self) -> Option<&Issue> {
        self.core.issue_store.selected_issue(&self.core.filter)
    }

    pub fn current_issue_id(&self) -> Option<String> {
        self.core.current_issue_id()
    }

    pub fn find_issue(&self, issue_id: &str) -> Option<Issue> {
        self.core.find_issue(issue_id)
    }
}
