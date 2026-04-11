use crate::ai::implement::{self, ImplStatus};
use crate::app::App;
use crate::bd;
use crate::core::{Core, View};

pub fn next(core: &mut Core) {
    match &mut core.view {
        View::IssueList => {
            let len = core.issue_store.displayed_issues(&core.filter).len();
            if len > 0 {
                core.issue_store.selected = (core.issue_store.selected + 1).min(len - 1);
            }
        }
        View::EpicDetail {
            children,
            child_selected,
            ..
        } => {
            if !children.is_empty() {
                *child_selected = (*child_selected + 1).min(children.len() - 1);
            }
        }
        View::IssueDetail { scroll_offset, .. } => {
            *scroll_offset = scroll_offset.saturating_add(1);
        }
    }
}

pub fn previous(core: &mut Core) {
    match &mut core.view {
        View::IssueList => {
            let len = core.issue_store.displayed_issues(&core.filter).len();
            if len > 0 {
                core.issue_store.selected = core.issue_store.selected.saturating_sub(1);
            }
        }
        View::EpicDetail { child_selected, .. } => {
            *child_selected = child_selected.saturating_sub(1);
        }
        View::IssueDetail { scroll_offset, .. } => {
            *scroll_offset = scroll_offset.saturating_sub(1);
        }
    }
}

pub fn back(core: &mut Core) {
    if let Some(prev) = core.view_stack.pop() {
        core.view = prev;
    }
}

// TODO: Step 6 で &mut Core に変更（dir, bd CLI, impl_manager が必要）
pub async fn open_detail(app: &mut App) {
    let Some(issue) = app.selected_issue() else {
        return;
    };
    let issue_id = issue.id.clone();

    // Clear unread label
    if issue.labels.contains(&"strand-unread".to_string()) {
        if let Some(issue) = app
            .core
            .issue_store
            .issues
            .get_mut(app.core.issue_store.selected)
        {
            issue.labels.retain(|l| l != "strand-unread");
        }
        let id = issue_id.clone();
        let dir = app.dir.clone();
        tokio::spawn(async move {
            let _ = bd::remove_label(dir.as_deref(), &id, "strand-unread").await;
        });
    }

    push_view_for_issue(app, &issue_id).await;
}

// TODO: Step 6 で &mut Core に変更
pub async fn open_child_detail(app: &mut App) {
    let issue_id = match &app.core.view {
        View::EpicDetail {
            children,
            child_selected,
            ..
        } => children.get(*child_selected).map(|i| i.id.clone()),
        _ => None,
    };
    let Some(issue_id) = issue_id else { return };

    push_view_for_issue(app, &issue_id).await;
}

// TODO: Step 6 で &mut Core に変更
pub async fn navigate_issue(app: &mut App, forward: bool) {
    let View::IssueDetail { issue_id, .. } = &app.core.view else {
        return;
    };
    let current_id = issue_id.clone();

    let parent = app.core.view_stack.last();
    let (issues, selected_idx) = match parent {
        Some(View::EpicDetail {
            children,
            child_selected,
            ..
        }) => {
            let idx = children
                .iter()
                .position(|i| i.id == current_id)
                .unwrap_or(*child_selected);
            (children.clone(), idx)
        }
        _ => {
            let idx = app
                .core
                .issue_store
                .issues
                .iter()
                .position(|i| i.id == current_id)
                .unwrap_or(app.core.issue_store.selected);
            (app.core.issue_store.issues.clone(), idx)
        }
    };

    if issues.is_empty() {
        return;
    }

    let new_idx = if forward {
        (selected_idx + 1).min(issues.len() - 1)
    } else {
        selected_idx.saturating_sub(1)
    };

    if new_idx == selected_idx {
        return;
    }

    let new_issue_id = issues[new_idx].id.clone();

    match app.core.view_stack.last_mut() {
        Some(View::EpicDetail { child_selected, .. }) => {
            *child_selected = new_idx;
        }
        _ => {
            app.core.issue_store.selected = new_idx;
        }
    }

    let children = bd::list_children(app.dir.as_deref(), &new_issue_id)
        .await
        .unwrap_or_default();

    if children.is_empty() {
        app.core.view = View::IssueDetail {
            issue_id: new_issue_id.clone(),
            scroll_offset: 0,
            diff: None,
        };
        load_issue_detail_diff(app, &new_issue_id).await;
    } else {
        let ready_ids = bd::list_ready_ids(app.dir.as_deref(), &new_issue_id)
            .await
            .unwrap_or_default();
        app.core.view = View::EpicDetail {
            epic_id: new_issue_id,
            children,
            ready_ids,
            child_selected: 0,
            scroll_offset: 0,
        };
    }
}

// TODO: Step 6 で &mut Core に変更
pub async fn reload_children(app: &mut App) {
    let epic_id = match &app.core.view {
        View::EpicDetail { epic_id, .. } => epic_id.clone(),
        _ => return,
    };
    let new_children = bd::list_children(app.dir.as_deref(), &epic_id)
        .await
        .unwrap_or_default();
    let new_ready = bd::list_ready_ids(app.dir.as_deref(), &epic_id)
        .await
        .unwrap_or_default();

    match &mut app.core.view {
        View::EpicDetail {
            children,
            ready_ids,
            ..
        } => {
            *children = new_children;
            *ready_ids = new_ready;
        }
        _ => {}
    }
}

// --- private helpers ---

async fn push_view_for_issue(app: &mut App, issue_id: &str) {
    let children = bd::list_children(app.dir.as_deref(), issue_id)
        .await
        .unwrap_or_default();

    let old = std::mem::replace(&mut app.core.view, View::IssueList);
    app.core.view_stack.push(old);

    if children.is_empty() {
        app.core.view = View::IssueDetail {
            issue_id: issue_id.to_string(),
            scroll_offset: 0,
            diff: None,
        };
        load_issue_detail_diff(app, issue_id).await;
    } else {
        let ready_ids = bd::list_ready_ids(app.dir.as_deref(), issue_id)
            .await
            .unwrap_or_default();
        app.core.view = View::EpicDetail {
            epic_id: issue_id.to_string(),
            children,
            ready_ids,
            child_selected: 0,
            scroll_offset: 0,
        };
    }
}

async fn load_issue_detail_diff(app: &mut App, issue_id: &str) {
    rebase_impl(app, issue_id).await;
    let computed = compute_diff(app, issue_id).await;
    match &mut app.core.view {
        View::IssueDetail { diff, .. } => {
            *diff = computed;
        }
        _ => {}
    }
}

async fn rebase_impl(app: &mut App, issue_id: &str) {
    let Some(job) = app.impl_manager.get_job(issue_id) else {
        return;
    };
    if !matches!(job.status, ImplStatus::Done) {
        return;
    }
    let wt_path = job.worktree_path.clone();
    let base = target_branch_for(app, issue_id);

    match implement::worktree::rebase_impl_branch(&wt_path, &base).await {
        Ok(_) => {}
        Err(e) => {
            app.notify(format!("Rebase failed (retry recommended): {e}"));
        }
    }
}

fn target_branch_for(app: &App, _issue_id: &str) -> String {
    app.find_parent_epic_id()
        .map(|eid| implement::epic_branch_name(&eid))
        .unwrap_or_else(|| "master".to_string())
}

async fn compute_diff(app: &App, issue_id: &str) -> Option<Vec<u8>> {
    let Some(job) = app.impl_manager.get_job(issue_id) else {
        return None;
    };
    if !matches!(job.status, ImplStatus::Done) {
        return None;
    }

    let branch = job.branch.clone();
    let repo_dir = app.repo_dir();

    let base = target_branch_for(app, issue_id);
    let range = format!("{base}..{branch}");

    let output = tokio::process::Command::new("sh")
        .args([
            "-c",
            &format!(
                "git diff --stat --color=always {range} && echo && git diff --color=always {range} | $(git config core.pager || echo cat)"
            ),
        ])
        .current_dir(&repo_dir)
        .output()
        .await;

    match output {
        Ok(out) if out.status.success() && !out.stdout.iter().all(|&b| b.is_ascii_whitespace()) => {
            Some(out.stdout)
        }
        _ => None,
    }
}
