pub enum EnrichEvent {
    Started { issue_id: String },
    Completed { issue_id: String },
    Failed { issue_id: String, error: String },
}
