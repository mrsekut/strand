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
    pub enriched_description: String,
    pub solutions: Vec<Solution>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Solution {
    pub title: String,
    pub description: String,
    pub pros: Vec<String>,
    pub cons: Vec<String>,
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
        r#"You are an issue analysis assistant. Given the following issue, please:

1. Enrich the description: Infer and describe the background, context, and scope of impact for this issue.
2. Propose 3 solution alternatives, each with a title, description, pros, and cons.

Issue Title: {title}{description_section}

Respond in JSON format exactly matching this structure:
{{
  "enriched_description": "...",
  "solutions": [
    {{
      "title": "...",
      "description": "...",
      "pros": ["..."],
      "cons": ["..."]
    }}
  ]
}}

Output only valid JSON, no markdown fences or extra text."#,
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
    let result: EnrichResult = serde_json::from_str(&wrapper.result)?;
    Ok(result)
}

pub fn format_description(original: Option<&str>, enriched: &str) -> String {
    match original {
        Some(orig) => format!("{orig}\n\n---\n## AI Enriched Description\n{enriched}"),
        None => enriched.to_string(),
    }
}

pub fn format_design(solutions: &[Solution]) -> String {
    solutions
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let pros = s
                .pros
                .iter()
                .map(|p| format!("- {p}"))
                .collect::<Vec<_>>()
                .join("\n");
            let cons = s
                .cons
                .iter()
                .map(|c| format!("- {c}"))
                .collect::<Vec<_>>()
                .join("\n");
            format!(
                "## Solution {num}: {title}\n{desc}\n\n**Pros:**\n{pros}\n\n**Cons:**\n{cons}",
                num = i + 1,
                title = s.title,
                desc = s.description,
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
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

    let description = format_description(
        request.description.as_deref(),
        &enrich_result.enriched_description,
    );
    let design = format_design(&enrich_result.solutions);

    bd::update_description(dir, &request.issue_id, &description).await?;
    bd::update_design(dir, &request.issue_id, &design).await?;
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
        let inner = r#"{"enriched_description":"This is enriched","solutions":[{"title":"Solution A","description":"Do A","pros":["fast"],"cons":["complex"]}]}"#;
        let json = format!(r#"{{"type":"result","result":{}}}"#, serde_json::to_string(inner).unwrap());
        let result = parse_result(&json).unwrap();
        assert_eq!(result.enriched_description, "This is enriched");
        assert_eq!(result.solutions.len(), 1);
        assert_eq!(result.solutions[0].title, "Solution A");
        assert_eq!(result.solutions[0].pros, vec!["fast"]);
        assert_eq!(result.solutions[0].cons, vec!["complex"]);
    }

    #[test]
    fn parse_result_invalid_json() {
        let json = "not valid json";
        assert!(parse_result(json).is_err());
    }

    #[test]
    fn format_description_with_original() {
        let result = format_description(Some("Original desc"), "Enriched desc");
        assert_eq!(
            result,
            "Original desc\n\n---\n## AI Enriched Description\nEnriched desc"
        );
    }

    #[test]
    fn format_description_without_original() {
        let result = format_description(None, "Enriched desc");
        assert_eq!(result, "Enriched desc");
    }

    #[test]
    fn format_design_multiple_solutions() {
        let solutions = vec![
            Solution {
                title: "Sol A".to_string(),
                description: "Do A".to_string(),
                pros: vec!["fast".to_string()],
                cons: vec!["complex".to_string()],
            },
            Solution {
                title: "Sol B".to_string(),
                description: "Do B".to_string(),
                pros: vec!["simple".to_string(), "cheap".to_string()],
                cons: vec!["slow".to_string()],
            },
        ];
        let md = format_design(&solutions);
        assert!(md.contains("## Solution 1: Sol A"));
        assert!(md.contains("## Solution 2: Sol B"));
        assert!(md.contains("**Pros:**\n- fast"));
        assert!(md.contains("**Cons:**\n- complex"));
        assert!(md.contains("- simple\n- cheap"));
    }
}
