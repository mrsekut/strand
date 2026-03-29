use anyhow::Result;
use tokio::sync::mpsc;

use crate::bd;

use super::SplitRequest;
use super::prompt;

pub enum SplitEvent {
    Started { issue_id: String },
    Completed { issue_id: String, task_count: usize },
    Failed { issue_id: String, error: String },
}

pub async fn run(
    request: SplitRequest,
    dir: Option<String>,
    tx: mpsc::Sender<SplitEvent>,
) -> Result<()> {
    let issue_id = request.issue_id.clone();

    let _ = tx
        .send(SplitEvent::Started {
            issue_id: issue_id.clone(),
        })
        .await;

    let result = run_inner(&request, dir.as_deref()).await;

    match result {
        Ok(count) => {
            let _ = tx
                .send(SplitEvent::Completed {
                    issue_id: issue_id.clone(),
                    task_count: count,
                })
                .await;
        }
        Err(e) => {
            let _ = tx
                .send(SplitEvent::Failed {
                    issue_id: issue_id.clone(),
                    error: e.to_string(),
                })
                .await;
            return Err(e);
        }
    }

    Ok(())
}

async fn run_inner(request: &SplitRequest, dir: Option<&str>) -> Result<usize> {
    let prompt_text = prompt::build_prompt(request);

    let output = tokio::process::Command::new("claude")
        .args(["-p", &prompt_text, "--output-format", "json"])
        .output()
        .await?;

    if !output.status.success() {
        anyhow::bail!(
            "claude command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let stdout = String::from_utf8(output.stdout)?;
    let split_result = prompt::parse_result(&stdout)?;

    // 親issueをepicに昇格
    bd::update_type(dir, &request.issue_id, "epic").await?;

    // 子タスクを作成
    let task_count = split_result.tasks.len();
    for task in &split_result.tasks {
        let child_id =
            bd::create_child(dir, &request.issue_id, &task.title, &task.description).await?;
        // auto-enrich対象外にする
        bd::add_label(dir, &child_id, "strand-enriched").await?;
    }

    Ok(task_count)
}
