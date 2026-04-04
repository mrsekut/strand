mod manager;
mod prompt;
mod run;

pub use manager::{EnrichManager, EnrichOutcome};
pub use run::EnrichEvent;

pub struct EnrichRequest {
    pub issue_id: String,
    pub title: String,
    pub description: Option<String>,
}
