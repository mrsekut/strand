use std::path::{Path, PathBuf};

use anyhow::Result;
use tokio::process::Command;

use crate::bd;

use super::{ImplJob, ImplStatus, branch_name, epic_branch_name};

pub fn worktree_path(repo_dir: &Path, issue_id: &str) -> PathBuf {
    let parent = repo_dir.parent().unwrap_or(repo_dir);
    parent.join(format!("strand-impl-{}", bd::short_id(issue_id)))
}

pub async fn create_worktree(
    repo_dir: &Path,
    issue_id: &str,
    base_branch: &str,
) -> Result<(PathBuf, String)> {
    let wt_path = worktree_path(repo_dir, issue_id);
    let branch = branch_name(issue_id);

    let output = Command::new("git")
        .args(["worktree", "add"])
        .arg(&wt_path)
        .args(["-b", &branch, base_branch])
        .current_dir(repo_dir)
        .output()
        .await?;

    if !output.status.success() {
        anyhow::bail!(
            "git worktree add failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok((wt_path, branch))
}

pub async fn remove_worktree(repo_dir: &Path, worktree_path: &Path) -> Result<()> {
    let wt = worktree_path.to_string_lossy();
    run_git(repo_dir, &["worktree", "remove", "--force", &wt]).await
}

pub async fn delete_branch(repo_dir: &Path, branch: &str) -> Result<()> {
    run_git(repo_dir, &["branch", "-D", branch]).await
}

pub async fn ensure_epic_branch(repo_dir: &Path, epic_id: &str) -> Result<String> {
    let branch = epic_branch_name(epic_id);

    let output = Command::new("git")
        .args(["rev-parse", "--verify", &branch])
        .current_dir(repo_dir)
        .output()
        .await?;

    if output.status.success() {
        return Ok(branch);
    }

    run_git(repo_dir, &["branch", &branch, "master"]).await?;
    Ok(branch)
}

pub async fn epic_branch_exists(repo_dir: &Path, epic_id: &str) -> bool {
    let branch = epic_branch_name(epic_id);
    let output = Command::new("git")
        .args(["rev-parse", "--verify", &branch])
        .current_dir(repo_dir)
        .output()
        .await;

    matches!(output, Ok(o) if o.status.success())
}

/// 既存のgit worktreeからImplJobを復元する
pub async fn discover_worktrees(repo_dir: &Path, issue_ids: &[String]) -> Vec<ImplJob> {
    let output = match Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .current_dir(repo_dir)
        .output()
        .await
    {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };

    let stdout = String::from_utf8_lossy(&output.stdout);

    let mut jobs = Vec::new();
    for block in stdout.split("\n\n") {
        let wt_path = match block.lines().find_map(|l| l.strip_prefix("worktree ")) {
            Some(p) => PathBuf::from(p),
            None => continue,
        };

        let dir_name = match wt_path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n,
            None => continue,
        };
        let sid = match dir_name.strip_prefix("strand-impl-") {
            Some(s) => s,
            None => continue,
        };

        let issue_id = match issue_ids.iter().find(|id| bd::short_id(id) == sid) {
            Some(id) => id.clone(),
            None => continue,
        };

        let branch = branch_name(&issue_id);

        let base_branch = guess_base_branch(repo_dir, &issue_id).await;
        let has = has_commits(repo_dir, &branch, &base_branch).await;
        let status = match has {
            true => ImplStatus::Done,
            false => ImplStatus::Interrupted,
        };

        let completed_at = if has {
            latest_commit_date(repo_dir, &branch).await
        } else {
            None
        };

        let session_id = read_session_id(&wt_path).await;

        jobs.push(ImplJob {
            issue_id,
            branch,
            worktree_path: wt_path,
            status,
            completed_at,
            session_id,
        });
    }

    jobs
}

async fn read_session_id(wt_path: &Path) -> Option<String> {
    let session_file = wt_path.join(".strand-session");
    let content = tokio::fs::read_to_string(&session_file).await.ok()?;
    let trimmed = content.trim().to_string();
    if trimmed.is_empty() { None } else { Some(trimmed) }
}

async fn guess_base_branch(repo_dir: &Path, issue_id: &str) -> String {
    if let Some(dot_pos) = issue_id.rfind('.') {
        let parent_id = &issue_id[..dot_pos];
        let epic_branch = epic_branch_name(parent_id);
        let output = Command::new("git")
            .args(["rev-parse", "--verify", &epic_branch])
            .current_dir(repo_dir)
            .output()
            .await;
        if let Ok(o) = output {
            if o.status.success() {
                return epic_branch;
            }
        }
    }
    "master".to_string()
}

async fn latest_commit_date(repo_dir: &Path, branch: &str) -> Option<String> {
    let output = Command::new("git")
        .args(["log", "-1", "--format=%aI", branch])
        .current_dir(repo_dir)
        .output()
        .await
        .ok()?;
    let date = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if date.is_empty() { None } else { Some(date) }
}

async fn has_commits(repo_dir: &Path, branch: &str, base_branch: &str) -> bool {
    let output = Command::new("git")
        .args(["log", &format!("{base_branch}..{branch}"), "--oneline"])
        .current_dir(repo_dir)
        .output()
        .await;

    match output {
        Ok(o) => !String::from_utf8_lossy(&o.stdout).trim().is_empty(),
        Err(_) => false,
    }
}

/// impl branchをtarget branchにrebaseする。失敗時は--abortしてErrを返す。
pub async fn rebase_impl_branch(worktree_path: &Path, target_branch: &str) -> Result<()> {
    let output = Command::new("git")
        .args(["rebase", target_branch])
        .current_dir(worktree_path)
        .output()
        .await?;

    if !output.status.success() {
        // rebase失敗 → abort
        let _ = Command::new("git")
            .args(["rebase", "--abort"])
            .current_dir(worktree_path)
            .output()
            .await;
        anyhow::bail!(
            "rebase failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    Ok(())
}

pub async fn run_git(repo_dir: &Path, args: &[&str]) -> Result<()> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_dir)
        .output()
        .await?;
    if !output.status.success() {
        anyhow::bail!(
            "git {} failed: {}",
            args.first().unwrap_or(&""),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}
