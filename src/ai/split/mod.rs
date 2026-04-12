pub mod handler;
mod manager;
pub mod prompt;
mod run;

pub use manager::{SplitManager, SplitOutcome};
pub use run::SplitEvent;

pub struct SplitRequest {
    pub title: String,
    pub description: Option<String>,
}
