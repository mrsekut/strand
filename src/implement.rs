use std::path::{Path, PathBuf};

use anyhow::Result;
use tokio::process::Command;
use tokio::sync::mpsc;

use crate::bd;

#[derive(Debug, Clone)]
pub enum ImplStatus {
    Running,
    Done,
    Failed(String),
}

#[derive(Debug, Clone)]
pub struct ImplJob {
    pub issue_id: String,
    pub branch: String,
    pub worktree_path: PathBuf,
    pub status: ImplStatus,
    pub completed_at: Option<String>,
}

pub enum ImplEvent {
    Started { issue_id: String },
    Completed { issue_id: String, summary: String },
    Failed { issue_id: String, error: String },
}

pub struct ImplRequest {
    pub issue_id: String,
    pub title: String,
    pub description: Option<String>,
    pub design: Option<String>,
    pub repo_dir: PathBuf,
    pub base_branch: String,
}

pub fn worktree_path(repo_dir: &PathBuf, issue_id: &str) -> PathBuf {
    let parent = repo_dir.parent().unwrap_or(repo_dir);
    parent.join(format!("strand-impl-{}", bd::short_id(issue_id)))
}

pub fn branch_name(issue_id: &str) -> String {
    format!("impl/{issue_id}")
}

pub fn epic_branch_name(epic_id: &str) -> String {
    format!("epic/{epic_id}")
}

/// epicブランチがなければmasterから作成する。既に存在すればスキップ。
pub async fn ensure_epic_branch(repo_dir: &Path, epic_id: &str) -> Result<String> {
    let branch = epic_branch_name(epic_id);

    // ブランチが既に存在するか確認
    let output = Command::new("git")
        .args(["rev-parse", "--verify", &branch])
        .current_dir(repo_dir)
        .output()
        .await?;

    if output.status.success() {
        return Ok(branch);
    }

    // masterから新規作成
    run_git(repo_dir, &["branch", &branch, "master"]).await?;
    Ok(branch)
}

fn build_prompt(request: &ImplRequest) -> String {
    let mut parts = vec![format!("Issue: {}", request.title)];

    if let Some(desc) = &request.description {
        parts.push(format!("Description:\n{desc}"));
    }
    if let Some(design) = &request.design {
        parts.push(format!("Design:\n{design}"));
    }

    parts.push(r#"Implement the issue above. Create or edit files as needed and leave the code in a working state.

## Commit rules
When done, commit your changes. The commit message body must record the background and reasoning behind the implementation.

```
<type>: <concise summary of change>

## Why
- Why this change was necessary

## What
- Key files changed and summary of modifications

## Decisions
- Alternative approaches considered and why they were rejected
- Rationale for the chosen approach
```

- Omit any section that does not apply
- For trivial changes (typo, fmt, etc.) a title-only message is fine
- Only include Decisions when multiple approaches were considered"#.to_string());

    parts.join("\n\n")
}

async fn create_worktree(
    repo_dir: &PathBuf,
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

async fn run_git(repo_dir: &Path, args: &[&str]) -> Result<()> {
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

pub async fn remove_worktree(repo_dir: &PathBuf, worktree_path: &PathBuf) -> Result<()> {
    let wt = worktree_path.to_string_lossy();
    run_git(repo_dir, ["worktree", "remove", "--force", &wt].as_slice()).await
}

pub async fn delete_branch(repo_dir: &PathBuf, branch: &str) -> Result<()> {
    run_git(repo_dir, ["branch", "-D", branch].as_slice()).await
}

/// source_branchをtarget_branchにmergeする（一時worktreeを使用）
pub async fn merge_into_branch(
    repo_dir: &Path,
    source_branch: &str,
    target_branch: &str,
) -> Result<()> {
    // 一時worktreeでtarget_branchをcheckoutしてmerge
    let tmp_dir = repo_dir
        .parent()
        .unwrap_or(repo_dir)
        .join(format!("strand-merge-tmp-{}", std::process::id()));

    // 一時worktree作成
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

    // merge実行
    let merge_result = run_git(&tmp_dir, &["merge", source_branch]).await;

    // 一時worktree削除（merge成否に関わらず）
    let _ = Command::new("git")
        .args(["worktree", "remove", "--force", &tmp_dir.to_string_lossy()])
        .current_dir(repo_dir)
        .output()
        .await;

    merge_result
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

    // porcelain形式: "worktree <path>\nHEAD ...\nbranch ...\n\n" のブロック
    let mut jobs = Vec::new();
    for block in stdout.split("\n\n") {
        let wt_path = match block.lines().find_map(|l| l.strip_prefix("worktree ")) {
            Some(p) => PathBuf::from(p),
            None => continue,
        };

        // strand-impl-{short_id} パターンのworktreeのみ対象
        let dir_name = match wt_path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n,
            None => continue,
        };
        let sid = match dir_name.strip_prefix("strand-impl-") {
            Some(s) => s,
            None => continue,
        };

        // short_idからissue_idを逆引き
        let issue_id = match issue_ids.iter().find(|id| bd::short_id(id) == sid) {
            Some(id) => id.clone(),
            None => continue,
        };

        let branch = branch_name(&issue_id);

        // issue_idから親epic_idを推定し、epicブランチがあればそれをbase_branchにする
        let base_branch = guess_base_branch(repo_dir, &issue_id).await;
        let has = has_commits(repo_dir, &branch, &base_branch).await;
        let status = match has {
            true => ImplStatus::Done,
            false => ImplStatus::Failed("interrupted".to_string()),
        };

        // Doneの場合、ブランチの最新commit日時を取得
        let completed_at = if has {
            latest_commit_date(repo_dir, &branch).await
        } else {
            None
        };

        jobs.push(ImplJob {
            issue_id,
            branch,
            worktree_path: wt_path,
            status,
            completed_at,
        });
    }

    jobs
}

/// issue_idから親epic_idを推定し、epicブランチが存在すればその名前を返す。
/// なければ"master"にフォールバック。
/// 例: "strand-unq.1.1" → parent = "strand-unq.1" → epic/strand-unq.1
async fn guess_base_branch(repo_dir: &Path, issue_id: &str) -> String {
    // issue_idの最後の`.N`を除去して親IDを推定
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

/// ブランチの最新commit日時をISO 8601で取得
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

pub async fn epic_branch_exists(repo_dir: &Path, epic_id: &str) -> bool {
    let branch = epic_branch_name(epic_id);
    let output = Command::new("git")
        .args(["rev-parse", "--verify", &branch])
        .current_dir(repo_dir)
        .output()
        .await;

    matches!(output, Ok(o) if o.status.success())
}

pub async fn merge_epic_to_master(repo_dir: &Path, epic_id: &str) -> Result<()> {
    let branch = epic_branch_name(epic_id);

    if !epic_branch_exists(repo_dir, epic_id).await {
        anyhow::bail!("epic branch '{}' does not exist", branch);
    }

    // 一時worktreeでmasterにmerge
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

    // merge成功後にepicブランチを削除
    run_git(repo_dir, &["branch", "-D", &branch]).await?;

    Ok(())
}

pub async fn run(request: ImplRequest, tx: mpsc::Sender<ImplEvent>) -> Result<()> {
    let issue_id = request.issue_id.clone();

    let _ = tx
        .send(ImplEvent::Started {
            issue_id: issue_id.clone(),
        })
        .await;

    let result = run_inner(&request).await;

    match result {
        Ok(summary) => {
            let _ = tx
                .send(ImplEvent::Completed {
                    issue_id: issue_id.clone(),
                    summary,
                })
                .await;
        }
        Err(e) => {
            let _ = tx
                .send(ImplEvent::Failed {
                    issue_id: issue_id.clone(),
                    error: e.to_string(),
                })
                .await;
            return Err(e);
        }
    }

    Ok(())
}

async fn run_inner(request: &ImplRequest) -> Result<String> {
    let (wt_path, _branch) =
        create_worktree(&request.repo_dir, &request.issue_id, &request.base_branch).await?;

    let prompt = build_prompt(request);

    let output = Command::new("claude")
        .args(["-p", &prompt, "--allowedTools", "Edit,Write,Bash"])
        .current_dir(&wt_path)
        .output()
        .await?;

    if !output.status.success() {
        anyhow::bail!(
            "claude command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let summary = String::from_utf8_lossy(&output.stdout).to_string();
    Ok(summary)
}
