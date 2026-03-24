use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::bd;

#[derive(Debug, Clone)]
pub struct EnrichRequest {
    pub issue_id: String,
    pub title: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EnrichResult {
    pub problems: Vec<String>,
    pub solutions: Vec<Solution>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Solution {
    pub label: String,
    pub title: String,
    pub description: String,
}

pub enum EnrichEvent {
    Started { issue_id: String },
    Completed { issue_id: String },
    Failed { issue_id: String, error: String },
}

pub fn build_prompt(request: &EnrichRequest) -> String {
    let description_section = match &request.description {
        Some(desc) => format!("\n\nDescription:\n{desc}"),
        None => String::new(),
    };

    format!(
        r#"You are an issue analysis assistant. Given the following issue, analyze it.

Issue Title: {title}{description_section}

Please provide:

1. **課題 (Problems)**: Bullet-point list of the core problems. Each bullet should be one concise sentence.
2. **ソリューション案 (Solutions)**: 2-3 concrete solution approaches, each with a short label (A, B, C...) and a brief description.

Respond in JSON format exactly matching this structure:
{{
  "problems": [
    "One concise problem statement per bullet",
    "Another problem statement"
  ],
  "solutions": [
    {{
      "label": "A",
      "title": "Short solution name",
      "description": "1-2 sentence description of approach"
    }}
  ]
}}

Important:
- Keep each bullet/sentence short and scannable (aim for under 80 chars)
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

pub fn parse_result(json_str: &str) -> Result<EnrichResult> {
    // claude --output-format json はラッパーJSONを返す。.resultフィールドに実際の出力がある
    let wrapper: ClaudeResponse = serde_json::from_str(json_str)?;
    let inner = extract_json(&wrapper.result)?;
    let result: EnrichResult = serde_json::from_str(inner)?;
    Ok(result)
}

/// resultフィールドからJSON部分を抽出する。
/// markdownフェンスや前後の余分なテキストを除去する。
fn extract_json(s: &str) -> Result<&str> {
    let trimmed = s.trim();

    // まずそのままパースを試みる
    if trimmed.starts_with('{') {
        return Ok(trimmed);
    }

    // ```json ... ``` のフェンスを除去
    if let Some(start) = trimmed.find('{') {
        let candidate = &trimmed[start..];
        if let Some(end) = candidate.rfind('}') {
            return Ok(&candidate[..=end]);
        }
    }

    anyhow::bail!(
        "No JSON object found in response: {}",
        &trimmed[..trimmed.len().min(200)]
    );
}

pub fn format_enriched(original: Option<&str>, result: &EnrichResult) -> String {
    let mut out = String::new();

    if let Some(orig) = original {
        out.push_str(orig);
        out.push_str("\n\n---\n");
    }

    // 課題
    out.push_str("### 課題\n");
    for p in &result.problems {
        out.push_str(&format!("- {p}\n"));
    }

    // ソリューション案
    out.push_str("\n### ソリューション案\n");
    for s in &result.solutions {
        out.push_str(&format!("- {}: {}\n", s.label, s.title));
        out.push_str(&format!("  {}\n", s.description));
    }

    out.trim_end().to_string()
}

pub async fn run(
    request: EnrichRequest,
    dir: Option<String>,
    tx: mpsc::Sender<EnrichEvent>,
) -> Result<()> {
    let issue_id = request.issue_id.clone();

    let _ = tx
        .send(EnrichEvent::Started {
            issue_id: issue_id.clone(),
        })
        .await;

    let result = run_inner(&request, dir.as_deref()).await;

    match result {
        Ok(_) => {
            let _ = tx
                .send(EnrichEvent::Completed {
                    issue_id: issue_id.clone(),
                })
                .await;
        }
        Err(e) => {
            let _ = tx
                .send(EnrichEvent::Failed {
                    issue_id: issue_id.clone(),
                    error: e.to_string(),
                })
                .await;
            return Err(e);
        }
    }

    Ok(())
}

async fn run_inner(request: &EnrichRequest, dir: Option<&str>) -> Result<()> {
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
    let enrich_result = parse_result(&stdout)?;

    let description = format_enriched(request.description.as_deref(), &enrich_result);

    bd::update_description(dir, &request.issue_id, &description).await?;
    bd::add_label(dir, &request.issue_id, "enriched").await?;
    bd::add_label(dir, &request.issue_id, "strand-unread").await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_prompt_title_only() {
        let request = EnrichRequest {
            issue_id: "beads-001".to_string(),
            title: "Fix login bug".to_string(),
            description: None,
        };
        let prompt = build_prompt(&request);
        assert!(prompt.contains("Fix login bug"));
        assert!(!prompt.contains("Description:"));
    }

    #[test]
    fn build_prompt_with_description() {
        let request = EnrichRequest {
            issue_id: "beads-001".to_string(),
            title: "Fix login bug".to_string(),
            description: Some("Users cannot log in".to_string()),
        };
        let prompt = build_prompt(&request);
        assert!(prompt.contains("Fix login bug"));
        assert!(prompt.contains("Description:"));
        assert!(prompt.contains("Users cannot log in"));
    }

    #[test]
    fn parse_result_valid_json() {
        // claude -p --output-format json のラッパー形式
        let inner = r#"{"problems":["Problem 1","Problem 2"],"solutions":[{"label":"A","title":"Solution A","description":"Do A"}]}"#;
        let json = format!(
            r#"{{"type":"result","result":{}}}"#,
            serde_json::to_string(inner).unwrap()
        );
        let result = parse_result(&json).unwrap();
        assert_eq!(result.problems, vec!["Problem 1", "Problem 2"]);
        assert_eq!(result.solutions.len(), 1);
        assert_eq!(result.solutions[0].label, "A");
        assert_eq!(result.solutions[0].title, "Solution A");
    }

    #[test]
    fn parse_result_with_markdown_fences() {
        let inner_json =
            r#"{"problems":["P1"],"solutions":[{"label":"A","title":"Sol","description":"Desc"}]}"#;
        // resultフィールドにmarkdownフェンスが含まれるケース
        let fenced = format!("```json\n{inner_json}\n```");
        let json = format!(
            r#"{{"type":"result","result":{}}}"#,
            serde_json::to_string(&fenced).unwrap()
        );
        let result = parse_result(&json).unwrap();
        assert_eq!(result.problems, vec!["P1"]);
    }

    #[test]
    fn parse_result_invalid_json() {
        let json = "not valid json";
        assert!(parse_result(json).is_err());
    }

    #[test]
    fn format_enriched_with_original() {
        let result = EnrichResult {
            problems: vec!["Problem 1".into(), "Problem 2".into()],
            solutions: vec![Solution {
                label: "A".into(),
                title: "Sol A".into(),
                description: "Do A".into(),
            }],
        };
        let text = format_enriched(Some("Original desc"), &result);
        assert!(text.starts_with("Original desc\n\n---\n"));
        assert!(text.contains("### 課題\n- Problem 1\n- Problem 2"));
        assert!(text.contains("### ソリューション案\n- A: Sol A\n  Do A"));
    }

    #[test]
    fn format_enriched_without_original() {
        let result = EnrichResult {
            problems: vec!["Problem 1".into()],
            solutions: vec![
                Solution {
                    label: "A".into(),
                    title: "Sol A".into(),
                    description: "Do A".into(),
                },
                Solution {
                    label: "B".into(),
                    title: "Sol B".into(),
                    description: "Do B".into(),
                },
            ],
        };
        let text = format_enriched(None, &result);
        assert!(text.starts_with("### 課題"));
        assert!(text.contains("- A: Sol A\n  Do A"));
        assert!(text.contains("- B: Sol B\n  Do B"));
    }
}
