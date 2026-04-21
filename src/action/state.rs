use crate::bd;
use crate::core::Core;

pub async fn set_status(core: &mut Core, issue_id: &str, status: &str) {
    if status == "closed" {
        match bd::close_issue(None, issue_id).await {
            Ok(_) => {
                core.notify(format!("Closed: {issue_id}"));
                let _ = core.load_issues().await;
                crate::action::navigate::reload_children(core).await;
                if core.issue_store.selected >= core.issue_store.issues.len()
                    && core.issue_store.selected > 0
                {
                    core.issue_store.selected -= 1;
                }
            }
            Err(e) => {
                core.notify(format!("Status change failed: {e}"));
            }
        }
    } else {
        match bd::update_status(None, issue_id, status).await {
            Ok(_) => {
                core.notify(format!("Status: {issue_id} → {status}"));
                let _ = core.load_issues().await;
                crate::action::navigate::reload_children(core).await;
            }
            Err(e) => {
                core.notify(format!("Status change failed: {e}"));
            }
        }
    }
}

pub async fn set_priority(core: &mut Core, issue_id: &str, priority: u8) {
    match bd::update_priority(None, issue_id, priority).await {
        Ok(_) => {
            core.notify(format!("Priority set: {issue_id} → P{priority}"));
            let _ = core.load_issues().await;
        }
        Err(e) => {
            core.notify(format!("Priority update failed: {e}"));
        }
    }
}

pub async fn set_estimate(core: &mut Core, issue_id: &str, minutes: u32) {
    match bd::update_estimate(None, issue_id, minutes).await {
        Ok(_) => {
            if minutes == 0 {
                core.notify(format!("Estimate cleared: {issue_id}"));
            } else {
                core.notify(format!("Estimate set: {issue_id} → {minutes}m"));
            }
            let _ = core.load_issues().await;
        }
        Err(e) => {
            core.notify(format!("Estimate update failed: {e}"));
        }
    }
}
