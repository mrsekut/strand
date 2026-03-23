use std::collections::HashSet;
use std::time::Instant;

use anyhow::Result;
use tokio::sync::mpsc;

use crate::bd::{self, Issue};
use crate::enrich::{self, EnrichEvent};

pub struct App {
    pub issues: Vec<Issue>,
    pub selected: usize,
    pub show_detail: bool,
    pub dir: Option<String>,
    pub enrich_tx: mpsc::Sender<EnrichEvent>,
    pub enrich_rx: mpsc::Receiver<EnrichEvent>,
    pub enriching_ids: HashSet<String>,
    pub notification: Option<(String, Instant)>,
}

impl App {
    pub fn new(dir: Option<String>) -> Self {
        let (enrich_tx, enrich_rx) = mpsc::channel(32);
        Self {
            issues: Vec::new(),
            selected: 0,
            show_detail: false,
            dir,
            enrich_tx,
            enrich_rx,
            enriching_ids: HashSet::new(),
            notification: None,
        }
    }

    pub async fn load_issues(&mut self) -> Result<()> {
        self.issues = bd::list_issues(self.dir.as_deref()).await?;
        Ok(())
    }

    pub fn next(&mut self) {
        if !self.issues.is_empty() {
            self.selected = (self.selected + 1) % self.issues.len();
        }
    }

    pub fn previous(&mut self) {
        if !self.issues.is_empty() {
            self.selected = (self.selected + self.issues.len() - 1) % self.issues.len();
        }
    }

    pub fn toggle_detail(&mut self) {
        self.show_detail = !self.show_detail;
    }

    pub fn selected_issue(&self) -> Option<&Issue> {
        self.issues.get(self.selected)
    }

    pub fn start_enrich(&mut self) {
        let Some(issue) = self.selected_issue() else {
            return;
        };
        let issue_id = issue.id.clone();
        let title = issue.title.clone();
        let description = issue.description.clone();

        if self.enriching_ids.contains(&issue_id) {
            return;
        }

        self.enriching_ids.insert(issue_id.clone());

        let request = enrich::EnrichRequest {
            issue_id,
            title,
            description,
        };
        let dir = self.dir.clone();
        let tx = self.enrich_tx.clone();

        tokio::spawn(async move {
            let _ = enrich::run(request, dir, tx).await;
        });
    }

    pub async fn handle_enrich_event(&mut self, event: EnrichEvent) {
        match event {
            EnrichEvent::Started { issue_id } => {
                self.notification = Some((format!("Enriching: {issue_id}..."), Instant::now()));
            }
            EnrichEvent::Completed { issue_id } => {
                self.enriching_ids.remove(&issue_id);
                self.notification = Some((format!("Enriched: {issue_id}"), Instant::now()));
                // 書き戻された内容を反映するためissue一覧を再読み込み
                let _ = self.load_issues().await;
            }
            EnrichEvent::Failed { issue_id, error } => {
                self.enriching_ids.remove(&issue_id);
                self.notification = Some((
                    format!("Enrich failed: {issue_id}: {error}"),
                    Instant::now(),
                ));
            }
        }
    }
}
