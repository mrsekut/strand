use std::path::PathBuf;

use anyhow::Result;

use crate::ai::job::{JobMeta, ResultData, SetupContext, WorkflowHandler};
use crate::bd::{self, Issue};
use crate::core::Core;

use super::run::ImplEvent;
use super::worktree;

/// Impl 固有の開始設定
pub struct ImplConfig {
    pub epic_id: Option<String>,
}

/// Impl workflow の WorkflowHandler 実装
pub struct ImplHandler;

impl WorkflowHandler for ImplHandler {
    type Event = ImplEvent;
    type Config = ImplConfig;

    fn workflow_name(&self) -> &str {
        "impl"
    }

    fn build_command(&self, issue: &Issue, _config: &ImplConfig) -> Vec<String> {
        let prompt = build_prompt(issue);
        vec![
            "claude".to_string(),
            "-p".to_string(),
            prompt,
            "--output-format".to_string(),
            "stream-json".to_string(),
            "--verbose".to_string(),
            "--dangerously-skip-permissions".to_string(),
        ]
    }

    fn working_dir(&self, meta: &JobMeta) -> PathBuf {
        meta.worktree_path
            .as_ref()
            .map(PathBuf::from)
            .unwrap_or_else(Core::repo_dir)
    }

    async fn setup(&self, issue: &Issue, config: &ImplConfig) -> Result<SetupContext> {
        let repo_dir = Core::repo_dir();

        let base_branch = if let Some(eid) = &config.epic_id {
            worktree::ensure_epic_branch(&repo_dir, eid).await?
        } else {
            worktree::detect_default_branch(&repo_dir)
        };

        let (wt_path, _branch) =
            worktree::create_worktree(&repo_dir, &issue.id, &base_branch).await?;

        Ok(SetupContext {
            worktree_path: Some(wt_path.to_string_lossy().to_string()),
        })
    }

    fn on_started(&self, issue_id: &str) -> ImplEvent {
        ImplEvent::Started {
            issue_id: issue_id.to_string(),
        }
    }

    async fn on_completed(&self, result: ResultData, meta: &JobMeta) -> ImplEvent {
        // description にログ追記
        let content = format!("## Implementation Log\n{}", result.result);
        let id = meta.issue_id.clone();
        let _ = bd::append_to_description(None, &id, &content).await;

        ImplEvent::Completed {
            issue_id: meta.issue_id.clone(),
            summary: result.result,
            session_id: result.session_id,
        }
    }

    fn on_failed(&self, issue_id: &str, error: String) -> ImplEvent {
        ImplEvent::Failed {
            issue_id: issue_id.to_string(),
            error,
            session_id: None,
        }
    }

    fn on_session_id_discovered(&self, issue_id: &str, session_id: String) -> Option<ImplEvent> {
        Some(ImplEvent::SessionIdDiscovered {
            issue_id: issue_id.to_string(),
            session_id,
        })
    }
}

fn build_prompt(issue: &Issue) -> String {
    let mut parts = vec![format!("Issue: {}", issue.title)];

    if let Some(desc) = &issue.description {
        parts.push(format!("Description:\n{desc}"));
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
