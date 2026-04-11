use crate::ai::AiManagers;
use crate::core::{Core, View};

pub async fn merge_impl(core: &mut Core, ai: &mut AiManagers, issue_id: &str) {
    let epic_id = core.find_parent_epic_id();
    let repo_dir = Core::repo_dir();
    if let Err(e) = ai
        .impl_
        .merge(&issue_id, epic_id.as_deref(), &repo_dir, None)
        .await
    {
        core.notify(format!("Merge failed: {e}"));
        return;
    }

    core.notify(format!("Merged & closed: {issue_id}"));
    let _ = core.load_issues().await;
    crate::action::navigate::reload_children(core).await;
}

pub async fn discard_impl(core: &mut Core, ai: &mut AiManagers, issue_id: &str) {
    let repo_dir = Core::repo_dir();
    if let Err(e) = ai.impl_.discard(issue_id, &repo_dir).await {
        core.notify(format!("Discard failed: {e}"));
        return;
    }

    core.notify(format!("Discarded: {issue_id}"));
}

pub async fn retry_impl(core: &mut Core, ai: &mut AiManagers, issue_id: &str) {
    let repo_dir = Core::repo_dir();
    if let Err(e) = ai.impl_.discard(issue_id, &repo_dir).await {
        core.notify(format!("Retry failed (discard): {e}"));
        return;
    }

    let epic_id = core.find_parent_epic_id();
    crate::action::ai::start_implement(core, ai, issue_id, epic_id.as_deref()).await;
}

pub async fn merge_epic(core: &mut Core, ai: &mut AiManagers, epic_id: &str) {
    if let View::EpicDetail { children, .. } = &core.view {
        let unclosed: Vec<String> = children
            .iter()
            .filter(|c| c.status != "closed")
            .map(|c| c.id.clone())
            .collect();
        if !unclosed.is_empty() {
            core.notify(format!("Unclosed children: {}", unclosed.join(", ")));
            return;
        }
    }

    let repo_dir = Core::repo_dir();

    match ai.impl_.merge_epic(&epic_id, &repo_dir, None).await {
        Ok(_) => {
            core.notify(format!("Merged & closed epic: {epic_id}"));
        }
        Err(e) => {
            let msg = e.to_string();
            if msg == "no_epic_branch" {
                core.notify(format!("No epic branch — closed: {epic_id}"));
            } else {
                core.notify(format!("Epic merge failed: {e}"));
                return;
            }
        }
    }

    crate::action::navigate::back(core);
    let _ = core.load_issues().await;
    if core.issue_store.selected >= core.issue_store.issues.len() && core.issue_store.selected > 0 {
        core.issue_store.selected -= 1;
    }
}
