mod filter;
mod issue_store;
mod overlay;
mod view;

pub use filter::{Filter, STATUSES};
pub use issue_store::IssueStore;
pub use overlay::Overlay;
pub use view::{ConfirmAction, View};

use std::time::Instant;

pub struct Core {
    pub issue_store: IssueStore,
    pub view: View,
    pub view_stack: Vec<View>,
    pub overlay: Overlay,
    pub filter: Filter,
    pub notification: Option<(String, Instant)>,
}

impl Core {
    pub fn new() -> Self {
        Self {
            issue_store: IssueStore::new(),
            view: View::IssueList,
            view_stack: Vec::new(),
            overlay: Overlay::None,
            filter: Filter::new(),
            notification: None,
        }
    }
}
