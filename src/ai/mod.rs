pub mod enrich;
pub mod implement;
pub mod job;
pub mod split;

use std::path::Path;

use tokio::sync::mpsc;

use crate::ai::enrich::EnrichManager;
use crate::ai::implement::ImplManager;
use crate::ai::split::SplitManager;

pub struct AiManagers {
    pub enrich: EnrichManager,
    pub impl_: ImplManager,
    pub split: SplitManager,
}

impl AiManagers {
    pub fn new(
        enrich_tx: mpsc::Sender<enrich::EnrichEvent>,
        impl_tx: mpsc::Sender<implement::ImplEvent>,
        split_tx: mpsc::Sender<split::SplitEvent>,
    ) -> Self {
        Self {
            enrich: EnrichManager::new(enrich_tx),
            impl_: ImplManager::new(impl_tx),
            split: SplitManager::new(split_tx),
        }
    }

    pub async fn restore_impl_jobs(&mut self, repo_dir: &Path, issue_ids: &[String]) {
        self.impl_.restore_jobs(repo_dir, issue_ids).await;
    }
}
