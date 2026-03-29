use anyhow::Result;
use tokio::process::Command;
use tokio::sync::mpsc;

use super::ImplRequest;
use super::worktree::create_worktree;

pub enum ImplEvent {
    Started { issue_id: String },
    Completed { issue_id: String, summary: String },
    Failed { issue_id: String, error: String },
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
