use tokio::sync::mpsc;

use crate::ai::enrich::{self, EnrichManager};
use crate::ai::implement::{self, ImplManager};
use crate::ai::split::{self, SplitManager};
use crate::core::Core;

pub struct App {
    pub core: Core,
    pub enrich_manager: EnrichManager,
    pub enrich_rx: mpsc::Receiver<enrich::EnrichEvent>,
    pub impl_manager: ImplManager,
    pub impl_rx: mpsc::Receiver<implement::ImplEvent>,
    pub split_manager: SplitManager,
    pub split_rx: mpsc::Receiver<split::SplitEvent>,
}

impl App {
    pub fn new() -> Self {
        let (enrich_tx, enrich_rx) = mpsc::channel(32);
        let (impl_tx, impl_rx) = mpsc::channel(32);
        let (split_tx, split_rx) = mpsc::channel(32);
        Self {
            core: Core::new(),
            enrich_manager: EnrichManager::new(enrich_tx),
            enrich_rx,
            impl_manager: ImplManager::new(impl_tx),
            impl_rx,
            split_manager: SplitManager::new(split_tx),
            split_rx,
        }
    }

    pub async fn restore_impl_jobs(&mut self) {
        let repo_dir = Core::repo_dir();
        let issue_ids: Vec<String> = self
            .core
            .issue_store
            .issues
            .iter()
            .map(|i| i.id.clone())
            .collect();
        self.impl_manager.restore_jobs(&repo_dir, &issue_ids).await;
    }
}
