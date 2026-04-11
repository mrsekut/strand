use crate::ai::enrich::{self, EnrichOutcome};
use crate::ai::implement::{self, ImplOutcome};
use crate::ai::split::{self, SplitOutcome};
use crate::app::App;
use crate::bd;
use crate::core::View;

// TODO: Step 6 で &mut Core に変更（enrich_manager, split_manager, impl_manager, dir が必要）

pub fn start_enrich(app: &mut App, issue_id: &str) {
    let Some(issue) = app.find_issue(issue_id) else {
        return;
    };
    app.enrich_manager.start(&issue, app.dir.clone());
}

pub fn auto_enrich(app: &mut App) {
    app.enrich_manager
        .auto_enrich(&app.core.issue_store.issues, app.dir.clone());
}

pub async fn handle_enrich_event(app: &mut App, event: enrich::EnrichEvent) {
    let outcome = app.enrich_manager.handle_event(event);
    match outcome {
        EnrichOutcome::Started { issue_id } => {
            app.notify(format!("Enriching: {issue_id}..."));
        }
        EnrichOutcome::Completed { issue_id } => {
            app.notify(format!("Enriched: {issue_id}"));
            let _ = app.load_issues().await;
        }
        EnrichOutcome::Failed { issue_id, error } => {
            app.notify(format!("Enrich failed: {issue_id}: {error}"));
        }
    }
}

pub fn start_split(app: &mut App, issue_id: &str) {
    let Some(issue) = app.find_issue(issue_id) else {
        return;
    };
    app.split_manager.start(&issue, app.dir.clone());
}

pub async fn handle_split_event(app: &mut App, event: split::SplitEvent) {
    let outcome = app.split_manager.handle_event(event);
    match outcome {
        SplitOutcome::Started { issue_id } => {
            app.notify(format!("Splitting: {issue_id}..."));
        }
        SplitOutcome::Completed {
            issue_id,
            task_count,
        } => {
            app.notify(format!("Split: {issue_id} → {task_count} tasks"));
            let _ = app.load_issues().await;
            let should_transition = matches!(
                &app.core.view,
                View::IssueDetail { issue_id: vid, .. } if *vid == issue_id
            );
            if should_transition {
                let children = bd::list_children(app.dir.as_deref(), &issue_id)
                    .await
                    .unwrap_or_default();
                if !children.is_empty() {
                    let ready_ids = bd::list_ready_ids(app.dir.as_deref(), &issue_id)
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
            app.notify(format!("Split failed: {issue_id}: {error}"));
        }
    }
}

pub async fn start_implement(app: &mut App, issue_id: &str, epic_id: Option<&str>) {
    let Some(issue) = app.find_issue(issue_id) else {
        return;
    };

    let repo_dir = app.repo_dir();
    if let Err(e) = app
        .impl_manager
        .start(&issue, epic_id, &repo_dir, app.dir.clone())
        .await
    {
        app.notify(format!("Failed to start impl: {e}"));
    }
}

pub fn handle_impl_event(app: &mut App, event: implement::ImplEvent) {
    let dir = app.repo_dir().to_string_lossy().to_string();
    let outcome = app.impl_manager.handle_event(event, &dir);
    match outcome {
        ImplOutcome::Started { issue_id } => {
            app.notify(format!("Implementing: {issue_id}..."));
        }
        ImplOutcome::Completed { issue_id, summary } => {
            app.notify(format!(
                "Implementation done: {issue_id} (log: {} bytes)",
                summary.len()
            ));
        }
        ImplOutcome::Failed { issue_id, error } => {
            app.notify(format!("Implement failed: {issue_id}: {error}"));
        }
    }
}
