use anyhow::Result;
use serde::Deserialize;
use tokio::sync::mpsc;

use crate::bd;

#[derive(Debug, Clone)]
pub struct SplitRequest {
    pub issue_id: String,
    pub title: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SplitResult {
    pub tasks: Vec<TaskDef>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TaskDef {
    pub title: String,
    pub description: String,
}

pub enum SplitEvent {
    Started { issue_id: String },
    Completed { issue_id: String, task_count: usize },
    Failed { issue_id: String, error: String },
}

fn build_prompt(request: &SplitRequest) -> String {
    let description_section = match &request.description {
        Some(desc) => format!("\n\nDescription:\n{desc}"),
        None => String::new(),
    };

    format!(
        r#"You are a task decomposition assistant. Given the following issue, break it down into concrete, independently implementable subtasks.

Issue Title: {title}{description_section}

Rules:
- Each subtask should be a single, focused unit of work
- Order subtasks by dependency (earlier tasks should not depend on later ones)
- Each subtask should be completable in a single implementation session
- Write titles as concise action items
- Write descriptions with enough context for an AI to implement without additional guidance
- Aim for 2-5 subtasks (fewer is better if the work is simple)

Respond in JSON format exactly matching this structure:
{{
  "tasks": [
    {{
      "title": "Short action item",
      "description": "1-3 sentence description of what to implement and how"
    }}
  ]
}}

Important:
- Output only valid JSON, no markdown fences or extra text
- Write in the same language as the issue title"#,
        title = request.title,
    )
}

/// claude -p --output-format json のラッパー構造
#[derive(Debug, Deserialize)]
struct ClaudeResponse {
    result: String,
}

fn parse_result(json_str: &str) -> Result<SplitResult> {
    let wrapper: ClaudeResponse = serde_json::from_str(json_str)?;
    let inner = extract_json(&wrapper.result)?;
    let result: SplitResult = serde_json::from_str(inner)?;
    Ok(result)
}

fn extract_json(s: &str) -> Result<&str> {
    let trimmed = s.trim();

    if trimmed.starts_with('{') {
        return Ok(trimmed);
    }

    if let Some(start) = trimmed.find('{') {
        let candidate = &trimmed[start..];
        if let Some(end) = candidate.rfind('}') {
            return Ok(&candidate[..=end]);
        }
    }

    let preview_end = trimmed
        .char_indices()
        .map(|(i, _)| i)
        .take_while(|&i| i <= 200)
        .last()
        .unwrap_or(0);
    anyhow::bail!(
        "No JSON object found in response: {}",
        &trimmed[..preview_end]
    );
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
    let prompt = build_prompt(request);

    let output = tokio::process::Command::new("claude")
        .args(["-p", &prompt, "--output-format", "json"])
        .output()
        .await?;

    if !output.status.success() {
        anyhow::bail!(
            "claude command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let stdout = String::from_utf8(output.stdout)?;
    let split_result = parse_result(&stdout)?;

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
