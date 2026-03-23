use std::path::PathBuf;

use anyhow::Result;
use tokio::process::Command;
use tokio::sync::mpsc;

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
    Completed { issue_id: String },
    Failed { issue_id: String, error: String },
}

pub struct ImplRequest {
    pub issue_id: String,
    pub title: String,
    pub description: Option<String>,
    pub design: Option<String>,
    pub repo_dir: PathBuf,
}

fn short_id(id: &str) -> &str {
    id.rsplit_once('-').map(|(_, s)| s).unwrap_or(id)
}

pub fn worktree_path(repo_dir: &PathBuf, issue_id: &str) -> PathBuf {
    let parent = repo_dir.parent().unwrap_or(repo_dir);
    parent.join(format!("strand-impl-{}", short_id(issue_id)))
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

    parts.push("上記のissueを実装してください。必要なファイルの作成・編集を行い、動作する状態にしてください。完了したらコミットしてください。".to_string());

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

pub async fn remove_worktree(repo_dir: &PathBuf, worktree_path: &PathBuf) -> Result<()> {
    let output = Command::new("git")
        .args(["worktree", "remove", "--force"])
        .arg(worktree_path)
        .current_dir(repo_dir)
        .output()
        .await?;

    if !output.status.success() {
        anyhow::bail!(
            "git worktree remove failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

pub async fn delete_branch(repo_dir: &PathBuf, branch: &str) -> Result<()> {
    let output = Command::new("git")
        .args(["branch", "-D", branch])
        .current_dir(repo_dir)
        .output()
        .await?;

    if !output.status.success() {
        anyhow::bail!(
            "git branch -D failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

pub async fn merge_branch(repo_dir: &PathBuf, branch: &str) -> Result<()> {
    let output = Command::new("git")
        .args(["merge", branch])
        .current_dir(repo_dir)
        .output()
        .await?;

    if !output.status.success() {
        anyhow::bail!(
            "git merge failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
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
        Ok(_) => {
            let _ = tx
                .send(ImplEvent::Completed {
                    issue_id: issue_id.clone(),
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

async fn run_inner(request: &ImplRequest) -> Result<()> {
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

    Ok(())
}
