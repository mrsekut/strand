use std::path::Path;

use anyhow::Result;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

use super::ImplRequest;
use super::worktree::create_worktree;

pub enum ImplEvent {
    Started {
        issue_id: String,
    },
    Completed {
        issue_id: String,
        summary: String,
        session_id: Option<String>,
    },
    Failed {
        issue_id: String,
        error: String,
        session_id: Option<String>,
    },
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

    parts.push(
        r#"Implement the issue above. Create or edit files as needed and leave the code in a working state.

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
- Only include Decisions when multiple approaches were considered"#
            .to_string(),
    );

    parts.join("\n\n")
}

/// ログファイルのパス (.strand-impl.jsonl in worktree)
pub fn log_file_path(worktree_path: &Path) -> std::path::PathBuf {
    worktree_path.join(".strand-impl.jsonl")
}

async fn run_inner(request: &ImplRequest) -> Result<(String, Option<String>)> {
    let (wt_path, _branch) =
        create_worktree(&request.repo_dir, &request.issue_id, &request.base_branch).await?;

    let prompt = build_prompt(request);

    let output = spawn_claude(&wt_path, &prompt).await?;

    // Persist session_id to worktree for restore
    if let Some(ref sid) = output.session_id {
        let session_file = wt_path.join(".strand-session");
        let _ = tokio::fs::write(&session_file, sid).await;
    }

    Ok((output.summary, output.session_id))
}

struct ClaudeOutput {
    summary: String,
    session_id: Option<String>,
}

/// Claude CLIをstream-jsonで起動し、ログをファイルに書き出しつつ最終結果を返す
async fn spawn_claude(wt_path: &Path, prompt: &str) -> Result<ClaudeOutput> {
    let mut child = Command::new("claude")
        .args([
            "-p",
            prompt,
            "--output-format",
            "stream-json",
            "--verbose",
            "--dangerously-skip-permissions",
        ])
        .current_dir(wt_path)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    let stdout = child.stdout.take().expect("stdout should be piped");
    let stderr = child.stderr.take().expect("stderr should be piped");

    let stderr_handle = drain_stderr(stderr);
    let (summary, session_id) = tee_stdout_and_extract_result(stdout, wt_path).await;

    let status = child.wait().await?;
    let stderr_output = stderr_handle.await.unwrap_or_default();

    if !status.success() {
        let err_detail = if stderr_output.is_empty() {
            format!("{status}")
        } else {
            stderr_output.trim().to_string()
        };
        anyhow::bail!("claude command failed: {err_detail}");
    }

    Ok(ClaudeOutput {
        summary: summary.unwrap_or_default(),
        session_id,
    })
}

/// stderrをバックグラウンドで読み切る（バッファ溢れデッドロック防止）
fn drain_stderr(stderr: tokio::process::ChildStderr) -> tokio::task::JoinHandle<String> {
    tokio::spawn(async move {
        let mut reader = BufReader::new(stderr);
        let mut buf = String::new();
        let _ = tokio::io::AsyncReadExt::read_to_string(&mut reader, &mut buf).await;
        buf
    })
}

/// stdoutを行ごとにログファイルへ書き出しつつ、最終resultを抽出する
async fn tee_stdout_and_extract_result(
    stdout: tokio::process::ChildStdout,
    wt_path: &Path,
) -> (Option<String>, Option<String>) {
    let log_path = log_file_path(wt_path);
    let log_file = match tokio::fs::File::create(&log_path).await {
        Ok(f) => f,
        Err(_) => return (None, None),
    };
    let mut log_writer = tokio::io::BufWriter::new(log_file);

    let reader = BufReader::new(stdout);
    let mut lines = reader.lines();

    let mut last_result: Option<String> = None;
    let mut last_session_id: Option<String> = None;

    while let Ok(Some(line)) = lines.next_line().await {
        let _ = log_writer.write_all(line.as_bytes()).await;
        let _ = log_writer.write_all(b"\n").await;
        let _ = log_writer.flush().await;

        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) {
            if json.get("type").and_then(|v| v.as_str()) == Some("result") {
                last_result = json
                    .get("result")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                last_session_id = json
                    .get("session_id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
            }
        }
    }

    (last_result, last_session_id)
}
