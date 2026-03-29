mod manager;
mod prompt;
mod run;

pub use manager::{SplitManager, SplitOutcome};
pub use run::SplitEvent;

pub struct SplitRequest {
    pub issue_id: String,
    pub title: String,
    pub description: Option<String>,
}
