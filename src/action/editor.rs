use ratatui::prelude::*;

use crate::app::App;
use crate::bd;

// TODO: enrich_manager が必要なため &mut App を取る（manager 集約後に再検討）

pub async fn quick_create_with_editor(
    app: &mut App,
    terminal: &mut ratatui::Terminal<CrosstermBackend<std::io::Stdout>>,
) {
    let result = crate::editor::open_editor_for_create(terminal);

    match result {
        Ok(Some(create)) => match bd::quick_create(None, &create.title).await {
            Ok(id) => {
                app.core.notify(format!("Created: {id}"));
                let _ = app.core.load_issues().await;
                crate::action::ai::auto_enrich(app);
            }
            Err(e) => {
                app.core.notify(format!("Create failed: {e}"));
            }
        },
        Ok(None) => {}
        Err(e) => {
            app.core.notify(format!("{e}"));
        }
    }
}

// TODO: bd CLI が必要なため &mut App を取る（manager 集約後に再検討）
pub async fn edit_description(
    app: &mut App,
    terminal: &mut ratatui::Terminal<CrosstermBackend<std::io::Stdout>>,
    issue_id: &str,
) {
    let Some(issue) = app.core.find_issue(issue_id) else {
        return;
    };
    let current_desc = issue.description.as_deref().unwrap_or_default();

    let result = crate::editor::open_editor(terminal, &issue.id, &issue.title, current_desc);

    match result {
        Ok(Some(edit)) => {
            let mut ok = true;
            if edit.title_changed {
                if let Err(e) = bd::update_title(None, &edit.issue_id, &edit.new_title).await {
                    app.core.notify(format!("Title update failed: {e}"));
                    ok = false;
                }
            }
            if edit.desc_changed {
                if let Err(e) = bd::update_description(None, &edit.issue_id, &edit.new_desc).await {
                    app.core.notify(format!("Description update failed: {e}"));
                    ok = false;
                }
            }
            if ok {
                app.core.notify(format!("Updated: {}", edit.issue_id));
                let _ = app.core.load_issues().await;
                crate::action::navigate::reload_children(&mut app.core).await;
            }
        }
        Ok(None) => {}
        Err(e) => {
            app.core.notify(format!("{e}"));
        }
    }
}
