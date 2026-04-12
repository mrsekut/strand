use std::path::Path;

use anyhow::Result;
use tokio::process::Command;

use super::epic_branch_name;
use super::worktree::{epic_branch_exists, run_git};

/// メインworktreeで直接mergeする（masterへのmerge用）
async fn merge_direct(repo_dir: &Path, source_branch: &str) -> Result<()> {
    run_git(repo_dir, &["merge", source_branch]).await
}

/// 一時worktreeを作ってmergeする（epic branchへのmerge用）
async fn merge_via_worktree(
    repo_dir: &Path,
    source_branch: &str,
    target_branch: &str,
) -> Result<()> {
    let tmp_dir = repo_dir
        .parent()
        .unwrap_or(repo_dir)
        .join(format!("strand-merge-tmp-{}", std::process::id()));

    let output = Command::new("git")
        .args([
            "worktree",
            "add",
            "-f",
            &tmp_dir.to_string_lossy(),
            target_branch,
        ])
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

/// source_branchをtarget_branchにmergeする
/// デフォルトブランチへは直接merge、それ以外は一時worktree経由
pub async fn merge_into_branch(
    repo_dir: &Path,
    source_branch: &str,
    target_branch: &str,
) -> Result<()> {
    let default_branch = super::worktree::detect_default_branch(repo_dir);
    if target_branch == default_branch {
        merge_direct(repo_dir, source_branch).await
    } else {
        merge_via_worktree(repo_dir, source_branch, target_branch).await
    }
}

/// epicブランチをデフォルトブランチにmerge後、epicブランチを削除
pub async fn merge_epic_to_default(repo_dir: &Path, epic_id: &str) -> Result<()> {
    let branch = epic_branch_name(epic_id);

    if !epic_branch_exists(repo_dir, epic_id).await {
        anyhow::bail!("epic branch '{}' does not exist", branch);
    }

    merge_direct(repo_dir, &branch).await?;
    run_git(repo_dir, &["branch", "-D", &branch]).await?;

    Ok(())
}
