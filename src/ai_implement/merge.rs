use std::path::Path;

use anyhow::Result;
use tokio::process::Command;

use crate::bd;

use super::epic_branch_name;
use super::worktree::{epic_branch_exists, run_git};

/// source_branchをtarget_branchにmergeする（一時worktreeを使用）
pub async fn merge_into_branch(
    repo_dir: &Path,
    source_branch: &str,
    target_branch: &str,
) -> Result<()> {
    let tmp_dir = repo_dir
        .parent()
        .unwrap_or(repo_dir)
        .join(format!("strand-merge-tmp-{}", std::process::id()));

    let output = Command::new("git")
        .args(["worktree", "add", &tmp_dir.to_string_lossy(), target_branch])
        .current_dir(repo_dir)
        .output()
        .await?;
    if !output.status.success() {
        anyhow::bail!(
            "git worktree add (merge tmp) failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let merge_result = run_git(&tmp_dir, &["merge", source_branch]).await;

    let _ = Command::new("git")
        .args(["worktree", "remove", "--force", &tmp_dir.to_string_lossy()])
        .current_dir(repo_dir)
        .output()
        .await;

    merge_result
}

/// epicブランチをmasterにmerge後、epicブランチを削除
pub async fn merge_epic_to_master(repo_dir: &Path, epic_id: &str) -> Result<()> {
    let branch = epic_branch_name(epic_id);

    if !epic_branch_exists(repo_dir, epic_id).await {
        anyhow::bail!("epic branch '{}' does not exist", branch);
    }

    let wt_path = repo_dir
        .parent()
        .unwrap_or(repo_dir)
        .join(format!("strand-merge-{}", bd::short_id(epic_id)));

    let add_output = Command::new("git")
        .args(["worktree", "add"])
        .arg(&wt_path)
        .arg("master")
        .current_dir(repo_dir)
        .output()
        .await?;

    if !add_output.status.success() {
        anyhow::bail!(
            "failed to create worktree for master: {}",
            String::from_utf8_lossy(&add_output.stderr)
        );
    }

    let merge_result = run_git(&wt_path, &["merge", &branch]).await;

    let _ = run_git(
        repo_dir,
        &["worktree", "remove", "--force", &wt_path.to_string_lossy()],
    )
    .await;

    merge_result?;

    run_git(repo_dir, &["branch", "-D", &branch]).await?;

    Ok(())
}
