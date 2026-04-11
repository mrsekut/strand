use tokio::sync::mpsc;

use crate::ai::AiManagers;
use crate::ai::enrich;
use crate::ai::implement;
use crate::ai::split;
use crate::core::Core;

pub struct App {
    pub core: Core,
    pub ai: AiManagers,
    pub enrich_rx: mpsc::Receiver<enrich::EnrichEvent>,
    pub impl_rx: mpsc::Receiver<implement::ImplEvent>,
    pub split_rx: mpsc::Receiver<split::SplitEvent>,
}

impl App {
    pub fn new() -> Self {
        let (enrich_tx, enrich_rx) = mpsc::channel(32);
        let (impl_tx, impl_rx) = mpsc::channel(32);
        let (split_tx, split_rx) = mpsc::channel(32);
        Self {
            core: Core::new(),
            ai: AiManagers::new(enrich_tx, impl_tx, split_tx),
            enrich_rx,
            impl_rx,
            split_rx,
        }
    }
}
