use crate::app::App;
use crate::bd;

// TODO: Step 6 で &mut Core に変更（dir, bd CLI, load_issues が必要）

pub async fn set_status(app: &mut App, issue_id: &str, status: &str) {
    if status == "closed" {
        match bd::close_issue(app.dir.as_deref(), issue_id).await {
            Ok(_) => {
                app.notify(format!("Closed: {issue_id}"));
                let _ = app.load_issues().await;
                crate::action::navigate::reload_children(app).await;
                if app.core.issue_store.selected >= app.core.issue_store.issues.len()
                    && app.core.issue_store.selected > 0
                {
                    app.core.issue_store.selected -= 1;
                }
            }
            Err(e) => {
                app.notify(format!("Status change failed: {e}"));
            }
        }
    } else {
        match bd::update_status(app.dir.as_deref(), issue_id, status).await {
            Ok(_) => {
                app.notify(format!("Status: {issue_id} → {status}"));
                let _ = app.load_issues().await;
                crate::action::navigate::reload_children(app).await;
            }
            Err(e) => {
                app.notify(format!("Status change failed: {e}"));
            }
        }
    }
}

pub async fn set_priority(app: &mut App, issue_id: &str, priority: u8) {
    match bd::update_priority(app.dir.as_deref(), issue_id, priority).await {
        Ok(_) => {
            app.notify(format!("Priority set: {issue_id} → P{priority}"));
            let _ = app.load_issues().await;
        }
        Err(e) => {
            app.notify(format!("Priority update failed: {e}"));
        }
    }
}
