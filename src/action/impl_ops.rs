use crate::app::App;
use crate::core::{Core, View};

// TODO: impl_manager が必要なため &mut App を取る（manager 集約後に再検討）

pub async fn merge_impl(app: &mut App, issue_id: &str) {
    let epic_id = app.core.find_parent_epic_id();
    let repo_dir = Core::repo_dir();
    if let Err(e) = app
        .impl_manager
        .merge(&issue_id, epic_id.as_deref(), &repo_dir, None)
        .await
    {
        app.core.notify(format!("Merge failed: {e}"));
        return;
    }

    app.core.notify(format!("Merged & closed: {issue_id}"));
    let _ = app.core.load_issues().await;
    crate::action::navigate::reload_children(&mut app.core).await;
}

pub async fn discard_impl(app: &mut App, issue_id: &str) {
    let repo_dir = Core::repo_dir();
    if let Err(e) = app.impl_manager.discard(issue_id, &repo_dir).await {
        app.core.notify(format!("Discard failed: {e}"));
        return;
    }

    app.core.notify(format!("Discarded: {issue_id}"));
}

pub async fn retry_impl(app: &mut App, issue_id: &str) {
    let repo_dir = Core::repo_dir();
    if let Err(e) = app.impl_manager.discard(issue_id, &repo_dir).await {
        app.core.notify(format!("Retry failed (discard): {e}"));
        return;
    }

    let epic_id = app.core.find_parent_epic_id();
    crate::action::ai::start_implement(app, issue_id, epic_id.as_deref()).await;
}

pub async fn merge_epic(app: &mut App, epic_id: &str) {
    if let View::EpicDetail { children, .. } = &app.core.view {
        let unclosed: Vec<String> = children
            .iter()
            .filter(|c| c.status != "closed")
            .map(|c| c.id.clone())
            .collect();
        if !unclosed.is_empty() {
            app.core
                .notify(format!("Unclosed children: {}", unclosed.join(", ")));
            return;
        }
    }

    let repo_dir = Core::repo_dir();

    match app.impl_manager.merge_epic(&epic_id, &repo_dir, None).await {
        Ok(_) => {
            app.core.notify(format!("Merged & closed epic: {epic_id}"));
        }
        Err(e) => {
            let msg = e.to_string();
            if msg == "no_epic_branch" {
                app.core
                    .notify(format!("No epic branch — closed: {epic_id}"));
            } else {
                app.core.notify(format!("Epic merge failed: {e}"));
                return;
            }
        }
    }

    crate::action::navigate::back(&mut app.core);
    let _ = app.core.load_issues().await;
    if app.core.issue_store.selected >= app.core.issue_store.issues.len()
        && app.core.issue_store.selected > 0
    {
        app.core.issue_store.selected -= 1;
    }
}
