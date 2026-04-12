use crate::ai::AiManagers;
use crate::core::Core;

pub fn copy_resume_command(core: &mut Core, ai: &AiManagers, issue_id: &str) {
    let Some(job) = ai.impl_.get_job(issue_id) else {
        core.notify("No impl job found");
        return;
    };
    let Some(session_id) = &job.session_id else {
        core.notify("No session ID available");
        return;
    };
    let path = job.worktree_path.display();
    let cmd = format!(
        "cd {} && claude --dangerously-skip-permissions --resume {}",
        path, session_id
    );
    match crate::clipboard::copy(&cmd) {
        Ok(_) => core.notify(format!("Copied: {cmd}")),
        Err(e) => core.notify(format!("Copy failed: {e}")),
    }
}

pub fn copy_worktree_path(core: &mut Core, ai: &AiManagers, issue_id: &str) {
    let Some(job) = ai.impl_.get_job(issue_id) else {
        core.notify("No impl job found");
        return;
    };
    let path = job.worktree_path.display().to_string();
    match crate::clipboard::copy(&path) {
        Ok(_) => core.notify(format!("Copied: {path}")),
        Err(e) => core.notify(format!("Copy failed: {e}")),
    }
}
