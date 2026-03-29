use std::collections::HashSet;

use tokio::sync::mpsc;

use crate::bd::Issue;

use super::EnrichRequest;
use super::run::{self, EnrichEvent};

/// handle_eventの結果。app側が必要なアクションを判断するために使う。
pub enum EnrichOutcome {
    Started { issue_id: String },
    Completed { issue_id: String },
    Failed { issue_id: String, error: String },
}

pub struct EnrichManager {
    enriching_ids: HashSet<String>,
    tx: mpsc::Sender<EnrichEvent>,
}

impl EnrichManager {
    pub fn new(tx: mpsc::Sender<EnrichEvent>) -> Self {
        Self {
            enriching_ids: HashSet::new(),
            tx,
        }
    }

    pub fn is_enriching(&self, issue_id: &str) -> bool {
        self.enriching_ids.contains(issue_id)
    }

    /// 手動enrich: issueを受け取って実行
    pub fn start(&mut self, issue: &Issue, dir: Option<String>) {
        if self.enriching_ids.contains(&issue.id) {
            return;
        }

        self.enriching_ids.insert(issue.id.clone());

        let request = EnrichRequest {
            issue_id: issue.id.clone(),
            title: issue.title.clone(),
            description: issue.description.clone(),
        };
        let tx = self.tx.clone();

        tokio::spawn(async move {
            let _ = run::run(request, dir, tx).await;
        });
    }

    /// 自動enrich: strand-needs-enrichラベルを持ち、まだenrich中でないissueを全て実行
    pub fn auto_enrich(&mut self, issues: &[Issue], dir: Option<String>) {
        let targets: Vec<Issue> = issues
            .iter()
            .filter(|issue| {
                issue.labels.contains(&"strand-needs-enrich".to_string())
                    && !self.enriching_ids.contains(&issue.id)
            })
            .cloned()
            .collect();

        for issue in targets {
            self.start(&issue, dir.clone());
        }
    }

    /// イベント処理。EnrichOutcomeでapp側に何が起きたか伝える。
    pub fn handle_event(&mut self, event: EnrichEvent) -> EnrichOutcome {
        match event {
            EnrichEvent::Started { issue_id } => EnrichOutcome::Started { issue_id },
            EnrichEvent::Completed { issue_id } => {
                self.enriching_ids.remove(&issue_id);
                EnrichOutcome::Completed { issue_id }
            }
            EnrichEvent::Failed { issue_id, error } => {
                self.enriching_ids.remove(&issue_id);
                EnrichOutcome::Failed { issue_id, error }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_issue(id: &str, labels: Vec<&str>) -> Issue {
        Issue {
            id: id.to_string(),
            title: format!("Issue {id}"),
            status: "open".to_string(),
            priority: Some(2),
            description: None,
            labels: labels.into_iter().map(String::from).collect(),
            issue_type: None,
            updated_at: None,
        }
    }

    #[tokio::test]
    async fn auto_enrich_filters_by_label() {
        let (tx, _rx) = mpsc::channel(32);
        let mut manager = EnrichManager::new(tx);

        let issues = vec![
            make_issue("a", vec!["strand-needs-enrich"]),
            make_issue("b", vec![]),
            make_issue("c", vec!["strand-needs-enrich", "other"]),
        ];

        manager.auto_enrich(&issues, None);

        assert!(manager.is_enriching("a"));
        assert!(!manager.is_enriching("b"));
        assert!(manager.is_enriching("c"));
    }

    #[tokio::test]
    async fn auto_enrich_skips_already_enriching() {
        let (tx, _rx) = mpsc::channel(32);
        let mut manager = EnrichManager::new(tx);

        let issues = vec![make_issue("a", vec!["strand-needs-enrich"])];

        manager.auto_enrich(&issues, None);
        assert!(manager.is_enriching("a"));

        // 2回目は重複しない（panicしない）
        manager.auto_enrich(&issues, None);
        assert!(manager.is_enriching("a"));
    }

    #[tokio::test]
    async fn start_skips_duplicate() {
        let (tx, _rx) = mpsc::channel(32);
        let mut manager = EnrichManager::new(tx);

        let issue = make_issue("a", vec![]);
        manager.start(&issue, None);
        assert!(manager.is_enriching("a"));

        // 2回目は何も起きない
        manager.start(&issue, None);
        assert!(manager.is_enriching("a"));
    }

    #[test]
    fn handle_completed_removes_from_enriching() {
        let (tx, _rx) = mpsc::channel(32);
        let mut manager = EnrichManager::new(tx);

        manager.enriching_ids.insert("a".to_string());
        assert!(manager.is_enriching("a"));

        let outcome = manager.handle_event(EnrichEvent::Completed {
            issue_id: "a".to_string(),
        });
        assert!(!manager.is_enriching("a"));
        assert!(matches!(outcome, EnrichOutcome::Completed { .. }));
    }

    #[test]
    fn handle_failed_removes_from_enriching() {
        let (tx, _rx) = mpsc::channel(32);
        let mut manager = EnrichManager::new(tx);

        manager.enriching_ids.insert("a".to_string());

        let outcome = manager.handle_event(EnrichEvent::Failed {
            issue_id: "a".to_string(),
            error: "timeout".to_string(),
        });
        assert!(!manager.is_enriching("a"));
        assert!(matches!(outcome, EnrichOutcome::Failed { .. }));
    }
}
