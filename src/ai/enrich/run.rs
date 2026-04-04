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
