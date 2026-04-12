use crate::ai::implement::worktree;
use crate::ai::AiManagers;
use crate::core::Core;

pub async fn start_session(core: &mut Core, issue_id: &str) {
    let repo_dir = Core::repo_dir();
    let wt_path = worktree::worktree_path(&repo_dir, issue_id);

    if wt_path.exists() {
        let cmd = format!("cd {}", wt_path.display());
        match crate::clipboard::copy(&cmd) {
            Ok(_) => core.notify(format!("Copied: {cmd}")),
            Err(e) => core.notify(format!("Copy failed: {e}")),
        }
        return;
    }

    let base_branch = match core.find_parent_epic_id() {
        Some(epic_id) => match worktree::ensure_epic_branch(&repo_dir, &epic_id).await {
            Ok(branch) => branch,
            Err(e) => {
                core.notify(format!("Failed to ensure epic branch: {e}"));
                return;
            }
        },
        None => core.default_branch.clone(),
    };

    match worktree::create_worktree(&repo_dir, issue_id, &base_branch).await {
        Ok((path, _branch)) => {
            let cmd = format!("cd {}", path.display());
            match crate::clipboard::copy(&cmd) {
                Ok(_) => core.notify(format!("Copied: {cmd}")),
                Err(e) => core.notify(format!("Copy failed: {e}")),
            }
        }
        Err(e) => core.notify(format!("Worktree creation failed: {e}")),
    }
}

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
