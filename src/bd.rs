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

pub async fn show_issue(dir: Option<&str>, id: &str) -> Result<Issue> {
    let output = bd_command(dir)
        .args(["show", id, "--json"])
        .output()
        .await?;

    if !output.status.success() {
        anyhow::bail!(
            "bd show failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let issue: Issue = serde_json::from_slice(&output.stdout)?;
    Ok(issue)
}
