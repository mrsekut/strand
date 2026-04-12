use std::path::PathBuf;

use anyhow::Result;

use crate::ai::job::{JobMeta, ResultData, SetupContext, WorkflowHandler};
use crate::bd::{self, Issue};
use crate::core::Core;

use super::prompt;
use super::run::SplitEvent;

pub struct SplitHandler;

impl WorkflowHandler for SplitHandler {
    type Event = SplitEvent;
    type Config = ();

    fn workflow_name(&self) -> &str {
        "split"
    }

    fn build_command(&self, issue: &Issue, _config: &()) -> Vec<String> {
        let prompt_text = prompt::build_prompt(&super::SplitRequest {
            issue_id: issue.id.clone(),
            title: issue.title.clone(),
            description: issue.description.clone(),
        });

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

    async fn setup(&self, _issue: &Issue, _config: &()) -> Result<SetupContext> {
        Ok(SetupContext {
            worktree_path: None,
        })
    }

    fn on_started(&self, issue_id: &str) -> SplitEvent {
        SplitEvent::Started {
            issue_id: issue_id.to_string(),
        }
    }

    async fn on_completed(&self, result: ResultData, meta: &JobMeta) -> SplitEvent {
        let issue_id = &meta.issue_id;

        let split_result = match prompt::parse_result_from_text(&result.result) {
            Ok(r) => r,
            Err(e) => {
                return SplitEvent::Failed {
                    issue_id: issue_id.clone(),
                    error: format!("failed to parse split result: {e}"),
                };
            }
        };

        // 親 issue を epic に昇格
        let _ = bd::update_type(None, issue_id, "epic").await;

        // 子タスクを作成
        let task_count = split_result.tasks.len();
        for task in &split_result.tasks {
            match bd::create_child(None, issue_id, &task.title, &task.description).await {
                Ok(child_id) => {
                    let _ = bd::add_label(None, &child_id, "strand-enriched").await;
                }
                Err(e) => {
                    return SplitEvent::Failed {
                        issue_id: issue_id.clone(),
                        error: format!("failed to create child: {e}"),
                    };
                }
            }
        }

        SplitEvent::Completed {
            issue_id: issue_id.clone(),
            task_count,
        }
    }

    fn on_failed(&self, issue_id: &str, error: String) -> SplitEvent {
        SplitEvent::Failed {
            issue_id: issue_id.to_string(),
            error,
        }
    }
}
