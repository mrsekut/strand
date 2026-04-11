use std::path::Path;
use std::time::SystemTime;

use crate::bd::Issue;
use crate::core::Filter;

pub struct IssueStore {
    pub issues: Vec<Issue>,
    pub selected: usize,
    pub last_db_mtime: Option<SystemTime>,
}

impl IssueStore {
    pub fn new() -> Self {
        Self {
            issues: Vec::new(),
            selected: 0,
            last_db_mtime: None,
        }
    }

    /// フィルタ適用済みのissueリスト
    pub fn displayed_issues(&self, filter: &Filter) -> Vec<&Issue> {
        if !filter.is_active() {
            self.issues.iter().collect()
        } else {
            self.issues.iter().filter(|i| filter.matches(i)).collect()
        }
    }

    pub fn selected_issue<'a>(&'a self, filter: &'a Filter) -> Option<&'a Issue> {
        self.displayed_issues(filter).get(self.selected).copied()
    }

    pub fn has_db_changed(&self, db_path: &Path) -> bool {
        let current = Self::db_mtime(db_path);
        match (&self.last_db_mtime, &current) {
            (Some(last), Some(now)) => now > last,
            (None, Some(_)) => true,
            _ => false,
        }
    }

    pub fn db_mtime(db_path: &Path) -> Option<SystemTime> {
        std::fs::metadata(db_path).and_then(|m| m.modified()).ok()
    }
}
