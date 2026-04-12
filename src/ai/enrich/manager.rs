use std::collections::HashSet;
use std::sync::Arc;

use tokio::sync::mpsc;

use crate::ai::job;
use crate::bd::Issue;
use crate::config::EnrichConfig;

use super::handler::EnrichHandler;
use super::run::EnrichEvent;

/// handle_eventの結果。app側が必要なアクションを判断するために使う。
pub enum EnrichOutcome {
    Started { issue_id: String },
    Completed { issue_id: String },
    Failed { issue_id: String, error: String },
}

pub struct EnrichManager {
    handler: Arc<EnrichHandler>,
    enriching_ids: HashSet<String>,
    tx: mpsc::Sender<EnrichEvent>,
    config: EnrichConfig,
}

impl EnrichManager {
    pub fn new(tx: mpsc::Sender<EnrichEvent>, config: EnrichConfig) -> Self {
        Self {
            handler: Arc::new(EnrichHandler),
            enriching_ids: HashSet::new(),
            tx,
            config,
        }
    }

    pub fn is_enriching(&self, issue_id: &str) -> bool {
        self.enriching_ids.contains(issue_id)
    }

    /// 手動enrich: issueを受け取って実行（デタッチ方式）
    pub fn start(&mut self, issue: &Issue, _dir: Option<String>) {
        if self.enriching_ids.contains(&issue.id) {
            return;
        }

        self.enriching_ids.insert(issue.id.clone());

        let handler = Arc::clone(&self.handler);
        let issue = issue.clone();
        let tx = self.tx.clone();
        let config = EnrichConfig {
            skill: self.config.skill.clone(),
        };

        tokio::spawn(async move {
            if let Err(e) = job::start_job(&handler, &issue, &config, &tx).await {
                let _ = tx
                    .send(EnrichEvent::Failed {
                        issue_id: issue.id,
                        error: e.to_string(),
                    })
                    .await;
            }
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

    /// 再起動時にジョブを復元
    pub async fn restore_jobs(&mut self) {
        let active_jobs = job::restore_jobs(&self.handler, &self.tx).await;
        for aj in &active_jobs {
            self.enriching_ids.insert(aj.issue_id.clone());
        }
    }

    /// イベント処理
    pub fn handle_event(&mut self, event: EnrichEvent) -> EnrichOutcome {
        match event {
            EnrichEvent::Started { issue_id } => EnrichOutcome::Started { issue_id },
            EnrichEvent::Completed { issue_id } => {
                self.enriching_ids.remove(&issue_id);
                // on_completed で既に description + label 更新済み
                // job ディレクトリ削除
                if let Ok(jobs_dir) = job::ensure_strand_dir() {
                    let short_id = crate::bd::short_id(&issue_id);
                    let job_dir = job::job_dir_path(&jobs_dir, "enrich", short_id);
                    job::cleanup_job(&job_dir);
                }
                EnrichOutcome::Completed { issue_id }
            }
            EnrichEvent::Failed { issue_id, error } => {
                self.enriching_ids.remove(&issue_id);
                if let Ok(jobs_dir) = job::ensure_strand_dir() {
                    let short_id = crate::bd::short_id(&issue_id);
                    let job_dir = job::job_dir_path(&jobs_dir, "enrich", short_id);
                    job::cleanup_job(&job_dir);
                }
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
        let mut manager = EnrichManager::new(tx, EnrichConfig::default());

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
        let mut manager = EnrichManager::new(tx, EnrichConfig::default());

        let issues = vec![make_issue("a", vec!["strand-needs-enrich"])];

        manager.auto_enrich(&issues, None);
        assert!(manager.is_enriching("a"));

        manager.auto_enrich(&issues, None);
        assert!(manager.is_enriching("a"));
    }

    #[tokio::test]
    async fn start_skips_duplicate() {
        let (tx, _rx) = mpsc::channel(32);
        let mut manager = EnrichManager::new(tx, EnrichConfig::default());

        let issue = make_issue("a", vec![]);
        manager.start(&issue, None);
        assert!(manager.is_enriching("a"));

        manager.start(&issue, None);
        assert!(manager.is_enriching("a"));
    }

    #[test]
    fn handle_completed_removes_from_enriching() {
        let (tx, _rx) = mpsc::channel(32);
        let mut manager = EnrichManager::new(tx, EnrichConfig::default());

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
        let mut manager = EnrichManager::new(tx, EnrichConfig::default());

        manager.enriching_ids.insert("a".to_string());

        let outcome = manager.handle_event(EnrichEvent::Failed {
            issue_id: "a".to_string(),
            error: "timeout".to_string(),
        });
        assert!(!manager.is_enriching("a"));
        assert!(matches!(outcome, EnrichOutcome::Failed { .. }));
    }
}
