use anyhow::Result;
use tokio::process::Command;
use tokio::sync::mpsc;

use super::ImplRequest;
use super::worktree::create_worktree;

pub enum ImplEvent {
    Started { issue_id: String },
    Completed { issue_id: String, summary: String, session_id: Option<String> },
    Failed { issue_id: String, error: String, session_id: Option<String> },
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
        Ok((summary, session_id)) => {
            let _ = tx
                .send(ImplEvent::Completed {
                    issue_id: issue_id.clone(),
                    summary,
                    session_id,
                })
                .await;
        }
        Err(e) => {
            let _ = tx
                .send(ImplEvent::Failed {
                    issue_id: issue_id.clone(),
                    error: e.to_string(),
                    session_id: None,
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

async fn run_inner(request: &ImplRequest) -> Result<(String, Option<String>)> {
    let (wt_path, _branch) =
        create_worktree(&request.repo_dir, &request.issue_id, &request.base_branch).await?;

    let prompt = build_prompt(request);

    let output = Command::new("claude")
        .args(["-p", &prompt, "--allowedTools", "Edit,Write,Bash", "--output-format", "json"])
        .current_dir(&wt_path)
        .output()
        .await?;

    if !output.status.success() {
        anyhow::bail!(
            "claude command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    let (summary, session_id) = match serde_json::from_str::<serde_json::Value>(&stdout) {
        Ok(json) => {
            let session_id = json.get("session_id").and_then(|v| v.as_str()).map(|s| s.to_string());
            let result = json.get("result").and_then(|v| v.as_str()).unwrap_or("").to_string();
            (result, session_id)
        }
        Err(_) => (stdout.to_string(), None),
    };

    // Persist session_id to worktree for restore
    if let Some(ref sid) = session_id {
        let session_file = wt_path.join(".strand-session");
        let _ = tokio::fs::write(&session_file, sid).await;
    }

    Ok((summary, session_id))
}
