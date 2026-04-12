pub enum SplitEvent {
    Started { issue_id: String },
    Completed { issue_id: String, task_count: usize },
    Failed { issue_id: String, error: String },
}
