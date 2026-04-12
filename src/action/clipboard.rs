use crate::ai::AiManagers;
use crate::ai::job;
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

pub fn copy_log_command(core: &mut Core, ai: &AiManagers, issue_id: &str) {
    let Some(job) = ai.impl_.get_job(issue_id) else {
        core.notify("No impl job found");
        return;
    };
    let log_cmd = if let Ok(jobs_dir) = job::ensure_strand_dir() {
        let short_id = crate::bd::short_id(issue_id);
        let job_dir = job::job_dir_path(&jobs_dir, "impl", short_id);
        let log_path = job_dir.join("output.jsonl");
        format!("tail -f {} | jq .", log_path.display())
    } else {
        // fallback to legacy path
        let log_path = job.worktree_path.join(".strand-impl.jsonl");
        format!("tail -f {} | jq .", log_path.display())
    };
    let cmd = log_cmd;
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
