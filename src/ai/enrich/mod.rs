pub mod handler;
mod manager;
pub mod prompt;
mod run;

pub use manager::{EnrichManager, EnrichOutcome};
pub use run::EnrichEvent;

pub struct EnrichRequest {
    pub title: String,
    pub description: Option<String>,
}
