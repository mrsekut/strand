use std::collections::HashSet;

use crate::bd::Issue;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmAction {
    Merge,
    Discard,
    MergeEpic,
    Retry,
}

impl ConfirmAction {
    pub fn label(&self) -> &'static str {
        match self {
            ConfirmAction::Merge => "confirm merge",
            ConfirmAction::Discard => "confirm discard",
            ConfirmAction::MergeEpic => "confirm merge epic to master",
            ConfirmAction::Retry => "confirm retry",
        }
    }

    pub fn confirm_message(&self) -> &'static str {
        match self {
            ConfirmAction::Merge => "Merge? (y/n)",
            ConfirmAction::Discard => "Discard? (y/n)",
            ConfirmAction::MergeEpic => "Merge epic to master? (y/n)",
            ConfirmAction::Retry => "Retry? (y/n)",
        }
    }
}

#[derive(Debug)]
pub enum View {
    IssueList,
    IssueDetail {
        issue_id: String,
        scroll_offset: u16,
        diff: Option<Vec<u8>>,
    },
    EpicDetail {
        epic_id: String,
        children: Vec<Issue>,
        ready_ids: HashSet<String>,
        child_selected: usize,
        scroll_offset: u16,
    },
}
