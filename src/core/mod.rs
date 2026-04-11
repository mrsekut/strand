mod filter;
mod issue_store;
mod view;

pub use filter::{Filter, STATUSES};
pub use issue_store::IssueStore;
pub use view::{ConfirmAction, View};

use std::time::Instant;

use crate::bd::Issue;
use crate::widget::keybar::KeyBar;

pub struct Core {
    pub issue_store: IssueStore,
    pub view: View,
    pub view_stack: Vec<View>,
    pub keybar: KeyBar,
    pub filter: Filter,
    pub notification: Option<(String, Instant)>,
}

impl Core {
    pub fn new() -> Self {
        Self {
            issue_store: IssueStore::new(),
            view: View::IssueList,
            view_stack: Vec::new(),
            keybar: KeyBar::Default,
            filter: Filter::new(),
            notification: None,
        }
    }

    /// 現在のview contextで対象となるissue_idを返す
    pub fn current_issue_id(&self) -> Option<String> {
        match &self.view {
            View::IssueDetail { issue_id, .. } => Some(issue_id.clone()),
            View::EpicDetail { epic_id, .. } => Some(epic_id.clone()),
            _ => self
                .issue_store
                .selected_issue(&self.filter)
                .map(|i| i.id.clone()),
        }
    }

    /// issue_id で Issue を検索する（top-level + 全 children）
    pub fn find_issue(&self, issue_id: &str) -> Option<Issue> {
        self.issue_store
            .issues
            .iter()
            .find(|i| i.id == issue_id)
            .cloned()
            .or_else(|| self.find_issue_in_stack(issue_id))
    }

    /// スタック内のEpicDetailのchildrenからissueを探す
    fn find_issue_in_stack(&self, issue_id: &str) -> Option<Issue> {
        for view in self.view_stack.iter().rev() {
            if let View::EpicDetail { children, .. } = view {
                if let Some(issue) = children.iter().find(|i| i.id == issue_id) {
                    return Some(issue.clone());
                }
            }
        }
        None
    }

    /// スタックを遡って直近のEpicDetailのepic_idを探す
    pub fn find_parent_epic_id(&self) -> Option<String> {
        for view in self.view_stack.iter().rev() {
            if let View::EpicDetail { epic_id, .. } = view {
                return Some(epic_id.clone());
            }
        }
        None
    }

    pub fn all_children_closed(&self) -> bool {
        match &self.view {
            View::EpicDetail { children, .. } => {
                !children.is_empty() && children.iter().all(|c| c.status == "closed")
            }
            _ => false,
        }
    }
}
