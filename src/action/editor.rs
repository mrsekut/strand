use ratatui::prelude::*;

use crate::app::App;
use crate::bd;

// TODO: Step 6 で &mut Core に変更（dir, bd CLI, terminal, auto_enrich が必要）

pub async fn quick_create_with_editor(
    app: &mut App,
    terminal: &mut ratatui::Terminal<CrosstermBackend<std::io::Stdout>>,
) {
    let result = crate::editor::open_editor_for_create(terminal);

    match result {
        Ok(Some(create)) => match bd::quick_create(app.dir.as_deref(), &create.title).await {
            Ok(id) => {
                app.notify(format!("Created: {id}"));
                let _ = app.load_issues().await;
                app.auto_enrich();
            }
            Err(e) => {
                app.notify(format!("Create failed: {e}"));
            }
        },
        Ok(None) => {}
        Err(e) => {
            app.notify(format!("{e}"));
        }
    }
}

pub async fn edit_description(
    app: &mut App,
    terminal: &mut ratatui::Terminal<CrosstermBackend<std::io::Stdout>>,
    issue_id: &str,
) {
    let Some(issue) = app.find_issue(issue_id) else {
        return;
    };
    let current_desc = issue.description.as_deref().unwrap_or_default();

    let result = crate::editor::open_editor(terminal, &issue.id, &issue.title, current_desc);

    match result {
        Ok(Some(edit)) => {
            let mut ok = true;
            if edit.title_changed {
                if let Err(e) =
                    bd::update_title(app.dir.as_deref(), &edit.issue_id, &edit.new_title).await
                {
                    app.notify(format!("Title update failed: {e}"));
                    ok = false;
                }
            }
            if edit.desc_changed {
                if let Err(e) =
                    bd::update_description(app.dir.as_deref(), &edit.issue_id, &edit.new_desc).await
                {
                    app.notify(format!("Description update failed: {e}"));
                    ok = false;
                }
            }
            if ok {
                app.notify(format!("Updated: {}", edit.issue_id));
                let _ = app.load_issues().await;
                crate::action::navigate::reload_children(app).await;
            }
        }
        Ok(None) => {}
        Err(e) => {
            app.notify(format!("{e}"));
        }
    }
}
