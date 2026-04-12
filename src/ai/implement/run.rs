pub enum ImplEvent {
    Started {
        issue_id: String,
    },
    SessionIdDiscovered {
        issue_id: String,
        session_id: String,
    },
    Completed {
        issue_id: String,
        summary: String,
        session_id: Option<String>,
    },
    Failed {
        issue_id: String,
        error: String,
        session_id: Option<String>,
    },
}
