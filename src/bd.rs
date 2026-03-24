use anyhow::Result;
use serde::Deserialize;
use tokio::process::Command;

#[derive(Debug, Clone, Deserialize)]
pub struct Issue {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub priority: Option<u8>,
    #[serde(default)]
    pub description: Option<String>,
}

fn bd_command(dir: Option<&str>) -> Command {
    let mut cmd = Command::new("bd");
    if let Some(d) = dir {
        cmd.arg("--db").arg(format!("{d}/.beads/beads.db"));
    }
    cmd
}

pub async fn list_issues(dir: Option<&str>) -> Result<Vec<Issue>> {
    let output = bd_command(dir)
        .args(["list", "--json", "--limit", "0"])
        .output()
        .await?;

    if !output.status.success() {
        anyhow::bail!(
            "bd list failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let issues: Vec<Issue> = serde_json::from_slice(&output.stdout)?;
    Ok(issues)
}

pub async fn update_title(dir: Option<&str>, id: &str, title: &str) -> Result<()> {
    let output = bd_command(dir)
        .args(["update", id, "--title", title])
        .output()
        .await?;
    if !output.status.success() {
        anyhow::bail!(
            "bd update title failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

pub async fn update_description(dir: Option<&str>, id: &str, description: &str) -> Result<()> {
    let output = bd_command(dir)
        .args(["update", id, "--description", description])
        .output()
        .await?;
    if !output.status.success() {
        anyhow::bail!(
            "bd update description failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

pub async fn update_design(dir: Option<&str>, id: &str, design: &str) -> Result<()> {
    let output = bd_command(dir)
        .args(["update", id, "--design", design])
        .output()
        .await?;
    if !output.status.success() {
        anyhow::bail!(
            "bd update design failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

pub async fn update_priority(dir: Option<&str>, id: &str, priority: u8) -> Result<()> {
    let output = bd_command(dir)
        .args(["update", id, "--priority", &priority.to_string()])
        .output()
        .await?;
    if !output.status.success() {
        anyhow::bail!(
            "bd update priority failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

pub async fn close_issue(dir: Option<&str>, id: &str) -> Result<()> {
    let output = bd_command(dir).args(["close", id]).output().await?;
    if !output.status.success() {
        anyhow::bail!(
            "bd close failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

pub async fn add_label(dir: Option<&str>, id: &str, label: &str) -> Result<()> {
    let output = bd_command(dir)
        .args(["label", "add", id, label])
        .output()
        .await?;
    if !output.status.success() {
        anyhow::bail!(
            "bd label add failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}
