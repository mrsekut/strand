use crate::ai::enrich::{self, EnrichOutcome};
use crate::ai::implement::{self, ImplOutcome};
use crate::ai::split::{self, SplitOutcome};
use crate::app::App;
use crate::bd;
use crate::core::{Core, View};

// TODO: manager が必要なため &mut App を取る（manager 集約後に再検討）

pub fn start_enrich(app: &mut App, issue_id: &str) {
    let Some(issue) = app.core.find_issue(issue_id) else {
        return;
    };
    app.enrich_manager.start(&issue, None);
}

pub fn auto_enrich(app: &mut App) {
    app.enrich_manager
        .auto_enrich(&app.core.issue_store.issues, None);
}

pub async fn handle_enrich_event(app: &mut App, event: enrich::EnrichEvent) {
    let outcome = app.enrich_manager.handle_event(event);
    match outcome {
        EnrichOutcome::Started { issue_id } => {
            app.core.notify(format!("Enriching: {issue_id}..."));
        }
        EnrichOutcome::Completed { issue_id } => {
            app.core.notify(format!("Enriched: {issue_id}"));
            let _ = app.core.load_issues().await;
        }
        EnrichOutcome::Failed { issue_id, error } => {
            app.core
                .notify(format!("Enrich failed: {issue_id}: {error}"));
        }
    }
}

pub fn start_split(app: &mut App, issue_id: &str) {
    let Some(issue) = app.core.find_issue(issue_id) else {
        return;
    };
    app.split_manager.start(&issue, None);
}

pub async fn handle_split_event(app: &mut App, event: split::SplitEvent) {
    let outcome = app.split_manager.handle_event(event);
    match outcome {
        SplitOutcome::Started { issue_id } => {
            app.core.notify(format!("Splitting: {issue_id}..."));
        }
        SplitOutcome::Completed {
            issue_id,
            task_count,
        } => {
            app.core
                .notify(format!("Split: {issue_id} → {task_count} tasks"));
            let _ = app.core.load_issues().await;
            let should_transition = matches!(
                &app.core.view,
                View::IssueDetail { issue_id: vid, .. } if *vid == issue_id
            );
            if should_transition {
                let children = bd::list_children(None, &issue_id).await.unwrap_or_default();
                if !children.is_empty() {
                    let ready_ids = bd::list_ready_ids(None, &issue_id)
                        .await
                        .unwrap_or_default();
                    app.core.view = View::EpicDetail {
                        epic_id: issue_id,
                        children,
                        ready_ids,
                        child_selected: 0,
                        scroll_offset: 0,
                    };
                }
            }
        }
        SplitOutcome::Failed { issue_id, error } => {
            app.core
                .notify(format!("Split failed: {issue_id}: {error}"));
        }
    }
}

pub async fn start_implement(app: &mut App, issue_id: &str, epic_id: Option<&str>) {
    let Some(issue) = app.core.find_issue(issue_id) else {
        return;
    };

    let repo_dir = Core::repo_dir();
    if let Err(e) = app
        .impl_manager
        .start(&issue, epic_id, &repo_dir, None)
        .await
    {
        app.core.notify(format!("Failed to start impl: {e}"));
    }
}

pub fn handle_impl_event(app: &mut App, event: implement::ImplEvent) {
    let dir = Core::repo_dir().to_string_lossy().to_string();
    let outcome = app.impl_manager.handle_event(event, &dir);
    match outcome {
        ImplOutcome::Started { issue_id } => {
            app.core.notify(format!("Implementing: {issue_id}..."));
        }
        ImplOutcome::Completed { issue_id, summary } => {
            app.core.notify(format!(
                "Implementation done: {issue_id} (log: {} bytes)",
                summary.len()
            ));
        }
        ImplOutcome::Failed { issue_id, error } => {
            app.core
                .notify(format!("Implement failed: {issue_id}: {error}"));
        }
    }
}
