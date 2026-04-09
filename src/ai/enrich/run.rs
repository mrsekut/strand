use anyhow::Result;
use tokio::sync::mpsc;

use crate::bd;

use super::EnrichRequest;
use super::prompt;

pub enum EnrichEvent {
    Started { issue_id: String },
    Completed { issue_id: String },
    Failed { issue_id: String, error: String },
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
    match std::env::var("STRAND_ENRICH_SKILL").ok() {
        Some(skill_name) => run_with_skill(request, dir, &skill_name).await,
        None => run_default(request, dir).await,
    }
}

async fn run_with_skill(
    request: &EnrichRequest,
    dir: Option<&str>,
    skill_name: &str,
) -> Result<()> {
    let description_section = match &request.description {
        Some(desc) => format!("\n\nDescription:\n{desc}"),
        None => String::new(),
    };

    let prompt_text = format!(
        "Use the {skill_name} skill to analyze the following issue. Output the result as plain text.\n\nIssue Title: {title}{description_section}",
        title = request.title,
    );

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
    let result_text = prompt::parse_text_result(&stdout)?;

    let current_issue = bd::get_issue(dir, &request.issue_id).await?;
    let description = match current_issue.description.as_deref() {
        Some(orig) => format!("{orig}\n\n---\n{result_text}"),
        None => result_text,
    };

    bd::update_description(dir, &request.issue_id, &description).await?;
    bd::remove_label(dir, &request.issue_id, "strand-needs-enrich").await?;
    bd::add_label(dir, &request.issue_id, "strand-enriched").await?;
    bd::add_label(dir, &request.issue_id, "strand-unread").await?;

    Ok(())
}

async fn run_default(request: &EnrichRequest, dir: Option<&str>) -> Result<()> {
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
    let enrich_result = prompt::parse_result(&stdout)?;

    // enrich実行中にユーザーがdescを編集している可能性があるため、
    // 書き込み直前に最新のdescを再取得してappendする
    let current_issue = bd::get_issue(dir, &request.issue_id).await?;
    let current_desc = current_issue.description.as_deref();
    let description = prompt::format_enriched(current_desc, &enrich_result);

    bd::update_description(dir, &request.issue_id, &description).await?;
    bd::remove_label(dir, &request.issue_id, "strand-needs-enrich").await?;
    bd::add_label(dir, &request.issue_id, "strand-enriched").await?;
    bd::add_label(dir, &request.issue_id, "strand-unread").await?;

    Ok(())
}
