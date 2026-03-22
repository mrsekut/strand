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

pub async fn list_issues() -> Result<Vec<Issue>> {
    let output = Command::new("bd")
        .args(["list", "--json"])
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

pub async fn show_issue(id: &str) -> Result<Issue> {
    let output = Command::new("bd")
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
