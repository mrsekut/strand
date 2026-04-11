use std::time::SystemTime;

use crate::bd::Issue;

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
}
