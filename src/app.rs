use anyhow::Result;

use crate::bd::{self, Issue};

pub struct App {
    pub issues: Vec<Issue>,
    pub selected: usize,
    pub show_detail: bool,
    pub dir: Option<String>,
}

impl App {
    pub fn new(dir: Option<String>) -> Self {
        Self {
            issues: Vec::new(),
            selected: 0,
            show_detail: false,
            dir,
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
}
