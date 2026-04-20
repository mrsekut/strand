use std::path::PathBuf;

use anyhow::Result;

use crate::ai::job::{JobMeta, ResultData, SetupContext, WorkflowHandler};
use crate::bd::{self, Issue};
use crate::config::EnrichConfig;
use crate::core::Core;

use super::prompt;
use super::run::EnrichEvent;

pub struct EnrichHandler;

impl WorkflowHandler for EnrichHandler {
    type Event = EnrichEvent;
    type Config = EnrichConfig;

    fn workflow_name(&self) -> &str {
        "enrich"
    }

    fn build_command(&self, issue: &Issue, config: &EnrichConfig) -> Vec<String> {
        let prompt_text = match &config.skill {
            Some(skill_name) => build_skill_prompt(issue, skill_name),
            None => prompt::build_prompt(&super::EnrichRequest {
                title: issue.title.clone(),
                description: issue.description.clone(),
            }),
        };

        vec![
            "claude".to_string(),
            "-p".to_string(),
            prompt_text,
            "--output-format".to_string(),
            "stream-json".to_string(),
            "--verbose".to_string(),
        ]
    }

    fn working_dir(&self, _meta: &JobMeta) -> PathBuf {
        Core::repo_dir()
    }

    async fn setup(&self, _issue: &Issue, _config: &EnrichConfig) -> Result<SetupContext> {
        Ok(SetupContext {
            worktree_path: None,
        })
    }

    fn on_started(&self, issue_id: &str) -> EnrichEvent {
        EnrichEvent::Started {
            issue_id: issue_id.to_string(),
        }
    }

    async fn on_completed(&self, result: ResultData, meta: &JobMeta) -> EnrichEvent {
        let issue_id = &meta.issue_id;

        // 既に enrich 済みなら description 更新をスキップ（再起動時の重複追記を防止）
        if let Ok(issue) = bd::get_issue(None, issue_id).await {
            if issue.labels.contains(&"strand-enriched".to_string()) {
                return EnrichEvent::Completed {
                    issue_id: issue_id.clone(),
                };
            }
        }

        // 結果を解釈して description を更新
        let config = crate::config::Config::load();
        let update_result = if config.enrich.skill.is_some() {
            // skill モード: result をそのまま追記
            update_description_with_text(issue_id, &result.result).await
        } else {
            // default モード: JSON パースして構造化追記
            update_description_with_parsed(issue_id, &result.result).await
        };

        if let Err(e) = update_result {
            return EnrichEvent::Failed {
                issue_id: issue_id.clone(),
                error: format!("failed to update description: {e}"),
            };
        }

        // ラベル更新
        let _ = bd::remove_label(None, issue_id, "strand-needs-enrich").await;
        let _ = bd::add_label(None, issue_id, "strand-enriched").await;
        let _ = bd::add_label(None, issue_id, "strand-unread").await;

        EnrichEvent::Completed {
            issue_id: issue_id.clone(),
        }
    }

    fn on_failed(&self, issue_id: &str, error: String) -> EnrichEvent {
        EnrichEvent::Failed {
            issue_id: issue_id.to_string(),
            error,
        }
    }
}

fn build_skill_prompt(issue: &Issue, skill_name: &str) -> String {
    let description_section = match &issue.description {
        Some(desc) => format!("\n\nDescription:\n{desc}"),
        None => String::new(),
    };

    format!(
        "Use the {skill_name} skill to analyze the following issue. Output the result as plain text.\n\nIssue Title: {title}{description_section}",
        title = issue.title,
    )
}

async fn update_description_with_text(issue_id: &str, result_text: &str) -> Result<()> {
    let current_issue = bd::get_issue(None, issue_id).await?;
    let description = match current_issue.description.as_deref() {
        Some(orig) => format!("{orig}\n\n---\n{result_text}"),
        None => result_text.to_string(),
    };
    bd::update_description(None, issue_id, &description).await?;
    Ok(())
}

async fn update_description_with_parsed(issue_id: &str, result_text: &str) -> Result<()> {
    let enrich_result = prompt::parse_result_from_text(result_text)?;

    let current_issue = bd::get_issue(None, issue_id).await?;
    let current_desc = current_issue.description.as_deref();
    let description = prompt::format_enriched(current_desc, &enrich_result);

    bd::update_description(None, issue_id, &description).await?;
    Ok(())
}
