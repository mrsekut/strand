use ratatui::prelude::*;

use crate::ai::AiManagers;
use crate::bd;
use crate::core::Core;

pub async fn quick_create_with_editor(
    core: &mut Core,
    ai: &mut AiManagers,
    terminal: &mut ratatui::Terminal<CrosstermBackend<std::io::Stdout>>,
) {
    let result = crate::editor::open_editor_for_create(terminal);

    match result {
        Ok(Some(create)) => match bd::quick_create(None, &create.title, &create.description).await {
            Ok(id) => {
                core.notify(format!("Created: {id}"));
                let _ = core.load_issues().await;
                crate::action::ai::auto_enrich(core, ai);
            }
            Err(e) => {
                core.notify(format!("Create failed: {e}"));
            }
        },
        Ok(None) => {}
        Err(e) => {
            core.notify(format!("{e}"));
        }
    }
}

pub async fn edit_description(
    core: &mut Core,
    terminal: &mut ratatui::Terminal<CrosstermBackend<std::io::Stdout>>,
    issue_id: &str,
) {
    let Some(issue) = core.find_issue(issue_id) else {
        return;
    };
    let current_desc = issue.description.as_deref().unwrap_or_default();

    let result = crate::editor::open_editor(terminal, &issue.id, &issue.title, current_desc);

    match result {
        Ok(Some(edit)) => {
            let mut ok = true;
            if edit.title_changed {
                if let Err(e) = bd::update_title(None, &edit.issue_id, &edit.new_title).await {
                    core.notify(format!("Title update failed: {e}"));
                    ok = false;
                }
            }
            if edit.desc_changed {
                if let Err(e) = bd::update_description(None, &edit.issue_id, &edit.new_desc).await {
                    core.notify(format!("Description update failed: {e}"));
                    ok = false;
                }
            }
            if ok {
                core.notify(format!("Updated: {}", edit.issue_id));
                let _ = core.load_issues().await;
                crate::action::navigate::reload_children(core).await;
            }
        }
        Ok(None) => {}
        Err(e) => {
            core.notify(format!("{e}"));
        }
    }
}
