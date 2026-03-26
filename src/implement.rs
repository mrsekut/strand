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
}

pub fn worktree_path(repo_dir: &PathBuf, issue_id: &str) -> PathBuf {
    let parent = repo_dir.parent().unwrap_or(repo_dir);
    parent.join(format!("strand-impl-{}", bd::short_id(issue_id)))
}

pub fn branch_name(issue_id: &str) -> String {
    format!("impl/{issue_id}")
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

async fn create_worktree(repo_dir: &PathBuf, issue_id: &str) -> Result<(PathBuf, String)> {
    let wt_path = worktree_path(repo_dir, issue_id);
    let branch = branch_name(issue_id);

    let output = Command::new("git")
        .args(["worktree", "add"])
        .arg(&wt_path)
        .args(["-b", &branch])
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

pub async fn merge_branch(repo_dir: &PathBuf, branch: &str) -> Result<()> {
    run_git(repo_dir, ["merge", branch].as_slice()).await
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

        // masterとの差分でstatus判定
        let status = match has_commits(repo_dir, &branch).await {
            true => ImplStatus::Done,
            false => ImplStatus::Failed("interrupted".to_string()),
        };

        jobs.push(ImplJob {
            issue_id,
            branch,
            worktree_path: wt_path,
            status,
        });
    }

    jobs
}

async fn has_commits(repo_dir: &Path, branch: &str) -> bool {
    let output = Command::new("git")
        .args(["log", &format!("master..{branch}"), "--oneline"])
        .current_dir(repo_dir)
        .output()
        .await;

    match output {
        Ok(o) => !String::from_utf8_lossy(&o.stdout).trim().is_empty(),
        Err(_) => false,
    }
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
    let (wt_path, _branch) = create_worktree(&request.repo_dir, &request.issue_id).await?;

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
