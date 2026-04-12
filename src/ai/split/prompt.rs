use anyhow::Result;
use serde::Deserialize;

use super::SplitRequest;

#[derive(Debug, Clone, Deserialize)]
pub struct SplitResult {
    pub tasks: Vec<TaskDef>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TaskDef {
    pub title: String,
    pub description: String,
}

pub fn build_prompt(request: &SplitRequest) -> String {
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

pub fn parse_result_from_text(text: &str) -> Result<SplitResult> {
    let inner = extract_json(text)?;
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
