use crate::app::App;

// TODO: impl_manager が必要なため &mut App を取る（manager 集約後に再検討）

pub fn copy_resume_command(app: &mut App, issue_id: &str) {
    let Some(job) = app.impl_manager.get_job(issue_id) else {
        app.core.notify("No impl job found");
        return;
    };
    let Some(session_id) = &job.session_id else {
        app.core.notify("No session ID available");
        return;
    };
    let path = job.worktree_path.display();
    let cmd = format!(
        "cd {} && claude --dangerously-skip-permissions --resume {}",
        path, session_id
    );
    match crate::clipboard::copy(&cmd) {
        Ok(_) => app.core.notify(format!("Copied: {cmd}")),
        Err(e) => app.core.notify(format!("Copy failed: {e}")),
    }
}

pub fn copy_log_command(app: &mut App, issue_id: &str) {
    let Some(job) = app.impl_manager.get_job(issue_id) else {
        app.core.notify("No impl job found");
        return;
    };
    let log_path = crate::ai::implement::run::log_file_path(&job.worktree_path);
    let cmd = format!("tail -f {} | jq .", log_path.display());
    match crate::clipboard::copy(&cmd) {
        Ok(_) => app.core.notify(format!("Copied: {cmd}")),
        Err(e) => app.core.notify(format!("Copy failed: {e}")),
    }
}

pub fn copy_worktree_path(app: &mut App, issue_id: &str) {
    let Some(job) = app.impl_manager.get_job(issue_id) else {
        app.core.notify("No impl job found");
        return;
    };
    let path = job.worktree_path.display().to_string();
    match crate::clipboard::copy(&path) {
        Ok(_) => app.core.notify(format!("Copied: {path}")),
        Err(e) => app.core.notify(format!("Copy failed: {e}")),
    }
}
