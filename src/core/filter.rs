use std::collections::{BTreeSet, HashSet};

use crate::bd::Issue;

pub const STATUSES: [&str; 4] = ["open", "in_progress", "deferred", "closed"];

pub struct Filter {
    pub statuses: HashSet<String>,
    pub labels: HashSet<String>,
    pub available_labels: Vec<String>,
}

impl Filter {
    pub fn new() -> Self {
        Self {
            statuses: HashSet::new(),
            labels: HashSet::new(),
            available_labels: Vec::new(),
        }
    }

    pub fn is_active(&self) -> bool {
        !self.statuses.is_empty() || !self.labels.is_empty()
    }

    /// issueがフィルタ条件に合致するか（status: OR, label: OR）
    pub fn matches(&self, issue: &Issue) -> bool {
        if !self.statuses.is_empty() && !self.statuses.contains(&issue.status) {
            return false;
        }
        if !self.labels.is_empty() {
            let user_labels: HashSet<&str> = issue
                .labels
                .iter()
                .filter(|l| !l.starts_with("strand-"))
                .map(|l| l.as_str())
                .collect();
            if !self.labels.iter().any(|l| user_labels.contains(l.as_str())) {
                return false;
            }
        }
        true
    }

    pub fn clear(&mut self) {
        self.statuses.clear();
        self.labels.clear();
    }

    /// issueリストからstrand-*以外のlabelを収集してavailable_labelsを更新
    pub fn refresh_labels(&mut self, issues: &[Issue]) {
        let labels: BTreeSet<String> = issues
            .iter()
            .flat_map(|i| i.labels.iter())
            .filter(|l| !l.starts_with("strand-"))
            .cloned()
            .collect();
        self.available_labels = labels.into_iter().collect();
    }

    /// アクティブなフィルタの表示文字列
    pub fn display_text(&self) -> String {
        let mut parts = Vec::new();
        if !self.statuses.is_empty() {
            let mut s: Vec<&str> = self.statuses.iter().map(|s| s.as_str()).collect();
            s.sort();
            parts.push(format!("status:{}", s.join(",")));
        }
        if !self.labels.is_empty() {
            let mut l: Vec<&str> = self.labels.iter().map(|l| l.as_str()).collect();
            l.sort();
            parts.push(format!("label:{}", l.join(",")));
        }
        parts.join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_issue(status: &str, labels: &[&str]) -> Issue {
        Issue {
            id: "test-1".into(),
            title: "test".into(),
            status: status.into(),
            priority: None,
            description: None,
            labels: labels.iter().map(|l| l.to_string()).collect(),
            issue_type: None,
            updated_at: None,
            estimate: None,
        }
    }

    #[test]
    fn empty_filter_matches_all() {
        let f = Filter::new();
        assert!(f.matches(&make_issue("open", &[])));
        assert!(f.matches(&make_issue("closed", &["bug"])));
    }

    #[test]
    fn status_filter_is_or() {
        let mut f = Filter::new();
        f.statuses.insert("open".into());
        f.statuses.insert("in_progress".into());

        assert!(f.matches(&make_issue("open", &[])));
        assert!(f.matches(&make_issue("in_progress", &[])));
        assert!(!f.matches(&make_issue("closed", &[])));
    }

    #[test]
    fn label_filter_is_or() {
        let mut f = Filter::new();
        f.labels.insert("bug".into());
        f.labels.insert("ui".into());

        assert!(f.matches(&make_issue("open", &["bug"])));
        assert!(f.matches(&make_issue("open", &["ui"])));
        assert!(f.matches(&make_issue("open", &["bug", "ui"])));
        assert!(!f.matches(&make_issue("open", &["backend"])));
    }

    #[test]
    fn combined_filter_is_and_between_status_and_label() {
        let mut f = Filter::new();
        f.statuses.insert("open".into());
        f.labels.insert("bug".into());

        assert!(f.matches(&make_issue("open", &["bug"])));
        assert!(!f.matches(&make_issue("closed", &["bug"])));
        assert!(!f.matches(&make_issue("open", &["ui"])));
    }

    #[test]
    fn strand_labels_are_ignored() {
        let mut f = Filter::new();
        f.labels.insert("bug".into());

        // strand-unreadはフィルタ対象外
        assert!(!f.matches(&make_issue("open", &["strand-unread"])));
        assert!(f.matches(&make_issue("open", &["bug", "strand-unread"])));
    }

    #[test]
    fn refresh_labels_excludes_strand() {
        let mut f = Filter::new();
        let issues = vec![
            make_issue("open", &["bug", "strand-unread"]),
            make_issue("open", &["ui", "strand-enriched"]),
        ];
        f.refresh_labels(&issues);
        assert_eq!(f.available_labels, vec!["bug", "ui"]);
    }
}
