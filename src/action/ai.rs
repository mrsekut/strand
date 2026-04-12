use crate::ai::AiManagers;
use crate::ai::enrich::{self, EnrichOutcome};
use crate::ai::implement::{self, ImplOutcome};
use crate::ai::split::{self, SplitOutcome};
use crate::bd;
use crate::core::{Core, View};

pub fn start_enrich(core: &mut Core, ai: &mut AiManagers, issue_id: &str) {
    let Some(issue) = core.find_issue(issue_id) else {
        return;
    };
    ai.enrich.start(&issue, None);
}

pub fn auto_enrich(core: &Core, ai: &mut AiManagers) {
    ai.enrich.auto_enrich(&core.issue_store.issues, None);
}

pub async fn handle_enrich_event(core: &mut Core, ai: &mut AiManagers, event: enrich::EnrichEvent) {
    let outcome = ai.enrich.handle_event(event);
    match outcome {
        EnrichOutcome::Started { issue_id } => {
            core.notify(format!("Enriching: {issue_id}..."));
        }
        EnrichOutcome::Completed { issue_id } => {
            core.notify(format!("Enriched: {issue_id}"));
            let _ = core.load_issues().await;
        }
        EnrichOutcome::Failed { issue_id, error } => {
            core.notify(format!("Enrich failed: {issue_id}: {error}"));
        }
    }
}

pub fn start_split(core: &mut Core, ai: &mut AiManagers, issue_id: &str) {
    let Some(issue) = core.find_issue(issue_id) else {
        return;
    };
    ai.split.start(&issue, None);
}

pub async fn handle_split_event(core: &mut Core, ai: &mut AiManagers, event: split::SplitEvent) {
    let outcome = ai.split.handle_event(event);
    match outcome {
        SplitOutcome::Started { issue_id } => {
            core.notify(format!("Splitting: {issue_id}..."));
        }
        SplitOutcome::Completed {
            issue_id,
            task_count,
        } => {
            core.notify(format!("Split: {issue_id} → {task_count} tasks"));
            let _ = core.load_issues().await;
            let should_transition = matches!(
                &core.view,
                View::IssueDetail { issue_id: vid, .. } if *vid == issue_id
            );
            if should_transition {
                let children = bd::list_children(None, &issue_id).await.unwrap_or_default();
                if !children.is_empty() {
                    let ready_ids = bd::list_ready_ids(None, &issue_id)
                        .await
                        .unwrap_or_default();
                    core.view = View::EpicDetail {
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
            core.notify(format!("Split failed: {issue_id}: {error}"));
        }
    }
}

pub async fn start_implement(
    core: &mut Core,
    ai: &mut AiManagers,
    issue_id: &str,
    epic_id: Option<&str>,
) {
    let Some(issue) = core.find_issue(issue_id) else {
        return;
    };

    let repo_dir = Core::repo_dir();
    if let Err(e) = ai.impl_.start(&issue, epic_id, &repo_dir, None).await {
        core.notify(format!("Failed to start impl: {e}"));
    }
}

pub fn handle_impl_event(core: &mut Core, ai: &mut AiManagers, event: implement::ImplEvent) {
    let dir = Core::repo_dir().to_string_lossy().to_string();
    let outcome = ai.impl_.handle_event(event, &dir);
    match outcome {
        ImplOutcome::Started { issue_id } => {
            core.notify(format!("Implementing: {issue_id}..."));
        }
        ImplOutcome::SessionIdDiscovered { .. } => {}
        ImplOutcome::Completed { issue_id, summary } => {
            core.notify(format!(
                "Implementation done: {issue_id} (log: {} bytes)",
                summary.len()
            ));
        }
        ImplOutcome::Failed { issue_id, error } => {
            core.notify(format!("Implement failed: {issue_id}: {error}"));
        }
    }
}
