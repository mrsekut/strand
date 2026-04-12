pub mod enrich;
pub mod implement;
pub mod job;
pub mod split;

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

    pub async fn restore_jobs(&mut self) {
        self.impl_.restore_jobs().await;
        self.enrich.restore_jobs().await;
        self.split.restore_jobs().await;
    }
}
