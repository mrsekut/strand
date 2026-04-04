use std::io::stdout;

use anyhow::Result;
use crossterm::ExecutableCommand;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::prelude::*;

/// エディタで新規作成された結果
pub struct CreateResult {
    pub title: String,
}

/// エディタを起動してissueのタイトルを入力させる（quick create用）。
/// TUI退避/復帰を含む。タイトルが入力されればCreateResultを返す。
pub fn open_editor_for_create(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
) -> Result<Option<CreateResult>> {
    let tmp = std::env::temp_dir().join("strand-new-issue.md");
    std::fs::write(&tmp, "")?;

    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".into());
    disable_raw_mode().ok();
    stdout().execute(LeaveAlternateScreen).ok();
    terminal.show_cursor().ok();

    let status = std::process::Command::new(&editor).arg(&tmp).status();

    stdout().execute(EnterAlternateScreen).ok();
    enable_raw_mode().ok();
    terminal.clear().ok();

    let result = match status {
        Ok(s) if s.success() => {
            let content = std::fs::read_to_string(&tmp)?;
            let title = content.lines().next().unwrap_or("").trim().to_string();
            if title.is_empty() {
                None
            } else {
                Some(CreateResult { title })
            }
        }
        _ => {
            anyhow::bail!("Editor exited with error");
        }
    };

    let _ = std::fs::remove_file(&tmp);
    Ok(result)
}

/// エディタで編集された結果
pub struct EditResult {
    pub issue_id: String,
    pub new_title: String,
    pub new_desc: String,
    pub title_changed: bool,
    pub desc_changed: bool,
}

/// エディタを起動してissueのtitle/descriptionを編集する。
/// TUI退避/復帰を含む。変更があればEditResultを返す。
pub fn open_editor(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    issue_id: &str,
    current_title: &str,
    current_desc: &str,
) -> Result<Option<EditResult>> {
    // 一時ファイルに書き出し（1行目: title, 2行目以降: description）
    let content = format!("{}\n\n{}", current_title, current_desc);
    let tmp = std::env::temp_dir().join(format!("strand-{issue_id}.md"));
    std::fs::write(&tmp, &content)?;

    // TUIを一時離脱してエディタ起動
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".into());
    disable_raw_mode().ok();
    stdout().execute(LeaveAlternateScreen).ok();
    terminal.show_cursor().ok();

    let status = std::process::Command::new(&editor).arg(&tmp).status();

    // TUI復帰
    stdout().execute(EnterAlternateScreen).ok();
    enable_raw_mode().ok();
    terminal.clear().ok();

    let result = match status {
        Ok(s) if s.success() => {
            let new_content = std::fs::read_to_string(&tmp)?;
            let new_title = new_content.lines().next().unwrap_or("").trim().to_string();
            let new_desc = new_content
                .lines()
                .skip(1)
                .collect::<Vec<_>>()
                .join("\n")
                .trim()
                .to_string();

            let title_changed = new_title != current_title.trim();
            let desc_changed = new_desc != current_desc.trim();

            if title_changed || desc_changed {
                Some(EditResult {
                    issue_id: issue_id.to_string(),
                    new_title,
                    new_desc,
                    title_changed,
                    desc_changed,
                })
            } else {
                None
            }
        }
        _ => {
            anyhow::bail!("Editor exited with error");
        }
    };

    let _ = std::fs::remove_file(&tmp);
    Ok(result)
}
