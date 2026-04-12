use std::path::{Path, PathBuf};

use anyhow::Result;
use tokio::process::Command;

use crate::bd;

use super::{branch_name, epic_branch_name};

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
